#!/usr/bin/env python3
"""Estimate current capacity, voltage drop, and copper loss for a KiCad net.

This is a screening tool, not a coupled electro-thermal field solver.  It uses
the IPC-2221 relationship published by KiCad and makes its geometric choices
visible in an SVG report so that an engineer can review the selected cuts.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import math
import re
import sys
from collections.abc import Iterable
from dataclasses import dataclass, field
from itertools import pairwise
from pathlib import Path
from typing import Any

import tomllib

try:
    import wx
except ImportError:  # pragma: no cover - pcbnew installations normally provide wx
    wx = None
else:
    # The current KiCad 10 distribution registers three enum properties with
    # no choices while importing pcbnew. They are unrelated to board loading
    # but trip wx debug assertions. Disable wx assertions in this short-lived
    # CLI process so normal runs and CI logs stay actionable.
    wx.DisableAsserts()

try:
    import pcbnew
except ImportError as error:  # pragma: no cover - exercised on unprepared hosts
    raise SystemExit(
        "pcbnew is required; install KiCad's Python bindings before running this tool"
    ) from error

try:
    from shapely.geometry import (
        GeometryCollection,
        LineString,
        MultiPolygon,
        Point,
        box,
    )
    from shapely.geometry import Polygon as ShapelyPolygon
    from shapely.ops import unary_union
except ImportError as error:  # pragma: no cover - exercised on unprepared hosts
    raise SystemExit(
        "shapely is required; install the Python shapely package before running this tool"
    ) from error


SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_CONFIG = SCRIPT_DIR / "pcb_power_scenarios.toml"
DEFAULT_OUTPUT_ROOT = Path("build/power-capacity")
COPPER_RESISTIVITY_OHM_M = 1.724e-8
COPPER_WEIGHT_UM_PER_OZ = 34.8
MM_PER_MIL = 0.0254
UM_PER_MIL = 25.4
GEOMETRY_ERROR_MM = 0.005
CONNECTIVITY_TOLERANCE_MM = 0.01
MIN_INTERSECTION_MM = 1e-6
IPC_EXTERNAL_K = 0.048
IPC_INTERNAL_K = 0.024
IPC_MAX_WIDTH_MIL = 400.0
IPC_MAX_EXTERNAL_CURRENT_A = 35.0
IPC_MAX_INTERNAL_CURRENT_A = 17.5
LAYER_COLORS = (
    "#d1495b",
    "#00798c",
    "#edae49",
    "#30638e",
    "#6a4c93",
    "#2a9d8f",
)
STACKUP_COLORS = (
    "#00798c",
    "#d95f02",
    "#6a4c93",
    "#2a9d8f",
)
THERMAL_PALETTE = (
    (0.0, (44, 123, 182)),
    (0.35, (0, 166, 202)),
    (0.55, (255, 255, 191)),
    (0.75, (253, 174, 97)),
    (1.0, (215, 25, 28)),
)


# KiCad 10's current Python bindings still use the Python 2-style ``next``
# method from a few collection helpers.
if not hasattr(pcbnew.SwigPyIterator, "next"):
    pcbnew.SwigPyIterator.next = pcbnew.SwigPyIterator.__next__


class AnalysisError(RuntimeError):
    """Raised for invalid input or a board that cannot be analyzed safely."""


@dataclass(frozen=True)
class Sink:
    selector: str
    current_a: float


@dataclass(frozen=True)
class StackupSpec:
    name: str
    label: str
    copper_overrides_um: dict[str, float] = field(default_factory=dict)


@dataclass
class Scenario:
    name: str
    board: Path
    net: str
    source: str
    sinks: list[Sink]
    supply_voltage_v: float | None = None
    temperature_rises_c: list[float] = field(default_factory=lambda: [10.0, 20.0])
    scan_pitch_mm: float = 0.25
    include_layers: list[str] | None = None
    copper_overrides_um: dict[str, float] = field(default_factory=dict)
    stackups: list[StackupSpec] = field(default_factory=list)
    manual_cuts: list[tuple[tuple[float, float], tuple[float, float]]] = field(
        default_factory=list
    )


@dataclass(frozen=True)
class Terminal:
    selector: str
    x_mm: float
    y_mm: float
    pad: Any
    current_a: float = 0.0


@dataclass
class LayerCopper:
    layer_id: int
    name: str
    thickness_um: float
    external: bool
    geometry: Any
    components: list[Any]


@dataclass
class CutResult:
    kind: str
    index: int
    distance_mm: float | None
    start_mm: tuple[float, float]
    end_mm: tuple[float, float]
    active_current_a: float
    widths_mm: dict[str, float]
    cross_sections_mm2: dict[str, float]
    shares: dict[str, float]
    layer_capacities_a: dict[str, dict[float, float]]
    natural_capacity_a: dict[float, float]
    ideal_capacity_a: dict[float, float]

    def margin(self, temperature_rise_c: float) -> float:
        if self.active_current_a <= 0:
            return math.inf
        return self.natural_capacity_a[temperature_rise_c] / self.active_current_a

    @property
    def total_cross_section_mm2(self) -> float:
        return sum(self.cross_sections_mm2.values())


class UnionFind:
    def __init__(self, size: int) -> None:
        self.parent = list(range(size))
        self.rank = [0] * size

    def find(self, item: int) -> int:
        while self.parent[item] != item:
            self.parent[item] = self.parent[self.parent[item]]
            item = self.parent[item]
        return item

    def union(self, left: int, right: int) -> None:
        left_root = self.find(left)
        right_root = self.find(right)
        if left_root == right_root:
            return
        if self.rank[left_root] < self.rank[right_root]:
            left_root, right_root = right_root, left_root
        self.parent[right_root] = left_root
        if self.rank[left_root] == self.rank[right_root]:
            self.rank[left_root] += 1


def mm(value: int) -> float:
    return value / 1_000_000.0


def point_mm(value: Any) -> tuple[float, float]:
    return mm(value.x), mm(value.y)


def copper_weight_oz(thickness_um: float) -> float:
    """Convert finished copper thickness to nominal PCB copper weight."""
    return thickness_um / COPPER_WEIGHT_UM_PER_OZ


def copper_thickness_label(thickness_um: float) -> str:
    """Format copper thickness in both metric and customary PCB units."""
    ounces = f"{copper_weight_oz(thickness_um):.1f}".rstrip("0").rstrip(".")
    return f"{thickness_um:g} µm ({ounces} oz copper)"


def load_board(board_path: Path) -> Any:
    if not board_path.is_file():
        raise AnalysisError(f"PCB file does not exist: {board_path}")
    # The distro KiCad 10 bindings emit non-actionable wx debug assertions
    # while constructing property metadata. Suppress wx logging only while the
    # board is loaded; analysis errors are raised explicitly below.
    log_silencer = wx.LogNull() if wx is not None else None
    try:
        board = pcbnew.LoadBoard(str(board_path))
    finally:
        del log_silencer
    if board is None:
        raise AnalysisError(f"KiCad could not load PCB: {board_path}")
    return board


def extract_balanced(text: str, start: int) -> str:
    depth = 0
    in_string = False
    escaped = False
    for index in range(start, len(text)):
        character = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
        elif character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    raise AnalysisError("Unbalanced S-expression while reading PCB stackup")


def stackup_copper_thicknesses_um(board_path: Path) -> dict[str, float]:
    text = board_path.read_text(encoding="utf-8")
    match = re.search(r"\(stackup\b", text)
    if match is None:
        return {}
    stackup = extract_balanced(text, match.start())
    thicknesses: dict[str, float] = {}
    for layer_match in re.finditer(r'\(layer\s+"([^"]+)"', stackup):
        block = extract_balanced(stackup, layer_match.start())
        name = layer_match.group(1)
        if not name.endswith(".Cu") or '(type "copper")' not in block:
            continue
        thickness_match = re.search(r"\(thickness\s+([0-9.]+)\)", block)
        if thickness_match is not None:
            thicknesses[name] = float(thickness_match.group(1)) * 1000.0
    return thicknesses


def copper_layer_ids(board: Any) -> list[int]:
    return [
        layer_id
        for layer_id in range(pcbnew.PCB_LAYER_ID_COUNT)
        if board.IsLayerEnabled(layer_id) and pcbnew.IsCopperLayer(layer_id)
    ]


def line_chain_points(chain: Any) -> list[tuple[float, float]]:
    return [point_mm(chain.CPoint(index)) for index in range(chain.PointCount())]


def polyset_to_shapely(polyset: Any) -> Any:
    polygons: list[Any] = []
    for outline_index in range(polyset.OutlineCount()):
        shell = line_chain_points(polyset.COutline(outline_index))
        if len(shell) < 3:
            continue
        holes = [
            line_chain_points(polyset.CHole(outline_index, hole_index))
            for hole_index in range(polyset.HoleCount(outline_index))
        ]
        polygon = ShapelyPolygon(shell, holes)
        if not polygon.is_valid:
            polygon = polygon.buffer(0)
        if not polygon.is_empty:
            polygons.append(polygon)
    if not polygons:
        return GeometryCollection()
    return unary_union(polygons)


def item_polygon(item: Any, layer_id: int) -> Any:
    polyset = pcbnew.SHAPE_POLY_SET()
    item.TransformShapeToPolygon(
        polyset,
        layer_id,
        0,
        pcbnew.FromMM(GEOMETRY_ERROR_MM),
        pcbnew.ERROR_OUTSIDE,
    )
    return polyset_to_shapely(polyset)


def polygon_components(geometry: Any) -> list[Any]:
    if geometry.is_empty:
        return []
    if isinstance(geometry, ShapelyPolygon):
        return [geometry]
    if isinstance(geometry, MultiPolygon):
        return list(geometry.geoms)
    components: list[Any] = []
    for part in getattr(geometry, "geoms", []):
        components.extend(polygon_components(part))
    return components


def extract_net_copper(
    board: Any,
    board_path: Path,
    net_name: str,
    include_layers: list[str] | None,
    overrides_um: dict[str, float],
) -> tuple[list[LayerCopper], list[str]]:
    known_nets = {
        net.GetNetname() for net in board.GetNetInfo().NetsByNetcode().values()
    }
    if net_name not in known_nets:
        close = sorted(name for name in known_nets if net_name.lower() in name.lower())
        suffix = f"; similar nets: {', '.join(close[:8])}" if close else ""
        raise AnalysisError(f"Net {net_name!r} is not present in {board_path}{suffix}")

    stackup_um = stackup_copper_thicknesses_um(board_path)
    warnings: list[str] = []
    layers: list[LayerCopper] = []
    net_zones = [zone for zone in board.Zones() if zone.GetNetname() == net_name]
    stale_zones = [
        zone for zone in net_zones if zone.NeedRefill() or not zone.IsFilled()
    ]
    if stale_zones:
        raise AnalysisError(
            f"Net {net_name!r} has {len(stale_zones)} unfilled or stale zone(s); "
            "refill and save the PCB before analysis"
        )

    tracks = [item for item in board.GetTracks() if item.GetNetname() == net_name]
    pads = [
        pad
        for footprint in board.GetFootprints()
        for pad in footprint.Pads()
        if pad.GetNetname() == net_name
    ]

    stackup_order = {name: index for index, name in enumerate(stackup_um)}
    enabled_copper_names = {
        board.GetLayerName(layer_id) for layer_id in copper_layer_ids(board)
    }
    if include_layers:
        unknown_layers = sorted(set(include_layers) - enabled_copper_names)
        if unknown_layers:
            raise AnalysisError(
                "Selected layers are not enabled copper layers: "
                + ", ".join(unknown_layers)
            )
    ordered_layer_ids = sorted(
        (
            layer_id
            for layer_id in copper_layer_ids(board)
            if not include_layers or board.GetLayerName(layer_id) in include_layers
        ),
        key=lambda layer_id: stackup_order.get(board.GetLayerName(layer_id), 10_000),
    )
    for layer_id in ordered_layer_ids:
        name = board.GetLayerName(layer_id)
        thickness_um = overrides_um.get(name, stackup_um.get(name))
        if thickness_um is None or thickness_um <= 0:
            raise AnalysisError(
                f"No positive copper thickness is available for {name}; "
                f"use --copper {name}=<micrometres>"
            )
        shapes: list[Any] = []
        for zone in net_zones:
            if zone.IsOnLayer(layer_id) and zone.HasFilledPolysForLayer(layer_id):
                geometry = polyset_to_shapely(zone.GetFilledPolysList(layer_id))
                if not geometry.is_empty:
                    shapes.append(geometry)
        for track in tracks:
            if track.IsOnLayer(layer_id):
                geometry = item_polygon(track, layer_id)
                if not geometry.is_empty:
                    shapes.append(geometry)
        for pad in pads:
            if pad.IsOnLayer(layer_id):
                geometry = item_polygon(pad, layer_id)
                if not geometry.is_empty:
                    shapes.append(geometry)
        geometry = unary_union(shapes) if shapes else GeometryCollection()
        if not geometry.is_valid:
            geometry = geometry.buffer(0)
        components = polygon_components(geometry)
        layers.append(
            LayerCopper(
                layer_id=layer_id,
                name=name,
                thickness_um=thickness_um,
                external=name in {"F.Cu", "B.Cu"},
                geometry=geometry,
                components=components,
            )
        )

    unused_overrides = sorted(set(overrides_um) - {layer.name for layer in layers})
    if unused_overrides:
        warnings.append(
            "Copper overrides did not match enabled layers: "
            + ", ".join(unused_overrides)
        )
    if not any(not layer.geometry.is_empty for layer in layers):
        raise AnalysisError(f"Net {net_name!r} has no extractable copper geometry")
    return layers, warnings


def find_pad(board: Any, selector: str, net_name: str) -> Any:
    try:
        reference, pad_number = selector.rsplit(".", 1)
    except ValueError as error:
        raise AnalysisError(
            f"Invalid pad selector {selector!r}; expected REF.PAD, for example J1.1"
        ) from error
    footprint = board.FindFootprintByReference(reference)
    if footprint is None:
        raise AnalysisError(f"Footprint {reference!r} from {selector!r} was not found")
    matches = [pad for pad in footprint.Pads() if pad.GetNumber() == pad_number]
    if not matches:
        raise AnalysisError(f"Pad {selector!r} was not found")
    pad = matches[0]
    if pad.GetNetname() != net_name:
        raise AnalysisError(
            f"Pad {selector!r} is on net {pad.GetNetname()!r}, not {net_name!r}"
        )
    return pad


def terminal(
    board: Any, selector: str, net_name: str, current_a: float = 0.0
) -> Terminal:
    pad = find_pad(board, selector, net_name)
    x_mm, y_mm = point_mm(pad.GetPosition())
    return Terminal(selector, x_mm, y_mm, pad, current_a)


def touching_component_ids(
    layers: list[LayerCopper],
    offsets: dict[str, int],
    layer_ids: Iterable[int],
    x_mm: float,
    y_mm: float,
    radius_mm: float,
) -> list[int]:
    probe = Point(x_mm, y_mm).buffer(max(radius_mm, CONNECTIVITY_TOLERANCE_MM))
    matches: list[int] = []
    requested = set(layer_ids)
    for layer in layers:
        if layer.layer_id not in requested:
            continue
        for component_index, component in enumerate(layer.components):
            if component.intersects(probe):
                matches.append(offsets[layer.name] + component_index)
    return matches


def pad_radius_mm(pad: Any, layer_id: int) -> float:
    try:
        size = pad.GetSize(layer_id)
    except TypeError:
        size = pad.GetSize()
    return max(mm(size.x), mm(size.y)) * 0.45


def connected_net_geometry(
    board: Any,
    net_name: str,
    layers: list[LayerCopper],
    source: Terminal,
    sinks: list[Terminal],
) -> tuple[dict[str, Any], list[str]]:
    offsets: dict[str, int] = {}
    component_count = 0
    for layer in layers:
        offsets[layer.name] = component_count
        component_count += len(layer.components)
    if component_count == 0:
        raise AnalysisError(f"Net {net_name!r} contains no connected copper components")
    union_find = UnionFind(component_count)
    warnings: list[str] = []

    def connect(ids: list[int]) -> None:
        for other in ids[1:]:
            union_find.union(ids[0], other)

    net_pads = [
        pad
        for footprint in board.GetFootprints()
        for pad in footprint.Pads()
        if pad.GetNetname() == net_name
    ]
    copper_ids = [layer.layer_id for layer in layers]
    for pad in net_pads:
        on_layers = [layer_id for layer_id in copper_ids if pad.IsOnLayer(layer_id)]
        if len(on_layers) < 2:
            continue
        x_mm, y_mm = point_mm(pad.GetPosition())
        radius = max(pad_radius_mm(pad, layer_id) for layer_id in on_layers)
        connect(touching_component_ids(layers, offsets, on_layers, x_mm, y_mm, radius))

    for item in board.GetTracks():
        if not isinstance(item, pcbnew.PCB_VIA) or item.GetNetname() != net_name:
            continue
        on_layers = [layer_id for layer_id in copper_ids if item.IsOnLayer(layer_id)]
        x_mm, y_mm = point_mm(item.GetPosition())
        radius = mm(item.GetWidth(item.TopLayer())) * 0.45
        connect(touching_component_ids(layers, offsets, on_layers, x_mm, y_mm, radius))

    source_ids: list[int] = []
    for layer in layers:
        if not source.pad.IsOnLayer(layer.layer_id):
            continue
        source_ids.extend(
            touching_component_ids(
                layers,
                offsets,
                [layer.layer_id],
                source.x_mm,
                source.y_mm,
                pad_radius_mm(source.pad, layer.layer_id),
            )
        )
    connect(source_ids)
    if not source_ids:
        raise AnalysisError(f"Source {source.selector} does not touch extracted copper")
    source_root = union_find.find(source_ids[0])

    selected: dict[str, Any] = {}
    for layer in layers:
        parts = [
            component
            for index, component in enumerate(layer.components)
            if union_find.find(offsets[layer.name] + index) == source_root
        ]
        selected[layer.name] = unary_union(parts) if parts else GeometryCollection()

    for sink in sinks:
        sink_ids: list[int] = []
        for layer in layers:
            if not sink.pad.IsOnLayer(layer.layer_id):
                continue
            sink_ids.extend(
                touching_component_ids(
                    layers,
                    offsets,
                    [layer.layer_id],
                    sink.x_mm,
                    sink.y_mm,
                    pad_radius_mm(sink.pad, layer.layer_id),
                )
            )
        if not sink_ids or all(
            union_find.find(item) != source_root for item in sink_ids
        ):
            raise AnalysisError(
                f"Sink {sink.selector} is not copper-connected to source {source.selector}"
            )

    omitted = sum(
        1
        for layer in layers
        for index in range(len(layer.components))
        if union_find.find(offsets[layer.name] + index) != source_root
    )
    if omitted:
        warnings.append(
            f"Excluded {omitted} disconnected copper island component(s) from the analysis"
        )
    return selected, warnings


def ipc2221_current_a(
    width_mm: float,
    thickness_um: float,
    temperature_rise_c: float,
    *,
    external: bool,
) -> float:
    if width_mm <= 0 or thickness_um <= 0 or temperature_rise_c <= 0:
        return 0.0
    area_sq_mil = (width_mm / MM_PER_MIL) * (thickness_um / UM_PER_MIL)
    coefficient = IPC_EXTERNAL_K if external else IPC_INTERNAL_K
    return coefficient * temperature_rise_c**0.44 * area_sq_mil**0.725


def analyze_cut(
    *,
    kind: str,
    index: int,
    distance_mm: float | None,
    start_mm: tuple[float, float],
    end_mm: tuple[float, float],
    active_current_a: float,
    layers: list[LayerCopper],
    selected_geometry: dict[str, Any],
    temperature_rises_c: list[float],
) -> CutResult:
    line = LineString([start_mm, end_mm])
    widths: dict[str, float] = {}
    cross_sections: dict[str, float] = {}
    for layer in layers:
        width = selected_geometry[layer.name].intersection(line).length
        if width <= MIN_INTERSECTION_MM:
            continue
        widths[layer.name] = width
        cross_sections[layer.name] = width * layer.thickness_um / 1000.0
    total_cross_section = sum(cross_sections.values())
    shares = (
        {name: area / total_cross_section for name, area in cross_sections.items()}
        if total_cross_section > 0
        else {}
    )

    layer_capacities: dict[str, dict[float, float]] = {}
    for layer in layers:
        if layer.name not in widths:
            continue
        layer_capacities[layer.name] = {
            temperature: ipc2221_current_a(
                widths[layer.name],
                layer.thickness_um,
                temperature,
                external=layer.external,
            )
            for temperature in temperature_rises_c
        }
    natural: dict[float, float] = {}
    ideal: dict[float, float] = {}
    for temperature in temperature_rises_c:
        ideal[temperature] = sum(
            capacities[temperature] for capacities in layer_capacities.values()
        )
        natural[temperature] = min(
            (
                layer_capacities[name][temperature] / share
                for name, share in shares.items()
                if share > 0
            ),
            default=0.0,
        )
    return CutResult(
        kind=kind,
        index=index,
        distance_mm=distance_mm,
        start_mm=start_mm,
        end_mm=end_mm,
        active_current_a=active_current_a,
        widths_mm=widths,
        cross_sections_mm2=cross_sections,
        shares=shares,
        layer_capacities_a=layer_capacities,
        natural_capacity_a=natural,
        ideal_capacity_a=ideal,
    )


def vector_length(vector: tuple[float, float]) -> float:
    return math.hypot(*vector)


def dot(left: tuple[float, float], right: tuple[float, float]) -> float:
    return left[0] * right[0] + left[1] * right[1]


def source_pad_exit_distance_mm(
    source: Terminal, direction: tuple[float, float]
) -> float:
    """Return the furthest source-pad extent in the sweep direction."""
    pad_geometry = item_polygon(source.pad, source.pad.GetLayer())
    if pad_geometry.is_empty:
        raise AnalysisError(
            f"Could not extract copper geometry for source {source.selector}"
        )
    projections = [
        dot((x - source.x_mm, y - source.y_mm), direction)
        for polygon in polygon_components(pad_geometry)
        for x, y in polygon.exterior.coords
    ]
    if not projections:
        raise AnalysisError(f"Source {source.selector} has no usable pad boundary")
    return max(0.0, max(projections))


def automatic_cuts(
    source: Terminal,
    sinks: list[Terminal],
    layers: list[LayerCopper],
    selected_geometry: dict[str, Any],
    temperature_rises_c: list[float],
    scan_pitch_mm: float,
) -> tuple[list[CutResult], tuple[float, float], list[tuple[Terminal, float]], float]:
    if scan_pitch_mm <= 0:
        raise AnalysisError("Scan pitch must be positive")
    total_load = sum(sink.current_a for sink in sinks)
    if total_load <= 0:
        raise AnalysisError("At least one sink must have positive current")
    weighted_x = sum(sink.x_mm * sink.current_a for sink in sinks) / total_load
    weighted_y = sum(sink.y_mm * sink.current_a for sink in sinks) / total_load
    direction_raw = (weighted_x - source.x_mm, weighted_y - source.y_mm)
    direction_length = vector_length(direction_raw)
    if direction_length <= 1e-9:
        raise AnalysisError("Source and weighted sink centroid have the same position")
    direction = (
        direction_raw[0] / direction_length,
        direction_raw[1] / direction_length,
    )
    normal = (-direction[1], direction[0])
    sink_projections = [
        (
            sink,
            dot((sink.x_mm - source.x_mm, sink.y_mm - source.y_mm), direction),
        )
        for sink in sinks
    ]
    behind = [sink.selector for sink, projection in sink_projections if projection <= 0]
    if behind:
        raise AnalysisError(
            "The automatic directional sweep puts sink(s) behind the source: "
            + ", ".join(behind)
            + "; use a scenario with a more suitable source or manual cuts"
        )

    bounds = unary_union(
        [geometry for geometry in selected_geometry.values() if not geometry.is_empty]
    ).bounds
    span = max(bounds[2] - bounds[0], bounds[3] - bounds[1]) * 1.5 + 10.0
    farthest = max(projection for _, projection in sink_projections)
    source_exit_distance = source_pad_exit_distance_mm(source, direction)
    if source_exit_distance >= farthest:
        raise AnalysisError(
            f"Every sink lies within the load-side extent of source {source.selector}"
        )
    sweep_length = farthest - source_exit_distance
    count = max(1, math.ceil(sweep_length / scan_pitch_mm))
    cuts: list[CutResult] = []
    for index in range(count):
        distance = min(
            source_exit_distance + (index + 0.5) * scan_pitch_mm,
            farthest - 1e-6,
        )
        if distance <= 0:
            continue
        center = (
            source.x_mm + direction[0] * distance,
            source.y_mm + direction[1] * distance,
        )
        start = (center[0] - normal[0] * span, center[1] - normal[1] * span)
        end = (center[0] + normal[0] * span, center[1] + normal[1] * span)
        active_current = sum(
            sink.current_a
            for sink, projection in sink_projections
            if projection >= distance
        )
        if active_current <= 0:
            continue
        cuts.append(
            analyze_cut(
                kind="automatic",
                index=index,
                distance_mm=distance,
                start_mm=start,
                end_mm=end,
                active_current_a=active_current,
                layers=layers,
                selected_geometry=selected_geometry,
                temperature_rises_c=temperature_rises_c,
            )
        )
    if not cuts:
        raise AnalysisError("The automatic sweep produced no current-carrying cuts")
    return cuts, direction, sink_projections, source_exit_distance


def current_across_manual_cut(
    source: Terminal,
    sinks: list[Terminal],
    start: tuple[float, float],
    end: tuple[float, float],
) -> float:
    line_vector = (end[0] - start[0], end[1] - start[1])
    if vector_length(line_vector) <= 1e-9:
        raise AnalysisError("Manual cut endpoints must be different")

    def side(x_mm: float, y_mm: float) -> float:
        return line_vector[0] * (y_mm - start[1]) - line_vector[1] * (x_mm - start[0])

    source_side = side(source.x_mm, source.y_mm)
    if abs(source_side) <= 1e-9:
        raise AnalysisError("A manual cut passes through the source pad center")
    return sum(
        sink.current_a for sink in sinks if side(sink.x_mm, sink.y_mm) * source_side < 0
    )


def add_manual_cuts(
    cuts: list[CutResult],
    manual: list[tuple[tuple[float, float], tuple[float, float]]],
    source: Terminal,
    sinks: list[Terminal],
    layers: list[LayerCopper],
    selected_geometry: dict[str, Any],
    temperature_rises_c: list[float],
) -> None:
    for manual_index, (start, end) in enumerate(manual):
        active_current = current_across_manual_cut(source, sinks, start, end)
        if active_current <= 0:
            raise AnalysisError(
                f"Manual cut {manual_index + 1} does not separate the source from a sink"
            )
        cuts.append(
            analyze_cut(
                kind="manual",
                index=manual_index,
                distance_mm=None,
                start_mm=start,
                end_mm=end,
                active_current_a=active_current,
                layers=layers,
                selected_geometry=selected_geometry,
                temperature_rises_c=temperature_rises_c,
            )
        )


def estimate_resistance_and_loss(
    automatic: list[CutResult],
    sinks_with_projection: list[tuple[Terminal, float]],
    scan_pitch_mm: float,
    resistivity_ohm_m: float,
) -> dict[str, Any]:
    ordered = sorted(
        (cut for cut in automatic if cut.total_cross_section_mm2 > 0),
        key=lambda cut: cut.distance_mm or 0.0,
    )
    sink_drop = {sink.selector: 0.0 for sink, _ in sinks_with_projection}
    total_loss_w = 0.0
    integrated_resistance_ohm = 0.0
    for cut in ordered:
        area_m2 = cut.total_cross_section_mm2 * 1e-6
        segment_length_m = scan_pitch_mm * 1e-3
        segment_resistance = resistivity_ohm_m * segment_length_m / area_m2
        integrated_resistance_ohm += segment_resistance
        current = cut.active_current_a
        total_loss_w += current * current * segment_resistance
        distance = cut.distance_mm or 0.0
        voltage_increment = current * segment_resistance
        for sink, projection in sinks_with_projection:
            if projection >= distance:
                sink_drop[sink.selector] += voltage_increment
    return {
        "method": "one-dimensional directional slice integration at 20 C",
        "resistivity_ohm_m": resistivity_ohm_m,
        "integrated_to_farthest_sink_ohm": integrated_resistance_ohm,
        "total_copper_loss_w": total_loss_w,
        "sink_voltage_drop_v": sink_drop,
    }


def via_report(
    board: Any,
    net_name: str,
    selected_geometry: dict[str, Any],
    layers: list[LayerCopper],
    temperature_rises_c: list[float],
    resistivity_ohm_m: float,
) -> dict[str, Any]:
    plating_mm = mm(board.GetDesignSettings().GetHolePlatingThickness())
    board_thickness_mm = mm(board.GetDesignSettings().GetBoardThickness())
    vias: list[dict[str, Any]] = []
    for item in board.GetTracks():
        if not isinstance(item, pcbnew.PCB_VIA) or item.GetNetname() != net_name:
            continue
        x_mm, y_mm = point_mm(item.GetPosition())
        point = Point(x_mm, y_mm)
        if not any(
            not selected_geometry[layer.name].is_empty
            and selected_geometry[layer.name].distance(point)
            <= CONNECTIVITY_TOLERANCE_MM
            for layer in layers
        ):
            continue
        drill_mm = mm(item.GetDrillValue())
        diameter_mm = mm(item.GetWidth(item.TopLayer()))
        barrel_area_mm2 = math.pi * (drill_mm + plating_mm) * plating_mm
        resistance_ohm = (
            resistivity_ohm_m * board_thickness_mm * 1e-3 / (barrel_area_mm2 * 1e-6)
            if barrel_area_mm2 > 0
            else math.inf
        )
        width_equivalent_mm = barrel_area_mm2 / (plating_mm or math.inf)
        capacities = {
            temperature: ipc2221_current_a(
                width_equivalent_mm,
                plating_mm * 1000.0,
                temperature,
                external=False,
            )
            for temperature in temperature_rises_c
        }
        vias.append(
            {
                "x_mm": x_mm,
                "y_mm": y_mm,
                "span": [
                    board.GetLayerName(item.TopLayer()),
                    board.GetLayerName(item.BottomLayer()),
                ],
                "diameter_mm": diameter_mm,
                "drill_mm": drill_mm,
                "plating_um": plating_mm * 1000.0,
                "barrel_area_mm2": barrel_area_mm2,
                "through_length_mm": board_thickness_mm,
                "resistance_ohm": resistance_ohm,
                "ipc2221_internal_proxy_capacity_a": capacities,
            }
        )
    return {
        "count": len(vias),
        "board_hole_plating_um": plating_mm * 1000.0,
        "note": (
            "Via capacities apply IPC-2221's internal-conductor equation to barrel "
            "cross-sectional area as a screening proxy. Vias are inventoried, not "
            "summed into the planar aggregate capacity."
        ),
        "vias": vias,
    }


def cut_to_dict(cut: CutResult, temperatures: list[float]) -> dict[str, Any]:
    return {
        "kind": cut.kind,
        "index": cut.index,
        "distance_mm": cut.distance_mm,
        "line_mm": [list(cut.start_mm), list(cut.end_mm)],
        "active_current_a": cut.active_current_a,
        "total_cross_section_mm2": cut.total_cross_section_mm2,
        "layers": {
            name: {
                "width_mm": cut.widths_mm[name],
                "cross_section_mm2": cut.cross_sections_mm2[name],
                "natural_share": cut.shares[name],
                "current_at_requested_load_a": cut.active_current_a * cut.shares[name],
                "capacity_a": {
                    str(temperature): cut.layer_capacities_a[name][temperature]
                    for temperature in temperatures
                },
            }
            for name in cut.widths_mm
        },
        "natural_capacity_a": {
            str(temperature): cut.natural_capacity_a[temperature]
            for temperature in temperatures
        },
        "ideal_sharing_ceiling_a": {
            str(temperature): cut.ideal_capacity_a[temperature]
            for temperature in temperatures
        },
        "margin_ratio": {
            str(temperature): cut.margin(temperature) for temperature in temperatures
        },
    }


def collect_validity_warnings(
    cuts: list[CutResult],
    layers: list[LayerCopper],
    temperature_rises_c: list[float],
) -> list[str]:
    warnings = [
        "IPC-2221 is a screening relationship, not a coupled electro-thermal simulation or released board rating.",
        "The automatic analysis is a one-dimensional sweep normal to the source-to-load direction; inspect the SVG cuts.",
        "Natural sharing assumes local current division in proportion to copper cross-sectional area.",
        "The ideal-sharing ceiling assumes current can redistribute to use every layer at its individual thermal limit.",
        "Via transition capacity is reported separately and is not included in the planar multilayer aggregate.",
        "Connector contacts, component terminals, shunts, and enclosure airflow are outside this copper-only calculation.",
    ]
    by_name = {layer.name: layer for layer in layers}
    width_exceeded: set[str] = set()
    current_exceeded: set[str] = set()
    for cut in cuts:
        for name, width_mm in cut.widths_mm.items():
            layer = by_name[name]
            if width_mm / MM_PER_MIL > IPC_MAX_WIDTH_MIL:
                width_exceeded.add(name)
            current_limit = (
                IPC_MAX_EXTERNAL_CURRENT_A
                if layer.external
                else IPC_MAX_INTERNAL_CURRENT_A
            )
            if any(
                cut.layer_capacities_a[name][temperature] > current_limit
                for temperature in temperature_rises_c
            ):
                current_exceeded.add(name)
    if width_exceeded:
        warnings.append(
            "IPC-2221 width validity (400 mil / 10.16 mm) is exceeded on: "
            + ", ".join(sorted(width_exceeded))
        )
    if current_exceeded:
        warnings.append(
            "KiCad's stated IPC-2221 current validity is exceeded on: "
            + ", ".join(sorted(current_exceeded))
        )
    if any(temperature > 100 for temperature in temperature_rises_c):
        warnings.append("KiCad's stated 100 C temperature-rise validity is exceeded")
    return warnings


def _analyze_single_stackup(
    scenario: Scenario,
) -> tuple[dict[str, Any], dict[str, Any]]:
    if not scenario.sinks:
        raise AnalysisError("At least one sink is required")
    if any(sink.current_a <= 0 for sink in scenario.sinks):
        raise AnalysisError("Every sink current must be positive")
    selectors = [sink.selector for sink in scenario.sinks]
    if len(selectors) != len(set(selectors)):
        raise AnalysisError("Sink pad selectors must be unique")
    if scenario.source in set(selectors):
        raise AnalysisError("The source pad cannot also be a sink")
    if not scenario.temperature_rises_c or any(
        temperature <= 0 for temperature in scenario.temperature_rises_c
    ):
        raise AnalysisError("Temperature rises must be positive")
    scenario.temperature_rises_c = sorted(set(scenario.temperature_rises_c))
    if scenario.supply_voltage_v is not None and scenario.supply_voltage_v <= 0:
        raise AnalysisError("Supply voltage must be positive")
    board = load_board(scenario.board)
    source = terminal(board, scenario.source, scenario.net)
    sinks = [
        terminal(board, sink.selector, scenario.net, sink.current_a)
        for sink in scenario.sinks
    ]
    layers, warnings = extract_net_copper(
        board,
        scenario.board,
        scenario.net,
        scenario.include_layers,
        scenario.copper_overrides_um,
    )
    selected_geometry, connectivity_warnings = connected_net_geometry(
        board, scenario.net, layers, source, sinks
    )
    warnings.extend(connectivity_warnings)
    cuts, direction, sink_projections, source_exit_distance = automatic_cuts(
        source,
        sinks,
        layers,
        selected_geometry,
        scenario.temperature_rises_c,
        scenario.scan_pitch_mm,
    )
    automatic = list(cuts)
    add_manual_cuts(
        cuts,
        scenario.manual_cuts,
        source,
        sinks,
        layers,
        selected_geometry,
        scenario.temperature_rises_c,
    )
    empty_cuts = [cut for cut in cuts if cut.total_cross_section_mm2 <= 0]
    if empty_cuts:
        cut = empty_cuts[0]
        raise AnalysisError(
            f"{cut.kind.capitalize()} cut {cut.index} did not intersect connected "
            "copper; inspect the path definition or provide a better manual cut"
        )
    valid_cuts = cuts
    limiting = {
        temperature: min(valid_cuts, key=lambda cut: cut.margin(temperature))
        for temperature in scenario.temperature_rises_c
    }
    warnings.extend(
        collect_validity_warnings(valid_cuts, layers, scenario.temperature_rises_c)
    )
    resistance = estimate_resistance_and_loss(
        automatic,
        sink_projections,
        scenario.scan_pitch_mm,
        COPPER_RESISTIVITY_OHM_M,
    )
    vias = via_report(
        board,
        scenario.net,
        selected_geometry,
        layers,
        scenario.temperature_rises_c,
        COPPER_RESISTIVITY_OHM_M,
    )
    total_load = sum(sink.current_a for sink in sinks)
    try:
        board_display = str(scenario.board.resolve().relative_to(repository_root()))
    except ValueError:
        board_display = str(scenario.board)
    report: dict[str, Any] = {
        "schema_version": 1,
        "scenario": scenario.name,
        "board": board_display,
        "board_sha256": hashlib.sha256(scenario.board.read_bytes()).hexdigest(),
        "net": scenario.net,
        "model": {
            "thermal": "IPC-2221 / KiCad track-width relationship",
            "external_k": IPC_EXTERNAL_K,
            "internal_k": IPC_INTERNAL_K,
            "formula": "I = k * delta_T^0.44 * (width_mil * thickness_mil)^0.725",
            "aggregate_natural_sharing": (
                "minimum per-layer IPC capacity divided by local cross-sectional-area share"
            ),
        },
        "inputs": {
            "source": {
                "pad": source.selector,
                "position_mm": [source.x_mm, source.y_mm],
            },
            "sinks": [
                {
                    "pad": sink.selector,
                    "position_mm": [sink.x_mm, sink.y_mm],
                    "current_a": sink.current_a,
                }
                for sink in sinks
            ],
            "total_current_a": total_load,
            "supply_voltage_v": scenario.supply_voltage_v,
            "temperature_rises_c": scenario.temperature_rises_c,
            "scan_pitch_mm": scenario.scan_pitch_mm,
            "scan_direction": list(direction),
            "source_pad_exit_distance_mm": source_exit_distance,
            "first_automatic_cut_distance_mm": automatic[0].distance_mm,
            "selected_layers": [layer.name for layer in layers],
        },
        "layers": [
            {
                "name": layer.name,
                "kind": "external" if layer.external else "internal",
                "thickness_um": layer.thickness_um,
                "copper_weight_oz_per_sq_ft": copper_weight_oz(layer.thickness_um),
                "connected_copper_area_mm2": selected_geometry[layer.name].area,
            }
            for layer in layers
            if not selected_geometry[layer.name].is_empty
        ],
        "limiting_cuts": {
            str(temperature): cut_to_dict(limiting[temperature], [temperature])
            for temperature in scenario.temperature_rises_c
        },
        "cuts": [cut_to_dict(cut, scenario.temperature_rises_c) for cut in valid_cuts],
        "electrical_estimate": resistance,
        "via_screening": vias,
        "warnings": list(dict.fromkeys(warnings)),
    }
    if scenario.supply_voltage_v:
        report["inputs"]["distributed_load_power_w"] = (
            scenario.supply_voltage_v * total_load
        )
        report["electrical_estimate"]["sink_voltage_drop_percent"] = {
            selector: drop / scenario.supply_voltage_v * 100.0
            for selector, drop in resistance["sink_voltage_drop_v"].items()
        }
    render_context = {
        "layers": layers,
        "selected_geometry": selected_geometry,
        "source": source,
        "sinks": sinks,
        "cuts": valid_cuts,
        "limiting": limiting,
        "source_pad_exit_distance_mm": source_exit_distance,
    }
    return report, render_context


def concise_copper_label(thickness_um: float) -> str:
    return copper_thickness_label(thickness_um).replace(" copper", "")


def stackup_thickness_summary(layers: list[dict[str, Any]]) -> str:
    external = sorted(
        {
            float(layer["thickness_um"])
            for layer in layers
            if layer["kind"] == "external"
        }
    )
    internal = sorted(
        {
            float(layer["thickness_um"])
            for layer in layers
            if layer["kind"] == "internal"
        }
    )
    if len(external) == 1 and len(internal) == 1:
        return (
            f"Outer {concise_copper_label(external[0])} · "
            f"inner {concise_copper_label(internal[0])}"
        )
    return " · ".join(
        f"{layer['name']} {concise_copper_label(float(layer['thickness_um']))}"
        for layer in layers
    )


def scenario_for_stackup(scenario: Scenario, stackup: StackupSpec) -> Scenario:
    return Scenario(
        name=scenario.name,
        board=scenario.board,
        net=scenario.net,
        source=scenario.source,
        sinks=list(scenario.sinks),
        supply_voltage_v=scenario.supply_voltage_v,
        temperature_rises_c=list(scenario.temperature_rises_c),
        scan_pitch_mm=scenario.scan_pitch_mm,
        include_layers=(
            list(scenario.include_layers)
            if scenario.include_layers is not None
            else None
        ),
        copper_overrides_um={
            **scenario.copper_overrides_um,
            **stackup.copper_overrides_um,
        },
        stackups=[],
        manual_cuts=list(scenario.manual_cuts),
    )


def estimated_temperature_rise_c(
    cut: CutResult, reference_temperature_rise_c: float
) -> float:
    """Invert IPC-2221 to estimate local rise at the requested cut current."""
    if cut.active_current_a <= 0:
        return 0.0
    reference_capacity = cut.natural_capacity_a[reference_temperature_rise_c]
    if reference_capacity <= 0:
        return math.inf
    return reference_temperature_rise_c * (
        cut.active_current_a / reference_capacity
    ) ** (1.0 / 0.44)


def analyze_scenario(scenario: Scenario) -> tuple[dict[str, Any], dict[str, Any]]:
    stackup_specs = scenario.stackups or [StackupSpec("board", "Board stackup")]
    analyzed: list[tuple[StackupSpec, dict[str, Any], dict[str, Any]]] = []
    for stackup in stackup_specs:
        child = scenario_for_stackup(scenario, stackup)
        stackup_report, stackup_context = _analyze_single_stackup(child)
        analyzed.append((stackup, stackup_report, stackup_context))

    primary_report = analyzed[0][1]
    primary_context = analyzed[0][2]
    stackup_reports: list[dict[str, Any]] = []
    stackup_contexts: list[dict[str, Any]] = []
    combined_warnings: list[str] = []
    reference_temperature = min(primary_report["inputs"]["temperature_rises_c"])
    for index, (stackup, report, context) in enumerate(analyzed):
        color = STACKUP_COLORS[index % len(STACKUP_COLORS)]
        thermal_cuts = [
            (cut, estimated_temperature_rise_c(cut, reference_temperature))
            for cut in context["cuts"]
            if cut.kind == "automatic" and cut.distance_mm is not None
        ]
        peak_cut, peak_delta = max(thermal_cuts, key=lambda item: item[1])
        entry = {
            "name": stackup.name,
            "label": stackup.label,
            "color": color,
            "thickness_summary": stackup_thickness_summary(report["layers"]),
            "layers": report["layers"],
            "limiting_cuts": report["limiting_cuts"],
            "cuts": report["cuts"],
            "electrical_estimate": report["electrical_estimate"],
            "temperature_rise_proxy": {
                "method": (
                    "local inversion of the IPC-2221 natural-sharing capacity; "
                    "does not model thermal spreading or airflow"
                ),
                "reference_temperature_rise_c": reference_temperature,
                "peak_delta_c": peak_delta,
                "peak_cut": {
                    "kind": peak_cut.kind,
                    "index": peak_cut.index,
                    "distance_mm": peak_cut.distance_mm,
                },
            },
            "warnings": report["warnings"],
        }
        stackup_reports.append(entry)
        stackup_contexts.append(
            {
                "name": stackup.name,
                "label": stackup.label,
                "color": color,
                "layers": context["layers"],
                "cuts": context["cuts"],
                "limiting": context["limiting"],
            }
        )
        combined_warnings.extend(report["warnings"])

    primary_report["schema_version"] = 2
    primary_report["stackups"] = stackup_reports
    primary_report["warnings"] = list(dict.fromkeys(combined_warnings))
    primary_context["stackups"] = stackup_contexts
    return primary_report, primary_context


def geometry_svg_path(geometry: Any) -> str:
    commands: list[str] = []
    for polygon in polygon_components(geometry):
        for ring in [polygon.exterior, *polygon.interiors]:
            coordinates = list(ring.coords)
            if not coordinates:
                continue
            commands.append(f"M {coordinates[0][0]:.4f},{coordinates[0][1]:.4f}")
            commands.extend(f"L {x:.4f},{y:.4f}" for x, y in coordinates[1:])
            commands.append("Z")
    return " ".join(commands)


def line_geometry_svg_path(geometry: Any) -> str:
    """Return SVG commands for the linear parts of a Shapely geometry."""
    if geometry.is_empty:
        return ""
    if geometry.geom_type in {"LineString", "LinearRing"}:
        coordinates = list(geometry.coords)
        if len(coordinates) < 2:
            return ""
        commands = [f"M {coordinates[0][0]:.4f},{coordinates[0][1]:.4f}"]
        commands.extend(f"L {x:.4f},{y:.4f}" for x, y in coordinates[1:])
        return " ".join(commands)
    return " ".join(
        path
        for part in getattr(geometry, "geoms", [])
        if (path := line_geometry_svg_path(part))
    )


def svg_board_transform(
    bounds: tuple[float, float, float, float],
    panel: tuple[float, float, float, float],
    padding_px: float = 24.0,
) -> tuple[float, float, float]:
    """Fit board-coordinate geometry into a pixel-coordinate SVG panel."""
    min_x, min_y, max_x, max_y = bounds
    panel_x, panel_y, panel_width, panel_height = panel
    geometry_width = max(max_x - min_x, 1e-6)
    geometry_height = max(max_y - min_y, 1e-6)
    available_width = max(panel_width - padding_px * 2, 1.0)
    available_height = max(panel_height - padding_px * 2, 1.0)
    scale = min(available_width / geometry_width, available_height / geometry_height)
    translate_x = panel_x + (panel_width - geometry_width * scale) / 2.0 - min_x * scale
    translate_y = (
        panel_y + (panel_height - geometry_height * scale) / 2.0 - min_y * scale
    )
    return translate_x, translate_y, scale


def svg_panel(svg: list[str], x: float, y: float, width: float, height: float) -> None:
    svg.append(
        f'<rect x="{x:.1f}" y="{y:.1f}" width="{width:.1f}" height="{height:.1f}" '
        'rx="12" class="panel"/>'
    )


def append_board_geometry(
    svg: list[str],
    geometry: Any,
    bounds: tuple[float, float, float, float],
    panel: tuple[float, float, float, float],
    fill: str,
    stroke: str,
    title: str,
    stackup_cuts: list[dict[str, Any]],
    marker_terminals: list[tuple[str, Terminal]] | None = None,
    vias: list[dict[str, Any]] | None = None,
) -> None:
    """Draw one board view with every stackup's limiting cut."""
    translate_x, translate_y, scale = svg_board_transform(bounds, panel)
    path = geometry_svg_path(geometry.simplify(0.015, preserve_topology=True))
    min_x, min_y, max_x, max_y = bounds
    guide_padding = max(max_x - min_x, max_y - min_y) * 0.015
    svg.append(
        f'<g transform="translate({translate_x:.4f} {translate_y:.4f}) '
        f'scale({scale:.6f})">'
    )
    svg.append(
        f'<path d="{path}" fill="{fill}" stroke="{stroke}" fill-rule="evenodd" '
        'vector-effect="non-scaling-stroke" stroke-width="1.2">'
        f"<title>{html.escape(title)}</title></path>"
    )
    signatures = [
        tuple(
            round(value, 6)
            for point in (item["cut"].start_mm, item["cut"].end_mm)
            for value in point
        )
        for item in stackup_cuts
    ]
    for cut_index, item in enumerate(stackup_cuts):
        limiting_cut: CutResult = item["cut"]
        cut_line = LineString([limiting_cut.start_mm, limiting_cut.end_mm])
        cut_guide = line_geometry_svg_path(
            cut_line.intersection(
                box(
                    min_x - guide_padding,
                    min_y - guide_padding,
                    max_x + guide_padding,
                    max_y + guide_padding,
                )
            )
        )
        clipped_cut = line_geometry_svg_path(geometry.intersection(cut_line))
        signature = signatures[cut_index]
        coincident_total = signatures.count(signature)
        coincident_rank = signatures[:cut_index].count(signature)
        remaining_layers = coincident_total - coincident_rank - 1
        guide_width = 2.0 + remaining_layers * 2.0
        cut_width = 4.0 + remaining_layers * 3.0
        common = (
            f'data-stackup="{html.escape(item["name"])}" '
            f'stroke="{item["color"]}" vector-effect="non-scaling-stroke"'
        )
        label = html.escape(item["label"])
        if cut_guide:
            svg.append(
                f'<path d="{cut_guide}" class="cut-guide" data-role="cut-guide" '
                f'{common} stroke-width="{guide_width:.1f}">'
                f"<title>{label} limiting cut guide "
                f"{limiting_cut.kind}:{limiting_cut.index}</title></path>"
            )
        if clipped_cut:
            svg.append(
                f'<path d="{clipped_cut}" class="limiting-cut" '
                f'data-role="limiting-cut" {common} stroke-width="{cut_width:.1f}">'
                f"<title>{label} limiting cut "
                f"{limiting_cut.kind}:{limiting_cut.index}</title></path>"
            )
    if vias:
        for via in vias:
            radius = max(via["diameter_mm"] / 2.0, 3.0 / scale)
            svg.append(
                f'<circle cx="{via["x_mm"]:.4f}" cy="{via["y_mm"]:.4f}" '
                f'r="{radius:.4f}" class="via" vector-effect="non-scaling-stroke"/>'
            )
    if marker_terminals:
        marker_radius = 10.5 / scale
        marker_font_size = 12.0 / scale
        marker_stroke = 2.0 / scale
        for marker, terminal_value in marker_terminals:
            marker_class = "source-marker" if marker == "S" else "sink-marker"
            svg.append(
                f'<circle cx="{terminal_value.x_mm:.4f}" '
                f'cy="{terminal_value.y_mm:.4f}" r="{marker_radius:.4f}" '
                f'class="{marker_class}" stroke-width="{marker_stroke:.5f}"/>'
            )
            svg.append(
                f'<text x="{terminal_value.x_mm:.4f}" '
                f'y="{terminal_value.y_mm + marker_font_size * 0.34:.4f}" '
                f'font-size="{marker_font_size:.5f}" class="terminal-marker-text">'
                f"{marker}</text>"
            )
    svg.append("</g>")


