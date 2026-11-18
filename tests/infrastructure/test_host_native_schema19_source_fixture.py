"""Raw source contracts for the schema19 → schema18 → pre-Host inverse."""
from hashlib import sha256
from pathlib import Path
import unittest

from tests.infrastructure.host_native_bridge_source_fixture import restore_host_native_bridge_v1
from tests.infrastructure.host_native_schema19_source_fixture import restore_host_native_schema18

ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "apps/desktop/src-tauri/src/lib.rs"


class HostNativeSchema19SourceFixtureTests(unittest.TestCase):
    def test_actual_source_restores_both_exact_predecessors(self):
        raw = LIB.read_bytes()
        stage = restore_host_native_schema18(raw)
        self.assertEqual(sha256(stage).hexdigest(),
                         "338fbef7689416a71ec101aaad3ac5afb91e433c55d5c1c4bf413242108944d5")
        self.assertEqual(sha256(restore_host_native_bridge_v1(stage)).hexdigest(),
                         "6650edec5c8b8ac3fcdf5ff03813eafe40b5847609fc008cedbf185c96102cdb")
        with self.assertRaises(ValueError):
            restore_host_native_bridge_v1(raw)
        self.assertEqual(LIB.read_bytes(), raw)

    def test_actual_predecessor_is_refused_as_a_current_candidate(self):
        previous = restore_host_native_schema18(LIB.read_bytes())
        with self.assertRaises(ValueError):
            restore_host_native_schema18(previous)

    def test_missing_duplicate_moved_and_mutated_handshake_anchors_are_refused(self):
        raw = LIB.read_bytes()
        for line in (
            b'        || response["result"]["database_schema_version"].as_u64() != Some(19)\n',
            b'    fn current_schema_requires_authenticated_exact_typed_nineteen() {\n',
            b'    fn generic_owner_path_cannot_finalize_integration() {\n',
        ):
            self.assertEqual(raw.count(line), 1)
            for changed in (
                raw.replace(line, b"", 1),
                raw.replace(line, line * 2, 1),
                raw.replace(line, b"", 1) + line,
                raw.replace(line, b"// changed " + line, 1),
            ):
                with self.subTest(anchor=line, digest=sha256(changed).hexdigest()):
                    with self.assertRaises(ValueError):
                        restore_host_native_schema18(changed)

    def test_unrelated_byte_and_type_changes_are_refused(self):
        raw = LIB.read_bytes()
        for changed in (
            raw + b"\n",
            raw.replace(b"mod hosts;\n", b"mod other;\n", 1),
            raw.replace(b"fn ensure_daemon(", b"fn alternate_daemon(", 1),
            b"\xef\xbb\xbf" + raw,
            raw.replace(b"\n", b"\r\n"),
            raw + b"\xff",
            raw.decode("utf-8"),
            bytearray(raw),
            memoryview(raw),
            b"",
            None,
        ):
            with self.subTest(value_type=type(changed)):
                with self.assertRaises(ValueError):
                    restore_host_native_schema18(changed)


if __name__ == "__main__":
    unittest.main()
