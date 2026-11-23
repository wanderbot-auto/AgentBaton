"""Exact driver and projector predecessor bytes, with synthetic legacy replay."""
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

from tests.infrastructure import host19_browser_ci_projector_source_fixture as I
from tests.infrastructure import test_integration_browser_ci_evidence as F
from tests.infrastructure import test_integration_browser_ci_evidence_runtime as R

ROOT = Path(__file__).resolve().parents[2]


class ExactSourceInverse(unittest.TestCase):
    def test_canonical_current_and_complete_predecessor_hashes(self):
        for path, inverse, current, previous in (
            (I.DRIVER_PATH, I.restore_driver_before_labels19, I.DRIVER_CURRENT_SHA256, I.DRIVER_BEFORE_SHA256),
            (I.PROJECTOR_PATH, I.restore_projector_before_schema19, I.PROJECTOR_CURRENT_SHA256,
             I.PROJECTOR_BEFORE_SHA256),
        ):
            with self.subTest(path=path):
                raw = (ROOT / path).read_bytes()
                from tests.infrastructure.mantine_picker_source_fixture import restore_mantine_picker_source
                self.assertEqual(I.digest(restore_mantine_picker_source(path, raw)), current)
                restored = inverse(path, raw)
                self.assertEqual(I.digest(restored), previous)
                self.assertEqual((ROOT / path).read_bytes(), raw)

    def test_partial_unknown_and_noncanonical_sources_refuse(self):
        for path, inverse, anchor in (
            (I.DRIVER_PATH, I.restore_driver_before_labels19, b"Core19 Unix IPC"),
            (I.PROJECTOR_PATH, I.restore_projector_before_schema19, b"def validated_candidate("),
        ):
            raw = (ROOT / path).read_bytes()
            for changed in (raw + b"\n", raw.replace(anchor, b"", 1),
                            raw.replace(anchor, anchor + anchor, 1), raw.replace(b"\n", b"\r\n"),
                            b"\xef\xbb\xbf" + raw, inverse(path, raw), bytearray(raw), raw.decode()):
                with self.subTest(path=path, changed_type=type(changed)), self.assertRaises(ValueError):
                    inverse(path, changed)
            for wrong in (Path(path), str(ROOT / path), "./" + path):
                with self.subTest(path=wrong), self.assertRaises(ValueError):
                    inverse(wrong, raw)


class LegacyProjectorReplay(unittest.TestCase):
    def projector(self):
        path = ROOT / I.PROJECTOR_PATH
        previous = I.restore_projector_before_schema19(I.PROJECTOR_PATH, path.read_bytes())
        module = types.ModuleType("historical_integration_projector")
        module.__file__ = str(path)
        exec(compile(previous, str(path), "exec"), module.__dict__)
        return module

    def prior_inputs(self, private, projector):
        manifest = private / "candidate.json"
        F.write(manifest, json.loads(manifest.read_bytes()) | {"schema": 18})
        F.write(private / "schema-probe.json", {"source": F.HEAD, "api_version": 1,
            "database_schema_version": 18, "daemon_exit": 0, "joined": True})
        for case in F.E.CASES:
            path = private / "driver" / case / "report.json"
            report = json.loads(path.read_bytes())
            report["dimension"] = projector.DIMENSION
            report["inputs"]["manifest_sha256"] = projector.raw_sha(manifest.read_bytes())
            report["inputs"]["driver_sha256"] = I.DRIVER_BEFORE_SHA256
            report["inputs"]["source_guard"]["driver_sha256"] = I.DRIVER_BEFORE_SHA256
            F.write(path, report)

    def test_exact_old_projector_accepts_its_success_and_failure_inputs(self):
        old = self.projector()
        for seed, outcome, code, status in (
            (F.seed, "success", 0, "passed"), (R.failed_seed, "failure", 1, "failed"),
        ):
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                private = seed(root)
                self.prior_inputs(private, old)
                args = ["--private", str(private), "--expected-source-head", F.HEAD,
                        "--driver-step-outcome", outcome]
                with mock.patch.object(old, "expected_driver", return_value=I.DRIVER_BEFORE_SHA256):
                    self.assertEqual(old.main([*args, "--output", str(root / "legacy")]), code)
                result = json.loads((root / "legacy" / "summary.json").read_bytes())
                self.assertEqual(result["status"], status)
                if outcome == "success":
                    self.assertEqual(result["actual_schema_probe"]["database_schema_version"], 18)
                    self.assertEqual({p.name for p in (root / "legacy").iterdir()}, F.E.PUBLIC_NAMES | {"summary.json"})
                else:
                    self.assertEqual(result["runtime_failure"]["status"], "observed")
                    self.assertEqual({p.name for p in (root / "legacy").iterdir()}, {"summary.json"})
                self.assertEqual(F.E.main([*args, "--output", str(root / "current")]), 1)
                self.assertEqual({p.name for p in (root / "current").iterdir()}, {"summary.json"})
                self.assertEqual(json.loads((root / "current" / "summary.json").read_bytes())
                                 ["runtime_failure"]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