def append_capacity_profile(
    svg: list[str],
    stackups: list[dict[str, Any]],
    temperatures: list[float],
    source: Terminal,
    sinks: list[Terminal],
    direction: tuple[float, float],
    source_exit_distance_mm: float,
    panel: tuple[float, float, float, float],
) -> None:
    """Draw the conservative-temperature capacity profile for each stackup."""
    primary_temperature = min(temperatures)
    automatic = sorted(
        (
            cut
            for cut in stackups[0]["cuts"]
            if cut.kind == "automatic" and cut.distance_mm is not None
        ),
        key=lambda cut: cut.distance_mm or 0.0,
    )
    if not automatic:
        return
    panel_x, panel_y, panel_width, panel_height = panel
    plot_x = panel_x + 92.0
    plot_y = panel_y + 112.0
    plot_width = panel_width - 128.0
    plot_height = panel_height - 180.0
    maximum_distance = max(cut.distance_mm or 0.0 for cut in automatic)
    plotted_values = [cut.active_current_a for cut in automatic]
    for stackup in stackups:
        plotted_values.extend(
            cut.natural_capacity_a[primary_temperature]
            for cut in stackup["cuts"]
            if cut.kind == "automatic" and cut.distance_mm is not None
        )
    maximum_current = max(plotted_values) * 1.1
    maximum_current = max(maximum_current, 1.0)

    def chart_x(distance: float) -> float:
        return plot_x + distance / maximum_distance * plot_width

    def chart_y(current: float) -> float:
        return plot_y + plot_height - current / maximum_current * plot_height

    svg.append('<g id="capacity-profile">')
    svg.append(
        f'<text x="{panel_x + 20:.1f}" y="{panel_y + 30:.1f}" class="panel-title">'
        f"{primary_temperature:g} °C capacity by stackup along the source-to-load sweep</text>"
    )
    svg.append(
        f'<text x="{panel_x + 20:.1f}" y="{panel_y + 50:.1f}" class="hint">'
        "Each colored profile and limiting cut corresponds to one stackup.</text>"
    )
    legend_entries = [
        (stackup["color"], f"{stackup['label']} capacity") for stackup in stackups
    ]
    legend_entries.append(("#17212b", "load crossing each cut"))
    legend_y = panel_y + 82.0
    legend_width = plot_width / len(legend_entries)
    svg.append('<g id="capacity-profile-legend" data-position="outside-plot">')
    for index, (color, label) in enumerate(legend_entries):
        legend_x = plot_x + index * legend_width
        svg.append(
            f'<line x1="{legend_x:.1f}" y1="{legend_y:.1f}" '
            f'x2="{legend_x + 28:.1f}" y2="{legend_y:.1f}" '
            f'stroke="{color}" stroke-width="3"/>'
        )
        svg.append(
            f'<text x="{legend_x + 38:.1f}" y="{legend_y + 5:.1f}" '
            f'class="legend-text">{html.escape(label)}</text>'
        )
    svg.append("</g>")
    svg.append(
        f'<rect id="capacity-profile-plot" x="{plot_x:.1f}" y="{plot_y:.1f}" '
        f'width="{plot_width:.1f}" height="{plot_height:.1f}" fill="none"/>'
    )
    for tick in range(6):
        current = maximum_current * tick / 5.0
        y_value = chart_y(current)
        svg.append(
            f'<line x1="{plot_x:.1f}" y1="{y_value:.1f}" '
            f'x2="{plot_x + plot_width:.1f}" y2="{y_value:.1f}" class="grid"/>'
        )
        svg.append(
            f'<text x="{plot_x - 12:.1f}" y="{y_value + 5:.1f}" '
            f'class="axis-label" text-anchor="end">{current:.0f}</text>'
        )
    for tick in range(6):
        distance = maximum_distance * tick / 5.0
        x_value = chart_x(distance)
        svg.append(
            f'<text x="{x_value:.1f}" y="{plot_y + plot_height + 25:.1f}" '
            f'class="axis-label" text-anchor="middle">{distance:.0f}</text>'
        )
    source_exit_x = chart_x(min(source_exit_distance_mm, maximum_distance))
    svg.append(
        f'<line x1="{source_exit_x:.1f}" y1="{plot_y:.1f}" '
        f'x2="{source_exit_x:.1f}" y2="{plot_y + plot_height:.1f}" '
        'class="source-pad-edge"/>'
    )
    svg.append(
        f'<text x="{source_exit_x + 6:.1f}" y="{plot_y - 9:.1f}" '
        'class="source-pad-edge-label">source pad exit</text>'
    )
    y_axis_x = panel_x + 29.0
    y_axis_y = plot_y + plot_height / 2.0
    svg.append(
        f'<text x="{y_axis_x:.1f}" y="{y_axis_y:.1f}" class="axis-title" '
        f'text-anchor="middle" transform="rotate(-90 {y_axis_x:.1f} {y_axis_y:.1f})">'
        "current and capacity (A)</text>"
    )
    svg.append(
        f'<text x="{plot_x + plot_width / 2:.1f}" '
        f'y="{plot_y + plot_height + 49:.1f}" class="axis-label" '
        'text-anchor="middle">distance from source (mm)</text>'
    )
    for sink_index, sink in enumerate(sinks, start=1):
        projection = dot((sink.x_mm - source.x_mm, sink.y_mm - source.y_mm), direction)
        projection = min(max(projection, 0.0), maximum_distance)
        sink_x = chart_x(projection)
        svg.append(
            f'<line x1="{sink_x:.1f}" y1="{plot_y:.1f}" '
            f'x2="{sink_x:.1f}" y2="{plot_y + plot_height:.1f}" class="sink-guide"/>'
        )
        svg.append(
            f'<text x="{sink_x:.1f}" y="{plot_y - 9:.1f}" '
            f'class="sink-guide-label" text-anchor="middle">{sink_index}</text>'
        )

    load_points = " ".join(
        f"{chart_x(cut.distance_mm or 0.0):.2f},{chart_y(cut.active_current_a):.2f}"
        for cut in automatic
    )
    svg.append(f'<polyline points="{load_points}" class="profile-load"/>')
    for stackup in stackups:
        stackup_automatic = sorted(
            (
                cut
                for cut in stackup["cuts"]
                if cut.kind == "automatic" and cut.distance_mm is not None
            ),
            key=lambda cut: cut.distance_mm or 0.0,
        )
        points = " ".join(
            f"{chart_x(cut.distance_mm or 0.0):.2f},"
            f"{chart_y(cut.natural_capacity_a[primary_temperature]):.2f}"
            for cut in stackup_automatic
        )
        svg.append(
            f'<polyline points="{points}" fill="none" '
            f'stroke="{stackup["color"]}" stroke-width="3" '
            f'data-role="stackup-capacity" '
            f'data-stackup="{html.escape(stackup["name"])}" '
            'stroke-linejoin="round"/>'
        )

    bottlenecks = [stackup["limiting"][primary_temperature] for stackup in stackups]
    distances = [
        round(cut.distance_mm, 6) if cut.distance_mm is not None else None
        for cut in bottlenecks
    ]
    labeled_distances: set[float] = set()
    for stackup_index, (stackup, bottleneck) in enumerate(
        zip(stackups, bottlenecks, strict=True)
    ):
        if bottleneck.distance_mm is None:
            continue
        distance_key = round(bottleneck.distance_mm, 6)
        coincident_total = distances.count(distance_key)
        coincident_rank = distances[:stackup_index].count(distance_key)
        remaining_layers = coincident_total - coincident_rank - 1
        bottleneck_x = chart_x(bottleneck.distance_mm)
        svg.append(
            f'<line x1="{bottleneck_x:.1f}" y1="{plot_y:.1f}" '
            f'x2="{bottleneck_x:.1f}" y2="{plot_y + plot_height:.1f}" '
            f'class="profile-limit" stroke="{stackup["color"]}" '
            f'stroke-width="{2.0 + remaining_layers * 3.0:.1f}" '
            f'data-role="profile-limiting-cut" '
            f'data-stackup="{html.escape(stackup["name"])}"/>'
        )
        if distance_key not in labeled_distances:
            same_cut_labels = [
                candidate["label"]
                for candidate, candidate_cut in zip(stackups, bottlenecks, strict=True)
                if candidate_cut.distance_mm is not None
                and round(candidate_cut.distance_mm, 6) == distance_key
            ]
            label = (
                "both stackups limit here"
                if len(same_cut_labels) == len(stackups) and len(stackups) == 2
                else " / ".join(same_cut_labels) + " limiting cut"
            )
            svg.append(
                f'<text x="{bottleneck_x + 7:.1f}" '
                f'y="{plot_y + plot_height - 10:.1f}" '
                f'class="limit-label">{html.escape(label)}</text>'
            )
            labeled_distances.add(distance_key)

    svg.append("</g>")


