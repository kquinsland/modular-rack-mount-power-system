from __future__ import annotations

import csv
import json
import sys
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import export_pcb_production as production


class ProductionExportUnitTests(unittest.TestCase):
    def test_publish_replaces_only_selected_release(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            production_root = root / "production"
            production_root.mkdir()
            (production_root / "legacy.zip").write_text("legacy export")
            older = production_root / "mrp.carrier-v2.1-26-09-09-abcdef0"
            older.mkdir()
            (older / "bom.csv").write_text("previous revision")
            current = production_root / "mrp.carrier-v2.1-26-09-10-abcdef1"
            current.mkdir()
            (current / "obsolete.csv").write_text("superseded output")
            staging = root / "staging"
            staging.mkdir()
            (staging / "bom.csv").write_text("new export")
            production.publish(staging, current)
            self.assertEqual((current / "bom.csv").read_text(), "new export")
            self.assertFalse((current / "obsolete.csv").exists())
            self.assertEqual((older / "bom.csv").read_text(), "previous revision")
            self.assertEqual((production_root / "legacy.zip").read_text(), "legacy export")

    def test_step_filename_matches_silkscreen_identity(self) -> None:
        variables = {
            "PROJECT_FAMILY": "mrp", "BOARD_NAME": "carrier",
            "BOARD_VERSION": "2.1", "BUILD_DATE": "26.09.10", "SHORT_HASH": "abcdef1",
        }
        self.assertEqual(production.step_filename(variables), "mrp.carrier-v2.1-26-09-10-abcdef1.step")
        self.assertEqual(production.silkscreen_id(variables), "mrp.carrier-v2.1-26-09-10-abcdef1")
        self.assertEqual(
            production.step_filename({**variables, "BOARD_NAME": "backplane", "BUILD_DATE": "26-09-10"}),
            "mrp.backplane-v2.1-26-09-10-abcdef1.step",
        )
        with self.assertRaisesRegex(RuntimeError, "BOARD_NAME"):
            production.step_filename({**variables, "BOARD_NAME": "../carrier"})
        with self.assertRaisesRegex(RuntimeError, "PROJECT_FAMILY"):
            production.step_filename({})

    def test_step_uses_original_project_and_includes_all_component_categories(self) -> None:
        config = production.BOARD_CONFIGS["carrier"]
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "carrier.step"
            with patch.object(production, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "missing or empty"):
                    production.export_step(config, output, {})
                output.write_text("ISO-10303-21;\nEND-ISO-10303-21;\n")
                settings = production.export_step(config, output, {"SHORT_HASH": "abc1234"})
            args = run.call_args.args
            # Moving the input to a staging directory would break KIPRJMOD models.
            self.assertEqual(args[-1], str(config.board))
            self.assertIn("SHORT_HASH=abc1234", args)
            self.assertEqual(args[args.index("--user-origin") + 1], "150.775000x113.225000mm")
            for flag in ("--cut-vias-in-body", "--include-silkscreen",
                         "--include-soldermask", "--subst-models"):
                self.assertIn(flag, args)
            for flag in ("--no-dnp", "--no-unspecified", "--no-components", "--board-only"):
                self.assertNotIn(flag, args)
            self.assertTrue(settings["include_dnp"])
            self.assertIn("J2", settings["footprints_without_3d_models"])

    def test_quote_counts_follow_bom_scope_and_pcbway_thresholds(self) -> None:
        def part(mount="smd", pins=2, footprint="R_0603"):
            return {"mount": mount, "pin_count": pins, "footprint": footprint,
                    "smt_contacts": pins if mount == "smd" else 0}

        footprints = {
            "R1": part(), "R2": part(), "U1": part(pins=16),
            "U2": part(pins=17), "J1": part(pins=10),
            "J2": part(pins=11), "J3": part("through_hole", 20),
            "TP1": part(), "LOGO": part(), "U99": part(pins=100),
        }
        rows = [{"Designator": ref, "LCSC Part #": code} for ref, code in
                [("R1", "C1"), ("R2", "C1"), ("U1", "C2"), ("U2", "C3"),
                 ("J1", "C4"), ("J2", "C5"), ("J3", "C6")]]
        counts = production.assembly_counts(rows, footprints)
        self.assertEqual(counts["unique_parts"], 6)
        self.assertEqual(counts["smd_parts"], 6)
        self.assertEqual(counts["through_hole_parts"], 1)
        self.assertEqual(counts["bga_qfp_parts"], 2)
        self.assertEqual(counts["smt_contacts"], 58)
        self.assertEqual([p["reference"] for p in counts["parts"] if p["pcbway_bga_qfp"]], ["J2", "U2"])
        with self.assertRaisesRegex(RuntimeError, "cannot find"):
            production.assembly_counts(rows, {})
        with self.assertRaisesRegex(RuntimeError, "Duplicate"):
            production.assembly_counts(rows + rows[:1], footprints)
        with self.assertRaisesRegex(RuntimeError, "attribute"):
            production.assembly_counts(rows, {**footprints, "R1": part("unspecified")})

    def test_natural_reference_order(self) -> None:
        references = ["R11", "R2", "C1", "R1"]
        self.assertEqual(
            sorted(references, key=production.natural_ref_key),
            ["C1", "R1", "R2", "R11"],
        )

    def test_resolve_output_rejects_repository_root(self) -> None:
        with self.assertRaises(RuntimeError):
            production.resolve_output(
                production.ROOT, production.BOARD_CONFIGS["carrier"]
            )

    def test_board_configs_resolve_their_cam_file_sets(self) -> None:
        for key, config in production.BOARD_CONFIGS.items():
            with self.subTest(board=key):
                self.assertTrue(all(path.is_file() for path in config.design_inputs))
                references = production.board_references(config.board)
                self.assertGreater(len(references), 0)
                self.assertEqual(len(references), len(set(references)))
                self.assertIn(
                    f"{config.stem}-job.gbrjob",
                    production.required_cam_files(config),
                )
                self.assertTrue(
                    all(
                        name.startswith(f"{config.stem}-")
                        for name in production.required_cam_files(config)
                    )
                )

    def test_current_boms_and_positions_have_matching_references(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            for key, config in production.BOARD_CONFIGS.items():
                with self.subTest(board=key):
                    board_output = output / key
                    board_output.mkdir()
                    references = production.export_bom(
                        config, board_output, board_output / "bom.csv"
                    )
                    production.export_positions(
                        config,
                        board_output,
                        board_output / "positions.csv",
                        references,
                    )

                    with (board_output / "positions.csv").open(
                        encoding="utf-8-sig", newline=""
                    ) as source:
                        position_references = {
                            row["Designator"] for row in csv.DictReader(source)
                        }
                    self.assertEqual(position_references, references)
                    self.assertGreater(len(references), 0)
                    counts = production.export_assembly_report(
                        config, board_output / "bom.csv", board_output / "assembly-report.md"
                    )
                    self.assertEqual({p["reference"] for p in counts["parts"]}, references)
                    self.assertEqual(counts["smd_parts"] + counts["through_hole_parts"], len(references))
                    parts = {p["reference"]: p for p in counts["parts"]}
                    # TI's exposed pad has four copper segments with one number.
                    self.assertEqual(parts["U2"]["pin_count"], 9)
                    self.assertEqual(parts["U2"]["smt_contacts"], 9)
                    self.assertTrue(parts["U1"]["pcbway_bga_qfp"])
                    self.assertNotIn("MOD1", parts)
                    self.assertNotIn("LOGO_PCBWAY", parts)

    def test_cam_timestamps_are_canonicalized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            (output / "layer.gbr").write_text(
                "%TF.CreationDate,2020-01-02T03:04:05-07:00*%\n"
                "G04 Created date 2020-01-02 03:04:05*\n",
                encoding="utf-8",
            )
            (output / "board.gbrjob").write_text(
                json.dumps({"Header": {"CreationDate": "2020-01-02T03:04:05-07:00"}}),
                encoding="utf-8",
            )

            expected = "2026-09-06T17:26:22-07:00"
            production.canonicalize_cam_timestamps(output, expected)

            self.assertIn(expected, (output / "layer.gbr").read_text())
            self.assertIn("2026-09-06 17:26:22", (output / "layer.gbr").read_text())
            job = json.loads((output / "board.gbrjob").read_text())
            self.assertEqual(job["Header"]["CreationDate"], expected)


if __name__ == "__main__":
    unittest.main()
