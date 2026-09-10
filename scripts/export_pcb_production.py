#!/usr/bin/env python3
"""Generate and validate PCB manufacturing release artifacts."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class BoardConfig:
    key: str
    label: str
    directory: Path
    stem: str
    sheet_names: tuple[str, ...]
    archive_name: str
    expected_layer_count: int
    expected_thickness_mm: float
    expected_finish: str
    required_text_variables: frozenset[str]

    @property
    def board(self) -> Path:
        return self.directory / f"{self.stem}.kicad_pcb"

    @property
    def project(self) -> Path:
        return self.directory / f"{self.stem}.kicad_pro"

    @property
    def schematic(self) -> Path:
        return self.directory / f"{self.stem}.kicad_sch"

    @property
    def output(self) -> Path:
        return self.directory / "production"

    @property
    def design_inputs(self) -> tuple[Path, ...]:
        return (
            self.board,
            self.project,
            self.schematic,
            *(self.directory / name for name in self.sheet_names),
            self.directory / f"{self.stem}.kicad_dru",
            self.directory / "fp-lib-table",
            self.directory / "sym-lib-table",
        )


BOARD_CONFIGS = {
    "carrier": BoardConfig(
        key="carrier",
        label="Carrier",
        directory=ROOT / "hardware/boards/carrier",
        stem="carrier",
        sheet_names=("01_power_control.kicad_sch", "02_housekeeping.kicad_sch"),
        archive_name="Mini_Rack_Power_Carrier_A.zip",
        expected_layer_count=4,
        expected_thickness_mm=1.6,
        expected_finish="ENIG",
        required_text_variables=frozenset(
            {"PROJECT_FAMILY", "BOARD_VERSION", "BUILD_DATE", "SHORT_HASH"}
        ),
    ),
    "backplane": BoardConfig(
        key="backplane",
        label="Backplane",
        directory=ROOT / "hardware/boards/backplane",
        stem="backplane",
        sheet_names=(
            "01_power_input.kicad_sch",
            "02_control.kicad_sch",
            "03_can_slots.kicad_sch",
            "04_fan_status.kicad_sch",
            "05_buck_converters.kicad_sch",
        ),
        archive_name="Backplane_A.zip",
        expected_layer_count=4,
        expected_thickness_mm=1.6,
        expected_finish="ENIG",
        required_text_variables=frozenset(
            {"PROJECT_FAMILY", "BOARD_VERSION", "BUILD_DATE", "SHORT_HASH"}
        ),
    ),
}

GERBER_LAYERS = (
    "F.Cu",
    "In1.Cu",
    "In2.Cu",
    "B.Cu",
    "F.Paste",
    "B.Paste",
    "F.Silkscreen",
    "B.Silkscreen",
    "F.Mask",
    "B.Mask",
    "Edge.Cuts",
)

CAM_FILE_SUFFIXES = {
    "F_Cu.gtl",
    "In1_Cu.g1",
    "In2_Cu.g2",
    "B_Cu.gbl",
    "F_Paste.gtp",
    "B_Paste.gbp",
    "F_Silkscreen.gto",
    "B_Silkscreen.gbo",
    "F_Mask.gts",
    "B_Mask.gbs",
    "Edge_Cuts.gm1",
    "PTH.drl",
    "NPTH.drl",
    "PTH-drl_map.gbr",
    "NPTH-drl_map.gbr",
    "job.gbrjob",
}

FOOTPRINT_START_RE = re.compile(r"^\s*\(footprint(?:\s|$)", re.MULTILINE)
REFERENCE_PROPERTY_RE = re.compile(
    r'^\s*\(property\s+"Reference"\s+"([^"]+)"(?:\s|$)', re.MULTILINE
)


def required_cam_files(config: BoardConfig) -> set[str]:
    return {f"{config.stem}-{suffix}" for suffix in CAM_FILE_SUFFIXES}


def run(*args: str, cwd: Path = ROOT, log: Path | None = None) -> None:
    print("+", " ".join(args), flush=True)
    if log is None:
        subprocess.run(args, cwd=cwd, check=True)
    else:
        result = subprocess.run(
            args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        log.write_text(result.stdout, encoding="utf-8")
        print(result.stdout, end="", flush=True)
        result.check_returncode()


def git_output(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def release_identity() -> dict[str, str]:
    full_revision = git_output("rev-parse", "HEAD")
    short_revision = git_output("rev-parse", "--short", "HEAD")
    build_date = git_output(
        "show", "-s", "--format=%cd", "--date=format:%y.%m.%d", "HEAD"
    )
    commit_time = git_output("show", "-s", "--format=%cI", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40,64}", full_revision):
        raise RuntimeError(f"Unexpected Git revision: {full_revision!r}")
    if not re.fullmatch(r"[0-9a-f]{7,16}", short_revision):
        raise RuntimeError(f"Unexpected short Git revision: {short_revision!r}")
    if not re.fullmatch(r"\d{2}\.\d{2}\.\d{2}", build_date):
        raise RuntimeError(f"Unexpected Git commit date: {build_date!r}")
    try:
        datetime.fromisoformat(commit_time)
    except ValueError as error:
        raise RuntimeError(f"Unexpected Git commit time: {commit_time!r}") from error
    return {
        "git_commit": full_revision,
        "short_hash": short_revision,
        "build_date": build_date,
        "commit_time": commit_time,
    }


def ensure_design_inputs_are_committed(config: BoardConfig) -> None:
    relative_inputs = [str(path.relative_to(ROOT)) for path in config.design_inputs]
    result = subprocess.run(
        [
            "git",
            "status",
            "--short",
            "--untracked-files=all",
            "--",
            *relative_inputs,
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        raise RuntimeError(
            f"{config.label} design inputs must be committed before a production export:\n"
            + result.stdout.rstrip()
        )


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8-sig", newline="") as source:
        return list(csv.DictReader(source))


def natural_ref_key(reference: str) -> tuple[object, ...]:
    return tuple(
        int(piece) if piece.isdigit() else piece
        for piece in re.split(r"(\d+)", reference)
        if piece
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def board_references(path: Path) -> list[str]:
    """Read every footprint reference without loading noisy pcbnew bindings."""

    board_text = path.read_text(encoding="utf-8")
    footprint_count = len(FOOTPRINT_START_RE.findall(board_text))
    references = REFERENCE_PROPERTY_RE.findall(board_text)
    if len(references) != footprint_count:
        raise RuntimeError(
            f"Expected one Reference property for each footprint in {path}: "
            f"footprints={footprint_count}, references={len(references)}"
        )
    return references


def run_electrical_checks(config: BoardConfig, work: Path) -> dict[str, object]:
    erc_path = work / "erc.json"
    drc_path = work / "drc.json"
    run(
        "kicad-cli",
        "sch",
        "erc",
        "--severity-all",
        "--format",
        "json",
        "--output",
        str(erc_path),
        str(config.schematic),
    )
    run(
        "kicad-cli",
        "pcb",
        "drc",
        "--severity-all",
        "--schematic-parity",
        "--refill-zones",
        "--format",
        "json",
        "--output",
        str(drc_path),
        str(config.board),
    )

    erc = json.loads(erc_path.read_text(encoding="utf-8"))
    erc_violations = [
        violation
        for sheet in erc.get("sheets", [])
        for violation in sheet.get("violations", [])
    ]
    if erc_violations:
        counts = Counter(item.get("severity", "unknown") for item in erc_violations)
        raise RuntimeError(f"ERC has release findings: {dict(sorted(counts.items()))}")

    drc = json.loads(drc_path.read_text(encoding="utf-8"))
    drc_violations = drc.get("violations", [])
    drc_errors = [item for item in drc_violations if item.get("severity") == "error"]
    unconnected = drc.get("unconnected_items", [])
    parity = drc.get("schematic_parity", [])
    parity_errors = [item for item in parity if item.get("severity") != "warning"]
    parity_warnings = [item for item in parity if item.get("severity") == "warning"]
    if drc_errors or unconnected or parity_errors:
        raise RuntimeError(
            "DRC release gate failed: "
            f"errors={len(drc_errors)}, unconnected={len(unconnected)}, "
            f"schematic_parity_errors={len(parity_errors)}"
        )

    warning_counts = Counter(
        item.get("type", "unknown")
        for item in drc_violations
        if item.get("severity") == "warning"
    )
    warning_counts.update(
        f"schematic_parity:{item.get('type', 'unknown')}" for item in parity_warnings
    )
    return {
        "erc_violation_count": 0,
        "drc_error_count": 0,
        "drc_unconnected_count": 0,
        "drc_schematic_parity_error_count": 0,
        "drc_schematic_parity_warning_count": len(parity_warnings),
        "drc_warning_count": sum(warning_counts.values()),
        "drc_warning_counts": dict(sorted(warning_counts.items())),
    }


def export_bom(config: BoardConfig, work: Path, destination: Path) -> set[str]:
    raw_path = work / "bom.kicad.csv"
    run(
        "kicad-cli",
        "sch",
        "export",
        "bom",
        "--exclude-dnp",
        "--fields",
        "Reference,Value,Footprint,LCSC,Manufacturer,MPN,QUANTITY",
        "--ref-range-delimiter",
        "",
        "--output",
        str(raw_path),
        str(config.schematic),
    )

    grouped: dict[tuple[str, str, str], list[str]] = defaultdict(list)
    seen: set[str] = set()
    for row in read_csv(raw_path):
        reference = row["Reference"].strip()
        footprint = row["Footprint"].strip().split(":")[-1]
        value = row["Value"].strip()
        lcsc = row["LCSC"].strip()
        missing = [
            name
            for name, field in (
                ("Reference", reference),
                ("Footprint", footprint),
                ("Value", value),
                ("LCSC", lcsc),
            )
            if not field
        ]
        if missing:
            raise RuntimeError(f"BOM row {reference or row!r} is missing {missing}")
        if reference in seen:
            raise RuntimeError(f"Duplicate BOM reference: {reference}")
        seen.add(reference)
        grouped[(footprint, value, lcsc)].append(reference)

    with destination.open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(
            output,
            fieldnames=["Designator", "Footprint", "Quantity", "Value", "LCSC Part #"],
            lineterminator="\n",
        )
        writer.writeheader()
        groups = sorted(
            grouped.items(),
            key=lambda item: natural_ref_key(min(item[1], key=natural_ref_key)),
        )
        for (footprint, value, lcsc), references in groups:
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
    return seen


def export_positions(
    config: BoardConfig,
    work: Path,
    destination: Path,
    bom_references: set[str],
) -> None:
    raw_path = work / "positions.kicad.csv"
    run(
        "kicad-cli",
        "pcb",
        "export",
        "pos",
        "--side",
        "both",
        "--format",
        "csv",
        "--units",
        "mm",
        "--exclude-dnp",
        "--output",
        str(raw_path),
        str(config.board),
    )
    by_reference = {row["Ref"]: row for row in read_csv(raw_path)}
    position_references = set(by_reference)
    if position_references != bom_references:
        raise RuntimeError(
            "BOM/CPL reference mismatch: "
            f"missing_from_cpl={sorted(bom_references - position_references, key=natural_ref_key)}, "
            f"missing_from_bom={sorted(position_references - bom_references, key=natural_ref_key)}"
        )

    with destination.open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(
            output,
            fieldnames=["Designator", "Mid X", "Mid Y", "Rotation", "Layer"],
            lineterminator="\n",
        )
        writer.writeheader()
        for reference in sorted(bom_references, key=natural_ref_key):
            row = by_reference[reference]
            rotation = float(row["Rot"]) % 360
            writer.writerow(
                {
                    "Designator": reference,
                    "Mid X": row["PosX"],
                    "Mid Y": row["PosY"],
                    "Rotation": f"{rotation:.6f}",
                    "Layer": row["Side"].lower(),
                }
            )


def export_designators(config: BoardConfig, destination: Path) -> int:
    references = sorted(board_references(config.board), key=natural_ref_key)
    if len(references) != len(set(references)):
        raise RuntimeError("PCB contains duplicate footprint references")
    destination.write_text(
        "\ufeff" + "".join(f"{reference}:1\n" for reference in references),
        encoding="utf-8",
    )
    return len(references)


def assembly_counts(bom_rows: list[dict[str, str]], footprints: dict) -> dict:
    """Count the assembly BOM, not every physical object on the PCB."""
    references = [
        ref.strip() for row in bom_rows for ref in row["Designator"].split(",")
    ]
    if len(references) != len(set(references)):
        raise RuntimeError("Duplicate reference in assembly report BOM")
    parts = []
    for ref in sorted(references, key=natural_ref_key):
        if ref not in footprints:
            raise RuntimeError(f"Assembly report cannot find PCB footprint {ref}")
        part = footprints[ref]
        if part["mount"] not in {"smd", "through_hole"}:
            raise RuntimeError(f"Assembly report needs an SMD/THT attribute for {ref}")
        # PCBWay's quote tooltip includes >16-pin ICs and >10-pin irregular
        # SMD parts. U is the repository's IC reference convention; common IC
        # package names also cover symbols with another reference prefix.
        is_ic = bool(re.match(r"U\d+$", ref)) or bool(
            re.search(r"(?:^|[_:])(?:[A-Z]*QF[NP]|[A-Z]*BGA|[A-Z]*SOP|SOIC|LGA)(?:[-_]|$)", part["footprint"])
        )
        special = part["mount"] == "smd" and part["pin_count"] > (16 if is_ic else 10)
        parts.append({"reference": ref, **part, "pcbway_bga_qfp": special})
    return {
        "unique_parts": len({row["LCSC Part #"].strip() for row in bom_rows}),
        "smd_parts": sum(p["mount"] == "smd" for p in parts),
        "bga_qfp_parts": sum(p["pcbway_bga_qfp"] for p in parts),
        "through_hole_parts": sum(p["mount"] == "through_hole" for p in parts),
        "smt_contacts": sum(p["smt_contacts"] for p in parts),
        "parts": parts,
    }


def pcbnew_bindings():
    """Load KiCad's read-only board API without its enum startup assertions."""
    try:
        import wx
        wx.DisableAsserts()  # KiCad 10 enum registration assertions; see power tool.
        import pcbnew
    except ImportError as error:
        raise RuntimeError("Production export requires KiCad's Python bindings (pcbnew)") from error
    if not hasattr(pcbnew.SwigPyIterator, "next"):
        pcbnew.SwigPyIterator.next = pcbnew.SwigPyIterator.__next__
    return pcbnew


