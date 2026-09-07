#!/usr/bin/env python3
"""Build PCB previews or gated releases from an immutable revision using pinned tools."""

from __future__ import annotations

import argparse
import fcntl
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from datetime import datetime, timezone
from pathlib import Path

from validate_pcb_release import (
    check_cam,
    check_stackup,
    deterministic_zip,
    sha256,
    write_json,
)

ROOT = Path(__file__).resolve().parents[1]
IMAGE = "ghcr.io/inti-cmnb/kicad10_auto:1.9.0@sha256:493666a06d900ed3352c50b0f75a76ccdfe194999c097d455021cab9e3c723fa"
BOARDS = {
    "carrier": "hardware/boards/carrier/carrier.kicad_pcb",
    "backplane": "hardware/boards/backplane-prototype/backplane-prototype.kicad_pcb",
}
BUNDLES = {
    "carrier": Path("site/content/latest/hardware/modules/carrier"),
    "backplane": Path("site/content/latest/hardware/backplane"),
    "panel": Path("site/content/latest/hardware"),
}


def git(*args: str, root: Path = ROOT) -> str:
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def resolve_identity(ref: str, root: Path = ROOT) -> dict:
    commit = git(
        "rev-parse", "--verify", "--end-of-options", ref + "^{commit}", root=root
    )
    epoch = int(git("show", "-s", "--format=%ct", commit, root=root))
    date = datetime.fromtimestamp(epoch, timezone.utc)
    return {
        "git_commit": commit,
        "short_hash": commit[:12],
        "epoch": epoch,
        "build_date": date.strftime("%y.%m.%d"),
        "commit_time": date.isoformat(),
    }


def materialize(commit: str, destination: Path, root: Path = ROOT) -> dict:
    # The active boards also reference symbols in the older backpack project.
    # Archive the complete hardware dependency tree, omitting generated exports.
    paths = ["hardware"]
    archive = subprocess.check_output(["git", "archive", commit, *paths], cwd=root)
    destination.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
        members = []
        for member in stream.getmembers():
            if any(
                part in {"production", "backups"} for part in Path(member.name).parts
            ):
                continue
            if not (member.isfile() or member.isdir()):
                raise ValueError(f"Unsupported release source entry: {member.name}")
            members.append(member)
        stream.extractall(destination, members=members, filter="data")
    # Git archives omit submodule content. Materialize each pinned library from
    # its local object database, never from the submodule's working files.
    for line in git(
        "ls-tree", "-r", commit, "hardware/libraries", root=root
    ).splitlines():
        meta, path = line.split("\t", 1)
        mode, kind, revision = meta.split()
        if mode != "160000":
            continue
        data = subprocess.check_output(
            ["git", "-C", str(root / path), "archive", revision]
        )
        target = destination / path
        target.mkdir(parents=True, exist_ok=True)
        with tarfile.open(fileobj=io.BytesIO(data)) as stream:
            stream.extractall(target, filter="data")
    return {
        p.relative_to(destination).as_posix(): sha256(p)
        for p in sorted(destination.rglob("*"))
        if p.is_file()
    }


def worker(work: Path, phase: str, *args: str) -> None:
    log = (
        work
        / "logs"
        / ("-".join([phase, *(a for a in args if not a.startswith("--"))]) + ".log")
    )
    log.parent.mkdir(exist_ok=True)
    command = [
        "podman",
        "run",
        "--rm",
        "--userns=keep-id",
        "--network=none",
        "--env",
        "HOME=/tmp",
        "--env",
        "LC_ALL=C.UTF-8",
        "--env",
        "TZ=UTC",
        "--env",
        "INTERACTIVE_HTML_BOM_NO_DISPLAY=True",
        "--env",
        "PYTHONHASHSEED=0",
        "--volume",
        f"{work / 'tooling'}:/tooling:ro",
        "--volume",
        f"{work}:/work:rw",
        "--workdir",
        "/work",
        IMAGE,
        "python3",
        "/tooling/scripts/pcb_release_worker.py",
        "/work/request.json",
        phase,
        *args,
    ]
    print(f"{phase} {' '.join(args)} (log: {log})", flush=True)
    with log.open("w") as output:
        result = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        tail = "\n".join(log.read_text(errors="replace").splitlines()[-25:])
        raise RuntimeError(f"{phase} failed ({result.returncode}); {log}\n{tail}")


