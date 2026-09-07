#!/usr/bin/env python3
"""Build the Rev A 6:1 carrier/backplane fabrication panel.

The released single-board fabrication files are the authority for this panel.
Their matching KiCad PCB revisions are materialized from Git, combined with
KiKit, and then exported with KiKit's JLCPCB and PCBWay fabrication commands.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import posixpath
import re
import shutil
import struct
import subprocess
import sys
import zipfile
from collections import Counter, defaultdict
from itertools import pairwise
from pathlib import Path, PurePosixPath

import pcbnew
from kikit.common import fromDegrees
from kikit.defs import Layer
from kikit.panelize import Origin, Panel
from kikit.units import mm
from shapely.geometry import LineString, box

CARRIER_BOARD = "hardware/boards/carrier/carrier.kicad_pcb"
CARRIER_BOM = "hardware/boards/carrier/production/bom.csv"
CARRIER_PRODUCTION_DIR = Path("hardware/boards/carrier/production")
CARRIER_ARCHIVE = "Mini_Rack_Power_Carrier_A.zip"
CARRIER_PROJECT_FILES = (
    "hardware/boards/carrier/carrier.kicad_pro",
    "hardware/boards/carrier/carrier.kicad_sch",
    "hardware/boards/carrier/01_power_control.kicad_sch",
    "hardware/boards/carrier/02_housekeeping.kicad_sch",
)
BACKPLANE_BOARD = "hardware/boards/backplane-prototype/backplane-prototype.kicad_pcb"
BACKPLANE_BOM = "hardware/boards/backplane-prototype/production/bom.csv"
BACKPLANE_PRODUCTION_DIR = Path("hardware/boards/backplane-prototype/production")
BACKPLANE_ARCHIVE = "Backplane_Prototype_A.zip"
BACKPLANE_PROJECT_FILES = (
    "hardware/boards/backplane-prototype/backplane-prototype.kicad_pro",
    "hardware/boards/backplane-prototype/backplane-prototype.kicad_sch",
    "hardware/boards/backplane-prototype/01_power_input.kicad_sch",
    "hardware/boards/backplane-prototype/02_control.kicad_sch",
    "hardware/boards/backplane-prototype/03_can_slots.kicad_sch",
    "hardware/boards/backplane-prototype/04_fan_status.kicad_sch",
    "hardware/boards/backplane-prototype/05_buck_converters.kicad_sch",
)

CARRIER_PROJECT = CARRIER_PROJECT_FILES[0]
BACKPLANE_PROJECT = BACKPLANE_PROJECT_FILES[0]

PANEL_NAME = "modular-rack-power-carrier6-backplane1-rev-a"
RENDER_NAME = f"{PANEL_NAME}-top.png"
DOC_RENDER_NAMES = {
    "carrier": "carrier-rev-a-top.png",
    "carrier_bottom": "carrier-rev-a-bottom.png",
    "backplane_prototype": "backplane-prototype-rev-a-top.png",
    "backplane_prototype_bottom": "backplane-prototype-rev-a-bottom.png",
    "combined_panel": "carrier6-backplane1-rev-a-top.png",
}

KIPRJMOD_MODEL_RE = re.compile(r'\(model "\$\{KIPRJMOD\}/([^"\n]+)"')

BOARD_GAP = 2 * mm
INTER_DESIGN_GAP = 9 * mm
FRAME_SIDE_WIDTH = 5 * mm
FRAME_TOP_BOTTOM_WIDTH = 7 * mm
FRAME_HORIZONTAL_GAP = 7 * mm
FRAME_VERTICAL_GAP = 2 * mm
TAB_WIDTH = 5 * mm
MOUSE_BITE_DIAMETER = 0.6 * mm
MOUSE_BITE_PITCH = 0.95 * mm
TOOLING_DIAMETER = 2 * mm
FIDUCIAL_COPPER_DIAMETER = 1 * mm
FIDUCIAL_MASK_OPENING = 2 * mm
MAX_PANEL_SIZE = 250 * mm
COORDINATE_MARGIN = 1 * mm
EXPECTED_MOUSE_BITES = 135

# These indicate that panelization introduced a fabrication geometry problem.
# The released boards have known courtyard/silkscreen/library DRC findings, so
# those are reported but are not silently represented as new panel failures.
FABRICATION_DRC_FAILURES = {
    "board_edge_missing",
    "board_edge_overlap",
    "clearance",
    "copper_edge_clearance",
    "drilled_holes_coincident",
    "edge_clearance",
    "hole_clearance",
    "hole_to_hole",
    "malformed_outline",
    "starved_thermal",
    "track_dangling",
    "track_width",
    "unconnected_items",
    "via_dangling",
}


# KiCad 10's Python bindings in the current Python 3.14 package still call the
# Python 2-style ``.next()`` method inside helpers such as GetDrawings().  SWIG
# exposes only ``.__next__()``.  KiKit legitimately uses those helpers, so add
# the missing alias until the distribution binding catches up.
if not hasattr(pcbnew.SwigPyIterator, "next"):
    pcbnew.SwigPyIterator.next = pcbnew.SwigPyIterator.__next__


def run(*args: str, cwd: Path | None = None) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run(args, cwd=cwd, check=True)


def git_root() -> Path:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip())


def panel_git_hash(root: Path) -> str:
    value = os.environ.get("PANEL_GIT_HASH", "").strip()
    if not value:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
        value = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-fA-F]{7,16}", value):
        raise RuntimeError(f"PANEL_GIT_HASH is not a short Git hash: {value!r}")
    return value.lower()


def resolve_git_revision(root: Path, revision: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", revision],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def validate_production_package(
    production_dir: Path,
    expected_revision: str,
    expected_board: str,
    archive_name: str,
) -> dict[str, object]:
    required_files = (
        production_dir / "bom.csv",
        production_dir / "positions.csv",
        production_dir / "validation.json",
        production_dir / archive_name,
    )
    missing = [str(path) for path in required_files if not path.is_file()]
    if missing:
        raise RuntimeError(f"Production package is incomplete: {missing}")

    validation = json.loads(
        (production_dir / "validation.json").read_text(encoding="utf-8")
    )
    if validation.get("status") not in {"pass", "pass_with_warnings"}:
        raise RuntimeError(f"Production package did not pass checks: {production_dir}")

    source = validation.get("source")
    if not isinstance(source, dict):
        raise TypeError(f"Production package has no source metadata: {production_dir}")
    if source.get("git_commit") != expected_revision:
        raise RuntimeError(
            f"Production package revision does not match panel revision: {production_dir}"
        )
    if source.get("board") != expected_board:
        raise RuntimeError(
            f"Production package board does not match expected board: {production_dir}"
        )

    assembly = validation.get("assembly")
    if not isinstance(assembly, dict) or not assembly.get(
        "bom_position_references_match"
    ):
        raise RuntimeError(
            f"Production package BOM and positions do not match: {production_dir}"
        )
    return validation


def materialize_git_file(
    root: Path, revision: str, source: str, destination: Path
) -> None:
    result = subprocess.run(
        ["git", "show", f"{revision}:{source}"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(result.stdout)


def materialize_git_board(
    root: Path,
    revision: str,
    source: str,
    destination: Path,
    staging_root: Path,
) -> None:
    """Stage a board and its repository-relative 3D models."""

    materialize_git_file(root, revision, source, destination)
    project_directory = PurePosixPath(source).parent
    board_text = destination.read_text(encoding="utf-8")
    for relative_model in sorted(set(KIPRJMOD_MODEL_RE.findall(board_text))):
        repository_model = PurePosixPath(
            posixpath.normpath((project_directory / relative_model).as_posix())
        )
        if repository_model.is_absolute() or (
            repository_model.parts and repository_model.parts[0] == ".."
        ):
            raise RuntimeError(
                f"3D model path escapes the repository: {source}: {relative_model}"
            )
        materialize_git_file(
            root,
            revision,
            repository_model.as_posix(),
            staging_root.joinpath(*repository_model.parts),
        )


def git_file_json(root: Path, revision: str, source: str) -> dict[str, object]:
    result = subprocess.run(
        ["git", "show", f"{revision}:{source}"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    return json.loads(result.stdout)


def project_text_variables(
    root: Path, revision: str, project_file: str
) -> dict[str, str]:
    project = git_file_json(root, revision, project_file)
    variables = project.get("text_variables", {})
    if not isinstance(variables, dict) or not all(
        isinstance(key, str) and isinstance(value, str)
        for key, value in variables.items()
    ):
        raise RuntimeError(f"Invalid text_variables in {revision}:{project_file}")
    return dict(variables)


def git_revision_date(root: Path, revision: str) -> str:
    result = subprocess.run(
        ["git", "show", "-s", "--format=%cd", "--date=format:%y.%m.%d", revision],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    value = result.stdout.strip()
    if not re.fullmatch(r"\d{2}\.\d{2}\.\d{2}", value):
        raise RuntimeError(f"Unexpected Git date for {revision}: {value!r}")
    return value


def release_text_variables(
    root: Path,
    revision: str,
    project_file: str,
    common_overrides: dict[str, str],
    board_overrides: dict[str, str],
) -> dict[str, str]:
    variables = project_text_variables(root, revision, project_file)

    # A working project deliberately advertises itself as unreleased. Once its
    # revision is pinned by this release builder, the source commit supplies
    # deterministic provenance unless the release command explicitly overrides
    # either value.
    if "BUILD_DATE" in variables:
        variables["BUILD_DATE"] = git_revision_date(root, revision)
    if "SHORT_HASH" in variables:
        variables["SHORT_HASH"] = revision[:12].lower()

    variables.update(common_overrides)
    variables.update(board_overrides)
    return variables


def bake_board_text_variables(board_path: Path, variables: dict[str, str]) -> set[str]:
    """Bake project text variables into board-level text for KiKit.

    KiCad CLI accepts ``-D KEY=VALUE``, but KiKit's board-copy path does not.
    Only board-level PCB_TEXT is touched here, leaving footprint fields such as
    ``${REFERENCE}`` and model paths such as ``${KIPRJMOD}`` intact.
    """

    board = pcbnew.LoadBoard(str(board_path))
    used: set[str] = set()
    changed = False

    for drawing in board.GetDrawings():
        if not isinstance(drawing, pcbnew.PCB_TEXT):
            continue
        original = drawing.GetText()
        updated = original
        for name, value in variables.items():
            token = f"${{{name}}}"
            if token in updated:
                updated = updated.replace(token, value)
                used.add(name)
        if updated != original:
            drawing.SetText(updated)
            changed = True

    if changed:
        pcbnew.SaveBoard(str(board_path), board)
    return used


def parse_text_variable(value: str) -> tuple[str, str]:
    try:
        name, replacement = value.split("=", 1)
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "text variables must use KEY=VALUE syntax"
        ) from error
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
        raise argparse.ArgumentTypeError(f"invalid text-variable name: {name!r}")
    if not replacement or "\n" in replacement or "\r" in replacement:
        raise argparse.ArgumentTypeError(
            "text-variable values must be non-empty single-line strings"
        )
    return name, replacement


def text_variable_map(values: list[tuple[str, str]] | None) -> dict[str, str]:
    return dict(values or [])


def board_dimensions(board_path: Path) -> tuple[int, int]:
    board = pcbnew.LoadBoard(str(board_path))
    bbox = board.GetBoardEdgesBoundingBox()
    return bbox.GetWidth(), bbox.GetHeight()


def add_board(
    panel: Panel,
    board_path: Path,
    x: int,
    y: int,
    prefix: str,
    rotation_degrees: float = 0,
) -> object:
    panel.appendBoard(
        board_path,
        pcbnew.VECTOR2I(x, y),
        origin=Origin.Center,
        rotationAngle=fromDegrees(rotation_degrees),
        tolerance=1 * mm,
        netRenamer=lambda _index, net: f"{prefix}-{net}",
        refRenamer=lambda _index, ref: f"{prefix}-{ref}",
        inheritDrc=False,
        bakeText=True,
        bakeRef=True,
    )
    return panel.substrates[-1]


def build_panel(
    carrier_path: Path,
    backplane_path: Path,
    panel_path: Path,
    build_git_hash: str,
    placed_component_count: int,
) -> dict[str, object]:
    carrier_w, carrier_h = board_dimensions(carrier_path)
    backplane_w, backplane_h = board_dimensions(backplane_path)

    carrier_array_h = 6 * carrier_h + 5 * BOARD_GAP
    rotated_backplane_w = backplane_h
    rotated_backplane_h = backplane_w
    content_h = max(carrier_array_h, rotated_backplane_h)

    panel = Panel(str(panel_path))
    stackup_source = pcbnew.LoadBoard(str(backplane_path))
    panel.inheritDesignSettings(stackup_source)
    panel.inheritProperties(stackup_source)
    panel.inheritLayerNames(stackup_source)
    title = stackup_source.GetTitleBlock()
    title.SetTitle("Mini Rack Power: 6 Carriers + 1 Backplane Prototype")
    title.SetRevision("A-PANEL")
    panel.setTitleBlock(title)

    carrier_x = rotated_backplane_w + INTER_DESIGN_GAP + carrier_w // 2
    carrier_y0 = (content_h - carrier_array_h) // 2
    carrier_substrates = []
    for instance in range(1, 7):
        center_y = (
            carrier_y0 + (instance - 1) * (carrier_h + BOARD_GAP) + carrier_h // 2
        )
        carrier_substrates.append(
            add_board(panel, carrier_path, carrier_x, center_y, f"CARRIER{instance}")
        )

    backplane_center_x = rotated_backplane_w // 2
    backplane_center_y = content_h // 2
    backplane_substrate = add_board(
        panel,
        backplane_path,
        backplane_center_x,
        backplane_center_y,
        "BACKPLANE1",
        rotation_degrees=90,
    )

    panel.makeFrame(
        widthH=FRAME_SIDE_WIDTH,
        widthV=FRAME_TOP_BOTTOM_WIDTH,
        hspace=FRAME_HORIZONTAL_GAP,
        vspace=FRAME_VERTICAL_GAP,
        maxWidth=MAX_PANEL_SIZE,
        maxHeight=MAX_PANEL_SIZE,
    )

    # Explicit bridges avoid ambiguity in KiKit's bounding-box partitioner when
    # the backplane begins or ends at the same Y coordinate as a carrier row.
    # KiKit owns the resulting substrate union and mouse-bite drilling.
    tab_cuts = []
    for upper, lower in pairwise(carrier_substrates):
        upper_min_x, _upper_min_y, _upper_max_x, upper_max_y = upper.bounds()
        _lower_min_x, lower_min_y, _lower_max_x, _lower_max_y = lower.bounds()
        for x in (upper_min_x + 20 * mm, upper_min_x + 75 * mm):
            panel.appendSubstrate(
                box(x - TAB_WIDTH / 2, upper_max_y, x + TAB_WIDTH / 2, lower_min_y)
            )
            tab_cuts.append(
                LineString(
                    [(x + TAB_WIDTH / 2, upper_max_y), (x - TAB_WIDTH / 2, upper_max_y)]
                )
            )
            tab_cuts.append(
                LineString(
                    [(x - TAB_WIDTH / 2, lower_min_y), (x + TAB_WIDTH / 2, lower_min_y)]
                )
            )

    # Add frame joins: two at each end of the carrier column and three along
    # the clear, frame-facing long edge of the backplane.
    first_min_x, first_min_y, _first_max_x, _first_max_y = carrier_substrates[
        0
    ].bounds()
    _last_min_x, _last_min_y, _last_max_x, last_max_y = carrier_substrates[-1].bounds()
    for x in (first_min_x + 20 * mm, first_min_x + 75 * mm):
        panel.appendSubstrate(
            box(
                x - TAB_WIDTH / 2,
                first_min_y - FRAME_VERTICAL_GAP,
                x + TAB_WIDTH / 2,
                first_min_y,
            )
        )
        tab_cuts.append(
            LineString(
                [(x - TAB_WIDTH / 2, first_min_y), (x + TAB_WIDTH / 2, first_min_y)]
            )
        )
        panel.appendSubstrate(
            box(
                x - TAB_WIDTH / 2,
                last_max_y,
                x + TAB_WIDTH / 2,
                last_max_y + FRAME_VERTICAL_GAP,
            )
        )
        tab_cuts.append(
            LineString(
                [(x + TAB_WIDTH / 2, last_max_y), (x - TAB_WIDTH / 2, last_max_y)]
            )
        )

    backplane_min_x, backplane_min_y, _backplane_max_x, backplane_max_y = (
        backplane_substrate.bounds()
    )
    for fraction in (0.25, 0.5, 0.75):
        y = backplane_min_y + int((backplane_max_y - backplane_min_y) * fraction)
        panel.appendSubstrate(
            box(
                backplane_min_x - FRAME_HORIZONTAL_GAP,
                y - TAB_WIDTH / 2,
                backplane_min_x,
                y + TAB_WIDTH / 2,
            )
        )
        tab_cuts.append(
            LineString(
                [
                    (backplane_min_x, y + TAB_WIDTH / 2),
                    (backplane_min_x, y - TAB_WIDTH / 2),
                ]
            )
        )

    panel.makeMouseBites(
        tab_cuts,
        diameter=MOUSE_BITE_DIAMETER,
        spacing=MOUSE_BITE_PITCH,
        offset=int(-0.1 * mm),
    )

    # KiKit's JLCPCB exporter plots relative to the auxiliary origin, while its
    # PCBWay exporter uses the absolute board origin. Normalize both to (0, 0)
    # and retain a small positive plotting margin around the physical panel.
    min_x, min_y, _max_x, _max_y = panel.panelBBox()
    panel.translate(
        pcbnew.VECTOR2I(COORDINATE_MARGIN - int(min_x), COORDINATE_MARGIN - int(min_y))
    )
    min_x, min_y, max_x, max_y = panel.panelBBox()

    tooling_positions = [
        (min_x + 3.5 * mm, min_y + 3.5 * mm),
        (max_x - 3.5 * mm, min_y + 3.5 * mm),
        (min_x + 3.5 * mm, max_y - 3.5 * mm),
        (max_x - 3.5 * mm, max_y - 3.5 * mm),
    ]
    for index, (x, y) in enumerate(tooling_positions, start=1):
        panel.addNPTHole(
            pcbnew.VECTOR2I(int(x), int(y)),
            TOOLING_DIAMETER,
            ref=f"PANEL-TOOL{index}",
            excludedFromPos=True,
            solderMaskMargin=int(0.15 * mm),
        )

    fiducial_positions = [
        (min_x + 18 * mm, min_y + 6 * mm),
        (max_x - 18 * mm, min_y + 6 * mm),
        (min_x + 18 * mm, max_y - 6 * mm),
    ]
    for index, (x, y) in enumerate(fiducial_positions, start=1):
        panel.addFiducial(
            pcbnew.VECTOR2I(int(x), int(y)),
            FIDUCIAL_COPPER_DIAMETER,
            FIDUCIAL_MASK_OPENING,
            ref=f"PANEL-FID{index}",
        )

    panel.addText(
        f"PANEL {build_git_hash}",
        pcbnew.VECTOR2I(int((min_x + max_x) / 2), int(min_y + 3.5 * mm)),
        width=int(1.2 * mm),
        height=int(1.2 * mm),
        thickness=int(0.2 * mm),
        layer=Layer.F_SilkS,
    )

    panel.setAuxiliaryOrigin(pcbnew.VECTOR2I(0, 0))
    panel.setGridOrigin(pcbnew.VECTOR2I(0, 0))

    panel.save(reconstructArcs=True, refillAllZones=False, edgeWidth=int(0.1 * mm))

    # Keep legacy source revisions with an unset finish reproducible, while
    # accepting current sources that already record the required ENIG finish.
    # Any other state means the generated panel no longer matches the order
    # documentation and should stop the release.
    panel_text = panel_path.read_text(encoding="utf-8")
    unset_finish_count = panel_text.count('(copper_finish "None")')
    enig_finish_count = panel_text.count('(copper_finish "ENIG")')
    if unset_finish_count == 1 and enig_finish_count == 0:
        panel_path.write_text(
            panel_text.replace('(copper_finish "None")', '(copper_finish "ENIG")'),
            encoding="utf-8",
        )
    elif unset_finish_count != 0 or enig_finish_count != 1:
        raise RuntimeError(
            "Expected exactly one ENIG or unset copper-finish entry in generated panel"
        )

    if panel.hasErrors():
        messages = "\n".join(
            f"at ({position.x / mm:.3f}, {position.y / mm:.3f}) mm: {message}"
            for position, message in panel.errors
        )
        raise RuntimeError(f"KiKit reported panelization errors:\n{messages}")

    width_mm = (max_x - min_x) / mm
    height_mm = (max_y - min_y) / mm
    if width_mm > 250 or height_mm > 250:
        raise RuntimeError(
            f"Panel is too large for JLCPCB PCBA: {width_mm} x {height_mm} mm"
        )
    return {
        "width_mm": round(width_mm, 3),
        "height_mm": round(height_mm, 3),
        "carrier_count": 6,
        "backplane_count": 1,
        "different_design_count": 2,
        "placed_component_count": placed_component_count,
        "layer_count": 4,
        "board_thickness_mm": 1.6,
        "outer_copper_oz": 2,
        "inner_copper_oz": 1,
        "finish": "ENIG",
        "shared_plot_and_position_origin_mm": [0, 0],
        "coordinate_margin_mm": COORDINATE_MARGIN / mm,
        "minimum_routed_gap_mm": BOARD_GAP / mm,
        "tab_width_mm": TAB_WIDTH / mm,
        "mouse_bite_diameter_mm": MOUSE_BITE_DIAMETER / mm,
        "mouse_bite_pitch_mm": MOUSE_BITE_PITCH / mm,
        "mouse_bite_edge_spacing_mm": (MOUSE_BITE_PITCH - MOUSE_BITE_DIAMETER) / mm,
        "mouse_bite_hole_count": EXPECTED_MOUSE_BITES,
        "tooling_hole_count": 4,
        "fiducial_count": 3,
        "carrier_source_revision": build_git_hash,
        "backplane_source_revision": build_git_hash,
        "panel_build_git_hash": build_git_hash,
    }


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8-sig", newline="") as source:
        return list(csv.DictReader(source))


def split_designators(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def natural_ref_key(reference: str) -> tuple[str, int, str]:
    prefix, digits = "", ""
    for char in reference:
        if char.isdigit():
            digits += char
        elif not digits:
            prefix += char
        else:
            return prefix, int(digits), reference
    return prefix, int(digits or 0), reference


def expanded_bom_entries(
    rows: list[dict[str, str]], prefixes: list[str]
) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    for row in rows:
        references = split_designators(row["Designator"])
        if len(references) != int(row["Quantity"]):
            raise RuntimeError(f"BOM quantity mismatch in row: {row}")
        for prefix in prefixes:
            for reference in references:
                entries.append(
                    {
                        "Designator": f"{prefix}-{reference}",
                        "Original Designator": reference,
                        "Footprint": row["Footprint"],
                        "Value": row["Value"],
                        "LCSC Part #": row["LCSC Part #"],
                    }
                )
    return entries


def write_jlc_bom(entries: list[dict[str, str]], destination: Path) -> None:
    grouped: dict[tuple[str, str, str], list[str]] = defaultdict(list)
    for entry in entries:
        grouped[(entry["Footprint"], entry["Value"], entry["LCSC Part #"])].append(
            entry["Designator"]
        )

    with destination.open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(
            output,
            fieldnames=["Designator", "Footprint", "Quantity", "Value", "LCSC Part #"],
        )
        writer.writeheader()
        for (footprint, value, lcsc), references in sorted(grouped.items()):
            references.sort(key=natural_ref_key)
            writer.writerow(
                {
                    "Designator": ", ".join(references),
                    "Footprint": footprint,
                    "Quantity": len(references),
                    "Value": value,
                    "LCSC Part #": lcsc,
                }
            )


def footprint_metadata(board_path: Path) -> dict[str, dict[str, str]]:
    board = pcbnew.LoadBoard(str(board_path))
    result: dict[str, dict[str, str]] = {}
    for footprint in board.GetFootprints():
        fields = {field.GetName(): field.GetText() for field in footprint.GetFields()}
        fields["Footprint"] = footprint.GetFPID().GetUniStringLibId().split(":")[-1]
        result[footprint.GetReference()] = fields
    return result


def write_pcbway_bom(
    entries: list[dict[str, str]],
    carrier_metadata: dict[str, dict[str, str]],
    backplane_metadata: dict[str, dict[str, str]],
    destination: Path,
) -> None:
    grouped: dict[tuple[str, ...], list[str]] = defaultdict(list)
    for entry in entries:
        metadata = (
            carrier_metadata
            if entry["Designator"].startswith("CARRIER")
            else backplane_metadata
        )[entry["Original Designator"]]
        manufacturer = metadata.get("Manufacturer", "").strip()
        part_number = metadata.get("MPN", "").strip()
        notes = ""
        if not manufacturer or not part_number:
            manufacturer = manufacturer or "Not specified in released BOM"
            part_number = part_number or entry["Value"]
            notes = (
                f"Use exact released LCSC part {entry['LCSC Part #']}; "
                "confirm manufacturer and do not substitute without approval."
            )
        key = (
            manufacturer,
            part_number,
            metadata.get("Description", "").strip(),
            entry["Footprint"],
            entry["Value"],
            entry["LCSC Part #"],
            notes,
        )
        grouped[key].append(entry["Designator"])

    fields = [
        "Item #",
        "Designator",
        "Qty",
        "Manufacturer",
        "Manufacturer Part Number",
        "Description",
        "Package / Footprint",
        "Value",
        "LCSC Part #",
        "Customer Notes",
    ]
    with destination.open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        for item_number, (key, references) in enumerate(
            sorted(grouped.items()), start=1
        ):
            manufacturer, part_number, description, footprint, value, lcsc, notes = key
            references.sort(key=natural_ref_key)
            writer.writerow(
                {
                    "Item #": item_number,
                    "Designator": ", ".join(references),
                    "Qty": len(references),
                    "Manufacturer": manufacturer,
                    "Manufacturer Part Number": part_number,
                    "Description": description,
                    "Package / Footprint": footprint,
                    "Value": value,
                    "LCSC Part #": lcsc,
                    "Customer Notes": notes,
                }
            )


def write_positions(
    panel_path: Path, allowed_references: set[str], destination: Path
) -> None:
    raw_position_path = destination.with_suffix(".kicad.csv")
    run(
        "kicad-cli",
        "pcb",
        "export",
        "pos",
        "--output",
        str(raw_position_path),
        "--side",
        "both",
        "--format",
        "csv",
        "--units",
        "mm",
        "--use-drill-file-origin",
        "--exclude-dnp",
        str(panel_path),
    )
    rows = read_csv(raw_position_path)
    by_reference = {row["Ref"]: row for row in rows}
    missing = allowed_references - by_reference.keys()
    if missing:
        raise RuntimeError(
            f"Position output is missing BOM references: {sorted(missing)}"
        )

    with destination.open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(
            output,
            fieldnames=["Designator", "Mid X", "Mid Y", "Rotation", "Layer"],
        )
        writer.writeheader()
        for reference in sorted(allowed_references, key=natural_ref_key):
            row = by_reference[reference]
            writer.writerow(
                {
                    "Designator": reference,
                    "Mid X": row["PosX"],
                    "Mid Y": row["PosY"],
                    "Rotation": row["Rot"],
                    "Layer": row["Side"].lower(),
                }
            )
    raw_position_path.unlink()


def find_gerber_archive(directory: Path) -> Path:
    archives = sorted(directory.glob("*.zip"))
    if len(archives) != 1:
        raise RuntimeError(
            f"Expected one Gerber archive in {directory}, found {archives}"
        )
    return archives[0]


def prepare_gerber_archive(source: Path, destination: Path) -> None:
    """Remove KiKit's V-cut/comment plot from this routed-only panel archive."""
    with (
        zipfile.ZipFile(source) as input_archive,
        zipfile.ZipFile(
            destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as output_archive,
    ):
        for source_info in input_archive.infolist():
            name = source_info.filename
            basename = Path(name).name
            is_comment_layer = basename in {
                f"{PANEL_NAME}-CmtUser.gbr",
                f"{PANEL_NAME}.gbr",
            }
            if is_comment_layer:
                continue

            data = input_archive.read(name)
            if name.endswith(".gbrjob"):
                job = json.loads(data)
                job["FilesAttributes"] = [
                    entry
                    for entry in job.get("FilesAttributes", [])
                    if entry.get("Path")
                    not in {f"{PANEL_NAME}-CmtUser.gbr", f"{PANEL_NAME}.gbr"}
                ]
                job.get("GeneralSpecs", {})["Finish"] = "ENIG"
                data = (json.dumps(job, indent=2) + "\n").encode()

            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            if name.endswith("/"):
                info.external_attr = (0o40755 << 16) | 0x10
            output_archive.writestr(info, data)


def validate_gerber_archive(archive_path: Path) -> dict[str, object]:
    with zipfile.ZipFile(archive_path) as archive:
        files = [name for name in archive.namelist() if not name.endswith("/")]

    lower_names = [name.lower() for name in files]
    required_groups = {
        "top_copper": (".gtl",),
        "bottom_copper": (".gbl",),
        "inner_1": ("-inner1.g1", ".g1"),
        "inner_2": ("-inner2.g2", ".g2"),
        "top_mask": (".gts",),
        "bottom_mask": (".gbs",),
        "top_silkscreen": (".gto",),
        "edge_cuts": (".gm1",),
        "pth_drill": ("-pth.drl",),
        "npth_drill": ("-npth.drl",),
    }
    missing = [
        group
        for group, suffixes in required_groups.items()
        if not any(
            any(name.endswith(suffix) for suffix in suffixes) for name in lower_names
        )
    ]
    if missing:
        raise RuntimeError(f"Gerber archive {archive_path} is missing: {missing}")

    comment_layers = [
        name
        for name in lower_names
        if name.endswith(("-cmtuser.gbr", f"/{PANEL_NAME.lower()}.gbr"))
    ]
    if comment_layers:
        raise RuntimeError(f"Gerber archive contains comment layers: {comment_layers}")

    return {"file_count": len(files), "required_layer_groups": sorted(required_groups)}


def run_drc(panel_path: Path, report_path: Path) -> dict[str, object]:
    run(
        "kicad-cli",
        "pcb",
        "drc",
        "--format",
        "json",
        "--output",
        str(report_path),
        str(panel_path),
    )
    report = json.loads(report_path.read_text(encoding="utf-8"))
    violations = report.get("violations", [])
    violation_counts = Counter(item["type"] for item in violations)
    forbidden = sorted(FABRICATION_DRC_FAILURES.intersection(violation_counts))
    unconnected_count = len(report.get("unconnected_items", []))
    if forbidden or unconnected_count:
        raise RuntimeError(
            "Panel has fabrication-critical DRC findings: "
            f"types={forbidden}, unconnected={unconnected_count}"
        )

    board = pcbnew.LoadBoard(str(panel_path))
    mouse_bites = sum(
        1
        for footprint in board.GetFootprints()
        if footprint.GetReference().startswith("KiKit_MB_")
    )
    tooling = sum(
        1
        for footprint in board.GetFootprints()
        if footprint.GetReference().startswith("PANEL-TOOL")
    )
    fiducials = sum(
        1
        for footprint in board.GetFootprints()
        if footprint.GetReference().startswith("PANEL-FID")
    )
    if (mouse_bites, tooling, fiducials) != (EXPECTED_MOUSE_BITES, 4, 3):
        raise RuntimeError(
            "Panel feature count mismatch: "
            f"mouse_bites={mouse_bites}, tooling={tooling}, fiducials={fiducials}"
        )

    return {
        "status": "pass_with_inherited_release_findings",
        "fabrication_critical_violation_count": 0,
        "unconnected_item_count": unconnected_count,
        "reported_violation_count": len(violations),
        "reported_violation_counts": dict(sorted(violation_counts.items())),
        "mouse_bite_hole_count": mouse_bites,
        "tooling_hole_count": tooling,
        "fiducial_count": fiducials,
        "note": (
            "Remaining findings are inherited courtyard, silkscreen, and library checks. "
            "Panel-created NPTH courtyard findings are mouse bites crossing the DNP MOD1 "
            "courtyard; they do not report copper, drill, or board-outline conflicts."
        ),
    }


def render_board(
    board_path: Path,
    destination: Path,
    width_px: int,
    height_px: int,
    side: str,
) -> dict[str, int]:
    destination.parent.mkdir(parents=True, exist_ok=True)
    run(
        "kicad-cli",
        "pcb",
        "render",
        "--output",
        str(destination),
        "--width",
        str(width_px),
        "--height",
        str(height_px),
        "--side",
        side,
        "--background",
        "opaque",
        "--quality",
        "basic",
        str(board_path),
    )
    png = destination.read_bytes()
    if not png.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(f"KiCad did not produce a valid PNG render at {destination}")
    actual_width_px, actual_height_px = struct.unpack(">II", png[16:24])
    return {
        "requested_width_px": width_px,
        "requested_height_px": height_px,
        "actual_width_px": actual_width_px,
        "actual_height_px": actual_height_px,
    }


def render_documentation(
    carrier_board: Path,
    backplane_board: Path,
    panel_path: Path,
    output_dir: Path,
    panel_info: dict[str, object],
) -> tuple[dict[str, Path], Path, dict[str, object]]:
    render_dir = output_dir / "renders"
    render_specs = {
        "carrier": {
            "board": carrier_board,
            "width_px": 1600,
            "height_px": 900,
            "side": "top",
            "source_board": CARRIER_BOARD,
            "source_revision": panel_info["carrier_source_revision"],
            "instances": 1,
        },
        "carrier_bottom": {
            "board": carrier_board,
            "width_px": 1600,
            "height_px": 900,
            "side": "bottom",
            "source_board": CARRIER_BOARD,
            "source_revision": panel_info["carrier_source_revision"],
            "instances": 1,
        },
        "backplane_prototype": {
            "board": backplane_board,
            "width_px": 1600,
            "height_px": 900,
            "side": "top",
            "source_board": BACKPLANE_BOARD,
            "source_revision": panel_info["backplane_source_revision"],
            "instances": 1,
        },
        "backplane_prototype_bottom": {
            "board": backplane_board,
            "width_px": 1600,
            "height_px": 900,
            "side": "bottom",
            "source_board": BACKPLANE_BOARD,
            "source_revision": panel_info["backplane_source_revision"],
            "instances": 1,
        },
        "combined_panel": {
            "board": panel_path,
            "width_px": 1400,
            "height_px": 2000,
            "side": "top",
            "source_board": panel_path.name,
            "source_revision": panel_info["panel_build_git_hash"],
            "instances": 7,
        },
    }

    render_paths: dict[str, Path] = {}
    render_entries: dict[str, object] = {}
    for key, spec in render_specs.items():
        destination = render_dir / DOC_RENDER_NAMES[key]
        dimensions = render_board(
            spec["board"],
            destination,
            spec["width_px"],
            spec["height_px"],
            spec["side"],
        )
        render_paths[key] = destination
        render_entries[key] = {
            "file": destination.name,
            "source_board": spec["source_board"],
            "source_revision": spec["source_revision"],
            "instances": spec["instances"],
            "side": spec["side"],
            **dimensions,
        }

    kicad_version = subprocess.run(
        ["kicad-cli", "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    manifest: dict[str, object] = {
        "schema_version": 1,
        "renderer": {
            "tool": "kicad-cli pcb render",
            "kicad_version": kicad_version,
            "background": "opaque",
            "quality": "basic",
        },
        "panel_sources": {
            "carrier_revision": panel_info["carrier_source_revision"],
            "carrier_instances": 6,
            "backplane_revision": panel_info["backplane_source_revision"],
            "backplane_instances": 1,
            "panel_build_git_hash": panel_info["panel_build_git_hash"],
        },
        "renders": render_entries,
    }
    manifest_path = render_dir / "renders.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return render_paths, manifest_path, manifest


def publish_documentation_renders(
    render_paths: dict[str, Path],
    manifest_path: Path,
    destination: Path,
) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    for render_path in render_paths.values():
        shutil.copy2(render_path, destination / render_path.name)
    shutil.copy2(manifest_path, destination / manifest_path.name)


def deterministic_bundle(destination: Path, files: list[tuple[Path, str]]) -> None:
    with zipfile.ZipFile(
        destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for source, archive_name in files:
            info = zipfile.ZipInfo(archive_name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, source.read_bytes())


def write_order_notes(
    destination: Path, vendor: str, panel_info: dict[str, float]
) -> None:
    vendor_instructions = {
        "jlcpcb": """Select 'Panelized PCBs already in Gerber' / customer-supplied panel and set
'Different Designs' to 2. For PCBA, use the complete panel BOM/CPL option and
prefer Standard PCBA; ask engineering to confirm the 2 oz outer / 1 oz inner
stack-up before payment.""",
        "pcbway": """Select customer-supplied panelization and set the number of different designs
to 2. Submit the centroid file as the pick-and-place file and ask engineering
to confirm the 2 oz outer / 1 oz inner stack-up before payment.""",
    }[vendor]
    text = f"""Mini Rack Power carrier/backplane Rev A panel

Vendor: {vendor}
Panel: 6 carrier PCBs + 1 backplane prototype PCB
Panel dimensions: {panel_info["width_mm"]:.3f} mm x {panel_info["height_mm"]:.3f} mm
Panel build Git revision: {panel_info["panel_build_git_hash"]}
Construction: FR-4, 4 layers, 1.6 mm, ENIG
Copper: 2 oz outer layers, 1 oz inner layers
Separation: routed gaps (2 mm minimum) with 5 mm mouse-bite tabs
Mouse bites: 0.6 mm NPTH, 0.95 mm pitch (0.35 mm edge-to-edge)
Assembly side: top only
Different designs in panel: 2

Upload the nested Gerber ZIP for PCB fabrication, then upload BOM.csv and
positions.csv in the assembly step. Do not ask the vendor to re-panelize or
depanelize before assembly. Ask CAM engineering to preserve the supplied
customer panel, tooling holes, fiducials, routed gaps, and mouse bites.

{vendor_instructions}

Order quantity is the number of complete panels. Each panel yields six carrier
boards and one backplane board. Do not permit component substitutions without
approval; PCBWay BOM rows marked 'Not specified in released BOM' must be matched
to the exact LCSC part stated in the Customer Notes field.
"""
    destination.write_text(text, encoding="utf-8")


def write_release_readme(destination: Path, panel_info: dict[str, float]) -> None:
    destination.write_text(
        f"""# Carrier + backplane Rev A fabrication panel

This customer panel contains six released carrier PCBs and one released
backplane-prototype PCB. It measures {panel_info["width_mm"]:.3f} x
{panel_info["height_mm"]:.3f} mm and is intended for top-side assembly.
The top rail is marked `PANEL {panel_info["panel_build_git_hash"]}` on F.SilkS.

- `jlcpcb/`: JLCPCB upload bundle, separate files, and order notes.
- `pcbway/`: PCBWay upload bundle, separate files, and order notes.
- `{PANEL_NAME}.kicad_pcb`: generated panel source for CAM review.
- `{RENDER_NAME}`: top-side PNG render for visual review.
- `panel-info.json`: dimensions, construction, source revisions, and counts.
- `validation.json`: generated DRC and Gerber archive checks.

Each vendor bundle contains one nested Gerber ZIP, `BOM.csv`, `positions.csv`,
and `ORDER-NOTES.txt`. Read the vendor order notes before placing an order.

The panel is reproducible with:

```sh
mise run panel:build
```
""",
        encoding="utf-8",
    )


def build(
    output_dir: Path,
    release_dir: Path | None,
    docs_render_dir: Path | None,
    carrier_production_dir: Path,
    backplane_production_dir: Path,
    docs_only: bool,
    common_text_variables: dict[str, str],
    carrier_text_variables: dict[str, str],
    backplane_text_variables: dict[str, str],
) -> None:
    root = git_root()
    build_git_hash = panel_git_hash(root)
    build_revision = resolve_git_revision(root, build_git_hash)
    production_validations: dict[str, dict[str, object]] = {}
    if not docs_only:
        production_validations = {
            "carrier": validate_production_package(
                carrier_production_dir,
                build_revision,
                CARRIER_BOARD,
                CARRIER_ARCHIVE,
            ),
            "backplane": validate_production_package(
                backplane_production_dir,
                build_revision,
                BACKPLANE_BOARD,
                BACKPLANE_ARCHIVE,
            ),
        }
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True)

    sources = output_dir / "sources"
    carrier_board = sources / CARRIER_BOARD
    carrier_bom = sources / "carrier-bom.csv"
    backplane_board = sources / BACKPLANE_BOARD
    backplane_bom = sources / "backplane-bom.csv"
    materialize_git_board(
        root,
        build_git_hash,
        CARRIER_BOARD,
        carrier_board,
        sources,
    )
    materialize_git_board(
        root,
        build_git_hash,
        BACKPLANE_BOARD,
        backplane_board,
        sources,
    )
    if docs_only:
        materialize_git_file(root, build_git_hash, CARRIER_BOM, carrier_bom)
        materialize_git_file(root, build_git_hash, BACKPLANE_BOM, backplane_bom)
    else:
        shutil.copy2(carrier_production_dir / "bom.csv", carrier_bom)
        shutil.copy2(backplane_production_dir / "bom.csv", backplane_bom)

    carrier_release_variables = release_text_variables(
        root,
        build_git_hash,
        CARRIER_PROJECT,
        common_text_variables,
        carrier_text_variables,
    )
    backplane_release_variables = release_text_variables(
        root,
        build_git_hash,
        BACKPLANE_PROJECT,
        common_text_variables,
        backplane_text_variables,
    )
    carrier_used_variables = bake_board_text_variables(
        carrier_board, carrier_release_variables
    )
    backplane_used_variables = bake_board_text_variables(
        backplane_board, backplane_release_variables
    )

    carrier_entries = expanded_bom_entries(
        read_csv(carrier_bom), [f"CARRIER{index}" for index in range(1, 7)]
    )
    backplane_entries = expanded_bom_entries(read_csv(backplane_bom), ["BACKPLANE1"])
    entries = carrier_entries + backplane_entries
    allowed_references = {entry["Designator"] for entry in entries}
    if len(allowed_references) != len(entries):
        raise RuntimeError("Expanded panel BOM contains duplicate designators")

    panel_path = output_dir / f"{PANEL_NAME}.kicad_pcb"
    panel_info = build_panel(
        carrier_board,
        backplane_board,
        panel_path,
        build_git_hash,
        len(allowed_references),
    )
    panel_render_board = sources / "hardware/boards/panel-render" / panel_path.name
    panel_render_board.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(panel_path, panel_render_board)
    panel_info["board_silkscreen_variables"] = {
        "carrier": {
            name: carrier_release_variables[name]
            for name in sorted(carrier_used_variables)
        },
        "backplane": {
            name: backplane_release_variables[name]
            for name in sorted(backplane_used_variables)
        },
    }
    if production_validations:
        panel_info["production_packages"] = {
            "carrier": {
                "status": production_validations["carrier"]["status"],
                "source_revision": build_revision,
                "archive": CARRIER_ARCHIVE,
            },
            "backplane": {
                "status": production_validations["backplane"]["status"],
                "source_revision": build_revision,
                "archive": BACKPLANE_ARCHIVE,
            },
        }
    (output_dir / "panel-info.json").write_text(
        json.dumps(panel_info, indent=2) + "\n", encoding="utf-8"
    )

    render_paths, render_manifest_path, render_manifest = render_documentation(
        carrier_board,
        backplane_board,
        panel_render_board,
        output_dir,
        panel_info,
    )
    if docs_render_dir is not None:
        publish_documentation_renders(
            render_paths,
            render_manifest_path,
            docs_render_dir,
        )
    if docs_only:
        # Stage schematic metadata only after the PCB/panel render path is
        # complete. A sibling .kicad_pro changes the design rules KiCad loads
        # for a PCB, so introducing it earlier could affect panel generation.
        for project_file in CARRIER_PROJECT_FILES:
            materialize_git_file(
                root,
                build_git_hash,
                project_file,
                sources / Path(project_file).name,
            )
        for project_file in BACKPLANE_PROJECT_FILES:
            materialize_git_file(
                root,
                build_git_hash,
                project_file,
                sources / Path(project_file).name,
            )
        print(json.dumps(render_manifest, indent=2))
        return

    common_positions = output_dir / "positions.csv"
    write_positions(panel_path, allowed_references, common_positions)
    carrier_metadata = footprint_metadata(carrier_board)
    backplane_metadata = footprint_metadata(backplane_board)

    drc_report_path = output_dir / "panel-drc.json"
    validation = run_drc(panel_path, drc_report_path)
    panel_render_path = render_paths["combined_panel"]
    validation["png_render"] = render_manifest["renders"]["combined_panel"]

    vendor_outputs: list[Path] = []
    for vendor in ("jlcpcb", "pcbway"):
        vendor_dir = output_dir / vendor
        fab_dir = vendor_dir / "fab"
        vendor_dir.mkdir(parents=True)
        run(
            "kikit",
            "fab",
            vendor,
            "--no-drc",
            "--no-assembly",
            str(panel_path),
            str(fab_dir),
        )
        gerber_source = find_gerber_archive(fab_dir)
        gerber_destination = vendor_dir / f"{PANEL_NAME}-gerbers.zip"
        prepare_gerber_archive(gerber_source, gerber_destination)
        validation[f"{vendor}_gerbers"] = validate_gerber_archive(gerber_destination)
        shutil.copy2(common_positions, vendor_dir / "positions.csv")

        if vendor == "jlcpcb":
            write_jlc_bom(entries, vendor_dir / "BOM.csv")
        else:
            write_pcbway_bom(
                entries,
                carrier_metadata,
                backplane_metadata,
                vendor_dir / "BOM.csv",
            )
        write_order_notes(vendor_dir / "ORDER-NOTES.txt", vendor, panel_info)
        bundle = vendor_dir / f"{PANEL_NAME}-{vendor}-bundle.zip"
        deterministic_bundle(
            bundle,
            [
                (gerber_destination, gerber_destination.name),
                (vendor_dir / "BOM.csv", "BOM.csv"),
                (vendor_dir / "positions.csv", "positions.csv"),
                (vendor_dir / "ORDER-NOTES.txt", "ORDER-NOTES.txt"),
            ],
        )
        vendor_outputs.append(vendor_dir)

    validation_path = output_dir / "validation.json"
    validation_path.write_text(
        json.dumps(validation, indent=2) + "\n", encoding="utf-8"
    )

    if release_dir is not None:
        if release_dir.exists():
            shutil.rmtree(release_dir)
        release_dir.mkdir(parents=True)
        shutil.copy2(panel_path, release_dir / panel_path.name)
        shutil.copy2(panel_render_path, release_dir / RENDER_NAME)
        shutil.copy2(output_dir / "panel-info.json", release_dir / "panel-info.json")
        shutil.copy2(validation_path, release_dir / "validation.json")
        write_release_readme(release_dir / "README.md", panel_info)
        for vendor_dir in vendor_outputs:
            destination = release_dir / vendor_dir.name
            destination.mkdir()
            for source in vendor_dir.iterdir():
                if source.is_file():
                    shutil.copy2(source, destination / source.name)

    print(json.dumps(panel_info, indent=2))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("build/carrier-backplane-panel"),
    )
    parser.add_argument("--release-dir", type=Path)
    parser.add_argument("--docs-render-dir", type=Path)
    parser.add_argument(
        "--carrier-production-dir",
        type=Path,
        default=CARRIER_PRODUCTION_DIR,
    )
    parser.add_argument(
        "--backplane-production-dir",
        type=Path,
        default=BACKPLANE_PRODUCTION_DIR,
    )
    parser.add_argument(
        "--docs-only",
        action="store_true",
        help="Build the panel and documentation PNGs without fabrication outputs",
    )
    parser.add_argument(
        "-D",
        "--define-var",
        action="append",
        type=parse_text_variable,
        metavar="KEY=VALUE",
        help="Override a project text variable for every source board",
    )
    parser.add_argument(
        "--carrier-define-var",
        action="append",
        type=parse_text_variable,
        metavar="KEY=VALUE",
        help="Override a project text variable for the carrier board",
    )
    parser.add_argument(
        "--backplane-define-var",
        action="append",
        type=parse_text_variable,
        metavar="KEY=VALUE",
        help="Override a project text variable for the backplane board",
    )
    return parser.parse_args()


if __name__ == "__main__":
    arguments = parse_args()
    try:
        if arguments.docs_only and arguments.release_dir:
            raise ValueError("--docs-only cannot be combined with --release-dir")
        build(
            arguments.output_dir.resolve(),
            arguments.release_dir.resolve() if arguments.release_dir else None,
            arguments.docs_render_dir.resolve() if arguments.docs_render_dir else None,
            arguments.carrier_production_dir.resolve(),
            arguments.backplane_production_dir.resolve(),
            arguments.docs_only,
            text_variable_map(arguments.define_var),
            text_variable_map(arguments.carrier_define_var),
            text_variable_map(arguments.backplane_define_var),
        )
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        raise