def thermal_color(fraction: float) -> str:
    fraction = min(max(fraction, 0.0), 1.0)
    for (left_stop, left_color), (right_stop, right_color) in pairwise(THERMAL_PALETTE):
        if fraction > right_stop:
            continue
        span = right_stop - left_stop
        position = (fraction - left_stop) / span if span else 0.0
        channels = tuple(
            round(left + (right - left) * position)
            for left, right in zip(left_color, right_color, strict=True)
        )
        return "#" + "".join(f"{channel:02x}" for channel in channels)
    return "#" + "".join(f"{channel:02x}" for channel in THERMAL_PALETTE[-1][1])


def append_temperature_rise_heatmaps(
    svg: list[str],
    geometry: Any,
    bounds: tuple[float, float, float, float],
    stackups: list[dict[str, Any]],
    stackup_reports: list[dict[str, Any]],
    source: Terminal,
    direction: tuple[float, float],
    reference_temperature: float,
    panel: tuple[float, float, float, float],
) -> None:
    """Draw aggregate copper colored by an IPC-derived local rise proxy."""
    panel_x, panel_y, panel_width, panel_height = panel
    profiles: list[list[tuple[CutResult, float]]] = []
    for stackup in stackups:
        profile = [
            (cut, estimated_temperature_rise_c(cut, reference_temperature))
            for cut in stackup["cuts"]
            if cut.kind == "automatic" and cut.distance_mm is not None
        ]
        profile.sort(key=lambda item: item[0].distance_mm or 0.0)
        profiles.append(profile)
    maximum_delta = max(delta for profile in profiles for _cut, delta in profile)
    scale_maximum = max(5.0, math.ceil(maximum_delta / 5.0) * 5.0)

    svg.append('<g id="temperature-rise-heatmaps">')
    svg.append(
        f'<text x="{panel_x + 20:.1f}" y="{panel_y + 30:.1f}" '
        'class="panel-title">Estimated temperature-rise proxy at requested load</text>'
    )
    svg.append(
        f'<text x="{panel_x + 20:.1f}" y="{panel_y + 51:.1f}" class="hint">'
        "IPC-2221 capacity inverted at each sweep cut; colors omit thermal "
        "spreading, components, airflow, and enclosure effects.</text>"
    )
    row_height = 140.0
    row_start = panel_y + 68.0
    path = geometry_svg_path(geometry.simplify(0.015, preserve_topology=True))
    for index, (stackup, stackup_report, profile) in enumerate(
        zip(stackups, stackup_reports, profiles, strict=True)
    ):
        row_y = row_start + index * row_height
        peak_delta = stackup_report["temperature_rise_proxy"]["peak_delta_c"]
        svg.append(
            f'<text x="{panel_x + 24:.1f}" y="{row_y + 18:.1f}" '
            f'class="stackup-title">{html.escape(stackup["label"])} · '
            f"peak ΔT ≈ {peak_delta:.1f} °C</text>"
        )
        geometry_panel = (
            panel_x + 20.0,
            row_y + 27.0,
            panel_width - 40.0,
            row_height - 35.0,
        )
        translate_x, translate_y, scale = svg_board_transform(
            bounds, geometry_panel, padding_px=10.0
        )
        first_distance = profile[0][0].distance_mm or 0.0
        last_distance = profile[-1][0].distance_mm or first_distance + 1.0
        span = max(last_distance - first_distance, 1e-9)
        gradient_id = "thermal-gradient-" + re.sub(
            r"[^A-Za-z0-9_-]", "-", stackup["name"]
        )
        start_x = source.x_mm + direction[0] * first_distance
        start_y = source.y_mm + direction[1] * first_distance
        end_x = source.x_mm + direction[0] * last_distance
        end_y = source.y_mm + direction[1] * last_distance
        svg.append("<defs>")
        svg.append(
            f'<linearGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" '
            f'x1="{start_x:.4f}" y1="{start_y:.4f}" '
            f'x2="{end_x:.4f}" y2="{end_y:.4f}" spreadMethod="pad">'
        )
        for cut, delta in profile:
            offset = ((cut.distance_mm or first_distance) - first_distance) / span
            svg.append(
                f'<stop offset="{offset:.6f}" '
                f'stop-color="{thermal_color(delta / scale_maximum)}"/>'
            )
        svg.append("</linearGradient></defs>")
        svg.append(
            f'<g transform="translate({translate_x:.4f} {translate_y:.4f}) '
            f'scale({scale:.6f})">'
        )
        svg.append(
            f'<path d="{path}" fill="url(#{gradient_id})" stroke="#52616d" '
            'fill-rule="evenodd" vector-effect="non-scaling-stroke" '
            'stroke-width="1.2" data-role="temperature-rise-map" '
            f'data-stackup="{html.escape(stackup["name"])}"/>'
        )
        svg.append("</g>")

    scale_id = "temperature-rise-color-scale"
    bar_width = min(620.0, panel_width - 220.0)
    bar_x = panel_x + (panel_width - bar_width) / 2.0
    bar_y = panel_y + panel_height - 45.0
    svg.append("<defs>")
    svg.append(f'<linearGradient id="{scale_id}" x1="0%" y1="0%" x2="100%" y2="0%">')
    for stop, _color in THERMAL_PALETTE:
        svg.append(
            f'<stop offset="{stop * 100:.1f}%" stop-color="{thermal_color(stop)}"/>'
        )
    svg.append("</linearGradient></defs>")
    svg.append(
        f'<text x="{bar_x:.1f}" y="{bar_y - 9:.1f}" class="axis-title">'
        "Estimated ΔT above ambient (°C) · shared scale</text>"
    )
    svg.append(
        f'<rect x="{bar_x:.1f}" y="{bar_y:.1f}" width="{bar_width:.1f}" '
        f'height="14" rx="7" fill="url(#{scale_id})"/>'
    )
    for tick in range(5):
        fraction = tick / 4.0
        tick_x = bar_x + fraction * bar_width
        svg.append(
            f'<text x="{tick_x:.1f}" y="{bar_y + 32:.1f}" class="axis-label" '
            f'text-anchor="middle">{scale_maximum * fraction:.1f}</text>'
        )
    svg.append("</g>")