def convert(source: Path, destination: Path, tooling: Path = ROOT) -> dict:
    result = subprocess.run(
        [
            "uv",
            "run",
            "--script",
            str(tooling / "scripts/convert_pcb_render.py"),
            str(source),
            str(destination),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def publish_files(files: dict[Path, Path]) -> None:
    """Replace a known set of files, rolling back on failure; never replace a bundle directory."""
    prepared: dict[Path, Path] = {}
    previous: dict[Path, bytes | None] = {}
    completed: list[Path] = []
    try:
        for target, source in files.items():
            if target.is_symlink():
                raise ValueError(f"Refusing to replace symlink: {target}")
            target.parent.mkdir(parents=True, exist_ok=True)
            previous[target] = target.read_bytes() if target.exists() else None
            fd, name = tempfile.mkstemp(prefix=".pcb-publish-", dir=target.parent)
            os.close(fd)
            prepared[target] = Path(name)
            shutil.copyfile(source, name)
        for target, temporary in prepared.items():
            os.replace(temporary, target)
            completed.append(target)
    except BaseException:
        for target in reversed(completed):
            if previous[target] is None:
                target.unlink(missing_ok=True)
            else:
                target.write_bytes(previous[target])
        raise
    finally:
        for temporary in prepared.values():
            temporary.unlink(missing_ok=True)


def publish_directory(source: Path, target: Path, after_publish=None) -> None:
    """Only replace a directory owned by this release builder."""
    target = target.absolute()
    if target.is_symlink():
        raise ValueError(f"Refusing to publish over symlink: {target}")
    if target.exists():
        marker = target / "manifest.json"
        if (
            not marker.is_file()
            or json.loads(marker.read_text()).get("producer") != "mrp-pcb-release"
        ):
            raise ValueError(f"Refusing to replace an unmanaged directory: {target}")
    target.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".pcb-publish-", dir=target.parent) as name:
        staging, backup = Path(name) / "new", Path(name) / "previous"
        shutil.copytree(source, staging)
        if target.exists():
            os.replace(target, backup)
        try:
            os.replace(staging, target)
            if after_publish is not None:
                after_publish()
        except BaseException:
            if target.exists():
                os.replace(target, staging)
            if backup.exists():
                os.replace(backup, target)
            raise


def publish_bundles(render_dir: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="pcb-bundle-manifests-") as temporary:
        _publish_bundles(render_dir, Path(temporary))


def _publish_bundles(render_dir: Path, metadata_dir: Path) -> None:
    manifest = json.loads((render_dir / "renders.json").read_text())
    files = {}
    for board, bundle in BUNDLES.items():
        entries = {k: v for k, v in manifest["renders"].items() if v["board"] == board}
        if not entries:
            continue
        for entry in entries.values():
            source = render_dir / entry["file"]
            if sha256(source) != entry["sha256"]:
                raise ValueError(f"Render hash mismatch: {source}")
            files[ROOT / bundle / source.name] = source
        view = manifest.get("assembly_views", {}).get(board)
        if view:
            source = render_dir / view["file"]
            if sha256(source) != view["sha256"]:
                raise ValueError(f"iBOM hash mismatch: {source}")
            files[ROOT / "site/static/assembly" / source.name] = source
        local_manifest = metadata_dir / f"{board}-renders.json"
        write_json(
            local_manifest,
            {
                **manifest,
                "renders": entries,
                "assembly_views": {board: view} if view else {},
            },
        )
        files[ROOT / bundle / "renders.json"] = local_manifest
    publish_files(files)


def canonicalize(directory: Path, identity: dict) -> None:
    # Reuse the established timestamp policy for Gerber and drill files.
    from export_pcb_production import canonicalize_cam_timestamps

    canonicalize_cam_timestamps(directory, identity["commit_time"])


def package_vendor(work: Path, result: Path, vendor: str, identity: dict) -> None:
    source = work / "vendors" / vendor
    target = result / vendor
    upload = target / "upload"
    upload.mkdir(parents=True)
    canonicalize(source / "cam", identity)
    panel_info = json.loads((result / "panel-info.json").read_text())
    check_stackup(
        source / "cam/stackup.gbrjob", (panel_info["width_mm"], panel_info["height_mm"])
    )
    shutil.copyfile(source / "cam/stackup.gbrjob", target / "stackup.gbrjob")
    deterministic_zip(upload / "gerbers.zip", check_cam(source / "cam"))
    for name in ("BOM.csv", "positions.csv"):
        shutil.copyfile(source / name, upload / name)
    deterministic_zip(
        target / f"{vendor}-upload.zip", {p.name: p for p in upload.iterdir()}
    )
    shutil.copyfile(source / "artwork.json", target / "artwork.json")
    shutil.copyfile(source / "netlist.d356", target / "netlist.d356")
    shutil.copytree(source / "checks", target / "checks")
    for side in ("top", "bottom"):
        convert(
            source / "proofs" / f"{side}.png",
            target / "proofs" / f"{vendor}-{side}.webp",
            work / "tooling",
        )
    (target / "ORDER-NOTES.txt").write_text(
        "Customer panel: six carriers + one backplane, two designs. Do not re-panelize.\n"
        "Four layers, 1.6 mm FR-4, ENIG, 2 oz outer / 1 oz inner copper.\n"
        "Top-side assembly. BOM excludes DNP/user-installed components.\n"
        "BOM/CPL and CAM use the panel's shared absolute/auxiliary origin.\n"
        "CPL Y follows KiCad Cartesian convention; negative Y is intentional.\n"
        "Confirm construction and BOM sourcing with the vendor before ordering.\n",
        encoding="utf-8",
    )


def build(args: argparse.Namespace) -> Path:
    identity = resolve_identity(args.ref)
    work_parent = ROOT / "build/pcb-release-work"
    work_parent.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix=identity["short_hash"] + "-", dir=work_parent))
    print(f"Source revision: {identity['git_commit']}\nWorkspace: {work}", flush=True)
    tooling_paths = [
        *ROOT.glob("scripts/*pcb*.py"),
        ROOT / "scripts/panel_layout.py",
        ROOT / "scripts/build_carrier_backplane_panel.py",
        *ROOT.glob(".kibot/release/*.yaml"),
        ROOT / ".kibot/docs-ibom.kibot.yaml",
    ]
    tooling = {}
    for path in sorted(tooling_paths):
        relative = path.relative_to(ROOT)
        destination = work / "tooling" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        tooling[relative.as_posix()] = sha256(destination)
    sources = materialize(identity["git_commit"], work / "sources")
    request = {
        "work": "/work",
        "identity": identity,
        "boards": {},
        "artwork": {"jlcpcb": ["barcode"], "pcbway": ["job-number", "logo"]},
    }
    for key, pcb in BOARDS.items():
        project = json.loads(
            (work / "sources" / Path(pcb).with_suffix(".kicad_pro")).read_text()
        )
        variables = project.get("text_variables", {})
        variables.update(
            {"BUILD_DATE": identity["build_date"], "SHORT_HASH": identity["short_hash"]}
        )
        variables.update(dict(args.define))
        variables.update(dict(getattr(args, key + "_define")))
        request["boards"][key] = {"pcb": pcb, "variables": variables}
    write_json(work / "request.json", request)
    # Keep container startup offline: install/pull its immutable image explicitly.
    subprocess.run(["podman", "image", "exists", IMAGE], check=True)
    selected = list(BOARDS) if args.board == "all" else [args.board]
    for board in selected:
        worker(work, "prepare", "--board", board)
    if args.command in {"check", "release"}:
        failures = []
        for board in selected:
            try:
                worker(work, "check", "--board", board)
            except RuntimeError as error:
                failures.append(str(error))
        if failures:
            raise RuntimeError("\n".join(failures))
    if args.command == "check":
        return work
    if args.command == "release":
        for board in selected:
            worker(work, "assembly", "--board", board)
    result = work / "result"
    renders = result / "renders"
    entries, assembly_views = {}, {}
    for board in selected:
        for side in ("top", "bottom"):
            worker(work, "render", "--board", board, "--side", side)
            name = f"{board}.webp" if side == "top" else f"{board}-bottom.webp"
            info = convert(
                work / "raster" / f"{board}-{side}.png",
                renders / name,
                work / "tooling",
            )
            entries[f"{board}-{side}"] = {
                **info,
                "file": name,
                "board": board,
                "side": side,
                "source_board": BOARDS[board],
            }
        if args.ibom:
            worker(work, "ibom", "--board", board)
            candidates = list((work / "ibom" / board).glob("*.html"))
            if len(candidates) != 1 or "var pcbdata" not in candidates[0].read_text():
                raise ValueError(f"Missing or invalid iBOM for {board}")
            target = renders / f"{board}-ibom.html"
            shutil.copyfile(candidates[0], target)
            assembly_views[board] = {
                "file": target.name,
                "sha256": sha256(target),
                "site_url": f"/assembly/{target.name}",
            }
    if args.panel or args.command == "release":
        worker(work, "panel")
        if args.command == "release":
            worker(work, "check", "--board", "panel")
        worker(work, "render", "--board", "panel")
        info = convert(
            work / "raster/panel-top.png", renders / "panel.webp", work / "tooling"
        )
        entries["panel-top"] = {
            **info,
            "file": "panel.webp",
            "board": "panel",
            "side": "top",
        }
        shutil.copyfile(
            work / "sources/hardware/boards/panel/panel.json",
            result / "panel-info.json",
        )
    manifest = {
        "schema_version": 1,
        "producer": "mrp-pcb-release",
        "source": identity,
        "toolchain_image": IMAGE,
        "tooling_sha256": tooling,
        "status": "validated" if args.command == "release" else "preview-unchecked",
        "variables": {k: v["variables"] for k, v in request["boards"].items()},
        "source_files": sources,
        "renders": entries,
        "assembly_views": assembly_views,
    }
    write_json(
        renders / "renders.json",
        {k: v for k, v in manifest.items() if k != "source_files"},
    )
    if args.command == "release":
        for vendor in ("jlcpcb", "pcbway"):
            worker(work, "vendor", "--board", "panel", "--vendor", vendor)
            package_vendor(work, result, vendor, identity)
        panel = result / "common/panel.kicad_pcb"
        panel.parent.mkdir()
        shutil.copyfile(work / "sources/hardware/boards/panel/panel.kicad_pcb", panel)
        shutil.copytree(work / "checks", result / "checks")
    manifest["artifacts"] = {
        p.relative_to(result).as_posix(): sha256(p)
        for p in sorted(result.rglob("*"))
        if p.is_file()
    }
    write_json(result / "manifest.json", manifest)
    # Serializes publication across preview/release tasks in this worktree.
    with (work_parent / "publish.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        after_publish = (
            (lambda: publish_bundles(args.output / "renders"))
            if args.publish_site
            else None
        )
        publish_directory(result, args.output, after_publish)
    return args.output


