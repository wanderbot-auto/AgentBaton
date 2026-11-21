"""CLI checks for the repository's pinned evidence and synthetic release controls."""

import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/report-compatibility-matrix.py"
SPEC = "specs/provider-compatibility.json"
CONFIG = "apps/desktop/src-tauri/tauri.conf.json"
INDEX = "docs/execution/compatibility-matrix/OBSERVATIONS.json"
FIXTURE_INDEX = "docs/execution/synthetic/index.json"
FIXTURE_EVIDENCE = "docs/execution/synthetic/observations.json"
FIXTURE_CANDIDATE = "docs/execution/synthetic/candidate.json"
SOURCE = "a" * 40
PROVIDERS = ("codex", "qoder", "claude-code")
PLATFORM_CHECKS = ("build", "native_app", "credential_store", "background", "install", "update")


def run_report(script, *args, format="json"):
    return subprocess.run(
        [sys.executable, str(script), "--format", format, *args],
        cwd=script.parent, capture_output=True, encoding="utf-8", timeout=15,
    )


def read_json(path):
    return json.loads(path.read_bytes())


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = (json.dumps(value, ensure_ascii=False) + "\n").encode("utf-8")
    path.write_bytes(raw)
    return hashlib.sha256(raw).hexdigest()


class PinnedEvidenceTests(unittest.TestCase):
    def test_actual_records_preserve_source_version_and_scope(self):
        result = run_report(SCRIPT)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["platform_targets"]["desktop_macos"], ["arm64"])
        self.assertEqual(report["platform_targets"]["declared_bundle_minimum"], "10.15")
        self.assertIsNone(report["minimum_system_evidence"]["confirmed_minimum"])
        self.assertEqual(report["minimum_system_evidence"]["assessment"], "candidate_not_selected")
        self.assertEqual(set(item["id"] for item in report["provider_targets"]),
                         {"codex", "qoder", "claude-code", "custom-acp"})
        for target in report["provider_targets"]:
            self.assertIsNone(target["pinned_provider_version"])
            self.assertEqual(target["candidate_real_account_status"], "unknown")
        sources = report["inputs"]["evidence"]
        self.assertEqual(len(sources), len(read_json(ROOT / INDEX)["sources"]))
        for source in sources:
            self.assertEqual(source["state"], "pinned")
            self.assertEqual(source["sha256"], hashlib.sha256((ROOT / source["path"]).read_bytes()).hexdigest())
        rows = {row["id"]: row for row in report["observations"]}
        codex = rows["codex-generated-startup-contract"]
        self.assertEqual((codex["dimension"], codex["provider_execution"], codex["status"]),
                         ("provider_contract", "generated", "passed"))
        self.assertIn("provider_version", codex["unrecorded_fields"])
        self.assertEqual(rows["apple-silicon-schema19-library"]["dimension"], "library")
        browser = rows["apple-silicon-generated-acp-browser"]
        self.assertEqual(browser["dimension"], "browser")
        self.assertIn("schema 19", browser["scope"])
        self.assertNotIn("Core18", browser["scope"])
        self.assertIn("Core18", browser["facts"]["source_dimension_label"])
        self.assertNotIn("platform_description", browser["facts"])
        self.assertIn("os_version", browser["unrecorded_fields"])
        receipt = browser["evidence"]["corroboration"]
        self.assertEqual(receipt["schema19_snapshot_count"], 12)
        verified_path = ROOT / receipt["path"]
        self.assertEqual(receipt["sha256"], hashlib.sha256(verified_path.read_bytes()).hexdigest())
        self.assertEqual(read_json(verified_path)["normal"]["report_sha256"], browser["evidence"]["sha256"])
        native = rows["apple-silicon-host-registration-app-final"]
        earlier = rows["apple-silicon-host-registration-app-frozen"]
        self.assertEqual(native["dimension"], "native_app")
        self.assertEqual(native["status"], "passed")
        self.assertEqual(earlier["status"], "failed")
        self.assertEqual(native["facts"]["source_commit"], earlier["facts"]["source_commit"])
        self.assertNotEqual(native["facts"]["driver_commit"], earlier["facts"]["driver_commit"])
        self.assertNotIn("artifact_sha256", native["facts"])
        self.assertEqual(rows["pr15-hosted-arm64"]["origin"], "hosted_ci")
        intel = rows["pr15-development-archive-x86_64"]
        self.assertEqual(intel["relevance"], "historical_intel")
        self.assertEqual(intel["facts"]["os_version"], "15.7.9")
        self.assertEqual(intel["facts"]["os_build"], "24G830")
        self.assertEqual(rows["pr15-manual-install-arm64"]["status"], "not_run")
        self.assertEqual(rows["pr15-development-archive-arm64"]["relevance"], "other_source")
        self.assertFalse(any(row["dimension"] == "provider_real" for row in rows.values()))

    def test_actual_outputs_are_reproducible(self):
        for name in ("json", "markdown"):
            with self.subTest(format=name):
                first = run_report(SCRIPT, format=name)
                second = run_report(SCRIPT, format=name)
                self.assertEqual(first.returncode, 0, first.stderr)
                self.assertEqual(second.returncode, 0, second.stderr)
                self.assertEqual(first.stdout, second.stdout)
        self.assertIn("最低支持系统待定", first.stdout)
        self.assertIn("host-registration-native-e2e/logs/app-final/result.json", first.stdout)


