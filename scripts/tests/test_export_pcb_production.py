from __future__ import annotations

import csv
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import export_pcb_production as production


class ProductionExportUnitTests(unittest.TestCase):
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
