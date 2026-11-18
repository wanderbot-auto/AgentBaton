"""Exercise the coverage report through its CLI with local inputs."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/report-requirements-coverage.py"
SPEC = Path("specs/requirements.json")
LEDGER = Path("docs/execution/requirements-progress.json")


def run_report(script, cwd, *args, format="json"):
    return subprocess.run(
        [sys.executable, str(script), "--format", format, *map(str, args)],
        cwd=cwd, capture_output=True, encoding="utf-8", timeout=15,
    )


def write_document(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, ensure_ascii=False), encoding="utf-8")


class DefaultCoverageReportTests(unittest.TestCase):
    def test_default_reads_design_and_ledger_from_script_checkout(self):
        result = run_report(SCRIPT, ROOT / "tests")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        spec = json.loads((ROOT / SPEC).read_bytes())
        ledger = json.loads((ROOT / LEDGER).read_bytes())
        self.assertEqual([row["id"] for row in report["requirements"]],
                         sorted(row["id"] for row in spec["requirements"]))
        self.assertEqual(report["summary"]["requirements"], len(spec["requirements"]))
        self.assertEqual(report["summary"]["partial_implementation"],
                         sum(row.get("coverage") == "partial" for row in ledger["requirements"]))
        self.assertEqual(report["summary"]["whole_requirement_accepted"],
                         sum(row["status"] == "accepted" for row in ledger["requirements"]))
        for label, path in (("spec", SPEC), ("ledger", LEDGER)):
            self.assertEqual(report["inputs"][label]["path"], str(ROOT / path))
            self.assertEqual(report["inputs"][label]["reference_root"], str(ROOT))
            self.assertEqual(report["inputs"][label]["sha256"],
                             hashlib.sha256((ROOT / path).read_bytes()).hexdigest())


class CoverageReportTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.container = Path(directory.name).resolve()
        self.checkout = self.container / "checkout"
        self.ledger_checkout = self.container / "ledger checkout"
        self.script = self.checkout / "scripts" / SCRIPT.name
        self.script.parent.mkdir(parents=True)
        shutil.copyfile(SCRIPT, self.script)

        self.spec = {
            "design_version": "0.2",
            "requirements": [
                {
                    "id": identifier, "work_package": package, "status": "specified",
                    "title": title, "document": "docs/design.md",
                    "acceptance": {"given": "输入", "when": "检查", "then": "保留结果"},
                }
                for identifier, package, title in (
                    ("AB-TST-002", "WP01", "显式验收"),
                    ("AB-TST-003", "WP02", "无记录"),
                    ("AB-TST-001", "WP01", "部分实现"),
                )
            ],
        }
        self.ledger = {
            "design_version": "0.2",
            "approval_id": "AB-TEST",
            "requirements": [
                {"id": "AB-TST-001", "work_package": "WP01", "status": "implementing",
                 "coverage": "partial", "verification": {
                     "unit": "passed",
                     "integration": "passed with a generated fixture only",
                     "provider": "passed only against a mock",
                     "native_arm64": "not_run",
                     "native_x86_64": "Intel historical note · passé",
                     "ci": "passed at a different source commit",
                     "older_ci_slice": "PASS in one job and FAIL in another",
                 }, "evidence": ["docs/execution/report.json"],
                 "verification_applicability": {"unit": "applicable"},
                 "limitations": ["历史说明"],
                 "verified_slices": [{"slice": "mock", "status": "passed", "ci": "old run",
                                      "evidence": "docs/execution/missing.json"}]},
                {"id": "AB-TST-002", "work_package": "WP01", "status": "accepted",
                 "verification": {"integration": "passed", "provider": "not_applicable", "ci": "passed",
                                  "native_arm64": "passed"},
                 "verification_applicability": {"provider": "not_applicable"}},
                {"id": "AB-TST-003", "work_package": "WP02", "status": "not_started"},
            ],
        }
        write_document(self.checkout / SPEC, self.spec)
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        (self.checkout / "docs").mkdir()
        (self.checkout / "docs/design.md").touch()
        # Deliberately uninterpretable evidence bytes. Only path existence is checked.
        (self.ledger_checkout / "docs/execution/report.json").write_bytes(b"\xff\x00")

    def run_fixture(self, *args, format="json"):
        return run_report(self.script, self.container, "--ledger", self.ledger_checkout / LEDGER,
                          *args, format=format)

    def test_join_and_count_explicit_coverage_and_acceptance(self):
        result = self.run_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        spec = report["inputs"]["spec"]
        ledger = report["inputs"]["ledger"]
        self.assertEqual(spec["reference_root"], str(self.checkout))
        self.assertEqual(ledger["reference_root"], str(self.ledger_checkout))
        self.assertEqual(ledger["approval_id"], "AB-TEST")
        rows = report["requirements"]
        self.assertEqual([row["id"] for row in rows], ["AB-TST-001", "AB-TST-002", "AB-TST-003"])
        self.assertEqual(rows[0]["acceptance_criteria"], self.spec["requirements"][2]["acceptance"])
        totals = report["summary"]
        self.assertEqual(totals["by_status"], {"implementing": 1, "accepted": 1, "not_started": 1})
        self.assertEqual(totals["partial_implementation"], 1)
        self.assertEqual(totals["whole_requirement_accepted"], 1)
        self.assertEqual(totals["by_work_package"]["WP01"], {
            "requirements": 2, "by_status": {"implementing": 1, "accepted": 1},
            "partial_implementation": 1, "whole_requirement_accepted": 1,
        })
        self.assertEqual(totals["by_work_package"]["WP02"]["requirements"], 1)
        self.assertTrue(rows[0]["partial_implementation"])
        self.assertFalse(rows[0]["whole_requirement_accepted"])
        self.assertFalse(rows[1]["partial_implementation"])
        self.assertTrue(rows[1]["whole_requirement_accepted"])

    def test_free_text_slices_and_paths_do_not_become_dimension_results(self):
        result = self.run_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        row, accepted, absent = json.loads(result.stdout)["requirements"]
        dimensions = row["verification"]
        recorded = self.ledger["requirements"][0]["verification"]
        for key in ("integration", "provider", "native_x86_64", "ci"):
            self.assertEqual(dimensions[key], {
                "applicability": None, "status": None, "recorded_note": recorded[key],
            })
        self.assertEqual(dimensions["unit"],
                         {"applicability": "applicable", "status": "passed", "recorded_note": None})
        self.assertEqual(dimensions["native_arm64"]["status"], "not_run")
        self.assertIsNone(dimensions["native_arm64"]["applicability"])
        self.assertEqual(row["recorded_verification_notes"],
                         {"older_ci_slice": recorded["older_ci_slice"]})
        self.assertEqual(row["recorded_ledger_notes"]["verified_slices"],
                         self.ledger["requirements"][0]["verified_slices"])
        self.assertEqual(accepted["verification"]["provider"]["status"], "not_applicable")
        self.assertEqual(accepted["verification"]["provider"]["applicability"], "not_applicable")
        self.assertEqual(accepted["verification"]["native_arm64"]["status"], "passed")
        self.assertEqual(accepted["verification"]["ci"]["status"], "passed")
        self.assertTrue(all(value == {"applicability": None, "status": None, "recorded_note": None}
                            for value in absent["verification"].values()))

    def test_reference_checks_use_own_root_and_stay_inside_it(self):
        evidence_dir = self.ledger_checkout / "docs/execution"
        outside = self.container / "outside"
        outside.mkdir()
        (outside / "exists.txt").touch()
        (outside / "bounce").symlink_to(evidence_dir, target_is_directory=True)
        (evidence_dir / "bad-dir").symlink_to(outside, target_is_directory=True)
        (evidence_dir / "bad-relative").symlink_to("../../../outside/exists.txt")
        (evidence_dir / "bad-file").symlink_to(outside / "exists.txt")
        (evidence_dir / "good").symlink_to("./report.json")
        (evidence_dir / "good-absolute").symlink_to(evidence_dir / "report.json")
        (evidence_dir / "dangling").symlink_to("absent")
        (evidence_dir / "loop").symlink_to("loop")
        os.mkfifo(evidence_dir / "fifo")
        (evidence_dir / "test.rs").touch()

        expected = {
            "docs/execution/report.json": "exists",
            "docs/execution/report.json#key": "exists",
            "docs/execution/test.rs::engine::test": "exists",
            "docs/execution/good": "exists",
            "docs/execution/good-absolute": "exists",
            "docs/execution/dangling": "missing",
            "docs/execution/missing.json": "missing",
            "docs/execution/report.json/child": "missing",
            "docs/execution/bad-dir/exists.txt": "unsafe",
            "docs/execution/bad-dir/bounce/report.json": "unsafe",
            "docs/execution/bad-relative": "unsafe",
            "docs/execution/bad-file": "unsafe",
            "docs/execution/loop": "unsafe",
            "docs/execution/fifo": "unsafe",
            "../outside/exists.txt": "unsafe",
            "docs/execution/../../docs/execution/report.json": "unsafe",
            str(outside / "exists.txt"): "unsafe",
            r"C:\outside\exists.txt": "unsafe",
            "https://example.invalid/evidence.json": "unsafe",
            "docs/execution/bad\x00name": "unsafe",
        }
        row = self.ledger["requirements"][0]
        row["implementation_refs"] = list(expected)
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        result = self.run_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        refs = {ref["reference"]: ref for ref in report["requirements"][0]["references"]}
        self.assertEqual({path: refs[path]["status"] for path in expected}, expected)
        self.assertEqual(refs["docs/design.md"]["status"], "exists")
        self.assertFalse(refs["docs/execution/report.json#key"]["selector_checked"])
        self.assertFalse(refs["docs/execution/test.rs::engine::test"]["selector_checked"])
        self.assertEqual(report["summary"]["references"], {
            "exists": 9, "missing": 4, "unsafe": 12, "unavailable": 0,
        })

    def test_explicit_reference_root_for_copied_ledger(self):
        copied = self.container / "snapshot.json"
        write_document(copied, self.ledger)
        result = run_report(self.script, self.container, "--ledger", copied,
                            "--evidence-root", self.ledger_checkout)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["inputs"]["ledger"]["reference_root"], str(self.ledger_checkout))
        self.assertEqual(report["inputs"]["ledger"]["path"], str(copied))
        self.assertEqual(next(ref["status"] for ref in report["requirements"][0]["references"]
                              if ref["reference"] == "docs/execution/report.json"), "exists")

    def test_both_formats_are_reproducible_and_display_unknowns(self):
        for name in ("json", "markdown"):
            with self.subTest(format=name):
                first = self.run_fixture(format=name)
                again = self.run_fixture(format=name)
                self.assertEqual(first.returncode, 0, first.stderr)
                self.assertEqual(again.returncode, 0, again.stderr)
                self.assertEqual(first.stdout, again.stdout)
        markdown = again.stdout
        self.assertIn("| 全部 | 3 | 1 | 1 |", markdown)
        self.assertIn("SHA-256", markdown)
        self.assertIn("引用检查根目录", markdown)
        self.assertIn("### AB-TST-003", markdown)
        self.assertIn("| 集成 | 未登记 | 未登记 | passed with a generated fixture only |", markdown)
        self.assertIn("Intel 记录仅供历史追溯。", markdown)
        self.assertIn("Intel historical note · passé", markdown)
        self.assertIn("历史说明", markdown)
        self.assertIn("**missing**", markdown)

    def assert_bad_input(self, expected):
        result = self.run_fixture()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(expected, result.stderr)
        self.assertEqual(result.stdout, "")

    def test_rejects_duplicate_ledger_id(self):
        self.ledger["requirements"].append(self.ledger["requirements"][0])
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        self.assert_bad_input("duplicate requirement ID AB-TST-001")

    def test_rejects_duplicate_spec_id(self):
        self.spec["requirements"].append(self.spec["requirements"][0])
        write_document(self.checkout / SPEC, self.spec)
        self.assert_bad_input("spec: duplicate requirement ID AB-TST-002")

    def test_rejects_missing_and_extra_ids(self):
        self.ledger["requirements"][2]["id"] = "AB-TST-999"
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        self.assert_bad_input("missing: AB-TST-003; extra: AB-TST-999")

    def test_rejects_work_package_and_design_version_mismatch(self):
        self.ledger["requirements"][0]["work_package"] = "WP03"
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        self.assert_bad_input("AB-TST-001: work_package mismatch")
        self.ledger["design_version"] = "0.1"
        write_document(self.ledger_checkout / LEDGER, self.ledger)
        self.assert_bad_input("design_version mismatch")

    def test_rejects_malformed_or_ambiguous_json(self):
        for raw, message in (
            (b"{", "Expecting property name"),
            (b'{"requirements": [], "requirements": []}', "duplicate JSON key"),
            (b'{"metric": NaN}', "non-finite JSON number"),
        ):
            with self.subTest(raw=raw):
                (self.ledger_checkout / LEDGER).write_bytes(raw)
                self.assert_bad_input(message)


if __name__ == "__main__":
    unittest.main()
