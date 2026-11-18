"""Live candidate preflight controls and exact predecessor restoration."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

from tests.infrastructure.host19_integration_browser_candidate_fixture import (
    CURRENT_GUARD,
    DRIVER_PATH,
    PARENT_GUARD,
    restore_integration_browser_before_candidate19,
)
from tests.infrastructure.host19_integration_browser_source_fixture import (
    restore_integration_browser_before_schema19,
)

ROOT = Path(__file__).resolve().parents[2]
HEAD = "a" * 40
SOURCE = ROOT / DRIVER_PATH


def load_driver(path):
    spec = importlib.util.spec_from_file_location("_candidate_schema_driver", path)
    driver = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(driver)
    return driver


DRIVER = load_driver(SOURCE)


class CurrentCandidateGuard(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        hashes = {}
        for name in ("batond", "baton", "chromium", "python"):
            path = self.root / name
            path.write_bytes(name.encode())
            path.chmod(0o500)
            hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.manifest = dict(schema=19, git_head=HEAD, compiled_core_source_head=HEAD,
            dirty=False, binaries={name: hashes[name] for name in ("batond", "baton")})
        self.args = argparse.Namespace(expected_source_head=HEAD, run=True,
            batond=self.root / "batond", chromium=self.root / "chromium",
            candidate_manifest=self.root / "manifest.json",
            frontend_build_record=self.root / "build.json", frontend=self.root / "frontend")
        self.args.frontend.mkdir()
        (self.args.frontend / "index.html").write_bytes(b"generated frontend fixture")
        self.args.frontend_build_record.write_text(json.dumps(dict(source=HEAD, exit=0,
            source_head_and_tracked_unchanged=True,
            command=["npm", "--prefix", "apps/desktop", "run", "build"])))

    def manifest_with(self, **patch):
        self.args.candidate_manifest.write_text(json.dumps({**self.manifest, **patch}))

    def test_integer_19_passes_preflight_with_current_driver_pin(self):
        self.manifest_with()
        diagnostic = DRIVER.new_driver_preflight_diagnostic(self.args)
        with mock.patch.object(DRIVER, "source_guard", return_value={"head": HEAD}), \
                mock.patch.object(sys, "executable", str(self.root / "python")):
            attribution = DRIVER.preflight(self.args, diagnostic)[3]
        self.assertEqual(diagnostic["stage"], "frontend_assets")
        self.assertEqual(attribution["source"], HEAD)
        self.assertEqual(attribution["driver_sha256"], hashlib.sha256(SOURCE.read_bytes()).hexdigest())

    def assert_cli_refuses_before_runtime(self, output_name, stage):
        output = self.root / output_name
        argv = [str(SOURCE), "--run", "--output", str(output),
            "--expected-source-head", HEAD, "--batond", str(self.args.batond),
            "--candidate-manifest", str(self.args.candidate_manifest),
            "--frontend-build-record", str(self.args.frontend_build_record),
            "--frontend", str(self.args.frontend), "--chromium", str(self.args.chromium)]
        with mock.patch.object(sys, "argv", argv), \
                mock.patch.object(sys, "executable", str(self.root / "python")), \
                mock.patch.object(DRIVER, "run_case") as run_case:
            self.assertEqual(DRIVER.main(), 2)
        run_case.assert_not_called()
        report = json.loads((output / "report.json").read_bytes())
        diagnostic = json.loads((output / "preflight-diagnostic.json").read_bytes())
        self.assertEqual(report["actual_runtime"], "NOT_RUN")
        self.assertTrue(report["no_process_token_or_listener"])
        self.assertEqual(diagnostic["stage"], stage)
        self.assertFalse(diagnostic["runtime_case_entered"])

    def test_wrong_versions_and_types_refuse_before_runtime(self):
        for index, version in enumerate((18, 20, True, False, "19", 19.0, None)):
            with self.subTest(version=version, kind=type(version)):
                self.manifest_with(schema=version)
                self.assert_cli_refuses_before_runtime(f"schema-{index}", "candidate_schema_guard")

    def test_source_hash_and_tool_permissions_still_refuse_before_runtime(self):
        self.manifest_with(compiled_core_source_head="b" * 40)
        self.assert_cli_refuses_before_runtime("wrong-source", "candidate_source_guard")
        self.manifest_with(binaries={"batond": "0" * 64, "baton": self.manifest["binaries"]["baton"]})
        self.assert_cli_refuses_before_runtime("wrong-tool", "candidate_tool_hash_guard")
        self.manifest_with()
        self.args.batond.chmod(0o520)
        self.assert_cli_refuses_before_runtime("writable-tool", "core_owner_mode_guard")

    def test_exact_predecessor_still_accepts_18_and_refuses_19(self):
        previous = self.root / DRIVER_PATH
        previous.parent.mkdir(parents=True)
        previous.write_bytes(restore_integration_browser_before_candidate19(DRIVER_PATH, SOURCE.read_bytes()))
        driver = load_driver(previous)
        self.manifest_with(schema=18)
        with mock.patch.object(driver, "source_guard", return_value={"head": HEAD}), \
                mock.patch.object(sys, "executable", str(self.root / "python")):
            attribution = driver.preflight(self.args)[3]
            self.assertEqual(attribution["source"], HEAD)
            self.assertEqual(attribution["driver_sha256"], hashlib.sha256(previous.read_bytes()).hexdigest())
            self.manifest_with(schema=19)
            with self.assertRaises(driver.Refusal):
                driver.preflight(self.args)


class CandidateSourceInverse(unittest.TestCase):
    def test_two_layers_recover_the_exact_old_driver_blobs(self):
        raw = SOURCE.read_bytes()
        self.assertEqual(raw.count(CURRENT_GUARD), 1)
        self.assertNotIn(PARENT_GUARD, raw)
        parent = restore_integration_browser_before_candidate19(DRIVER_PATH, raw)
        self.assertEqual(hashlib.sha256(parent).hexdigest(),
            "e5036fd15afa431228a9b11553b0602e144636dda58fecf4fbbbcaac8633f6f3")
        self.assertEqual(parent.count(PARENT_GUARD), 1)
        earlier = restore_integration_browser_before_schema19(DRIVER_PATH, parent)
        self.assertEqual(hashlib.sha256(earlier).hexdigest(),
            "0131bee1842f3856507def3f41d4d45f0a9b38511167886461fc96d8a32d37fe")
        self.assertEqual(SOURCE.read_bytes(), raw)

    def test_mutated_missing_repeated_old_and_noncanonical_bytes_refuse(self):
        raw = SOURCE.read_bytes()
        parent = restore_integration_browser_before_candidate19(DRIVER_PATH, raw)
        mutations = (parent, b"\xef\xbb\xbf" + raw, raw.replace(b"\n", b"\r\n"), raw + b"\n",
            raw.replace(CURRENT_GUARD, b"", 1),
            raw.replace(CURRENT_GUARD, CURRENT_GUARD * 2, 1),
            raw.replace(CURRENT_GUARD, CURRENT_GUARD.replace(b"19", b"20", 1), 1),
            raw.decode(), bytearray(raw))
        for variant in mutations:
            with self.subTest(kind=type(variant)), self.assertRaises(ValueError):
                restore_integration_browser_before_candidate19(DRIVER_PATH, variant)
        for path in (str(SOURCE), "./" + DRIVER_PATH, "scripts/integration-real-ipc.py", Path(DRIVER_PATH)):
            with self.subTest(path=path), self.assertRaises(ValueError):
                restore_integration_browser_before_candidate19(path, raw)
