"""Release invariants shared by the CLI, container worker, and tests."""

from __future__ import annotations

import csv
import hashlib
import json
import math
import re
import zipfile
from collections import Counter
from pathlib import Path


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8-sig", newline="") as stream:
        return list(csv.DictReader(stream))


def check_reports(drc: dict, erc: dict | None = None) -> dict:
    if not all(key in drc for key in ("violations", "unconnected_items")):
        raise ValueError("Incomplete DRC report")
    active = lambda values: [v for v in values if not v.get("excluded", False)]
    violations = active(drc["violations"])
    parity = active(drc.get("schematic_parity", []))
    disconnected = active(drc["unconnected_items"])
    if erc is not None and ("sheets" not in erc or "schematic_parity" not in drc):
        raise ValueError("Missing ERC or schematic parity results")
    electrical = active(
        [v for s in (erc or {}).get("sheets", []) for v in s.get("violations", [])]
    )
    errors = [v for v in violations + parity if v.get("severity") != "warning"]
    result = {
        "errors": len(errors),
        "unconnected": len(disconnected),
        "erc_findings": len(electrical),
        "warnings": dict(
            sorted(
                Counter(
                    v.get("type", "unknown")
                    for v in violations + parity
                    if v.get("severity") == "warning"
                ).items()
            )
        ),
    }
    if errors or disconnected or electrical:
        raise ValueError(f"Release gate failed: {result}")
    return result


def check_assembly(bom: Path, positions: Path, *, panel: bool = False) -> set[str]:
    refs: list[str] = []
    for row in read_csv(bom):
        group = [ref.strip() for ref in row["Designator"].split(",") if ref.strip()]
        qty = row.get("Quantity", row.get("Qty"))
        if not group or (qty is not None and int(qty) != len(group)):
            raise ValueError(f"BOM quantity mismatch: {row}")
        if not re.fullmatch(r"C\d+", row.get("LCSC Part #", "")):
            raise ValueError(f"Missing LCSC number: {row}")
        refs.extend(group)
    rows = read_csv(positions)
    placed = [row["Designator"] for row in rows]
    if not refs or len(refs) != len(set(refs)) or len(placed) != len(set(placed)):
        raise ValueError("Empty assembly data or duplicate references")
    if set(refs) != set(placed):
        raise ValueError(f"BOM/CPL mismatch: {sorted(set(refs) ^ set(placed))}")
    for row in rows:
        x, y, rotation = (float(row[key]) for key in ("Mid X", "Mid Y", "Rotation"))
        if (
            not all(math.isfinite(v) for v in (x, y, rotation))
            or not 0 <= rotation < 360
        ):
            raise ValueError(f"Invalid placement coordinates: {row}")
        if row["Layer"].lower() not in {"top", "bottom"}:
            raise ValueError(f"Invalid placement side: {row}")
        # KiCad CPL uses Cartesian Y (negative below the absolute origin).
        if panel and (x < 0 or row["Layer"].lower() != "top"):
            raise ValueError(
                f"Panel assembly must be top-side with nonnegative X: {row}"
            )
    return set(refs)


def deterministic_zip(path: Path, files: dict[str, Path]) -> None:
    if any(Path(name).name != name for name in files):
        raise ValueError("Archives must have a flat, explicit file set")
    with zipfile.ZipFile(
        path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for name, source in sorted(files.items()):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, source.read_bytes(), compresslevel=9)
    with zipfile.ZipFile(path) as archive:
        if set(archive.namelist()) != set(files) or archive.testzip() is not None:
            raise ValueError(f"Invalid archive: {path}")


def check_cam(directory: Path) -> dict[str, Path]:
    required = {
        "F_Cu.gtl",
        "In1_Cu.g1",
        "In2_Cu.g2",
        "B_Cu.gbl",
        "F_Mask.gts",
        "B_Mask.gbs",
        "F_Silkscreen.gto",
        "B_Silkscreen.gbo",
        "F_Paste.gtp",
        "B_Paste.gbp",
        "Edge_Cuts.gm1",
        "PTH.drl",
        "NPTH.drl",
    }
    files = {path.name: path for path in directory.iterdir() if path.is_file()}
    # KiBot also emits construction metadata, retained beside the upload ZIP.
    files.pop("stackup.gbrjob", None)
    if set(files) != required or any(not p.stat().st_size for p in files.values()):
        raise ValueError(
            f"CAM file set mismatch: missing={required - files.keys()}, extra={files.keys() - required}"
        )
    return files


def check_stackup(path: Path, expected_size: tuple[float, float]) -> dict:
    def within(value: object, expected: float, tolerance: float) -> bool:
        return (
            type(value) in (int, float)
            and math.isfinite(value)
            and abs(value - expected) <= tolerance
        )

    job = json.loads(path.read_text())
    specs = job.get("GeneralSpecs", {})
    if (
        specs.get("LayerNumber") != 4
        or specs.get("BoardThickness") != 1.6
        or specs.get("Finish") != "ENIG"
    ):
        raise ValueError(f"Panel construction differs from the order notes: {specs}")
    size = specs.get("Size", {})
    if any(
        not within(size.get(axis), value, 0.2)
        for axis, value in zip(("X", "Y"), expected_size)
    ):
        raise ValueError(
            f"CAM/panel outline bounds disagree: {size}, expected={expected_size}"
        )
    copper = [
        layer.get("Thickness")
        for layer in job.get("MaterialStackup", [])
        if layer.get("Type") == "Copper"
    ]
    if len(copper) != 4 or any(
        not within(value, expected, 0.004)
        for value, expected in zip(copper, (0.07, 0.035, 0.035, 0.07))
    ):
        raise ValueError(f"Expected 2/1/1/2 oz panel copper; CAM stackup={copper}")
    return specs
