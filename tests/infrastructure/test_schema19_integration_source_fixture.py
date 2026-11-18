"""Source-only checks for the Integration schema19 restoration boundary."""
from hashlib import sha256
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from schema19_integration_source_fixture import (
    IPC_PATH,
    RUNTIME_PATH,
    read_schema18_source,
    restore_schema18_source,
)
from test_integration_ipc_diagnostics import strip_post_join_layer
from test_integration_stage_evidence_source import fixture_inverse, host_inverse

ROOT = Path(__file__).resolve().parents[2]


class Schema19IntegrationSourceFixtureTests(unittest.TestCase):
    def test_actual_candidate_restores_each_pinned_schema18_predecessor(self):
        for path, expected in (
            (IPC_PATH, "58f68b35230b06b2d64f4143fcadeb1d075d2e77c8763c1007e666e738da4199"),
            (RUNTIME_PATH, "bf9a348a6923432c0ecb66d5fefbe1b40aee9d46729eddddcd62e04079d3d504"),
        ):
            with self.subTest(path=path):
                source = ROOT / path
                raw = source.read_bytes()
                restored = restore_schema18_source(path, raw)
                self.assertNotEqual(raw, restored)
                self.assertEqual(sha256(restored).hexdigest(), expected)
                self.assertEqual(read_schema18_source(ROOT, source).encode("utf-8"), restored)
                self.assertEqual(restore_schema18_source(path, restored), restored)
                self.assertEqual(source.read_bytes(), raw)

    def test_actual_candidate_runs_unchanged_historical_chains(self):
        for path, inverse, expected in (
            (IPC_PATH, host_inverse, "90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb"),
            (RUNTIME_PATH, fixture_inverse, "149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06"),
        ):
            with self.subTest(path=path):
                current = (ROOT / path).read_bytes()
                with self.assertRaises(AssertionError):
                    inverse(current.decode("utf-8"))
                text = restore_schema18_source(path, current).decode("utf-8")
                historical = inverse(text)
                self.assertEqual(sha256(historical.encode("utf-8")).hexdigest(), expected)
                self.assertEqual(strip_post_join_layer(path, text), historical)
                with self.assertRaises(AssertionError):
                    restore_schema18_source(path, historical.encode("utf-8"))

    def test_each_current_anchor_must_remain_present_unique_and_exact(self):
        for path, anchor in (
            (IPC_PATH, b"        assert_eq!(version, 19);\n"),
            (RUNTIME_PATH, b"    assert_eq!(version, 19);\n"),
            (RUNTIME_PATH, b'["database_schema_version"],\n            19\n'),
        ):
            raw = (ROOT / path).read_bytes()
            # The IPC assertion has extra indentation. This runtime fixture
            # contains only one version assertion and one snapshot anchor.
            self.assertEqual(raw.count(anchor), 1)
            for change in (
                raw.replace(anchor, b"", 1),
                raw.replace(anchor, anchor + anchor, 1),
                raw.replace(anchor, anchor.replace(b"19", b"20"), 1),
                raw.replace(anchor, b"", 1) + anchor,
            ):
                with self.subTest(path=path, anchor=anchor, digest=sha256(change).hexdigest()):
                    with self.assertRaises(AssertionError):
                        restore_schema18_source(path, change)

    def test_raw_and_disk_reads_refuse_extra_bytes_bom_crlf_and_oversize(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            for path in (IPC_PATH, RUNTIME_PATH):
                source = root / path
                source.parent.mkdir(parents=True, exist_ok=True)
                raw = (ROOT / path).read_bytes()
                predecessor = restore_schema18_source(path, raw)
                changes = [b"", b"x" * 1_048_577]
                for exact in (raw, predecessor):
                    changes.extend((exact + b"\n", b"\xef\xbb\xbf" + exact, exact.replace(b"\n", b"\r\n")))
                for changed in changes:
                    with self.subTest(path=path, digest=sha256(changed).hexdigest()):
                        with self.assertRaises(AssertionError):
                            restore_schema18_source(path, changed)
                        source.write_bytes(changed)
                        with self.assertRaises(AssertionError):
                            read_schema18_source(root, source)

    def test_path_and_type_checks_refuse_aliases_and_swapped_sources(self):
        ipc = (ROOT / IPC_PATH).read_bytes()
        with self.assertRaises(AssertionError):
            restore_schema18_source(RUNTIME_PATH, ipc)
        with self.assertRaises(AssertionError):
            restore_schema18_source(IPC_PATH, (ROOT / RUNTIME_PATH).read_bytes())
        for path in (f"./{IPC_PATH}", str(ROOT / IPC_PATH), Path(IPC_PATH), "", None):
            with self.subTest(path=path), self.assertRaises(AssertionError):
                restore_schema18_source(path, ipc)
        for raw in (ipc.decode("utf-8"), bytearray(ipc), memoryview(ipc), None):
            with self.subTest(source_type=type(raw)), self.assertRaises(AssertionError):
                restore_schema18_source(IPC_PATH, raw)
        with self.assertRaises(AssertionError):
            read_schema18_source(ROOT, ROOT.parent / IPC_PATH)
        with self.assertRaises(AssertionError):
            read_schema18_source(ROOT, ROOT / "missing-file.rs")


if __name__ == "__main__":
    unittest.main()