def write_svg(
    report: dict[str, Any], context: dict[str, Any], destination: Path
) -> None:
    layers: list[LayerCopper] = context["layers"]
    selected_geometry: dict[str, Any] = context["selected_geometry"]
    source: Terminal = context["source"]
    sinks: list[Terminal] = context["sinks"]
    stackups: list[dict[str, Any]] = context["stackups"]
    stackup_reports = report["stackups"]
    temperatures = [float(value) for value in report["inputs"]["temperature_rises_c"]]
    primary_temperature = min(temperatures)
    stackup_cuts = [
        {
            "name": stackup["name"],
            "label": stackup["label"],
            "color": stackup["color"],
            "cut": stackup["limiting"][primary_temperature],
        }
        for stackup in stackups
    ]
    combined = unary_union(
        [geometry for geometry in selected_geometry.values() if not geometry.is_empty]
    )
    nonempty_layers = [
        layer for layer in layers if not selected_geometry[layer.name].is_empty
    ]
    svg_width = 1400
    margin = 24.0
    header_height = 220.0
    overview_height = 420.0
    layer_panel_height = 260.0
    layer_rows = math.ceil(len(nonempty_layers) / 2)
    section_gap = 22.0
    layer_section_y = margin + header_height + overview_height + section_gap * 2
    profile_y = layer_section_y + 34.0 + layer_rows * (layer_panel_height + 14.0)
    profile_height = 430.0
    thermal_y = profile_y + profile_height + section_gap
    thermal_height = 150.0 + len(stackups) * 140.0
    svg_height = math.ceil(thermal_y + thermal_height + margin)
    title = html.escape(f"{report['scenario']}: {report['net']} copper capacity")
    svg: list[str] = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{svg_width}" '
            f'height="{svg_height}" viewBox="0 0 {svg_width} {svg_height}" '
            'style="max-width:100%;height:auto" '
            'role="img" aria-labelledby="title description">'
        ),
        f'<title id="title">{title}</title>',
        (
            '<desc id="description">Connected copper overview, separate layer '
            "views, and a capacity profile with the limiting cross-section highlighted.</desc>"
        ),
        """<style>
            text { font-family: Inter, ui-sans-serif, system-ui, sans-serif; fill: #17212b; }
            .page { fill: #f3f6f8; }
            .panel { fill: #ffffff; stroke: #d6dee4; stroke-width: 1.2; }
            .report-title { font-size: 30px; font-weight: 700; }
            .subtitle { font-size: 15px; fill: #52616d; }
            .panel-title { font-size: 20px; font-weight: 650; }
            .hint { font-size: 13px; fill: #647580; }
            .metric-label { font-size: 12px; fill: #65747e; }
            .metric-value { font-size: 17px; font-weight: 700; }
            .metric-detail { font-size: 13px; font-weight: 650; }
            .metric-card { stroke-width: 1.2; }
            .stackup-card { fill: #ffffff; stroke: #d6dee4; stroke-width: 1.2; }
            .stackup-title { font-size: 16px; font-weight: 700; }
            .stackup-detail { font-size: 12px; fill: #647580; }
            .stackup-result { font-size: 14px; font-weight: 650; }
            .pass-card { fill: #effaf5; stroke: #a8dbc4; }
            .fail-card { fill: #fff2f2; stroke: #e7b5b8; }
            .pass { fill: #147d50; }
            .fail { fill: #bc2f36; }
            .terminal-key { font-size: 14px; }
            .terminal-detail { font-size: 12px; fill: #647580; }
            .source-marker { fill: #17212b; stroke: #ffffff; }
            .sink-marker { fill: #26547c; stroke: #ffffff; }
            .terminal-marker-text { fill: #ffffff; font-weight: 700; text-anchor: middle; }
            .cut-guide { fill: none; stroke-dasharray: 7 5; stroke-linecap: round; opacity: 0.72; }
            .limiting-cut { fill: none; stroke-linecap: round; }
            .via { fill: none; stroke: #7b2cbf; stroke-width: 2; }
            .cut-key { stroke-width: 4; stroke-linecap: round; }
            .grid { stroke: #e4e9ed; stroke-width: 1; }
            .axis-label { font-size: 12px; fill: #65747e; }
            .axis-title { font-size: 13px; fill: #52616d; }
            .legend-text { font-size: 13px; }
            .profile-load { fill: none; stroke: #17212b; stroke-width: 3; stroke-linejoin: round; }
            .profile-limit { stroke-dasharray: 7 5; }
            .limit-label { font-size: 12px; fill: #bc2f36; font-weight: 650; }
            .sink-guide { stroke: #8aa1b1; stroke-width: 1; stroke-dasharray: 3 5; }
            .sink-guide-label { font-size: 12px; fill: #52616d; font-weight: 650; }
            .source-pad-edge { stroke: #687983; stroke-width: 1.5; stroke-dasharray: 4 4; }
            .source-pad-edge-label { font-size: 12px; fill: #52616d; font-weight: 650; }
        </style>""",
        f'<rect width="{svg_width}" height="{svg_height}" class="page"/>',
    ]

    total_load = report["inputs"]["total_current_a"]
    svg.extend(
        [
            f'<text x="{margin:.1f}" y="48" class="report-title">{title}</text>',
            (
                f'<text x="{margin:.1f}" y="76" class="subtitle">'
                "IPC-2221 screening estimate · connected net copper · "
                f"{len(nonempty_layers)} layers · {total_load:.2f} A total load</text>"
            ),
        ]
    )
    card_gap = 14.0
    card_y = 92.0
    card_height = 110.0
    card_width = (svg_width - margin * 2 - card_gap * (len(stackups) - 1)) / len(
        stackups
    )
    svg.append('<g id="stackup-summaries">')
    for stackup_index, (stackup, stackup_report) in enumerate(
        zip(stackups, stackup_reports, strict=True)
    ):
        card_x = margin + stackup_index * (card_width + card_gap)
        svg.extend(
            [
                (
                    f'<rect x="{card_x:.1f}" y="{card_y:.1f}" '
                    f'width="{card_width:.1f}" height="{card_height:.1f}" '
                    f'rx="8" class="stackup-card" data-role="stackup-summary" '
                    f'data-stackup="{html.escape(stackup["name"])}"/>'
                ),
                (
                    f'<line x1="{card_x + 5:.1f}" y1="{card_y + 9:.1f}" '
                    f'x2="{card_x + 5:.1f}" y2="{card_y + card_height - 9:.1f}" '
                    f'stroke="{stackup["color"]}" stroke-width="5" '
                    'stroke-linecap="round"/>'
                ),
                (
                    f'<text x="{card_x + 18:.1f}" y="{card_y + 24:.1f}" '
                    f'class="stackup-title">{html.escape(stackup["label"])}</text>'
                ),
                (
                    f'<text x="{card_x + 18:.1f}" y="{card_y + 43:.1f}" '
                    'class="stackup-detail">'
                    f"{html.escape(stackup_report['thickness_summary'])}</text>"
                ),
            ]
        )
        for temperature_index, temperature in enumerate(temperatures):
            result = stackup["limiting"][temperature]
            margin_ratio = result.margin(temperature)
            result_class = "pass" if margin_ratio >= 1.0 else "fail"
            result_label = "PASS" if margin_ratio >= 1.0 else "FAIL"
            row_y = card_y + 69.0 + temperature_index * 23.0
            svg.append(
                f'<text x="{card_x + 18:.1f}" y="{row_y:.1f}" '
                f'class="stackup-result {result_class}">'
                f"{temperature:g} °C: "
                f"{result.natural_capacity_a[temperature]:.2f} A capacity / "
                f"{result.active_current_a:.2f} A load · "
                f"{margin_ratio:.2f}× {result_label}</text>"
            )
    svg.append("</g>")

    overview_x = margin
    overview_y = margin + header_height
    overview_width = svg_width - margin * 2
    svg_panel(svg, overview_x, overview_y, overview_width, overview_height)
    svg.append('<g id="aggregate-overview">')
    svg.append(
        f'<text x="{overview_x + 20:.1f}" y="{overview_y + 31:.1f}" '
        'class="panel-title">Aggregate connected VCC copper</text>'
    )
    svg.append(
        f'<text x="{overview_x + 20:.1f}" y="{overview_y + 52:.1f}" class="hint">'
        "Shared copper geometry with one color-coded limiting cut per stackup.</text>"
    )
    key_width = 300.0
    board_panel = (
        overview_x + 16.0,
        overview_y + 66.0,
        overview_width - key_width - 34.0,
        overview_height - 82.0,
    )
    terminals = [
        ("S", source),
        *[(str(index), sink) for index, sink in enumerate(sinks, 1)],
    ]
    append_board_geometry(
        svg,
        combined,
        combined.bounds,
        board_panel,
        "#b9c7cf",
        "#6d838f",
        "All connected selected-layer copper",
        stackup_cuts,
        marker_terminals=terminals,
        vias=report["via_screening"]["vias"],
    )
    key_x = overview_x + overview_width - key_width + 8.0
    svg.append(
        f'<line x1="{key_x - 14:.1f}" y1="{overview_y + 72:.1f}" '
        f'x2="{key_x - 14:.1f}" y2="{overview_y + overview_height - 18:.1f}" '
        'stroke="#e1e7eb"/>'
    )
    svg.append(
        f'<text x="{key_x:.1f}" y="{overview_y + 91:.1f}" '
        'class="terminal-key" font-weight="650">Terminals</text>'
    )
    key_y = overview_y + 112.0
    terminal_rows = [("S", source.selector, "source")]
    terminal_rows.extend(
        (str(index), sink.selector, f"{sink.current_a:g} A load")
        for index, sink in enumerate(sinks, 1)
    )
    for marker, selector, detail in terminal_rows:
        marker_fill = "#17212b" if marker == "S" else "#26547c"
        svg.append(
            f'<circle cx="{key_x + 10:.1f}" cy="{key_y - 4:.1f}" r="10" '
            f'fill="{marker_fill}"/>'
        )
        svg.append(
            f'<text x="{key_x + 10:.1f}" y="{key_y:.1f}" fill="#fff" '
            'font-size="12" font-weight="700" text-anchor="middle">'
            f"{marker}</text>"
        )
        svg.append(
            f'<text x="{key_x + 30:.1f}" y="{key_y - 2:.1f}" class="terminal-key">'
            f"{html.escape(selector)}</text>"
        )
        svg.append(
            f'<text x="{key_x + 30:.1f}" y="{key_y + 14:.1f}" '
            f'class="terminal-detail">{html.escape(detail)}</text>'
        )
        key_y += 31.0
    cut_key_y = key_y + 7.0
    for cut_index, item in enumerate(stackup_cuts):
        row_y = cut_key_y + cut_index * 24.0
        cut = item["cut"]
        distance = (
            f"{cut.distance_mm:.2f} mm" if cut.distance_mm is not None else "manual"
        )
        svg.append(
            f'<line x1="{key_x:.1f}" y1="{row_y:.1f}" '
            f'x2="{key_x + 34:.1f}" y2="{row_y:.1f}" '
            f'class="cut-key" stroke="{item["color"]}"/>'
        )
        svg.append(
            f'<text x="{key_x + 45:.1f}" y="{row_y + 5:.1f}" '
            f'class="terminal-key">{html.escape(item["label"])} · {distance}</text>'
        )
    svg.append("</g>")

    svg.append(
        f'<text x="{margin:.1f}" y="{layer_section_y - 9:.1f}" class="panel-title">'
        "Copper by layer</text>"
    )
    svg.append('<g id="layer-panels">')
    column_gap = 14.0
    layer_panel_width = (svg_width - margin * 2 - column_gap) / 2.0
    for index, layer in enumerate(nonempty_layers):
        row, column = divmod(index, 2)
        panel_x = margin + column * (layer_panel_width + column_gap)
        panel_y = layer_section_y + 14.0 + row * (layer_panel_height + 14.0)
        svg_panel(svg, panel_x, panel_y, layer_panel_width, layer_panel_height)
        color = LAYER_COLORS[layers.index(layer) % len(LAYER_COLORS)]
        kind = "external" if layer.external else "internal"
        svg.append(
            f'<text x="{panel_x + 18:.1f}" y="{panel_y + 29:.1f}" class="panel-title">'
            f"{html.escape(layer.name)}</text>"
        )
        svg.append(
            f'<text x="{panel_x + 18:.1f}" y="{panel_y + 49:.1f}" class="hint">'
            f"{kind} layer · shared copper geometry</text>"
        )
        append_board_geometry(
            svg,
            selected_geometry[layer.name],
            combined.bounds,
            (
                panel_x + 12.0,
                panel_y + 58.0,
                layer_panel_width - 24.0,
                layer_panel_height - 70.0,
            ),
            color,
            color,
            f"{layer.name}: {kind} copper geometry",
            stackup_cuts,
        )
    svg.append("</g>")

    svg_panel(svg, margin, profile_y, svg_width - margin * 2, profile_height)
    append_capacity_profile(
        svg,
        stackups,
        temperatures,
        source,
        sinks,
        tuple(report["inputs"]["scan_direction"]),
        context["source_pad_exit_distance_mm"],
        (margin, profile_y, svg_width - margin * 2, profile_height),
    )
    svg_panel(svg, margin, thermal_y, svg_width - margin * 2, thermal_height)
    append_temperature_rise_heatmaps(
        svg,
        combined,
        combined.bounds,
        stackups,
        stackup_reports,
        source,
        tuple(report["inputs"]["scan_direction"]),
        primary_temperature,
        (margin, thermal_y, svg_width - margin * 2, thermal_height),
    )
    svg.append("</svg>")
    destination.write_text("\n".join(svg) + "\n", encoding="utf-8")


