"""Fixture and receipt contracts; real sidecar replay is recorded separately."""
from contextlib import closing
from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import sqlite3
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/rehearse-development-update.py'
SPEC = spec_from_file_location('development_update_rehearsal', SCRIPT)
R = module_from_spec(SPEC)
sys.modules[SPEC.name] = R
SPEC.loader.exec_module(R)

ACTIVITY_TABLES = ('connections', 'attempts', 'dispatch_intents', 'sessions',
                   'conversation_dispatches', 'goal_runs', 'factory_runs')
HOST_DDL = '''
CREATE TABLE host_registrations(
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision=1),
    created_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),
    document TEXT NOT NULL
);
CREATE INDEX host_registrations_name_id ON host_registrations(name_key,id);
'''


class RehearsalContract(unittest.TestCase):
    def setUp(self):
        root = self.enterContext(tempfile.TemporaryDirectory())
        self.task = Path(root).resolve()
        self.inputs = self.task / 'update-inputs'
        self.inputs.mkdir(mode=0o700)
        self.package = self.inputs / R.PACKAGE
        self.daemon = self.inputs / 'batond'
        self.database = self.inputs / 'historical-store.sqlite3'
        R.create_file(self.package, b'controlled package bytes')
        R.create_file(self.daemon, struct.pack('<IIII', 0xfeedfacf, 0x0100000c, 0, 2), 0o500)
        R.create_file(self.database, b'')
        self.edit('''
            PRAGMA user_version=18;
            CREATE TABLE projects(id TEXT PRIMARY KEY);
            CREATE TABLE tasks(document TEXT NOT NULL);
            CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL);
            CREATE TABLE idempotency(id TEXT PRIMARY KEY);
            INSERT INTO projects VALUES('fixture');
            INSERT INTO tasks VALUES('{"state":"draft"}'),('{"state":"draft"}'),('{"state":"cancelled"}');
            INSERT INTO events(document) VALUES('{"kind":"project.imported"}'),
                ('{"kind":"task.created"}'),('{"kind":"task.created"}'),('{"kind":"task.created"}'),
                ('{"kind":"task.cancelled"}');
            INSERT INTO idempotency VALUES('1'),('2'),('3'),('4');
        ''' + ''.join(f'CREATE TABLE {name}(state TEXT);' for name in ACTIVITY_TABLES))
        self.enterContext(patch.multiple(R, PACKAGE_SHA=R.sha(self.package.read_bytes()),
                                       DAEMON_SHA=R.sha(self.daemon.read_bytes())))
        self.start = self.enterContext(patch.object(R, 'migrate_with_candidate', side_effect=self.fake_migration))

    def edit(self, sql):
        with closing(sqlite3.connect(self.database)) as db:
            db.executescript(sql)

    def rehearse(self):
        with patch.object(R, 'FIXTURE_SHA', R.sha(self.database.read_bytes())):
            return R.rehearse(self.task)

    def fake_migration(self, binary, root, runtime):
        self.assertEqual(binary.parent, root)
        self.assertEqual(binary.read_bytes(), self.daemon.read_bytes())
        self.assertEqual(runtime.parent, root)
        self.assertEqual(root.parent, self.task)
        live = runtime / 'baton.sqlite3'
        R.create_file(runtime / 'baton.pre-v19-test.sqlite3', live.read_bytes())
        with closing(sqlite3.connect(live)) as db:
            db.executescript(HOST_DDL + 'PRAGMA user_version=19;')
        # The test runner stands in for the process lifecycle; SQLite, file
        # boundaries and all receipt cross-checks use the real harness.
        return {'ready': True, 'joined': True, 'exit': 0}

    def assert_refused(self, code, started=False):
        with self.assertRaisesRegex(R.Refused, '^' + code + '$'):
            self.rehearse()
        self.assertFalse(list(self.task.glob('*-evidence')))
        self.assertFalse(list(self.task.glob('core-*')))
        self.assertEqual(self.start.called, started)

    def test_backup_migrate_replace_restore_and_cross_linked_receipt(self):
        original = self.database.read_bytes()
        old = self.database.stat()
        output = self.rehearse()
        receipt = json.loads((output / 'receipt.json').read_text())
        self.assertFalse(receipt['installation_authorized'])
        self.assertEqual([receipt[step]['schema'] for step in ('backup', 'migration', 'recovery')],
                         [18, 19, 18])
        self.assertEqual(receipt['migration']['input_backup_sha256'], receipt['backup']['sha256'])
        self.assertEqual(receipt['recovery']['migration_output_sha256'], receipt['migration']['sha256'])
        self.assertEqual(receipt['recovery']['sha256'], receipt['backup']['sha256'])
        self.assertEqual(receipt['migration']['sequence'], 5)
        self.assertEqual(receipt['core_migration_backup']['sequence'], 5)
        self.assertEqual((output / 'restored.sqlite3').read_bytes(), original)
        self.assertEqual(self.database.read_bytes(), original)
        self.assertEqual(R.fingerprint(self.database.stat()), R.fingerprint(old))
        self.assertEqual(len({(output / name).stat().st_ino for name in
                              ('backup.sqlite3', 'migrated.sqlite3', 'restored.sqlite3')}), 3)
        self.assertEqual({path.name for path in output.iterdir()},
                         {'backup.sqlite3', 'migrated.sqlite3', 'restored.sqlite3', 'receipt.json'})
        self.assertFalse(list(self.task.glob('core-*')))

    def test_wrong_package_refused_before_daemon_start(self):
        self.package.write_bytes(b'different candidate using the same name')
        self.assert_refused('CANDIDATE_PACKAGE_MISMATCH')

    def test_wrong_daemon_refused_before_daemon_start(self):
        self.daemon.chmod(0o700)
        self.daemon.write_bytes(b'other daemon')
        self.daemon.chmod(0o500)
        self.assert_refused('CANDIDATE_DAEMON_MISMATCH')

    def test_matching_digest_with_wrong_native_header_refused(self):
        self.daemon.chmod(0o700)
        self.daemon.write_bytes(struct.pack('<IIII', 0xfeedfacf, 0x01000007, 0, 2))
        self.daemon.chmod(0o500)
        with patch.object(R, 'DAEMON_SHA', R.sha(self.daemon.read_bytes())):
            self.assert_refused('CANDIDATE_ARCHITECTURE_MISMATCH')

    def test_wrong_historical_fixture_refused_before_database_open(self):
        with patch.object(R, 'inspect_db') as inspect:
            with self.assertRaisesRegex(R.Refused, '^HISTORICAL_FIXTURE_MISMATCH$'):
                R.rehearse(self.task)
        inspect.assert_not_called()
        self.start.assert_not_called()

    def test_pending_or_real_connection_lanes_refused_even_when_fixture_digest_matches(self):
        for table, state in [('attempts', 'outcome_unknown'), ('dispatch_intents', 'prepared'),
                             ('sessions', 'running'), ('goal_runs', 'queued'),
                             ('factory_runs', 'waiting_child'), ('connections', 'ready')]:
            with self.subTest(table=table), closing(sqlite3.connect(self.database)) as db:
                db.execute(f'INSERT INTO {table} VALUES(?)', (state,))
                db.commit()
                self.assert_refused('FIXTURE_WORK_OR_CONTENT_UNPROVED')
                db.execute(f'DELETE FROM {table}')
                db.commit()

    def test_active_unknown_or_missing_task_state_refused_with_same_counts(self):
        for document in ['{"state":"running"}', '{"state":"future"}', '{}']:
            with self.subTest(document=document), closing(sqlite3.connect(self.database)) as db:
                db.execute('UPDATE tasks SET document=? WHERE rowid=1', (document,))
                db.commit()
                self.assert_refused('FIXTURE_WORK_OR_CONTENT_UNPROVED')

    def test_unknown_or_missing_work_evidence_refused(self):
        self.edit('DROP TABLE dispatch_intents;')
        self.assert_refused('FIXTURE_STATE_UNPROVED')

    def test_schema_pin_not_satisfied_by_future_store(self):
        self.edit('PRAGMA user_version=20;')
        self.assert_refused('FIXTURE_STATE_UNPROVED')

    def test_dangling_sidecar_refused(self):
        self.database.with_name(self.database.name + '-wal').symlink_to(self.inputs / 'absent')
        self.assert_refused('SQLITE_SIDECAR_PRESENT')

    def test_hard_link_candidate_refused(self):
        os.link(self.package, self.inputs / 'alias')
        self.assert_refused('UNSAFE_INPUT_FILE')

    def test_missing_migration_backup_refuses_to_publish(self):
        def no_backup(binary, root, runtime):
            evidence = self.fake_migration(binary, root, runtime)
            (runtime / 'baton.pre-v19-test.sqlite3').unlink()
            return evidence

        self.start.side_effect = no_backup
        self.assert_refused('CORE_BACKUP_UNPROVED', started=True)

    def test_migration_that_edits_historical_row_without_sequence_change_refuses(self):
        def changes_task(binary, root, runtime):
            evidence = self.fake_migration(binary, root, runtime)
            with closing(sqlite3.connect(runtime / 'baton.sqlite3')) as db:
                db.execute("UPDATE tasks SET document='{}' WHERE rowid=1")
                db.commit()
            return evidence

        self.start.side_effect = changes_task
        self.assert_refused('MIGRATION_CONTENT_MISMATCH', started=True)

    def test_failed_ready_or_join_never_publishes_a_receipt(self):
        self.start.side_effect = R.Refused('DAEMON_JOIN_UNPROVED')
        self.assert_refused('DAEMON_JOIN_UNPROVED', started=True)

    def test_replacement_with_identical_input_bytes_is_refused(self):
        def replace_input(binary, root, runtime):
            evidence = self.fake_migration(binary, root, runtime)
            replacement = self.inputs / 'replacement'
            R.create_file(replacement, self.package.read_bytes())
            os.replace(replacement, self.package)
            return evidence

        self.start.side_effect = replace_input
        self.assert_refused('INPUT_CHANGED', started=True)


if __name__ == '__main__':
    unittest.main()