def export_assembly_report(config: BoardConfig, bom: Path, destination: Path) -> dict:
    pcbnew = pcbnew_bindings()
    board = pcbnew.LoadBoard(str(config.board))
    footprints = {}
    for fp in board.GetFootprints():
        attr = fp.GetAttributes()
        pads = [p for p in fp.Pads() if p.GetNumber() and p.IsOnCopperLayer()]
        mount = "smd" if attr & pcbnew.FP_SMD else (
            "through_hole" if attr & pcbnew.FP_THROUGH_HOLE else "unspecified"
        )
        classification = "footprint attribute"
        if mount == "unspecified":
            # Some custom inductors omit the footprint mount attribute.
            # Infer only unambiguous homogeneous electrical pads.
            pad_types = {p.GetAttribute() for p in pads}
            if pad_types == {pcbnew.PAD_ATTRIB_SMD}:
                mount = "smd"
            elif pad_types == {pcbnew.PAD_ATTRIB_PTH}:
                mount = "through_hole"
            classification = "pad types (unspecified footprint attribute)"
        footprints[fp.GetReference()] = {
            "footprint": str(fp.GetFPID().GetLibItemName()),
            "mount": mount,
            "mount_classification": classification,
            "pin_count": len({p.GetNumber() for p in pads}),
            "smt_contacts": len({p.GetNumber() for p in pads if p.GetAttribute() == pcbnew.PAD_ATTRIB_SMD}),
        }
    counts = assembly_counts(read_csv(bom), footprints)
    special_refs = ", ".join(p["reference"] for p in counts["parts"] if p["pcbway_bga_qfp"]) or "none"
    destination.write_text(
        f"# {config.label} — PCBWay assembly quote\n\n"
        "Counts per single board, for the exported BOM/CPL assembly scope.\n\n"
        "| PCBWay field | Enter |\n|---|---:|\n"
        f"| Number of Unique Parts | {counts['unique_parts']} |\n"
        f"| Number of SMD Parts | {counts['smd_parts']} |\n"
        f"| Number of BGA/QFP Parts | {counts['bga_qfp_parts']} |\n"
        f"| Number of Through-Hole Parts | {counts['through_hole_parts']} |\n\n"
        "These are component counts, not pad counts or batch totals. The form's "
        "'SMT Pads'/'Thru Holes' image labels are misleading; its tooltips specify parts. "
        "BGA/QFP is a subset of SMD: ICs with more than 16 pins (including SOP/QFN), "
        "plus other SMD parts with more than 10 pins. "
        f"Qualifying references: {special_refs}.\n\n"
        f"For reference only: **{counts['smt_contacts']} SMT contacts** "
        "(distinct numbered copper SMD pads per component; repeated exposed-pad "
        "segments count once; paste-only apertures do not count).\n\n"
        "Unique parts are distinct LCSC part numbers in bom.csv. Only BOM references "
        "are counted; DNP, hand-installed parts excluded from the BOM, test points, "
        "tooling and artwork are excluded. A zero THT count does not mean the PCB "
        "has no through-hole connectors. Counts must change if the assembler's scope changes.\n\n"
        "SMD/THT classification uses footprint attributes, or homogeneous electrical pad "
        "types when the attribute is unspecified. IC classification uses U references "
        "or common IC package names; review unusual packages before quoting. "
        "Per-reference classification is retained in validation.json.\n\n"
        "Source: [PCBWay assembly quote tooltips](https://www.pcbway.com/quotesmt.aspx) "
        "(definitions checked 2026-09-09).\n",
        encoding="utf-8",
    )
    return counts


