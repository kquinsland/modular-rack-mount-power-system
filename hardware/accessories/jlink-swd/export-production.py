#!/usr/bin/python3
"""Export the standalone adapter with KiCad and the JLCPCB Fabrication Toolkit."""

import argparse
import csv
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from importlib import import_module
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

import wx

wx.DisableAsserts()
import pcbnew

BOARD = Path(__file__).with_name("jlink-swd.kicad_pcb")
LAYERS = "F.Cu,B.Cu,F.Mask,B.Mask,F.Silkscreen,B.Silkscreen,F.Paste,B.Paste,Edge.Cuts"
# Ordering choices for generic headers with no schematic LCSC field.
HEADER_PARTS = {
    "IDC-Header_2x03_P2.54mm_Vertical": ("BH254V-6P", "C492441"),
    "PinHeader_2x02_P2.54mm_Vertical": ("PZ2.54-2*2", "C5156675"),
    "PinHeader_1x02_P2.54mm_Vertical": ("2.54-1*2", "C5116481"),
    "PinHeader_1x03_P2.54mm_Vertical": ("2.54-1*3PPin", "C49257"),
}


def toolkit():
    version = ".".join(pcbnew.Version().split(".")[:2])
    data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    third_party = os.environ.get(
        f"KICAD{version.split('.')[0]}_3RD_PARTY", data / "kicad" / version / "3rdparty"
    )
    sys.path.insert(0, str(Path(third_party) / "plugins"))
    return import_module("com_github_bennymeg_JLC-Plugin-for-KiCad.process").ProcessManager


def export(args, source, output):
    subprocess.run(["kicad-cli", *args, "-o", output, source], check=True)


def build(output, date, revision):
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output) as directory:
        work = Path(directory)
        cam = work / "cam"
        cam.mkdir()
        export(["pcb", "export", "gerbers", "--layers", LAYERS, "--subtract-soldermask",
                "--check-zones", "-D", f"BUILD_DATE={date}", "-D", f"SHORT_HASH={revision}"],
               BOARD, cam)
        export(["pcb", "export", "drill", "--excellon-separate-th", "--generate-map",
                "--map-format", "gerberx2"], BOARD, cam)
        suffixes = ("F_Cu.gtl", "B_Cu.gbl", "F_Mask.gts", "B_Mask.gbs",
                    "F_Silkscreen.gto", "B_Silkscreen.gbo", "F_Paste.gtp", "B_Paste.gbp",
                    "Edge_Cuts.gm1", "PTH.drl", "NPTH.drl", "PTH-drl_map.gbr", "NPTH-drl_map.gbr")
        archive_name = BOARD.stem + ".zip"
        with ZipFile(work / archive_name, "w", ZIP_DEFLATED) as archive:
            for suffix in suffixes:
                path = cam / f"{BOARD.stem}-{suffix}"
                if not path.stat().st_size:
                    raise ValueError(f"Empty CAM file: {path.name}")
                archive.write(path, path.name)

        raw = work / "schematic.csv"
        export(["sch", "export", "bom", "--exclude-dnp", "--fields", "Reference,Value,Footprint,LCSC"],
               BOARD.with_suffix(".kicad_sch"), raw)
        with raw.open(encoding="utf-8-sig", newline="") as source:
            schematic = {row["Reference"]: row for row in csv.DictReader(source)}

        manager = toolkit()(pcbnew.LoadBoard(str(BOARD)))
        # The adapter's central header is marked "(DNP)" in its value.
        for footprint in manager.board.GetFootprints():
            if footprint.GetValue().endswith("(DNP)"):
                footprint.SetDNP(True)
        manager.generate_tables(directory, auto_translate=True, exclude_dnp=True)
        for part in manager.bom:
            refs = part["Designator"].split(", ")
            codes = set()
            for ref in refs:
                row = schematic[ref]
                footprint = row["Footprint"].split(":")[-1]
                if footprint != part["Footprint"] or row["Value"] != part["Value"]:
                    raise ValueError(f"Schematic and board disagree for {ref}")
                if row["LCSC"]:
                    codes.add((row["Value"], row["LCSC"]))
                else:
                    codes.add(HEADER_PARTS[footprint])
            if len(codes) != 1:
                raise ValueError(f"Conflicting ordering codes for {refs}")
            part["Value"], part["LCSC Part #"] = codes.pop()
        manager.generate_bom(directory)
        # All populated components are hand-soldered. Overwrite any older CPL
        # on re-export so a previous assembly run cannot leave live placements.
        with (work / "positions.csv").open("w", encoding="utf-8-sig", newline="") as table:
            csv.writer(table).writerow(["Designator", "Mid X", "Mid Y", "Rotation", "Layer"])
        manager.generate_netlist(directory)
        files = [work / name for name in
                 (archive_name, "bom.csv", "designators.csv", "netlist.ipc", "positions.csv")]
        for path in files:
            if not path.stat().st_size:
                raise ValueError(f"Empty production file: {path.name}")
        for path in files:
            path.replace(output / path.name)
    print(f"Production files: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output_directory", type=Path, nargs="?",
                        help="defaults to production/YY.MM.DD-<hash> beside the board")
    args = parser.parse_args()
    commit, timestamp = subprocess.check_output(
        ["git", "show", "-s", "--format=%H %ct", "HEAD"], cwd=BOARD.parent, text=True
    ).split()
    date = datetime.fromtimestamp(int(timestamp), timezone.utc).strftime("%y.%m.%d")
    revision = commit[:12]
    output = args.output_directory or BOARD.parent / "production" / f"{date}-{revision}"
    build(output, date, revision)