def variable(value: str) -> tuple[str, str]:
    name, sep, replacement = value.partition("=")
    if not sep or not name.isidentifier() or not replacement or "\n" in replacement:
        raise argparse.ArgumentTypeError("Expected KEY=VALUE")
    return name, replacement


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "preview", "release"])
    parser.add_argument("--ref", default=os.environ.get("PANEL_GIT_HASH", "HEAD"))
    parser.add_argument("--board", choices=["all", *BOARDS], default="all")
    parser.add_argument(
        "--panel", action="store_true", help="Include the mixed panel in preview mode"
    )
    parser.add_argument(
        "--publish-site",
        action="store_true",
        help="Publish WebP files into Hugo bundles",
    )
    parser.add_argument(
        "--ibom",
        action="store_true",
        help="Include interactive assembly HTML (published under site/static/assembly)",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("-D", "--define", type=variable, action="append", default=[])
    parser.add_argument("--carrier-define", type=variable, action="append", default=[])
    parser.add_argument(
        "--backplane-define", type=variable, action="append", default=[]
    )
    args = parser.parse_args()
    if (args.panel or args.command == "release") and args.board != "all":
        parser.error("Panel/release builds require both boards")
    args.output = (args.output or ROOT / "build" / f"pcb-{args.command}").absolute()
    print(f"Completed: {build(args)}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