class MinimumSystemControls(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.script = self.root / "scripts" / SCRIPT.name
        self.script.parent.mkdir()
        shutil.copyfile(SCRIPT, self.script)
        write_json(self.root / SPEC, read_json(ROOT / SPEC))
        write_json(self.root / CONFIG, read_json(ROOT / CONFIG))
        self.build = {
            "source_commit": SOURCE, "product_version": "99.0.0-fixture",
            "os": "macos", "os_version": "13.4", "os_build": "SYNTHETIC",
            "architecture": "arm64", "artifact_sha256": "b" * 64,
        }
        self.versions = {
            provider: {"provider_version": "synthetic-1", "adapter": "fixture",
                       "adapter_version": "a1", "protocol_version": "p1"}
            for provider in PROVIDERS
        }
        self.document = {}
        self.receipts = {}
        self.index = {
            "schema_version": 1,
            "sources": [{"path": FIXTURE_EVIDENCE, "sha256": None, "observations": []}],
        }
        write_json(self.root / FIXTURE_CANDIDATE, {**self.build, "providers": self.versions})

    def add_observation(self, identifier, dimension, status="passed", *, provider=None,
                        mode="none", overrides=None, context=None):
        facts = dict(self.build)
        if provider:
            facts.update(provider_id=provider, **self.versions[provider])
        facts.update(overrides or {})
        if context:
            for key in context:
                facts.pop(key, None)
        self.document[identifier] = {**facts, "status": status}
        entry = {
            "id": identifier, "dimension": dimension, "origin": "local",
            "provider_execution": mode, "scope": "synthetic test evidence",
            "result": {"path": f"/{identifier}/status", "kind": "status", "value": status},
            "fields": {key: f"/{identifier}/{key}" for key in facts},
        }
        if context:
            entry["context"] = context
        self.index["sources"][0]["observations"].append(entry)
        return entry

    def add_real_provider(self, identifier, provider, *, overrides=None, status="passed"):
        receipt_path = f"docs/execution/synthetic/{identifier}-account.json"
        receipt = {
            "schema_version": 1, "kind": "real_provider_execution",
            "build": self.build.copy(),
            "provider": {"provider_id": provider, **self.versions[provider], **(overrides or {})},
            "status": status,
            "account_execution": {
                "mode": "real_account", "provider_invoked": True, "authenticated": True,
                "task_completed": status == "passed", "run_ref": f"synthetic/{identifier}",
            },
        }
        self.receipts[receipt_path] = receipt
        entry = {
            "id": identifier, "dimension": "provider_real", "origin": "local",
            "provider_execution": "real_account", "scope": "synthetic raw receipt",
        }
        self.index["sources"].append({"path": receipt_path, "sha256": None, "observations": [entry]})
        return receipt, entry

    def save(self):
        self.index["sources"][0]["sha256"] = write_json(self.root / FIXTURE_EVIDENCE, self.document)
        for source in self.index["sources"][1:]:
            source["sha256"] = write_json(self.root / source["path"], self.receipts[source["path"]])
        write_json(self.root / FIXTURE_INDEX, self.index)

    def invoke(self, use_candidate=True):
        args = ("--candidate", FIXTURE_CANDIDATE) if use_candidate else ("--source", SOURCE)
        return run_report(self.script, "--index", FIXTURE_INDEX, *args)

    def report(self, use_candidate=True):
        result = self.invoke(use_candidate)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def assert_failed(self, message):
        result = self.invoke()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertIn(message, result.stderr)

    def test_full_exact_candidate_evidence_only_opens_release_review(self):
        for dimension in PLATFORM_CHECKS:
            self.add_observation(dimension, dimension)
        for provider in PROVIDERS:
            self.add_real_provider(provider, provider)
        self.save()
        report = self.report()
        minimum = report["minimum_system_evidence"]
        self.assertEqual(minimum["assessment"], "ready_for_release_review")
        self.assertIsNone(minimum["confirmed_minimum"])
        self.assertTrue(all(gate["status"] == "passed" for gate in minimum["required_checks"]))
        self.assertTrue(all(row["candidate_match"] == "exact" for row in report["observations"]))
        codex = next(row for row in report["observations"] if row["id"] == "codex")
        self.assertEqual(codex["evidence"]["result_pointer"], "/status")
        self.assertEqual(codex["evidence"]["provider_run_reference"], "synthetic/codex")
        self.assertTrue(codex["evidence"]["account_execution"]["authenticated"])

    def test_gates_keep_distinct_outcomes_and_exclude_other_builds(self):
        self.add_observation("failed-build", "build", "failed")
        self.add_observation("passed-build", "build")
        self.add_observation("install", "install", "not_run")
        self.add_observation("background", "background", "blocked")
        self.add_observation("credential-store", "credential_store", "unsupported")
        self.add_observation("update", "update", "not_applicable")
        self.add_observation("changed-build", "native_app", overrides={"os_build": "OTHER"})
        self.add_observation("changed-digest", "native_app", overrides={"artifact_sha256": "c" * 64})
        self.add_observation("other-source", "native_app", overrides={"source_commit": "d" * 40})
        self.add_observation("browser", "browser", mode="generated")
        self.add_observation("policy-note", "native_app", context={"os_build": "SYNTHETIC"})
        self.add_observation("ci", "library")
        self.add_real_provider("old-version", "codex", overrides={"adapter_version": "different"})
        self.add_observation("contract", "provider_contract", provider="codex", mode="generated")
        qoder, _ = self.add_real_provider("qoder", "qoder", status="failed")
        qoder["account_execution"]["authenticated"] = False
        self.save()
        report = self.report()
        minimum = report["minimum_system_evidence"]
        self.assertEqual(minimum["assessment"], "evidence_incomplete")
        self.assertIsNone(minimum["confirmed_minimum"])
        gates = {(row["dimension"], row["provider_id"]): row["status"]
                 for row in minimum["required_checks"]}
        self.assertEqual(gates, {
            ("build", None): "failed", ("native_app", None): "unknown",
            ("credential_store", None): "unsupported", ("background", None): "blocked",
            ("install", None): "not_run", ("update", None): "not_applicable",
            ("provider_real", "codex"): "unknown", ("provider_real", "qoder"): "failed",
            ("provider_real", "claude-code"): "unknown",
        })
        rows = {row["id"]: row for row in report["observations"]}
        self.assertEqual(rows["changed-build"]["candidate_match"], "different_build")
        self.assertEqual(rows["other-source"]["relevance"], "other_source")
        self.assertEqual(rows["policy-note"]["candidate_match"], "unrecorded_build_identity")
        self.assertEqual(rows["old-version"]["candidate_match"], "different_or_unrecorded_provider_version")

    def test_real_account_gate_rejects_generated_observation(self):
        self.add_observation("false-pass", "provider_real", provider="codex", mode="generated")
        self.save()
        self.assert_failed("provider_real requires real_account execution")

    def test_matching_generated_evidence_cannot_be_relabeled_real(self):
        for dimension in PLATFORM_CHECKS:
            self.add_observation(dimension, dimension)
        for provider in ("qoder", "claude-code"):
            self.add_real_provider(provider, provider)
        generated = self.add_observation("generated", "provider_contract", provider="codex", mode="generated")
        self.document["generated"]["account_mode"] = "generated"
        self.document["generated"]["note"] = "A real_account run would require separate evidence."
        self.save()
        self.assertEqual(self.report()["minimum_system_evidence"]["assessment"], "evidence_incomplete")
        generated.update(dimension="provider_real", provider_execution="real_account")
        self.save()
        self.assert_failed("expected a raw real_provider_execution receipt")

    def test_receipt_requires_actual_invocation_authentication_and_task_completion(self):
        receipt, _ = self.add_real_provider("claim", "codex")
        original = receipt["account_execution"].copy()
        for field, value, error in (
            ("mode", "generated", "actual real-account Provider invocation"),
            ("provider_invoked", False, "actual real-account Provider invocation"),
            ("provider_invoked", 1, "actual real-account Provider invocation"),
            ("authenticated", False, "successful authentication and task completion"),
            ("task_completed", False, "successful authentication and task completion"),
        ):
            with self.subTest(field=field, value=value):
                receipt["account_execution"] = {**original, field: value}
                self.save()
                self.assert_failed(error)

    def test_index_cannot_borrow_success_from_another_test_in_same_receipt(self):
        receipt, entry = self.add_real_provider("claim", "codex", status="failed")
        receipt["generated_exit"] = 0
        entry["result"] = {"kind": "exit_code", "path": "/generated_exit", "value": 0}
        self.save()
        self.assert_failed("the index cannot override real Provider receipt fields")

    def test_candidate_requires_every_formal_provider_and_arm64(self):
        candidate = read_json(self.root / FIXTURE_CANDIDATE)
        self.save()
        invalid = copy.deepcopy(candidate)
        del invalid["providers"]["codex"]
        write_json(self.root / FIXTURE_CANDIDATE, invalid)
        self.assert_failed("candidate must pin every formal Provider")
        write_json(self.root / FIXTURE_CANDIDATE, {**candidate, "architecture": "x86_64"})
        self.assert_failed("minimum-system candidate must be macOS arm64")

    def test_mismatched_digest_and_result_fail_closed(self):
        row = self.add_observation("native", "native_app")
        self.save()
        write_json(self.root / FIXTURE_EVIDENCE, {"native": {"status": "failed"}})
        self.assert_failed("SHA-256 mismatch")
        row["result"]["value"] = "failed"
        self.save()
        self.assert_failed("evidence field does not match the index: /native/status")

    def test_observed_exit_code_cannot_be_relabelled_or_use_boolean(self):
        row = self.add_observation("build", "build", 101)
        row["result"].update(kind="exit_code", status="passed")
        self.save()
        self.assertEqual(self.report()["observations"][0]["status"], "failed")
        self.document["build"]["status"] = True
        row["result"]["value"] = True
        self.save()
        self.assert_failed("exit_code requires an integer result")

    def test_browser_snapshot_receipt_must_bind_the_exact_report_and_have_a_count(self):
        row = self.add_observation("browser", "browser", mode="generated")
        self.document["case"] = "normal"
        self.save()
        digest = self.index["sources"][0]["sha256"]
        path = "docs/execution/synthetic/browser-verified.json"
        for report_sha, count, error in (
            ("0" * 64, 12, "evidence field does not match the index: /normal/report_sha256"),
            (digest, True, "expected a positive schema 19 snapshot count"),
            (digest, 0, "expected a positive schema 19 snapshot count"),
        ):
            with self.subTest(report_sha=report_sha, count=count):
                receipt = {"normal": {"report_sha256": report_sha, "actual_schema19_snapshot_count": count}}
                row["corroboration"] = {"path": path, "sha256": write_json(self.root / path, receipt)}
                self.save()
                self.assert_failed(error)

    def test_missing_reference_visible_even_with_other_fixed_input(self):
        self.add_observation("native", "native_app")
        self.save()
        index = copy.deepcopy(self.index)
        index["sources"].append({**index["sources"][0], "path": "docs/execution/synthetic/missing.json"})
        write_json(self.root / FIXTURE_INDEX, index)
        report = self.report()
        self.assertEqual(report["inputs"]["evidence"][1]["state"], "missing")
        self.assertEqual(len(report["observations"]), 1)

    def test_path_escape_and_symlink_refuse_inputs(self):
        self.add_observation("native", "native_app")
        self.save()
        self.index["sources"][0]["path"] = "docs/execution/../../../outside.json"
        write_json(self.root / FIXTURE_INDEX, self.index)
        self.assert_failed("expected a repository-relative JSON path")
        self.index["sources"][0]["path"] = FIXTURE_EVIDENCE
        self.save()
        evidence = self.root / FIXTURE_EVIDENCE
        with tempfile.TemporaryDirectory() as outside:
            target = Path(outside).resolve() / "unrelated-test-file.json"
            evidence.rename(target)
            evidence.symlink_to(target)
            self.assert_failed("Too many levels of symbolic links")

    def test_rejects_duplicate_ids_and_ambiguous_json(self):
        row = self.add_observation("native", "native_app")
        self.index["sources"][0]["observations"].append(row)
        self.save()
        self.assert_failed("duplicate observation ID")
        raw = b'{"schema_version": 1, "sources": [], "sources": []}'
        (self.root / FIXTURE_INDEX).write_bytes(raw)
        self.assert_failed("duplicate JSON key")


if __name__ == "__main__":
    unittest.main()