def silkscreen_id(variables: dict[str, str]) -> str:
    """Use the silkscreen release identity with filename-friendly date separators."""
    names = ("PROJECT_FAMILY", "BOARD_NAME", "BOARD_VERSION", "BUILD_DATE", "SHORT_HASH")
    for name in names:
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", variables.get(name, "")):
            raise RuntimeError(f"Missing or unsafe release ID variable: {name}")
    date = variables["BUILD_DATE"].replace(".", "-")
    return (
        f"{variables['PROJECT_FAMILY']}.{variables['BOARD_NAME']}"
        f"-v{variables['BOARD_VERSION']}-{date}-{variables['SHORT_HASH']}"
    )


def step_filename(variables: dict[str, str]) -> str:
    return f"{silkscreen_id(variables)}.step"


def export_step(
    config: BoardConfig, destination: Path, variables: dict[str, str]
) -> dict[str, object]:
    pcbnew = pcbnew_bindings()
    board = pcbnew.LoadBoard(str(config.board))
    center = board.GetBoardEdgesBoundingBox().Centre()
    origin = [pcbnew.ToMM(center.x), pcbnew.ToMM(center.y)]
    definitions = [
        arg for name, value in sorted(variables.items())
        for arg in ("--define-var", f"{name}={value}")
    ]
    # Export the original path so KIPRJMOD-relative component models resolve.
    # Match CAM's baked text through CLI overrides. Omitting --no-dnp and
    # --no-unspecified deliberately includes all component categories.
    run(
        "kicad-cli", "pcb", "export", "step",
        "--output", str(destination),
        "--user-origin", f"{origin[0]:.6f}x{origin[1]:.6f}mm",
        "--cut-vias-in-body", "--include-silkscreen", "--include-soldermask",
        "--subst-models", *definitions, str(config.board),
        log=destination.with_suffix(".step.log"),
    )
    if not destination.is_file() or destination.stat().st_size == 0:
        raise RuntimeError(f"STEP export is missing or empty: {destination}")
    with destination.open(encoding="utf-8", errors="replace") as step:
        if step.readline().strip() != "ISO-10303-21;":
            raise RuntimeError(f"Invalid STEP file header: {destination}")
    without_models = sorted(
        (fp.GetReference() for fp in board.GetFootprints() if not list(fp.Models())),
        key=natural_ref_key,
    )
    return {
        "file": destination.name,
        "origin": "board_outline_bounding_box_center",
        "origin_mm": {"x": origin[0], "y": origin[1]},
        "cut_vias_in_body": True,
        "include_silkscreen": True,
        "include_soldermask": True,
        "include_dnp": True,
        "include_unspecified": True,
        "substitute_step_models": True,
        "footprints_without_3d_models": without_models,
        "export_log": destination.with_suffix(".step.log").name,
    }


