"""KiCad/KiBot operations on disposable release copies, inside the pinned image."""

from __future__ import annotations

import argparse
import csv
import json
import shutil
import subprocess
from pathlib import Path

import build_carrier_backplane_panel as legacy
import pcbnew
from panel_layout import EXPECTED_MOUSE_BITES, build_panel
from validate_pcb_release import check_assembly, check_reports, read_csv, write_json

TOOLING = Path(__file__).resolve().parents[1]


def kibot(
    config: str,
    board: Path,
    output: Path,
    schematic: Path | None = None,
    targets: tuple[str, ...] = (),
) -> None:
    output.mkdir(parents=True, exist_ok=True)
    cmd = [
        "kibot",
        "-c",
        str(TOOLING / ".kibot/release" / (config + ".kibot.yaml")),
        "-b",
        str(board),
        "-d",
        str(output),
    ]
    if schematic is not None:
        cmd += ["-e", str(schematic)]
    cmd.extend(targets)
    subprocess.run(cmd, check=True)


def normalize_positions(
    source: Path, destination: Path, allowed: set[str] | None = None
) -> None:
    rows = read_csv(source)
    if len(rows) != len({row["Ref"] for row in rows}):
        raise ValueError("Duplicate references in KiBot placement output")
    if allowed is not None:
        # Panel tooling and fiducials are intentionally absent from the assembly BOM.
        extras = {r["Ref"] for r in rows} - allowed
        if any(not ref.startswith(("KiKit_", "PANEL-")) for ref in extras):
            raise ValueError(f"Unexpected panel placement references: {sorted(extras)}")
        rows = [r for r in rows if r["Ref"] in allowed]
    with destination.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(
            stream,
            fieldnames=["Designator", "Mid X", "Mid Y", "Rotation", "Layer"],
            lineterminator="\n",
        )
        writer.writeheader()
        for row in sorted(rows, key=lambda r: r["Ref"]):
            writer.writerow(
                {
                    "Designator": row["Ref"],
                    "Mid X": row["PosX"],
                    "Mid Y": row["PosY"],
                    "Rotation": f"{float(row['Rot']) % 360:.6f}",
                    "Layer": row["Side"].lower(),
                }
            )


def prepare(board: Path, variables: dict) -> None:
    legacy.bake_board_text_variables(board, variables)


def verify(board: Path, output: Path, schematic: Path | None) -> None:
    # KiBot performs checks/refills; the project validator separately rejects
    # disconnected items even if someone downgraded their project severity.
    kibot("checks" if schematic else "panel-checks", board, output, schematic)
    drc = json.loads((output / "drc.json").read_text())
    erc = json.loads((output / "erc.json").read_text()) if schematic else None
    write_json(output / "validation.json", check_reports(drc, erc))


def assembly(board: Path, schematic: Path, output: Path) -> None:
    kibot("board", board, output, schematic)
    normalize_positions(output / "raw-positions.csv", output / "positions.csv")
    refs = check_assembly(output / "bom.csv", output / "positions.csv")
    # Independent KiCad BOM/CPL export catches disagreements in KiBot filters.
    native = output / "native-bom.csv"
    subprocess.run(
        [
            "kicad-cli",
            "sch",
            "export",
            "bom",
            "--exclude-dnp",
            "--fields",
            "Reference,LCSC",
            "--output",
            str(native),
            str(schematic),
        ],
        check=True,
    )
    native_parts = {row["Reference"]: row["LCSC"] for row in read_csv(native)}
    native_refs = set(native_parts)
    if refs != native_refs:
        raise ValueError(
            f"KiBot/native KiCad BOM mismatch: {sorted(refs ^ native_refs)}"
        )
    kibot_parts = {
        ref.strip(): row["LCSC Part #"]
        for row in read_csv(output / "bom.csv")
        for ref in row["Designator"].split(",")
    }
    if native_parts != kibot_parts:
        raise ValueError("KiBot/native KiCad LCSC sourcing mismatch")


def panel(request: dict, root: Path, output: Path) -> None:
    carrier = root / request["boards"]["carrier"]["pcb"]
    backplane = root / request["boards"]["backplane"]["pcb"]
    entries = []
    for key, prefixes in (
        ("carrier", [f"CARRIER{i}" for i in range(1, 7)]),
        ("backplane", ["BACKPLANE1"]),
    ):
        bom = Path(request["work"]) / "assembly" / key / "bom.csv"
        if bom.is_file():
            entries.extend(legacy.expanded_bom_entries(read_csv(bom), prefixes))
    output.parent.mkdir(parents=True, exist_ok=True)
    info = build_panel(
        carrier, backplane, output, request["identity"]["short_hash"], len(entries)
    )
    pcb = pcbnew.LoadBoard(str(output))
    references = [fp.GetReference() for fp in pcb.GetFootprints()]
    if len(references) != len(set(references)):
        raise ValueError("Duplicate panel references")
    counts = tuple(
        sum(ref.startswith(prefix) for ref in references)
        for prefix in ("KiKit_MB_", "PANEL-TOOL", "PANEL-FID")
    )
    if counts != (EXPECTED_MOUSE_BITES, 4, 3):
        raise ValueError(f"Panel feature count changed: {counts}")
    if (
        pcb.GetCopperLayerCount() != 4
        or pcb.GetDesignSettings().GetBoardThickness() != pcbnew.FromMM(1.6)
    ):
        raise ValueError("Panel must be four layers and 1.6 mm thick")
    write_json(output.with_suffix(".json"), info)
    write_json(output.parent / "assembly.json", entries)
    write_json(
        output.parent / "metadata.json",
        {
            "carrier": legacy.footprint_metadata(carrier),
            "backplane": legacy.footprint_metadata(backplane),
        },
    )


