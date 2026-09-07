from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

from PIL import Image, PngImagePlugin

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build_pcb_release as release
import convert_pcb_render as webp
import validate_pcb_release as validate


class RenderTests(unittest.TestCase):
    def test_lossless_conversion_preserves_transparent_pixels_and_strips_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            image = Image.new("RGBA", (4, 3))
            image.putdata([(i * 20, 50, 100, i * 20) for i in range(12)])
            metadata = PngImagePlugin.PngInfo()
            metadata.add_text("private-note", "not for publication")
            image.save(root / "in.png", pnginfo=metadata)
            first = webp.convert(root / "in.png", root / "out.webp")
            second = webp.convert(root / "in.png", root / "again.webp")
            self.assertEqual(first, second)
            self.assertTrue(first["alpha"])
            with Image.open(root / "out.webp") as result:
                self.assertEqual(result.convert("RGBA").tobytes(), image.tobytes())
                self.assertNotIn("private-note", result.info)

    def test_bad_input_does_not_replace_existing_image(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "in.png").write_bytes(b"not an image")
            (root / "out.webp").write_bytes(b"existing")
            with self.assertRaises(OSError):
                webp.convert(root / "in.png", root / "out.webp")
            self.assertEqual((root / "out.webp").read_bytes(), b"existing")
            with self.assertRaises(ValueError):
                webp.convert(root / "in.png", root / "out.png")


class ValidationTests(unittest.TestCase):
    def test_stackup_must_match_the_order_notes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stackup.gbrjob"
            job = {
                "GeneralSpecs": {
                    "LayerNumber": 4,
                    "BoardThickness": 1.6,
                    "Finish": "ENIG",
                    "Size": {"X": 163.8, "Y": 237.35},
                },
                "MaterialStackup": [
                    {"Type": "Copper", "Thickness": value}
                    for value in (0.07, 0.035, 0.035, 0.07)
                ],
            }
            path.write_text(json.dumps(job))
            validate.check_stackup(path, (163.8, 237.35))
            for invalid in (0.035, None, "0.07", float("nan")):
                job["MaterialStackup"][0]["Thickness"] = invalid
                path.write_text(json.dumps(job))
                with self.assertRaises(ValueError):
                    validate.check_stackup(path, (163.8, 237.35))
            job["MaterialStackup"][0]["Thickness"] = 0.07
            job["GeneralSpecs"]["Size"]["X"] = float("nan")
            path.write_text(json.dumps(job))
            with self.assertRaises(ValueError):
                validate.check_stackup(path, (163.8, 237.35))

    def test_gate_rejects_downgraded_unconnected_and_erc_warnings(self):
        drc = {"violations": [], "unconnected_items": [], "schematic_parity": []}
        erc = {"sheets": []}
        self.assertEqual(validate.check_reports(drc, erc)["errors"], 0)
        with self.assertRaises(ValueError):
            validate.check_reports(
                {**drc, "unconnected_items": [{"severity": "warning"}]}, erc
            )
        with self.assertRaises(ValueError):
            validate.check_reports(
                drc, {"sheets": [{"violations": [{"severity": "warning"}]}]}
            )
        with self.assertRaises(ValueError):
            validate.check_reports({})
        self.assertEqual(
            validate.check_reports(
                {**drc, "violations": [{"severity": "error", "excluded": True}]}, erc
            )["errors"],
            0,
        )

    def test_assembly_duplicate_and_mismatch_detection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bom, cpl = root / "bom.csv", root / "cpl.csv"
            bom.write_text('Designator,Quantity,LCSC Part #\n"R1, R2",2,C123\n')
            header = "Designator,Mid X,Mid Y,Rotation,Layer\n"
            cpl.write_text(header + "R1,1,-2,90,top\nR2,2,-2,270,top\n")
            self.assertEqual(
                validate.check_assembly(bom, cpl, panel=True), {"R1", "R2"}
            )
            for rows in (
                "R1,1,-2,90,top\nR1,2,-2,270,top\n",
                "R1,nan,-2,90,top\nR2,2,-2,270,top\n",
                "R1,1,-2,90,bottom\nR2,2,-2,270,top\n",
            ):
                cpl.write_text(header + rows)
                with self.assertRaises(ValueError):
                    validate.check_assembly(bom, cpl, panel=True)

    def test_cam_rejects_images_and_archives_are_reproducible(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "unexpected.webp").write_bytes(b"image")
            with self.assertRaises(ValueError):
                validate.check_cam(root)
            a, b = root / "a.csv", root / "b.csv"
            a.write_text("a")
            b.write_text("b")
            validate.deterministic_zip(root / "one.zip", {"a.csv": a, "b.csv": b})
            validate.deterministic_zip(root / "two.zip", {"b.csv": b, "a.csv": a})
            self.assertEqual(
                (root / "one.zip").read_bytes(), (root / "two.zip").read_bytes()
            )
            with zipfile.ZipFile(root / "one.zip") as archive:
                self.assertEqual(archive.namelist(), ["a.csv", "b.csv"])


class PublicationTests(unittest.TestCase):
    def test_bundle_publication_rolls_back_and_preserves_content(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            a, b, content, source = (
                root / n for n in ("a.webp", "b.webp", "index.md", "new.webp")
            )
            a.write_bytes(b"old-a")
            b.write_bytes(b"old-b")
            content.write_text("my page")
            source.write_bytes(b"new")
            original = release.os.replace

            def fail_second(src, dest):
                if dest == b:
                    raise OSError("simulated disk failure")
                return original(src, dest)

            with patch.object(release.os, "replace", side_effect=fail_second):
                with self.assertRaises(OSError):
                    release.publish_files({a: source, b: source})
            self.assertEqual(a.read_bytes(), b"old-a")
            self.assertEqual(b.read_bytes(), b"old-b")
            self.assertEqual(content.read_text(), "my page")
            self.assertFalse(list(root.glob(".pcb-publish-*")))

    def test_directory_publication_rejects_unmanaged_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / "source", root / "target"
            source.mkdir()
            target.mkdir()
            (target / "user.txt").write_text("keep")
            with self.assertRaises(ValueError):
                release.publish_directory(source, target)
            self.assertEqual((target / "user.txt").read_text(), "keep")

    def test_site_failure_restores_previous_release_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / "source", root / "target"
            source.mkdir()
            target.mkdir()
            (source / "new").write_text("new release")
            (target / "manifest.json").write_text(
                json.dumps({"producer": "mrp-pcb-release"})
            )
            (target / "old").write_text("old release")

            def fail():
                raise OSError("site publication failed")

            with self.assertRaises(OSError):
                release.publish_directory(source, target, fail)
            self.assertEqual((target / "old").read_text(), "old release")
            self.assertFalse((target / "new").exists())

    def test_historical_sources_do_not_use_worktree_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            for key, value in (
                ("user.name", "Test"),
                ("user.email", "test@example.com"),
            ):
                subprocess.run(
                    ["git", "-C", str(root), "config", key, value], check=True
                )
            hardware = root / "hardware"
            hardware.mkdir()
            (hardware / "source.txt").write_text("committed")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(
                ["git", "-C", str(root), "commit", "-qm", "fixture"], check=True
            )
            identity = release.resolve_identity("HEAD", root)
            (hardware / "source.txt").write_text("dirty")
            release.materialize(identity["git_commit"], root / "export", root)
            self.assertEqual(
                (root / "export/hardware/source.txt").read_text(), "committed"
            )


if __name__ == "__main__":
    unittest.main()
