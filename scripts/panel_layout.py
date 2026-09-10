"""Project-specific mixed-design panel geometry, executed in the pinned KiKit image."""

from itertools import pairwise
import json
from pathlib import Path

import pcbnew
from kikit.common import fromDegrees
from kikit.defs import Layer
from kikit.panelize import Origin, Panel
from kikit.sexpr import parseSexprS
from kikit.units import mm
from shapely.geometry import LineString, box

from pcb_panel_policy import (
    bake_visible_reference,
    namespace_component_classes,
    namespace_reference_conditions,
)
from pcb_panel_mousebites import MOUSE_BITE_RULE, validate_mouse_bites
from validate_pcb_release import write_json

if not hasattr(pcbnew.SwigPyIterator, "next"):
    pcbnew.SwigPyIterator.next = pcbnew.SwigPyIterator.__next__

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
    first_rule = len(panel.customDRCRules)
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
        bakeRef=False,
    )
    footprints = [
        fp
        for fp in panel.board.GetFootprints()
        if fp.GetReference().startswith(prefix + "-")
    ]
    references = {fp.GetReference()[len(prefix) + 1 :] for fp in footprints}
    for fp in footprints:
        bake_visible_reference(
            panel.board,
            fp.Reference(),
            fp.GetReference()[len(prefix) + 1 :],
            pcbnew.PCB_TEXT,
        )
    # KiKit namespaces NetName comparisons, but leaves Reference comparisons
    # untouched. Fix the rules of this instance only, not those already appended.
    for rule in panel.customDRCRules[first_rule:]:
        for clause in rule.items[2:]:
            if clause.items[0].value == "condition":
                clause.items[1].value = namespace_reference_conditions(
                    clause.items[1].value, prefix, references
                )
    return panel.substrates[-1]


def build_panel(
    carrier_path: Path,
    backplane_path: Path,
    panel_path: Path,
    build_git_hash: str,
    placed_component_count: int,
) -> dict[str, object]:
    # KiKit creates fresh UUIDs for rails, copied items, and breakaways. Seed
    # KiCad's generator to reduce variation; this does not make every output
    # byte-identical (see the release tooling guide).
    pcbnew.KIID.SeedGenerator(int(build_git_hash[:8], 16))
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

    before_mouse_bites = {fp.m_Uuid.AsString() for fp in panel.board.GetFootprints()}
    panel.makeMouseBites(
        tab_cuts,
        diameter=MOUSE_BITE_DIAMETER,
        spacing=MOUSE_BITE_PITCH,
        offset=int(-0.1 * mm),
    )
    mouse_bites = []
    for fp in panel.board.GetFootprints():
        if fp.m_Uuid.AsString() in before_mouse_bites:
            continue
        pads = list(fp.Pads())
        if len(pads) != 1:
            raise ValueError(
                f"Mouse bite {fp.GetReference()} must contain exactly one pad"
            )
        pad = pads[0]
        mouse_bites.append(
            {
                "reference": fp.GetReference(),
                "x": pad.GetPosition().x / mm,
                "y": pad.GetPosition().y / mm,
                "drill_x": pad.GetDrillSize().x / mm,
                "drill_y": pad.GetDrillSize().y / mm,
                "npth": pad.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH,
                "net": pad.GetNetname(),
            }
        )
    mouse_bite_info = validate_mouse_bites(mouse_bites, expected_sets=len(tab_cuts))
    if mouse_bite_info["mouse_bite_hole_count"] != EXPECTED_MOUSE_BITES:
        raise ValueError("Mouse-bite hole count changed; review the panel geometry")
    # Later rules take precedence. This overrides only the hole-to-hole limit
    # between generated NPTH mouse bites, never ordinary holes or a mixed pair.
    # Copper clearance and component courtyards remain fully checked.
    panel.customDRCRules.append(parseSexprS(MOUSE_BITE_RULE))

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

    # This is disposable generated project metadata, never a source project.
    # KiKit's inheritDesignSettings only transfers board design settings; KiCad
    # stores component-class assignments separately at the project top level.
    panel_project_path = panel_path.with_suffix(".kicad_pro")
    panel_project = json.loads(panel_project_path.read_text())
    assignments = []
    for source, prefixes in (
        (carrier_path, [f"CARRIER{i}" for i in range(1, 7)]),
        (backplane_path, ["BACKPLANE1"]),
    ):
        source_project = json.loads(source.with_suffix(".kicad_pro").read_text())
        for prefix in prefixes:
            references = {
                fp.GetReference()[len(prefix) + 1 :]
                for fp in panel.board.GetFootprints()
                if fp.GetReference().startswith(prefix + "-")
            }
            assignments.extend(
                namespace_component_classes(source_project, prefix, references)
            )
    panel_project.setdefault("component_class_settings", {})["assignments"] = (
        assignments
    )
    write_json(panel_project_path, panel_project)

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
        **mouse_bite_info,
        "mouse_bite_requested_pitch_mm": MOUSE_BITE_PITCH / mm,
        "tooling_hole_count": 4,
        "fiducial_count": 3,
        "carrier_source_revision": build_git_hash,
        "backplane_source_revision": build_git_hash,
        "panel_build_git_hash": build_git_hash,
    }