def artwork_variant(
    board: Path, destination: Path, vendor: str, required: list[str]
) -> dict:
    pcb = pcbnew.LoadBoard(str(board))
    retained, removed = [], []
    for footprint in list(pcb.GetFootprints()):
        fields = {f.GetName(): f.GetText() for f in footprint.GetFields()}
        owner = fields.get("ReleaseVendor", "").strip().lower()
        if not owner:
            continue
        kind = fields.get("ReleaseArtwork", "").strip().lower()
        if owner not in {"jlcpcb", "pcbway"} or kind not in {
            "barcode",
            "job-number",
            "logo",
        }:
            raise ValueError(
                f"Invalid release artwork fields: {footprint.GetReference()}"
            )
        if list(footprint.Pads()) or list(footprint.Models()):
            raise ValueError("Vendor artwork must be padless and have no 3D models")
        excluded = pcbnew.FP_EXCLUDE_FROM_BOM | pcbnew.FP_EXCLUDE_FROM_POS_FILES
        if footprint.GetAttributes() & excluded != excluded:
            raise ValueError("Vendor artwork must be excluded from BOM and positions")
        if any(
            g.GetLayer() not in {pcbnew.F_SilkS, pcbnew.B_SilkS}
            for g in footprint.GraphicalItems()
        ):
            raise ValueError("Vendor artwork graphics must be entirely on silkscreen")
        if owner == vendor:
            retained.append({"reference": footprint.GetReference(), "kind": kind})
        else:
            removed.append(footprint.GetReference())
            pcb.Remove(footprint)
    missing = set(required) - {item["kind"] for item in retained}
    if missing:
        raise ValueError(
            f"{vendor}: missing registered artwork {sorted(missing)}; see .kibot/release/README.md"
        )
    destination.parent.mkdir(parents=True, exist_ok=True)
    pcbnew.SaveBoard(str(destination), pcb)
    # A PCB without its sibling project/rules silently uses different DRC
    # defaults. Artwork variants must keep the checked common panel's policy.
    for suffix in (".kicad_pro", ".kicad_dru"):
        sibling = board.with_suffix(suffix)
        if not sibling.is_file():
            raise ValueError(f"Missing common panel settings: {sibling}")
        shutil.copyfile(sibling, destination.with_suffix(suffix))
    return {"retained": retained, "removed": removed}


def vendor_export(request: dict, board: Path, output: Path, vendor: str) -> None:
    variant = board.parent / vendor / board.name
    artwork = artwork_variant(board, variant, vendor, request["artwork"][vendor])
    verify(variant, output / "checks", None)
    kibot(vendor, variant, output, targets=("gerbers", "drills", "electrical_netlist"))
    # Proofs use the exact filtered board used for these CAM outputs.
    kibot("proofs", variant, output / "proofs")
    kibot("board", variant, output, targets=("raw_positions",))
    entries = json.loads((board.parent / "assembly.json").read_text())
    metadata = json.loads((board.parent / "metadata.json").read_text())
    if vendor == "jlcpcb":
        legacy.write_jlc_bom(entries, output / "BOM.csv")
    else:
        legacy.write_pcbway_bom(
            entries, metadata["carrier"], metadata["backplane"], output / "BOM.csv"
        )
    normalize_positions(
        output / "raw-positions.csv",
        output / "positions.csv",
        {e["Designator"] for e in entries},
    )
    check_assembly(output / "BOM.csv", output / "positions.csv", panel=True)
    write_json(output / "artwork.json", artwork)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request", type=Path)
    parser.add_argument(
        "phase",
        choices=["prepare", "check", "assembly", "panel", "render", "vendor", "ibom"],
    )
    parser.add_argument("--board", default="carrier")
    parser.add_argument("--side", choices=["top", "bottom"], default="top")
    parser.add_argument("--vendor", choices=["jlcpcb", "pcbway"])
    args = parser.parse_args()
    request = json.loads(args.request.read_text())
    work = Path(request["work"])
    root = work / "sources"
    if args.board == "panel":
        board = root / "hardware/boards/panel/panel.kicad_pcb"
        schematic = None
    else:
        board = root / request["boards"][args.board]["pcb"]
        schematic = board.with_suffix(".kicad_sch")
    if args.phase == "prepare":
        prepare(board, request["boards"][args.board]["variables"])
    elif args.phase == "check":
        verify(board, work / "checks" / args.board, schematic)
    elif args.phase == "assembly":
        assembly(board, schematic, work / "assembly" / args.board)
    elif args.phase == "panel":
        panel(request, root, root / "hardware/boards/panel/panel.kicad_pcb")
    elif args.phase == "vendor":
        vendor_export(request, board, work / "vendors" / args.vendor, args.vendor)
    elif args.phase == "ibom":
        output = work / "ibom" / args.board
        output.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                "kibot",
                "-c",
                str(TOOLING / ".kibot/docs-ibom.kibot.yaml"),
                "-e",
                str(schematic),
                "-b",
                str(board),
                "-d",
                str(output),
                "docs_ibom",
            ],
            check=True,
        )
    elif args.phase == "render":
        dimensions = (1400, 2000) if args.board == "panel" else (1600, 900)
        legacy.render_board(
            board,
            work / "raster" / f"{args.board}-{args.side}.png",
            *dimensions,
            args.side,
        )


if __name__ == "__main__":
    main()
