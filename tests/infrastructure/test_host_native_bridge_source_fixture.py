"""Source checks for the exact Host dispatcher inverse."""
from hashlib import sha256
from pathlib import Path
import unittest

from tests.infrastructure.host_native_bridge_source_fixture import restore_host_native_bridge_v1
from tests.infrastructure.host_native_schema19_source_fixture import restore_host_native_schema18


ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "apps/desktop/src-tauri/src/lib.rs"
MODULE = b"mod hosts;\n"
HANDLERS = (
    b"        hosts::hosts_register,\n"
    b"        hosts::hosts_list,\n"
    b"        hosts::hosts_get,\n"
    b"        hosts::hosts_registration_pending,\n"
    b"        hosts::hosts_registration_reconcile,\n"
    b"        hosts::hosts_registration_retry,\n"
    b"        hosts::hosts_registration_acknowledge,\n"
)


class HostNativeBridgeSourceFixtureTests(unittest.TestCase):
    def test_actual_candidate_restores_only_eight_reviewed_lines(self):
        raw = LIB.read_bytes()
        current = restore_host_native_schema18(raw)
        self.assertEqual(
            sha256(current).hexdigest(),
            "338fbef7689416a71ec101aaad3ac5afb91e433c55d5c1c4bf413242108944d5",
        )
        self.assertEqual(current.count(MODULE), 1)
        self.assertEqual(current.count(HANDLERS), 1)

        predecessor = restore_host_native_bridge_v1(current)
        self.assertEqual(predecessor, current.replace(MODULE, b"", 1).replace(HANDLERS, b"", 1))
        self.assertEqual(
            sha256(predecessor).hexdigest(),
            "6650edec5c8b8ac3fcdf5ff03813eafe40b5847609fc008cedbf185c96102cdb",
        )
        self.assertEqual(LIB.read_bytes(), raw)

    def test_dispatch_insertions_cannot_be_missing_duplicated_moved_or_changed(self):
        current = restore_host_native_schema18(LIB.read_bytes())
        for insertion in (MODULE, HANDLERS):
            without = current.replace(insertion, b"", 1)
            changes = {
                "missing": without,
                "duplicate": current.replace(insertion, insertion * 2, 1),
                "misplaced": without + insertion,
                "different literal": current.replace(insertion, insertion.replace(b"hosts", b"other", 1), 1),
            }
            for label, changed in changes.items():
                with self.subTest(insertion=insertion.splitlines()[0], change=label):
                    self.assertNotEqual(changed, current)
                    with self.assertRaises(ValueError):
                        restore_host_native_bridge_v1(changed)

    def test_extra_handler_anchor_context_or_unrelated_production_edits_are_refused(self):
        current = restore_host_native_schema18(LIB.read_bytes())
        changes = {
            "extra handler": current.replace(HANDLERS, HANDLERS + b"        hosts::extra,\n", 1),
            "module neighbor": current.replace(
                b"mod detached_daemon;\nmod hosts;\n", b"mod detached_daemon;\n\nmod hosts;\n", 1,
            ),
            "handler neighbor": current.replace(
                HANDLERS + b"        connections_list,\n",
                HANDLERS + b"        connections_list, // different\n",
                1,
            ),
            "startup definition": current.replace(b"fn ensure_daemon(", b"fn alternate_daemon(", 1),
            "extra suffix": current + b"// unfamiliar source\n",
        }
        for label, changed in changes.items():
            with self.subTest(change=label):
                self.assertNotEqual(changed, current)
                with self.assertRaises(ValueError):
                    restore_host_native_bridge_v1(changed)

    def test_predecessor_and_raw_byte_changes_are_refused(self):
        current = restore_host_native_schema18(LIB.read_bytes())
        predecessor = restore_host_native_bridge_v1(current)
        for label, changed in (
            ("exact predecessor", predecessor),
            ("changed predecessor", predecessor + b"\n"),
            ("CRLF", current.replace(b"\n", b"\r\n")),
            ("BOM", b"\xef\xbb\xbf" + current),
            ("invalid UTF-8", current + b"\xff"),
            ("empty", b""),
        ):
            with self.subTest(change=label), self.assertRaises(ValueError):
                restore_host_native_bridge_v1(changed)

    def test_text_and_byte_like_values_are_refused(self):
        current = restore_host_native_schema18(LIB.read_bytes())
        for changed in (current.decode("utf-8"), bytearray(current), memoryview(current), None):
            with self.subTest(source_type=type(changed).__name__), self.assertRaises(ValueError):
                restore_host_native_bridge_v1(changed)


if __name__ == "__main__":
    unittest.main()