def write_markdown(report: dict[str, Any], destination: Path) -> None:
    temperatures = [float(value) for value in report["inputs"]["temperature_rises_c"]]
    lines = [
        f"# PCB Power-Capacity Report: `{report['scenario']}`",
        "",
        "![Annotated copper and limiting cuts](report.svg)",
        "",
        f"- Board: `{report['board']}`",
        f"- Net: `{report['net']}`",
        f"- Source: `{report['inputs']['source']['pad']}`",
        f"- Requested current: {report['inputs']['total_current_a']:.3f} A",
    ]
    if report["inputs"].get("supply_voltage_v") is not None:
        lines.extend(
            [
                f"- Supply voltage: {report['inputs']['supply_voltage_v']:.3f} V",
                f"- Distributed load: {report['inputs']['distributed_load_power_w']:.1f} W",
            ]
        )
    lines.extend(
        [
            f"- Automatic scan pitch: {report['inputs']['scan_pitch_mm']:.3f} mm",
            (
                "- Source-pad exit in the scan direction: "
                f"{report['inputs']['source_pad_exit_distance_mm']:.3f} mm from "
                "the pad center"
            ),
            (
                "- First automatic cut: "
                f"{report['inputs']['first_automatic_cut_distance_mm']:.3f} mm "
                "from the pad center"
            ),
            "",
            "## Connected copper geometry",
            "",
            "The geometry is shared by every compared stackup.",
            "",
            "| Layer | Type | Connected net area |",
            "| --- | --- | ---: |",
        ]
    )
    for layer in report["layers"]:
        lines.append(
            f"| {layer['name']} | {layer['kind']} "
            f"| {layer['connected_copper_area_mm2']:.2f} mm² |"
        )
    lines.extend(
        [
            "",
            "## Stackup comparison",
            "",
            (
                "Capacity/load is the estimated copper capacity divided by the "
                "current crossing the cut. Values below 1.00× do not meet the "
                "requested load."
            ),
            "",
            "| Stackup | Copper | Temperature rise | Load at cut | Natural-sharing capacity | Capacity/load | Ideal-sharing ceiling | Cut |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |",
        ]
    )
    primary_temperature = min(temperatures)
    for stackup in report["stackups"]:
        for temperature in temperatures:
            cut = stackup["limiting_cuts"][str(temperature)]
            lines.append(
                f"| {stackup['label']} | {stackup['thickness_summary']} "
                f"| {temperature:g} °C | {cut['active_current_a']:.2f} A "
                f"| {cut['natural_capacity_a'][str(temperature)]:.2f} A "
                f"| {cut['margin_ratio'][str(temperature)]:.2f}× "
                f"| {cut['ideal_sharing_ceiling_a'][str(temperature)]:.2f} A "
                f"| {cut['kind']}:{cut['index']} |"
            )
    lines.extend(
        [
            "",
            "## Estimated temperature-rise proxy",
            "",
            (
                "The SVG heatmaps invert the IPC-2221 natural-sharing capacity "
                "at each sweep cut. They indicate relative hot regions but do "
                "not model thermal spreading, components, airflow, or enclosure effects."
            ),
            "",
            "| Stackup | Peak estimated ΔT | Peak cut |",
            "| --- | ---: | --- |",
        ]
    )
    for stackup in report["stackups"]:
        proxy = stackup["temperature_rise_proxy"]
        peak = proxy["peak_cut"]
        lines.append(
            f"| {stackup['label']} | {proxy['peak_delta_c']:.1f} °C "
            f"| {peak['kind']}:{peak['index']} at {peak['distance_mm']:.2f} mm |"
        )
    lines.extend(
        [
            "",
            "## Electrical screening estimate",
            "",
            "| Stackup | Copper loss | Resistance to farthest sink | Farthest-sink drop |",
            "| --- | ---: | ---: | ---: |",
        ]
    )
    for stackup in report["stackups"]:
        electrical = stackup["electrical_estimate"]
        farthest_drop = max(electrical["sink_voltage_drop_v"].values())
        lines.append(
            f"| {stackup['label']} | {electrical['total_copper_loss_w']:.3f} W "
            f"| {electrical['integrated_to_farthest_sink_ohm'] * 1000:.3f} mΩ "
            f"| {farthest_drop * 1000:.2f} mV |"
        )
    capacity_headings = " | ".join(
        f"{stackup['label']} drop" for stackup in report["stackups"]
    )
    lines.extend(
        [
            "",
            f"| Sink | Current | {capacity_headings} |",
            f"| --- | ---: | {' | '.join('---:' for _ in report['stackups'])} |",
        ]
    )
    currents = {sink["pad"]: sink["current_a"] for sink in report["inputs"]["sinks"]}
    selectors = report["stackups"][0]["electrical_estimate"]["sink_voltage_drop_v"]
    for selector in selectors:
        drops = " | ".join(
            f"{stackup['electrical_estimate']['sink_voltage_drop_v'][selector] * 1000:.2f} mV"
            for stackup in report["stackups"]
        )
        lines.append(f"| {selector} | {currents[selector]:.2f} A | {drops} |")

    for stackup in report["stackups"]:
        limiting = stackup["limiting_cuts"][str(primary_temperature)]
        lines.extend(
            [
                "",
                f"### {stackup['label']}: per-layer detail at the {primary_temperature:g} °C limiting cut",
                "",
                "| Layer | Intersected width | Cross-section | Natural share | Requested current | IPC capacity |",
                "| --- | ---: | ---: | ---: | ---: | ---: |",
            ]
        )
        for name, layer in limiting["layers"].items():
            lines.append(
                f"| {name} | {layer['width_mm']:.3f} mm "
                f"| {layer['cross_section_mm2']:.5f} mm² "
                f"| {layer['natural_share'] * 100:.1f}% "
                f"| {layer['current_at_requested_load_a']:.2f} A "
                f"| {layer['capacity_a'][str(primary_temperature)]:.2f} A |"
            )
    via = report["via_screening"]
    lines.extend(
        [
            "",
            "## Via screening",
            "",
            (
                f"Found {via['count']} via(s) on the connected net using "
                f"{via['board_hole_plating_um']:.1f} µm board hole plating."
            ),
            "",
            via["note"],
        ]
    )
    if via["vias"]:
        capacity_headings = " | ".join(
            f"{temperature:g} °C proxy" for temperature in temperatures
        )
        lines.extend(
            [
                "",
                f"| Location | Span | Drill / plating | Resistance | {capacity_headings} |",
                f"| --- | --- | ---: | ---: | {' | '.join('---:' for _ in temperatures)} |",
            ]
        )
        for item in via["vias"]:
            capacities = " | ".join(
                f"{item['ipc2221_internal_proxy_capacity_a'][temperature]:.2f} A"
                for temperature in temperatures
            )
            lines.append(
                f"| {item['x_mm']:.3f}, {item['y_mm']:.3f} mm "
                f"| {item['span'][0]}–{item['span'][1]} "
                f"| {item['drill_mm']:.3f} mm / {item['plating_um']:.1f} µm "
                f"| {item['resistance_ohm'] * 1000:.3f} mΩ | {capacities} |"
            )
    lines.extend(["", "## Qualifications and warnings", ""])
    lines.extend(f"- {warning}" for warning in report["warnings"])
    lines.extend(
        [
            "",
            "The complete cut inventory and numeric data are in [`report.json`](report.json).",
            "",
        ]
    )
    destination.write_text("\n".join(lines), encoding="utf-8")