def bake_release_board(
    config: BoardConfig, destination: Path, identity: dict[str, str]
) -> dict[str, str]:
    project = json.loads(config.project.read_text(encoding="utf-8"))
    variables = dict(project.get("text_variables", {}))
    variables["BUILD_DATE"] = identity["build_date"]
    variables["SHORT_HASH"] = identity["short_hash"]

    board_text = config.board.read_text(encoding="utf-8")
    used: dict[str, str] = {}
    for name, value in variables.items():
        token = f"${{{name}}}"
        if token in board_text:
            board_text = board_text.replace(token, value)
            used[name] = value

    if not config.required_text_variables.issubset(used):
        raise RuntimeError(
            f"{config.label} silkscreen did not consume text variables: "
            f"{sorted(config.required_text_variables - used.keys())}"
        )
    destination.write_text(board_text, encoding="utf-8")
    return dict(sorted(used.items()))


def export_cam(
    config: BoardConfig,
    work: Path,
    release_board: Path,
    destination: Path,
    commit_time: str,
) -> dict[str, object]:
    destination.mkdir()
    run(
        "kicad-cli",
        "pcb",
        "export",
        "gerbers",
        "--output",
        str(destination),
        "--layers",
        ",".join(GERBER_LAYERS),
        "--subtract-soldermask",
        "--check-zones",
        str(release_board),
    )

    drill_report = work / "drill-report.txt"
    run(
        "kicad-cli",
        "pcb",
        "export",
        "drill",
        "--output",
        str(destination),
        "--format",
        "excellon",
        "--drill-origin",
        "absolute",
        "--excellon-units",
        "mm",
        "--excellon-separate-th",
        "--generate-map",
        "--map-format",
        "gerberx2",
        "--generate-report",
        "--report-path",
        str(drill_report),
        str(release_board),
    )
    canonicalize_cam_timestamps(destination, commit_time)

    required_files = required_cam_files(config)
    generated = {path.name for path in destination.iterdir() if path.is_file()}
    missing = required_files - generated
    unexpected = generated - required_files
    if missing or unexpected:
        raise RuntimeError(
            f"CAM file-set mismatch: missing={sorted(missing)}, unexpected={sorted(unexpected)}"
        )
    for name in required_files:
        if destination.joinpath(name).stat().st_size == 0:
            raise RuntimeError(f"CAM output is empty: {name}")

    job = json.loads(destination.joinpath(f"{config.stem}-job.gbrjob").read_text())
    specs = job.get("GeneralSpecs", {})
    if specs.get("LayerNumber") != config.expected_layer_count:
        raise RuntimeError(
            f"Gerber job layer count does not match {config.label} configuration: "
            f"{specs}"
        )
    if specs.get("BoardThickness") != config.expected_thickness_mm:
        raise RuntimeError(
            f"Gerber job thickness does not match {config.label} configuration: {specs}"
        )
    if specs.get("Finish") != config.expected_finish:
        raise RuntimeError(
            f"Gerber job finish does not match {config.label} configuration: {specs}"
        )
    return {
        "file_count": len(generated),
        "files": sorted(generated),
        "board_size_mm": specs.get("Size"),
        "layer_count": specs["LayerNumber"],
        "board_thickness_mm": specs["BoardThickness"],
        "finish": specs["Finish"],
    }


