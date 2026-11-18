"""Current driver version controls and strict historical source restoration."""
from contextlib import closing
import hashlib
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest import mock

from tests.infrastructure.host19_browser_ci_projector_source_fixture import (
    restore_driver_before_labels19,
)
from tests.infrastructure.host19_integration_browser_candidate_fixture import (
    restore_integration_browser_before_candidate19,
)
from tests.infrastructure.host19_integration_browser_source_fixture import (
    DRIVER_PATH,
    GUARD_EDITS,
    restore_integration_browser_before_schema19,
)

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / DRIVER_PATH
spec = importlib.util.spec_from_file_location("_host19_integration_browser_driver", SOURCE)
DRIVER = importlib.util.module_from_spec(spec)
spec.loader.exec_module(DRIVER)


class ReachedProjectImport(Exception):
    pass


class CurrentDriverSchema(unittest.TestCase):
    def test_bootstrap_only_accepts_integer_19(self):
        for version in (19, 18, 20, 19.0, "19", True, None):
            with self.subTest(value=version, kind=type(version)), tempfile.TemporaryDirectory() as folder:
                def result(method, *_args):
                    if method == "system.snapshot":
                        return {"database_schema_version": version}
                    self.assertEqual(method, "projects.import")
                    raise ReachedProjectImport

                host = mock.Mock()
                host.result.side_effect = result
                accepts = type(version) is int and version == 19
                error = ReachedProjectImport if accepts else DRIVER.Refusal
                with mock.patch.object(DRIVER, "git", return_value="generated-target"):
                    with self.assertRaises(error):
                        DRIVER.bootstrap(host, Path(folder), False)
                self.assertEqual(
                    [call.args[0] for call in host.result.call_args_list],
                    ["system.snapshot", "projects.import"] if accepts else ["system.snapshot"],
                )

    def test_real_sqlite_only_opens_19_in_a_readonly_transaction(self):
        for version in (18, 19, 20):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as folder:
                runtime = Path(folder)
                path = runtime / "baton.sqlite3"
                with closing(sqlite3.connect(path)) as writer:
                    writer.execute(f"PRAGMA user_version={version}")
                    writer.execute("CREATE TABLE history (id INTEGER PRIMARY KEY, value TEXT)")
                    writer.execute("INSERT INTO history VALUES (1, 'unchanged')")
                    writer.commit()
                earlier = path.read_bytes()
                if version == 19:
                    with DRIVER.database(runtime) as db:
                        self.assertTrue(db.in_transaction)
                        self.assertEqual(db.execute("PRAGMA query_only").fetchone(), (1,))
                        self.assertEqual(db.execute("PRAGMA user_version").fetchone(), (19,))
                        self.assertEqual(db.execute("SELECT * FROM history").fetchall(), [(1, "unchanged")])
                        with self.assertRaisesRegex(sqlite3.OperationalError, "readonly"):
                            db.execute("UPDATE history SET value='modified'")
                else:
                    with self.assertRaises(DRIVER.Refusal), DRIVER.database(runtime):
                        self.fail("Wrong schema yielded a database connection")
                self.assertEqual(path.read_bytes(), earlier)

    def test_sqlite_guard_refuses_equal_valued_float_and_wrong_row_types(self):
        for row in ((19.0,), ("19",), (True,), (), None):
            with self.subTest(row=row):
                db = mock.Mock()
                db.execute.return_value.fetchone.return_value = row
                with mock.patch.object(DRIVER.sqlite3, "connect", return_value=db):
                    with self.assertRaises(DRIVER.Refusal), DRIVER.database(Path("/")):
                        self.fail("Invalid schema read yielded a database connection")
                db.close.assert_called_once_with()


class HistoricalDriverSource(unittest.TestCase):
    def setUp(self):
        self.current = SOURCE.read_bytes()
        prior_labels = restore_driver_before_labels19(DRIVER_PATH, self.current)
        self.raw = restore_integration_browser_before_candidate19(DRIVER_PATH, prior_labels)

    def test_two_exact_guards_restore_complete_parent(self):
        for current, parent in GUARD_EDITS:
            self.assertEqual(self.raw.count(current), 1)
            self.assertNotIn(parent, self.raw)
        restored = restore_integration_browser_before_schema19(DRIVER_PATH, self.raw)
        self.assertEqual(hashlib.sha256(restored).hexdigest(),
            "0131bee1842f3856507def3f41d4d45f0a9b38511167886461fc96d8a32d37fe")
        for current, parent in GUARD_EDITS:
            self.assertEqual(restored.count(parent), 1)
            self.assertNotIn(current, restored)
        self.assertEqual(SOURCE.read_bytes(), self.current)

    def test_parent_bom_line_endings_and_unreviewed_bytes_refuse(self):
        parent = restore_integration_browser_before_schema19(DRIVER_PATH, self.raw)
        controls = (parent, b"\xef\xbb\xbf" + self.raw,
            self.raw.replace(b"\n", b"\r\n"), self.raw + b"\n", b"")
        for raw in controls:
            with self.subTest(sha=hashlib.sha256(raw).hexdigest()), self.assertRaises(ValueError):
                restore_integration_browser_before_schema19(DRIVER_PATH, raw)

    def test_missing_duplicated_or_changed_anchors_refuse(self):
        for anchor, _ in GUARD_EDITS:
            controls = (
                self.raw.replace(anchor, b"", 1),
                self.raw.replace(anchor, anchor + anchor, 1),
                self.raw.replace(anchor, anchor.replace(b"19", b"20", 1), 1),
            )
            self.assertTrue(all(raw != self.raw for raw in controls))
            for raw in controls:
                with self.subTest(sha=hashlib.sha256(raw).hexdigest()), self.assertRaises(ValueError):
                    restore_integration_browser_before_schema19(DRIVER_PATH, raw)

    def test_noncanonical_path_and_nonbyte_inputs_refuse(self):
        for path in (str(SOURCE), "./" + DRIVER_PATH, "scripts/integration-real-ipc.py", Path(DRIVER_PATH)):
            with self.subTest(path=path), self.assertRaises(ValueError):
                restore_integration_browser_before_schema19(path, self.raw)
        for raw in (self.raw.decode(), bytearray(self.raw), None):
            with self.subTest(kind=type(raw)), self.assertRaises(ValueError):
                restore_integration_browser_before_schema19(DRIVER_PATH, raw)