def write_report(
    report: dict[str, Any], context: dict[str, Any], output_dir: Path
) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n",
        encoding="utf-8",
    )
    write_svg(report, context, output_dir / "report.svg")
    write_markdown(report, output_dir / "report.md")


def write_report_index(reports: list[dict[str, Any]], output_root: Path) -> None:
    summary = {
        "schema_version": 2,
        "reports": [
            {
                "scenario": report["scenario"],
                "board": report["board"],
                "board_sha256": report["board_sha256"],
                "net": report["net"],
                "requested_current_a": report["inputs"]["total_current_a"],
                "stackups": [
                    {
                        "name": stackup["name"],
                        "label": stackup["label"],
                        "thickness_summary": stackup["thickness_summary"],
                        "temperature_rises": {
                            temperature: {
                                "natural_capacity_a": cut["natural_capacity_a"][
                                    temperature
                                ],
                                "ideal_sharing_ceiling_a": cut[
                                    "ideal_sharing_ceiling_a"
                                ][temperature],
                                "margin_ratio": cut["margin_ratio"][temperature],
                                "passes_requested_load": cut["margin_ratio"][
                                    temperature
                                ]
                                >= 1.0,
                            }
                            for temperature, cut in stackup["limiting_cuts"].items()
                        },
                    }
                    for stackup in report["stackups"]
                ],
                "report": f"{report['scenario']}/report.md",
            }
            for report in reports
        ],
    }
    output_root.mkdir(parents=True, exist_ok=True)
    (output_root / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True, allow_nan=False) + "\n",
        encoding="utf-8",
    )
    lines = [
        "# PCB Power-Capacity Reports",
        "",
        "| Scenario | Board/net | Requested current | Temperature-rise results |",
        "| --- | --- | ---: | --- |",
    ]
    for item in summary["reports"]:
        results = "<br>".join(
            f"{stackup['label']}: "
            + "; ".join(
                f"{float(temperature):g} °C {values['natural_capacity_a']:.2f} A, "
                f"{values['margin_ratio']:.2f}×"
                for temperature, values in stackup["temperature_rises"].items()
            )
            for stackup in item["stackups"]
        )
        lines.append(
            f"| [{item['scenario']}]({item['report']}) "
            f"| `{item['board']}` / `{item['net']}` "
            f"| {item['requested_current_a']:.2f} A | {results} |"
        )
    lines.append("")
    (output_root / "index.md").write_text("\n".join(lines), encoding="utf-8")


