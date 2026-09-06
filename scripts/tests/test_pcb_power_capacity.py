from __future__ import annotations

import json
import math
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from shapely.geometry import box

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import pcb_power_capacity as power


class FormulaTests(unittest.TestCase):
    def test_matches_documented_backplane_single_layer_example(self) -> None:
        current = power.ipc2221_current_a(6.9, 70.0, 10.0, external=True)
        self.assertAlmostEqual(current, 16.04, places=2)

    def test_two_equal_external_layers_have_double_capacity(self) -> None:
        geometry = box(0, 0, 20, 6.9)
        layers = [
            power.LayerCopper(0, "F.Cu", 70.0, True, geometry, [geometry]),
            power.LayerCopper(1, "B.Cu", 70.0, True, geometry, [geometry]),
        ]
        cut = power.analyze_cut(
            kind="test",
            index=0,
            distance_mm=10,
            start_mm=(10, -1),
            end_mm=(10, 8),
            active_current_a=30,
            layers=layers,
            selected_geometry={layer.name: geometry for layer in layers},
            temperature_rises_c=[10.0],
        )
        self.assertAlmostEqual(cut.widths_mm["F.Cu"], 6.9)
        self.assertAlmostEqual(cut.shares["F.Cu"], 0.5)
        self.assertAlmostEqual(cut.natural_capacity_a[10.0], 32.07, places=2)
        self.assertAlmostEqual(cut.natural_capacity_a[10.0], cut.ideal_capacity_a[10.0])

    def test_internal_layer_can_limit_natural_current_sharing(self) -> None:
        geometry = box(0, 0, 20, 6.9)
        layers = [
            power.LayerCopper(0, "F.Cu", 70.0, True, geometry, [geometry]),
            power.LayerCopper(1, "In1.Cu", 35.0, False, geometry, [geometry]),
        ]
        cut = power.analyze_cut(
            kind="test",
            index=0,
            distance_mm=10,
            start_mm=(10, -1),
            end_mm=(10, 8),
            active_current_a=10,
            layers=layers,
            selected_geometry={layer.name: geometry for layer in layers},
            temperature_rises_c=[10.0],
        )
        self.assertLess(cut.natural_capacity_a[10.0], cut.ideal_capacity_a[10.0])
        self.assertTrue(math.isclose(sum(cut.shares.values()), 1.0))


class BoardIntegrationTests(unittest.TestCase):
    def test_reads_checked_in_stackups(self) -> None:
        backplane = power.stackup_copper_thicknesses_um(
            ROOT / "hardware/boards/backplane-prototype/backplane-prototype.kicad_pcb"
        )
        carrier = power.stackup_copper_thicknesses_um(
            ROOT / "hardware/boards/carrier/carrier.kicad_pcb"
        )
        self.assertEqual(
            backplane,
            {"F.Cu": 70.0, "In1.Cu": 35.0, "In2.Cu": 35.0, "B.Cu": 70.0},
        )
        self.assertEqual(
            carrier,
            {"F.Cu": 35.0, "In1.Cu": 17.5, "In2.Cu": 17.5, "B.Cu": 35.0},
        )

    def test_analyzes_named_backplane_scenario(self) -> None:
        scenarios = power.load_scenarios(power.DEFAULT_CONFIG, ROOT)
        report, context = power.analyze_scenario(scenarios["backplane-vcc"])
        self.assertEqual(report["inputs"]["total_current_a"], 33.0)
        self.assertEqual(len(report["inputs"]["sinks"]), 6)
        self.assertGreater(len(report["cuts"]), 100)
        self.assertEqual(report["via_screening"]["count"], 6)
        self.assertEqual(len(report["board_sha256"]), 64)
        for temperature in (10.0, 20.0):
            limiting = report["limiting_cuts"][str(temperature)]
            self.assertGreater(limiting["natural_capacity_a"][str(temperature)], 0)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            report_output = output / "backplane-vcc"
            power.write_report(report, context, report_output)
            json.loads((report_output / "report.json").read_text())
            svg_path = report_output / "report.svg"
            svg_tree = ET.parse(svg_path)
            svg_root = svg_tree.getroot()
            self.assertEqual(svg_root.attrib["width"], "1400")
            self.assertGreater(int(svg_root.attrib["height"]), 1000)
            self.assertEqual(svg_root.attrib["viewBox"].split()[:2], ["0", "0"])
            namespace = {"svg": "http://www.w3.org/2000/svg"}
            self.assertIsNotNone(
                svg_root.find("svg:g[@id='aggregate-overview']", namespace)
            )
            self.assertIsNotNone(svg_root.find("svg:g[@id='layer-panels']", namespace))
            self.assertIsNotNone(
                svg_root.find("svg:g[@id='capacity-profile']", namespace)
            )
            svg_text = svg_path.read_text(encoding="utf-8")
            self.assertNotIn("six lowest-margin", svg_text)
            self.assertIn('data-role="limiting-cut"', svg_text)
            self.assertTrue((report_output / "report.md").is_file())
            power.write_report_index([report], output)
            summary = json.loads((output / "summary.json").read_text())
            result = summary["reports"][0]
            self.assertFalse(
                result["temperature_rises"]["10.0"]["passes_requested_load"]
            )
            self.assertTrue((output / "index.md").is_file())

    def test_can_limit_analysis_to_selected_copper_layers(self) -> None:
        scenarios = power.load_scenarios(power.DEFAULT_CONFIG, ROOT)
        scenario = scenarios["carrier-vcc"]
        scenario.include_layers = ["F.Cu"]
        report, _context = power.analyze_scenario(scenario)
        self.assertEqual(report["inputs"]["selected_layers"], ["F.Cu"])
        self.assertEqual([layer["name"] for layer in report["layers"]], ["F.Cu"])


if __name__ == "__main__":
    unittest.main()
