"""JLCPCB mouse-bite policy, separate from ordinary component/pad-hole rules.

Reference: https://jlcpcb.com/blog/pcb-design-efficiency-mouse-bites
Verified 2026-09-07: 5–8 holes/set, diameter 0.6 mm, edge gap 0.35–0.4 mm
recommended, 0.3 mm minimum. KiKit may redistribute its requested pitch;
validate the actual generated holes, not just the generator's parameters.
"""

from collections import defaultdict
from itertools import pairwise
from math import dist, isclose, isfinite
import re


MOUSE_BITE_RULE = """(rule "JLCPCB generated mouse-bite hole spacing"
    (condition "A.Type == 'Pad' && B.Type == 'Pad' && A.Pad_Type == 'NPTH, mechanical' && B.Pad_Type == 'NPTH, mechanical' && A.memberOfFootprint('KiKit_MB_*') && B.memberOfFootprint('KiKit_MB_*')")
    (constraint hole_to_hole (min 0.3mm)))"""


def validate_mouse_bites(holes: list[dict], expected_sets: int) -> dict:
    """Validate newly generated holes and return measured panel metadata.

    Each record contains reference, x/y and drill_x/drill_y in millimetres,
    npth (bool), and net (string). Only the generator's newly added footprints
    may be supplied; namespaced source-board holes cannot qualify.
    """
    groups = defaultdict(list)
    seen = set()
    for hole in holes:
        ref = hole["reference"]
        match = re.fullmatch(r"KiKit_MB_([1-9]\d*)_([1-9]\d*)", ref)
        if not match or ref in seen:
            raise ValueError(f"Unexpected or duplicate generated mouse bite: {ref}")
        seen.add(ref)
        dimensions = [hole[k] for k in ("x", "y", "drill_x", "drill_y")]
        if (
            not all(isfinite(v) for v in dimensions)
            or not hole["npth"]
            or hole["net"]
            or not isclose(hole["drill_x"], 0.6, abs_tol=1e-6)
            or not isclose(hole["drill_y"], 0.6, abs_tol=1e-6)
        ):
            raise ValueError(f"Mouse bite {ref} must be an unconnected 0.6 mm NPTH")
        groups[int(match[1])].append((int(match[2]), hole))
    if expected_sets < 1 or len(groups) != expected_sets:
        raise ValueError(f"Expected {expected_sets} mouse-bite sets, got {len(groups)}")

    pitches, gaps = [], []
    for number, group in sorted(groups.items()):
        if not 5 <= len(group) <= 8:
            raise ValueError(
                f"Mouse-bite set {number} needs 5–8 holes, got {len(group)}"
            )
        group.sort(key=lambda item: item[0])
        points = [(hole["x"], hole["y"]) for _, hole in group]
        if not any(
            max(p[axis] for p in points) - min(p[axis] for p in points) <= 1e-6
            for axis in (0, 1)
        ):
            raise ValueError(f"Mouse-bite set {number} is not horizontal or vertical")
        for (index_a, a), (index_b, b) in pairwise(group):
            pitch = dist((a["x"], a["y"]), (b["x"], b["y"]))
            gap = pitch - (a["drill_x"] + b["drill_x"]) / 2
            if index_b != index_a + 1 or not 0.3 - 1e-6 <= gap <= 0.4 + 1e-6:
                raise ValueError(
                    f"Mouse-bite set {number} has an invalid gap: {gap:.6f} mm"
                )
            pitches.append(pitch)
            gaps.append(gap)
    # This layout intentionally uses one common pitch. Never publish a nominal
    # value that disguises redistributed or differently spaced rows.
    if max(pitches) - min(pitches) > 1e-6:
        raise ValueError("Mouse-bite sets do not have a common pitch")
    return {
        "mouse_bite_diameter_mm": 0.6,
        "mouse_bite_pitch_mm": round(min(pitches), 6),
        "mouse_bite_edge_spacing_mm": round(min(gaps), 6),
        "mouse_bite_minimum_edge_spacing_mm": 0.3,
        "mouse_bite_hole_count": len(holes),
        "mouse_bite_set_count": len(groups),
    }