def parse_sink(value: str) -> Sink:
    try:
        selector, current = value.rsplit("=", 1)
        current_a = float(current)
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "sinks use PAD=AMPS syntax, for example J1.1=5.5"
        ) from error
    if not selector or current_a <= 0:
        raise argparse.ArgumentTypeError("sink current must be positive")
    return Sink(selector, current_a)


def parse_copper(value: str) -> tuple[str, float]:
    try:
        layer, thickness = value.rsplit("=", 1)
        thickness_um = float(thickness)
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "copper overrides use LAYER=MICROMETRES syntax, for example F.Cu=70"
        ) from error
    if not layer or thickness_um <= 0:
        raise argparse.ArgumentTypeError("copper thickness must be positive")
    return layer, thickness_um


def parse_cut(value: str) -> tuple[tuple[float, float], tuple[float, float]]:
    try:
        start_text, end_text = value.split(":", 1)
        start = tuple(float(item) for item in start_text.split(","))
        end = tuple(float(item) for item in end_text.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "cuts use X1,Y1:X2,Y2 millimetre coordinates"
        ) from error
    if len(start) != 2 or len(end) != 2 or start == end:
        raise argparse.ArgumentTypeError(
            "cuts use two distinct X,Y millimetre coordinates"
        )
    return (start[0], start[1]), (end[0], end[1])


