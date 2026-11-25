"""Exact-byte historical regression and current Task Workspace table contract."""
from contextlib import closing
from pathlib import Path
import sqlite3
import tempfile
import types
import unittest

from tests.infrastructure import host19_task_workspace_source_fixture as I
from tests.infrastructure import test_task_workspace_ci_evidence as F

ROOT = Path(__file__).resolve().parents[2]


def module_from_bytes(path, raw, name):
    module = types.ModuleType(name)
    module.__file__ = str(ROOT / path)
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


class ExactSourceInverse(unittest.TestCase):
    def test_complete_current_bytes_restore_pinned_predecessors(self):
        for path, current, previous in (
            (I.DRIVER_PATH, I.DRIVER_UI_SHA256, I.DRIVER_PREVIOUS_SHA256),
            (I.PROJECTOR_PATH, I.PROJECTOR_CURRENT_SHA256, I.PROJECTOR_PREVIOUS_SHA256),
        ):
            with self.subTest(path=path):
                raw = (ROOT / path).read_bytes()
                self.assertEqual(I.digest(raw), current)
                self.assertEqual(I.digest(I.restore_schema18_source(path, raw)), previous)
                self.assertEqual((ROOT / path).read_bytes(), raw)

    def test_ui_layer_restores_the_unchanged_schema19_driver(self):
        raw = (ROOT / I.DRIVER_PATH).read_bytes()
        self.assertEqual(I.digest(I.restore_ui_navigation(raw)), I.DRIVER_CURRENT_SHA256)
        with self.assertRaises(ValueError):
            I.restore_ui_navigation(raw + b"\n")

    def test_unknown_bytes_partial_anchors_and_noncanonical_paths_are_refused(self):
        for path, anchor in ((I.DRIVER_PATH, I.CURRENT_DB_IMAGE),
                             (I.PROJECTOR_PATH, b"    require(counts['host_registrations'] == 0)\n")):
            raw = (ROOT / path).read_bytes()
            for changed in (raw + b"\n", raw.replace(anchor, b"", 1),
                            raw.replace(anchor, anchor + anchor, 1), raw.replace(b"\n", b"\r\n"),
                            b"\xef\xbb\xbf" + raw, I.restore_schema18_source(path, raw),
                            raw.decode(), bytearray(raw)):
                with self.subTest(path=path, raw_type=type(changed)), self.assertRaises(ValueError):
                    I.restore_schema18_source(path, changed)
            for wrong in (Path(path), str(ROOT / path), "./" + path):
                with self.subTest(path=wrong), self.assertRaises(ValueError):
                    I.restore_schema18_source(wrong, raw)


class HistoricalTableEvidence(unittest.TestCase):
    def test_schema18_driver_and_projector_roundtrip_with_only_the_predecessor_tables(self):
        driver_raw = (ROOT / I.DRIVER_PATH).read_bytes()
        current = module_from_bytes(I.DRIVER_PATH, driver_raw, "current_task_workspace_driver")
        old_driver = module_from_bytes(
            I.DRIVER_PATH, I.restore_schema18_source(I.DRIVER_PATH, driver_raw),
            "historical_task_workspace_driver",
        )
        projector_raw = (ROOT / I.PROJECTOR_PATH).read_bytes()
        old_projector = module_from_bytes(
            I.PROJECTOR_PATH, I.restore_schema18_source(I.PROJECTOR_PATH, projector_raw),
            "historical_task_workspace_projector",
        )
        self.assertEqual(current.TABLES, F.P.TABLES)
        self.assertEqual(set(current.TABLES), set(old_projector.TABLES) | {"host_registrations"})
        self.assertNotIn("host_registrations", old_projector.TABLES)
        with tempfile.TemporaryDirectory() as temporary:
            runtime = Path(temporary)
            with closing(sqlite3.connect(runtime / "baton.sqlite3")) as db, db:
                db.execute("PRAGMA user_version=18")
                db.execute("CREATE TABLE tasks (id INTEGER PRIMARY KEY AUTOINCREMENT)")
                for table in old_projector.TABLES:
                    if table not in ("tasks", "sqlite_sequence"):
                        db.execute(f'CREATE TABLE "{table}" (id INTEGER PRIMARY KEY)')
            value = F.report()
            value["no_execution"] = old_driver.no_execution(runtime)
            self.assertEqual(set(value["no_execution"]["table_counts"]), set(old_projector.TABLES))
            summary = old_projector.project(value, "f" * 64, F.HEAD, F.HARNESS)
            self.assertEqual(summary["status"], "passed")
            self.assertEqual(summary["no_execution"]["schema"], 18)
            with self.assertRaises(F.P.Refused):
                F.P.project(value, "f" * 64, F.HEAD, F.HARNESS)
            with self.assertRaises(ValueError):
                current.no_execution(runtime)
            with self.assertRaises(old_projector.Refused):
                old_projector.project(F.report(), "f" * 64, F.HEAD, F.HARNESS)


if __name__ == "__main__":
    unittest.main()
