from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/check-model-usability.py"
SPEC = importlib.util.spec_from_file_location("check_model_usability", MODULE_PATH)
assert SPEC and SPEC.loader
module = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = module
SPEC.loader.exec_module(module)


class IsolationTests(unittest.TestCase):
    def test_default_root_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "isolated"):
            module.validate_isolated_root(module.DEFAULT_DATA_ROOT)

    def test_relative_root_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "absolute"):
            module.validate_isolated_root("target/example")

    def test_home_directory_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "home"):
            module.validate_isolated_root(Path.home())


class SummaryTests(unittest.TestCase):
    def result(self, status: str, case_id: str = "C01") -> module.CaseResult:
        return module.build_case_result(case_id, status, "synthetic")

    def test_incomplete_when_required_case_is_not_run(self) -> None:
        summary = module.summarize([self.result("passed"), self.result("not_run", "C02")])
        self.assertEqual(summary["overall"], "incomplete")

    def test_failure_takes_precedence(self) -> None:
        summary = module.summarize(
            [self.result("failed"), self.result("blocked", "C02")]
        )
        self.assertEqual(summary["overall"], "failed")

    def test_all_selected_cases_passed(self) -> None:
        summary = module.summarize([self.result("passed"), self.result("passed", "C02")])
        self.assertEqual(summary["overall"], "passed")

    def test_generation_pass_requires_quality_review(self) -> None:
        summary = module.summarize([self.result("passed", "G01")])
        gated = module.apply_quality_gate(summary, ["G01"], None)
        self.assertEqual(gated["overall"], "incomplete")

    def test_passed_quality_review_preserves_generation_pass(self) -> None:
        summary = module.summarize([self.result("passed", "G01")])
        review = {
            "status": "passed",
            "meets_quality_bar": True,
        }
        self.assertEqual(module.apply_quality_gate(summary, ["G01"], review)["overall"], "passed")

    def test_log_text_normalizes_crlf_and_trailing_blanks(self) -> None:
        self.assertEqual(module.normalized_log_text(b"one\r\ntwo\r\n\r\n"), "one\ntwo\n")


class DailyEvidenceTests(unittest.TestCase):
    def test_missing_native_manifest_is_not_a_pass(self) -> None:
        result = module.daily_case("D01", None, None)
        self.assertEqual(result.status, "not_run")

    def test_passed_native_case_requires_existing_evidence(self) -> None:
        manifest = {
            "schema_version": 1,
            "cases": {
                "D01": {
                    "status": "passed",
                    "evidence_kind": "native_ui",
                    "observations": "Native app opened and showed the actual default data root.",
                    "evidence_paths": ["/definitely/missing/image-studio-evidence.png"],
                }
            },
        }
        result = module.daily_case("D01", manifest, None)
        self.assertEqual(result.status, "failed")
        self.assertIn("existing evidence", result.reason)

    def test_historical_reference_cannot_claim_native_ui_pass(self) -> None:
        manifest = {
            "schema_version": 1,
            "cases": {
                "D01": {
                    "status": "passed",
                    "evidence_kind": "historical_reference",
                    "observations": "A previous screenshot claimed the native flow worked.",
                    "evidence_paths": ["verification/model-catalog-1280-light.png"],
                }
            },
        }
        result = module.daily_case("D01", manifest, None)
        self.assertEqual(result.status, "failed")
        self.assertIn("evidence_kind", result.reason)

    def test_quality_review_requires_all_generation_cases(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "quality.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "status": "passed",
                        "meets_quality_bar": True,
                        "cases": {"G01": {"status": "passed"}},
                    }
                )
            )
            with self.assertRaisesRegex(ValueError, "G02"):
                module.read_quality_review(path)


class CatalogContractTests(unittest.TestCase):
    def test_current_catalog_scope_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result = module.run_contract_c04(Path(temporary))
        self.assertEqual(result.status, "passed")
        self.assertEqual(
            result.details["implemented"],
            ["flux2-klein-4b-distilled", "z-image-turbo-mflux-v1"],
        )
        self.assertEqual(len(result.details["planned"]), 2)


class CliTests(unittest.TestCase):
    def test_default_daily_root_cannot_be_used_for_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            command = [
                sys.executable,
                str(MODULE_PATH),
                "--profile",
                "contracts",
                "--data-root",
                str(module.DEFAULT_DATA_ROOT),
                "--report-dir",
                temporary,
            ]
            process = subprocess.run(command, capture_output=True, text=True, timeout=30)
        self.assertEqual(process.returncode, 3)
        self.assertIn("isolated", process.stderr)

    def test_c04_writes_schema_report(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            command = [
                sys.executable,
                str(MODULE_PATH),
                "--profile",
                "contracts",
                "--data-root",
                str(root / "isolated-data"),
                "--report-dir",
                str(root / "reports"),
                "--case-id",
                "C04",
            ]
            process = subprocess.run(command, capture_output=True, text=True, timeout=30)
            self.assertEqual(process.returncode, 0, process.stderr)
            reports = sorted((root / "reports").glob("contracts-model-usability-*.json"))
            self.assertEqual(len(reports), 1)
            report = json.loads(reports[0].read_text())
            self.assertEqual(report["schema_version"], 1)
            self.assertEqual(report["summary"]["overall"], "passed")
            self.assertEqual(report["cases"][0]["case_id"], "C04")
            self.assertEqual(report["cases"][0]["evidence_kind"], "contract_fixture")


if __name__ == "__main__":
    unittest.main()