def load_scenarios(config_path: Path, root: Path) -> dict[str, Scenario]:
    if not config_path.is_file():
        raise AnalysisError(f"Scenario configuration does not exist: {config_path}")
    data = tomllib.loads(config_path.read_text(encoding="utf-8"))
    raw_scenarios = data.get("scenarios")
    if not isinstance(raw_scenarios, dict):
        raise AnalysisError(f"No [scenarios] table is present in {config_path}")
    scenarios: dict[str, Scenario] = {}
    for name, raw in raw_scenarios.items():
        try:
            sinks = [
                Sink(item["pad"], float(item["current_a"])) for item in raw["sinks"]
            ]
            stackups = [
                StackupSpec(
                    name=str(item["name"]),
                    label=str(item.get("label", item["name"])),
                    copper_overrides_um={
                        key: float(value)
                        for key, value in item.get("copper_overrides_um", {}).items()
                    },
                )
                for item in raw.get("stackups", [])
            ]
            if len({stackup.name for stackup in stackups}) != len(stackups):
                raise ValueError("stackup names must be unique")
            board_path = Path(raw["board"])
            if not board_path.is_absolute():
                board_path = root / board_path
            scenarios[name] = Scenario(
                name=name,
                board=board_path,
                net=raw["net"],
                source=raw["source"],
                sinks=sinks,
                supply_voltage_v=(
                    float(raw["supply_voltage_v"])
                    if "supply_voltage_v" in raw
                    else None
                ),
                temperature_rises_c=[
                    float(value) for value in raw.get("temperature_rises_c", [10, 20])
                ],
                scan_pitch_mm=float(raw.get("scan_pitch_mm", 0.25)),
                include_layers=(
                    [str(value) for value in raw["layers"]] if "layers" in raw else None
                ),
                copper_overrides_um={
                    key: float(value)
                    for key, value in raw.get("copper_overrides_um", {}).items()
                },
                stackups=stackups,
                manual_cuts=[parse_cut(value) for value in raw.get("manual_cuts", [])],
            )
        except (KeyError, TypeError, ValueError) as error:
            raise AnalysisError(
                f"Invalid scenario {name!r} in {config_path}: {error}"
            ) from error
    return scenarios


def repository_root() -> Path:
    candidate = SCRIPT_DIR.parent
    if (candidate / "mise.toml").is_file():
        return candidate
    return Path.cwd()


def scenario_from_args(
    args: argparse.Namespace,
    configured: dict[str, Scenario],
    root: Path,
) -> Scenario:
    if args.scenario:
        if args.scenario not in configured:
            raise AnalysisError(
                f"Unknown scenario {args.scenario!r}; choices: {', '.join(sorted(configured))}"
            )
        base = configured[args.scenario]
        scenario = Scenario(
            name=base.name,
            board=base.board,
            net=base.net,
            source=base.source,
            sinks=list(base.sinks),
            supply_voltage_v=base.supply_voltage_v,
            temperature_rises_c=list(base.temperature_rises_c),
            scan_pitch_mm=base.scan_pitch_mm,
            include_layers=(
                list(base.include_layers) if base.include_layers is not None else None
            ),
            copper_overrides_um=dict(base.copper_overrides_um),
            stackups=list(base.stackups),
            manual_cuts=list(base.manual_cuts),
        )
    else:
        if not (args.board and args.net and args.source and args.sink):
            raise AnalysisError(
                "Provide a scenario name, or provide --board, --net, --source, and --sink"
            )
        board = Path(args.board)
        scenario = Scenario(
            name=args.name or board.stem + "-" + args.net.lower(),
            board=board if board.is_absolute() else root / board,
            net=args.net,
            source=args.source,
            sinks=list(args.sink),
        )
    if args.board:
        board = Path(args.board)
        scenario.board = board if board.is_absolute() else root / board
    if args.net:
        scenario.net = args.net
    if args.source:
        scenario.source = args.source
    if args.sink:
        scenario.sinks = list(args.sink)
    if args.supply_voltage is not None:
        scenario.supply_voltage_v = args.supply_voltage
    if args.temp_rise:
        scenario.temperature_rises_c = sorted(set(args.temp_rise))
    if args.scan_pitch is not None:
        scenario.scan_pitch_mm = args.scan_pitch
    if args.layer:
        scenario.include_layers = list(dict.fromkeys(args.layer))
    if args.copper:
        scenario.stackups = [StackupSpec("custom", "Custom stackup")]
        scenario.copper_overrides_um.update(dict(args.copper))
    scenario.manual_cuts.extend(args.cut or [])
    return scenario


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Estimate multilayer KiCad net current capacity and emit visible reports"
    )
    parser.add_argument(
        "scenario", nargs="?", help="named scenario from the TOML config"
    )
    parser.add_argument(
        "--all", action="store_true", help="generate every configured scenario"
    )
    parser.add_argument("--list", action="store_true", help="list configured scenarios")
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--name", help="report name for an ad-hoc scenario")
    parser.add_argument("--board", help="KiCad .kicad_pcb path")
    parser.add_argument("--net", help="net name")
    parser.add_argument("--source", help="source pad selector, for example RSH1.2")
    parser.add_argument(
        "--sink", action="append", type=parse_sink, help="sink as PAD=AMPS; repeatable"
    )
    parser.add_argument("--supply-voltage", type=float)
    parser.add_argument(
        "--temp-rise", action="append", type=float, help="temperature rise in degrees C"
    )
    parser.add_argument("--scan-pitch", type=float, help="automatic sweep pitch in mm")
    parser.add_argument(
        "--layer", action="append", help="copper layer to include; repeatable"
    )
    parser.add_argument(
        "--copper",
        action="append",
        type=parse_copper,
        help="override copper as LAYER=MICROMETRES; repeatable",
    )
    parser.add_argument(
        "--cut",
        action="append",
        type=parse_cut,
        help="manual visible cut as X1,Y1:X2,Y2 in board millimetres",
    )
    return parser


def print_summary(report: dict[str, Any], output_dir: Path) -> None:
    print(
        f"{report['scenario']}: {report['net']} at {report['inputs']['total_current_a']:.2f} A"
    )
    for stackup in report["stackups"]:
        print(f"  {stackup['label']}:")
        for temperature in report["inputs"]["temperature_rises_c"]:
            key = str(float(temperature))
            cut = stackup["limiting_cuts"][key]
            print(
                f"    {float(temperature):g} C rise: "
                f"{cut['natural_capacity_a'][key]:.2f} A natural, "
                f"{cut['ideal_sharing_ceiling_a'][key]:.2f} A ideal, "
                f"{cut['margin_ratio'][key]:.2f}x capacity/load"
            )
    print(f"  report: {output_dir / 'report.md'}")


def main(argv: list[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    root = repository_root()
    try:
        configured = load_scenarios(args.config, root)
        if args.list:
            for name, scenario in configured.items():
                try:
                    board_display = scenario.board.relative_to(root)
                except ValueError:
                    board_display = scenario.board
                print(f"{name}\t{board_display}\t{scenario.net}")
            return 0
        if args.all and args.scenario:
            raise AnalysisError("Do not combine --all with a scenario name")
        if args.all:
            if any(
                value
                for value in (
                    args.board,
                    args.net,
                    args.source,
                    args.sink,
                    args.name,
                    args.cut,
                    args.supply_voltage,
                    args.temp_rise,
                    args.scan_pitch,
                    args.layer,
                    args.copper,
                )
            ):
                raise AnalysisError(
                    "Board/path overrides cannot be combined with --all"
                )
            output_root = args.output_dir or (root / DEFAULT_OUTPUT_ROOT)
            reports: list[dict[str, Any]] = []
            for scenario in configured.values():
                report, context = analyze_scenario(scenario)
                destination = output_root / scenario.name
                write_report(report, context, destination)
                print_summary(report, destination)
                reports.append(report)
            write_report_index(reports, output_root)
            return 0
        scenario = scenario_from_args(args, configured, root)
        output_dir = args.output_dir or (root / DEFAULT_OUTPUT_ROOT / scenario.name)
        report, context = analyze_scenario(scenario)
        write_report(report, context, output_dir)
        print_summary(report, output_dir)
        return 0
    except AnalysisError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    sys.exit(main())