def canonicalize_cam_timestamps(directory: Path, commit_time: str) -> None:
    """Replace volatile KiCad creation times with the source commit time."""

    parsed = datetime.fromisoformat(commit_time)
    timestamp_with_zone = parsed.isoformat(timespec="seconds")
    timestamp_without_zone = parsed.replace(tzinfo=None).isoformat(timespec="seconds")
    timestamp_with_space = parsed.replace(tzinfo=None).strftime("%Y-%m-%d %H:%M:%S")

    for path in directory.iterdir():
        if path.suffix == ".gbrjob":
            job = json.loads(path.read_text(encoding="utf-8"))
            job.setdefault("Header", {})["CreationDate"] = timestamp_with_zone
            path.write_text(json.dumps(job, indent=2) + "\n", encoding="utf-8")
            continue

        text = path.read_text(encoding="utf-8")
        text = re.sub(
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}",
            timestamp_with_zone,
            text,
        )
        text = re.sub(
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}",
            timestamp_without_zone,
            text,
        )
        text = re.sub(
            r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}",
            timestamp_with_space,
            text,
        )
        path.write_text(text, encoding="utf-8")


def write_cam_archive(config: BoardConfig, cam: Path, destination: Path) -> None:
    with zipfile.ZipFile(
        destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for source in sorted(cam.iterdir()):
            info = zipfile.ZipInfo(source.name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, source.read_bytes())

    with zipfile.ZipFile(destination) as archive:
        archived = {Path(name).name for name in archive.namelist()}
    required_files = required_cam_files(config)
    if archived != required_files:
        raise RuntimeError(
            f"Gerber archive file-set mismatch: {sorted(archived ^ required_files)}"
        )


def publish(staging: Path, output: Path) -> None:
    backup = output.parent / f".{output.name}.previous"
    if backup.exists():
        raise RuntimeError(f"Refusing to overwrite recovery directory: {backup}")
    moved_previous = False
    try:
        if output.exists():
            output.rename(backup)
            moved_previous = True
        staging.rename(output)
    except Exception:
        if moved_previous and not output.exists():
            backup.rename(output)
        raise
    if moved_previous:
        shutil.rmtree(backup)


def resolve_output(value: Path | None, config: BoardConfig) -> Path:
    requested = config.output if value is None else value
    output = requested if requested.is_absolute() else ROOT / requested
    output = output.resolve()
    if output == ROOT or output in ROOT.parents:
        raise RuntimeError(f"Unsafe production output directory: {output}")
    return output


def build(config: BoardConfig, output_root: Path) -> dict[str, object]:
    ensure_design_inputs_are_committed(config)
    identity = release_identity()
    output_root.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(
        prefix=f".{config.key}-production-", dir=output_root.parent
    ) as temporary:
        work = Path(temporary)
        staging = work / "production"
        staging.mkdir()

        release_board = work / f"{config.stem}.kicad_pcb"
        silkscreen_variables = bake_release_board(config, release_board, identity)
        release_id = silkscreen_id(silkscreen_variables)
        output = output_root / release_id

        checks = run_electrical_checks(config, work)
        bom_references = export_bom(config, work, staging / "bom.csv")
        export_positions(config, work, staging / "positions.csv", bom_references)
        footprint_count = export_designators(config, staging / "designators.csv")
        quote_counts = export_assembly_report(
            config, staging / "bom.csv", staging / "assembly-report.md"
        )
        run(
            "kicad-cli",
            "pcb",
            "export",
            "ipcd356",
            "--output",
            str(staging / "netlist.ipc"),
            str(config.board),
        )

        cam = work / "cam"
        cam_validation = export_cam(
            config, work, release_board, cam, identity["commit_time"]
        )
        write_cam_archive(config, cam, staging / config.archive_name)
        step_path = staging / step_filename(silkscreen_variables)
        step_validation = export_step(config, step_path, silkscreen_variables)

        artifacts = [
            staging / "bom.csv",
            staging / "positions.csv",
            staging / "designators.csv",
            staging / "netlist.ipc",
            staging / "assembly-report.md",
            staging / config.archive_name,
            step_path,
            step_path.with_suffix(".step.log"),
        ]
        validation: dict[str, object] = {
            "schema_version": 1,
            "board_key": config.key,
            "release_id": release_id,
            "status": ("pass_with_warnings" if checks["drc_warning_count"] else "pass"),
            "source": {
                **identity,
                "board": str(config.board.relative_to(ROOT)),
                "board_sha256": sha256(config.board),
                "schematic": str(config.schematic.relative_to(ROOT)),
                "schematic_sha256": sha256(config.schematic),
            },
            "tools": {
                "kicad_cli": subprocess.run(
                    ["kicad-cli", "--version"],
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip(),
            },
            "silkscreen_variables": silkscreen_variables,
            "electrical_checks": checks,
            "assembly": {
                "bom_reference_count": len(bom_references),
                "position_reference_count": len(bom_references),
                "bom_position_references_match": True,
                "pcb_footprint_count_including_dnp_and_board_only": footprint_count,
                "pcbway_quote": quote_counts,
            },
            "cam": cam_validation,
            "step": step_validation,
            "artifacts": {
                artifact.name: {
                    "bytes": artifact.stat().st_size,
                    "sha256": sha256(artifact),
                }
                for artifact in artifacts
            },
        }
        (staging / "validation.json").write_text(
            json.dumps(validation, indent=2) + "\n", encoding="utf-8"
        )
        output.parent.mkdir(parents=True, exist_ok=True)
        publish(staging, output)

    print(
        f"{config.label} production export: {validation['status']}\n"
        f"  revision: {identity['short_hash']} ({identity['build_date']})\n"
        f"  BOM/CPL references: {len(bom_references)}\n"
        f"  DRC warnings: {checks['drc_warning_count']}\n"
        f"  assembly counts: {output / 'assembly-report.md'}\n"
        f"  output: {output}"
    )
    return validation


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate and validate PCB Gerber, BOM, and CPL files"
    )
    parser.add_argument("board", choices=sorted(BOARD_CONFIGS))
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="output root; files go in <root>/<silkscreen-id>/ (default: the board's production directory)",
    )
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    config = BOARD_CONFIGS[arguments.board]
    build(config, resolve_output(arguments.output_dir, config))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
