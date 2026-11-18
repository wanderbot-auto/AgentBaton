"""Preparation guards and owned-process cleanup, not a Core/runtime substitute."""
import contextlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('factory_test_tools', ROOT / 'scripts/prepare-factory-test-tools.py')
TOOLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOLS)
IDENTITY = {'git_head': 'a' * 40, 'git_tree': 'b' * 40, 'tracked_clean': True}
CURRENT_PROBE = {
    'schema': 19, 'query_only': True, 'sequence': 0,
    'zero_tables': dict.fromkeys((*TOOLS.FACTORY_TABLES, *TOOLS.INTEGRATION_TABLES, 'host_registrations'), 0),
    'runtime_mode': '0o700', 'socket_utf8_bytes': 50,
    'snapshot_api_version': 1, 'snapshot_database_schema_version': 19, 'snapshot_sequence': 0,
    'cli_snapshot_exit': 0, 'original_daemon_SIGTERM_wait_exit': 0,
    'sigterm_sent': True, 'forced_kill': False,
}



class FactoryTestToolTests(unittest.TestCase):
    def setUp(self):
        # Cleanup controls must reach cleanup even when macOS defaults to a long /var/folders path.
        self.temporary = tempfile.TemporaryDirectory(prefix='factory-tool-guards-', dir='/tmp')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / 'source'
        self.source.mkdir()
        for name in ('batond', 'baton'):
            path = self.source / name
            path.write_bytes(b'Never executed: generated file for copy/hash guard only. ' + name.encode())
            path.chmod(0o700)
        self.output = self.root / 'frozen'
        self.evidence = self.root / 'evidence'

    def prepare(self):
        # Synthetic probe result isolates copy/hash controls. No Core success claimed.
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'probe', return_value=CURRENT_PROBE):
            return TOOLS.prepare(ROOT, self.source, self.output, self.evidence)

    def database(self, schema=18, missing=None, populated=None):
        with sqlite3.connect(self.root / 'baton.sqlite3') as conn:
            conn.execute(f'PRAGMA user_version={schema}')
            conn.execute('CREATE TABLE events(sequence INTEGER)')
            tables = (*TOOLS.FACTORY_TABLES, *TOOLS.INTEGRATION_TABLES)
            if schema == 19:
                tables = (*tables, 'host_registrations', 'tasks')
            for name in tables:
                if name != missing:
                    conn.execute('CREATE TABLE ' + name + '(id INTEGER)')
                    if name == populated:
                        conn.execute('INSERT INTO ' + name + ' VALUES(1)')

    def test_frozen_bytes_mode_source_identity_and_source_unchanged(self):
        original = {name: (TOOLS.sha256(self.source / name), (self.source / name).stat().st_mode) for name in ('batond', 'baton')}
        record = self.prepare()
        for name, (digest, mode) in original.items():
            self.assertEqual(TOOLS.sha256(self.source / name), digest)
            self.assertEqual((self.source / name).stat().st_mode, mode)
            self.assertEqual(TOOLS.sha256(self.output / name), digest)
            self.assertEqual((self.output / name).stat().st_mode & 0o777, 0o500)
            self.assertEqual((self.output / name).stat().st_nlink, 1)
        self.assertEqual(self.output.stat().st_mode & 0o777, 0o700)
        self.assertEqual(record['binaries'], record['source_binaries_after'])
        self.assertEqual(record['binaries'], record['frozen_binaries_after'])
        self.assertEqual(record['git_head'], IDENTITY['git_head'])
        self.assertEqual(record['git_tree'], IDENTITY['git_tree'])
        self.assertEqual((self.output / 'manifest.json').read_bytes(), (self.evidence / 'manifest.json').read_bytes())

    def test_absolute_paths_and_stale_output_rejected(self):
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY):
            for source, output in [(Path('relative'), self.output), (self.source, Path('relative'))]:
                with self.subTest(source=source, output=output), self.assertRaises(ValueError):
                    TOOLS.prepare(ROOT, source, output, self.evidence)
        self.prepare()
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), self.assertRaises(FileExistsError):
            TOOLS.prepare(ROOT, self.source, self.output, self.root / 'other-evidence')

    def test_single_link_copies_preserve_hardlinked_source_without_execution(self):
        aliases = {name: self.root / (name + '-alias') for name in ('batond', 'baton')}
        for name, alias in aliases.items():
            os.link(self.source / name, alias)
        before = {name: ((self.source / name).stat(), TOOLS.sha256(self.source / name)) for name in aliases}
        self.prepare()
        for name, (metadata, digest) in before.items():
            after = (self.source / name).stat()
            self.assertEqual((after.st_ino, after.st_nlink, after.st_mode),
                             (metadata.st_ino, 2, metadata.st_mode))
            self.assertEqual(TOOLS.sha256(aliases[name]), digest)
            self.assertEqual((self.output / name).stat().st_nlink, 1)
            self.assertEqual(TOOLS.sha256(self.output / name), digest)

    def test_frozen_mode_hardlink_symlink_and_nonregular_refused_before_hash(self):
        self.prepare()
        target = self.output / 'baton'
        original = target.read_bytes()
        for kind in ('mode', 'hardlink', 'symlink', 'fifo'):
            target.unlink()
            if kind == 'symlink':
                target.symlink_to(self.output / 'batond')
            elif kind == 'fifo':
                os.mkfifo(target, 0o500)
            else:
                target.write_bytes(original); target.chmod(0o700 if kind == 'mode' else 0o500)
                if kind == 'hardlink':
                    os.link(target, self.output / 'extra-link')
            with self.subTest(kind=kind), mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'sha256') as read, self.assertRaises(ValueError):
                TOOLS.verify(ROOT, self.output)
            read.assert_not_called()
            (self.output / 'extra-link').unlink(missing_ok=True)

    def test_missing_nonexecutable_and_symlink_source_rejected_without_execution(self):
        for kind in ('missing', 'nonexecutable', 'symlink'):
            path = self.source / 'baton'
            path.unlink(missing_ok=True)
            if kind == 'nonexecutable':
                path.write_bytes(b'not executable'); path.chmod(0o600)
            elif kind == 'symlink':
                path.symlink_to(self.source / 'batond')
            with self.subTest(kind=kind), mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'probe') as probe, self.assertRaises(ValueError):
                TOOLS.prepare(ROOT, self.source, self.root / ('out-' + kind), self.root / ('evidence-' + kind))
            probe.assert_not_called()

    def test_changed_source_during_probe_rejected_and_never_manifest_success(self):
        def change(*args):
            (self.source / 'baton').write_bytes(b'changed during probe')
            return {}
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'probe', side_effect=change), self.assertRaises(ValueError):
            TOOLS.prepare(ROOT, self.source, self.output, self.evidence)
        self.assertFalse((self.output / 'manifest.json').exists())

    def test_changed_checkout_during_probe_rejected(self):
        with mock.patch.object(TOOLS, 'source_identity', side_effect=[IDENTITY, {**IDENTITY, 'git_head': 'c' * 40}]), mock.patch.object(TOOLS, 'probe', return_value={}), self.assertRaises(ValueError):
            TOOLS.prepare(ROOT, self.source, self.output, self.evidence)
        self.assertFalse((self.output / 'manifest.json').exists())

    def test_probe_failure_is_preserved_not_manifest_success(self):
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'probe', side_effect=TimeoutError('unit negative')), self.assertRaises(TimeoutError):
            TOOLS.prepare(ROOT, self.source, self.output, self.evidence)
        self.assertEqual(json.loads((self.evidence / 'probe-failure.json').read_text())['error_type'], 'TimeoutError')
        self.assertFalse((self.output / 'manifest.json').exists())

    def test_readonly_exact_schema17_all_twelve_factory_and_four_integration_tables(self):
        self.database()
        before = (self.root / 'baton.sqlite3').read_bytes()
        observed = TOOLS.observe_database(self.root, expected_schema=18)
        self.assertEqual(observed['schema'], 18)
        self.assertEqual(len(observed['zero_tables']), 16)
        self.assertTrue(observed['query_only'])
        self.assertEqual((self.root / 'baton.sqlite3').read_bytes(), before)

    def test_old_and_future_schema_rejected(self):
        for schema in (13, 14, 15, 16, 17, 19):
            with self.subTest(schema=schema):
                self.database(schema=schema)
                with self.assertRaises(ValueError):
                    TOOLS.observe_database(self.root, expected_schema=18)
                (self.root / 'baton.sqlite3').unlink()

    def test_each_missing_or_nonzero_table_rejected(self):
        for name in (*TOOLS.FACTORY_TABLES, *TOOLS.INTEGRATION_TABLES):
            for kind in ('missing', 'populated'):
                with self.subTest(name=name, kind=kind):
                    self.database(**{kind: name})
                    with self.assertRaises((sqlite3.OperationalError, ValueError)):
                        TOOLS.observe_database(self.root, expected_schema=18)
                    (self.root / 'baton.sqlite3').unlink()

    def test_any_event_rejected(self):
        self.database()
        with sqlite3.connect(self.root / 'baton.sqlite3') as conn:
            conn.execute('INSERT INTO events VALUES(1)')
        with self.assertRaises(ValueError):
            TOOLS.observe_database(self.root, expected_schema=18)

    def test_verify_rejects_bytes_identity_path_version_and_missing_binary_binding(self):
        self.prepare()
        historical = json.loads((self.output / 'manifest.json').read_text())
        historical['schema'] = 18
        historical['probe'] = {'unit_only': True}
        original = json.dumps(historical).encode()
        (self.output / 'manifest.json').write_bytes(original)
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY):
            TOOLS._verify_manifest(ROOT, self.output, 18)
            for key, value in [('schema', 13), ('schema', 14), ('schema', 15), ('schema', 16), ('schema', 17), ('schema', 19), ('schema', 14.0), ('schema', 15.0), ('schema', 16.0), ('schema', 18.0), ('schema', '16'), ('schema', '17'), ('schema', True), ('git_head', 'c' * 40), ('git_tree', 'd' * 40), ('frozen_directory', '/wrong'), ('binaries', {'baton': 'x'})]:
                record = json.loads(original); record[key] = value
                (self.output / 'manifest.json').write_text(json.dumps(record))
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    TOOLS._verify_manifest(ROOT, self.output, 18)
            (self.output / 'manifest.json').write_bytes(original)
            (self.output / 'baton').chmod(0o700)
            (self.output / 'baton').write_bytes(b'changed frozen bytes')
            with self.assertRaises(ValueError):
                TOOLS._verify_manifest(ROOT, self.output, 18)

    def test_dirty_source_rejected_before_copy(self):
        with mock.patch.object(TOOLS.subprocess, 'check_output', return_value=' M tracked-file\n'), self.assertRaises(ValueError):
            TOOLS.source_identity(ROOT)
        self.assertFalse(self.output.exists())

    def test_owned_process_readiness_failure_reaps_original_child(self):
        # This is a real sleeping Python child, NOT a Core substitute or a success.
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(lambda: child.poll() is None and (child.kill(), child.wait()))
        self.evidence.mkdir()
        with mock.patch.object(TOOLS.subprocess, 'Popen', return_value=child), mock.patch.object(TOOLS.time, 'monotonic', side_effect=[0, 6]), mock.patch.object(TOOLS.time, 'time', side_effect=AssertionError('wall clock used')), self.assertRaises(TimeoutError):
            TOOLS.probe(self.source, self.evidence)
        self.assertIsNotNone(child.returncode)
        with self.assertRaises(ProcessLookupError):
            os.kill(child.pid, 0)
        self.assertTrue((self.evidence / 'probe-stderr.log').exists())

    def test_long_canonical_probe_path_fails_before_process_start(self):
        runtime = self.root / ('x' * 104)
        runtime.mkdir(mode=0o700)
        self.assertGreater(len(str(runtime / 'batond.sock').encode('utf-8')), 103)
        with mock.patch.object(TOOLS.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(str(runtime))), mock.patch.object(TOOLS.subprocess, 'Popen') as spawn, self.assertRaisesRegex(ValueError, 'short Unix socket'):
            TOOLS.probe(self.source, self.evidence)
        spawn.assert_not_called()
        self.assertEqual(runtime.stat().st_mode & 0o777, 0o700)

    def test_forced_kill_cannot_be_success(self):
        # A protocol-shaped mock reaches cleanup ONLY to test rejection. No Core
        # runtime success is claimed and neither generated tool file is executed.
        runtime = self.root / 'runtime'
        runtime.mkdir()
        (runtime / 'batond.sock').touch()
        process = mock.Mock(pid=999999999)
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired('owned-child', 5), -signal.SIGKILL]
        reply = subprocess.CompletedProcess([], 0, b'{"result":{"tasks":[],"sequence":0,"api_version":1,"database_schema_version":19}}', b'')
        self.evidence.mkdir()
        with mock.patch.object(TOOLS.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(str(runtime))), mock.patch.object(TOOLS.subprocess, 'Popen', return_value=process), mock.patch.object(TOOLS.subprocess, 'run', return_value=reply), mock.patch.object(TOOLS, 'observe_database', return_value={'schema': 19, 'sequence': 0}), self.assertRaises(RuntimeError):
            TOOLS.probe(self.source, self.evidence)
        process.terminate.assert_called_once_with()
        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_count, 2)
        cleanup = json.loads((self.evidence / 'probe-cleanup.json').read_text())
        self.assertTrue(cleanup['forced_kill'])
        self.assertEqual(cleanup['wait_exit'], -signal.SIGKILL)

    def test_no_sigterm_or_nonzero_exit_cannot_be_success(self):
        for poll, exit_code in [(0, 0), (None, 1)]:
            with self.subTest(poll=poll, exit_code=exit_code):
                runtime = self.root / ('runtime-' + str(exit_code))
                runtime.mkdir(); (runtime / 'batond.sock').touch()
                evidence = self.root / ('evidence-' + str(exit_code)); evidence.mkdir()
                process = mock.Mock(pid=999999999)
                process.poll.return_value = poll; process.wait.return_value = exit_code
                reply = subprocess.CompletedProcess([], 0, b'{"result":{"tasks":[],"sequence":0,"api_version":1,"database_schema_version":19}}', b'')
                with mock.patch.object(TOOLS.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(str(runtime))), mock.patch.object(TOOLS.subprocess, 'Popen', return_value=process), mock.patch.object(TOOLS.subprocess, 'run', return_value=reply), mock.patch.object(TOOLS, 'observe_database', return_value={'schema': 19, 'sequence': 0}), self.assertRaises(RuntimeError):
                    TOOLS.probe(self.source, evidence)
                process.kill.assert_not_called()


    def test_current_readonly_schema19_observes_empty_host_table_and_rejects_old_or_future(self):
        for schema in (18, 20, 19):
            self.database(schema=schema)
            path = self.root / 'baton.sqlite3'
            with self.subTest(schema=schema):
                if schema == 19:
                    before = path.read_bytes()
                    observed = TOOLS.observe_database(self.root)
                    self.assertEqual(observed['schema'], 19)
                    self.assertEqual(len(observed['zero_tables']), 17)
                    self.assertEqual(observed['zero_tables']['host_registrations'], 0)
                    self.assertIs(observed['query_only'], True)
                    self.assertEqual(path.read_bytes(), before)
                else:
                    with self.assertRaisesRegex(ValueError, 'exact schema19'):
                        TOOLS.observe_database(self.root)
                path.unlink()

    def test_current_host_or_tasks_must_be_present_and_empty_and_no_zero_sequence_events(self):
        for table in ('host_registrations', 'tasks'):
            for defect in ('missing', 'populated'):
                with self.subTest(table=table, defect=defect):
                    self.database(schema=19, **{defect: table})
                    with self.assertRaises((sqlite3.OperationalError, ValueError)):
                        TOOLS.observe_database(self.root)
                    (self.root / 'baton.sqlite3').unlink()
        self.database(schema=19)
        with sqlite3.connect(self.root / 'baton.sqlite3') as conn:
            conn.execute('INSERT INTO events VALUES(0)')
        with self.assertRaisesRegex(ValueError, 'unexpectedly created'):
            TOOLS.observe_database(self.root)

    def test_readonly_database_replacement_even_with_identical_bytes_refused(self):
        self.database(schema=19)
        original = self.root / 'baton.sqlite3'
        replacement = self.root / 'replacement'
        replacement.write_bytes(original.read_bytes())
        connect = sqlite3.connect

        def replace_then_open(*args, **kwargs):
            replacement.replace(original)
            return connect(*args, **kwargs)

        with mock.patch.object(TOOLS.sqlite3, 'connect', side_effect=replace_then_open):
            with self.assertRaisesRegex(ValueError, 'database identity changed'):
                TOOLS.observe_database(self.root)

    def test_mismatched_cli_and_actual_readonly_database_refuses_after_owned_stop(self):
        self.database(schema=19)
        (self.root / 'batond.sock').touch()
        for key, value in (('database_schema_version', 18), ('database_schema_version', 19.0),
                           ('api_version', True), ('api_version', 2), ('sequence', 1), ('sequence', False)):
            with self.subTest(key=key, value=value):
                evidence = self.root / f'case-{key}-{value}'
                evidence.mkdir()
                process = mock.Mock(pid=999999999)
                process.poll.return_value = None
                process.wait.return_value = 0
                result = {'tasks': [], 'sequence': 0, 'api_version': 1, 'database_schema_version': 19, key: value}
                cli = subprocess.CompletedProcess([], 0, json.dumps({'result': result}).encode(), b'')
                with mock.patch.object(TOOLS.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(str(self.root))), mock.patch.object(TOOLS.subprocess, 'Popen', return_value=process), mock.patch.object(TOOLS.subprocess, 'run', return_value=cli):
                    with self.assertRaises(ValueError):
                        TOOLS.probe(self.source, evidence)
                process.terminate.assert_called_once_with()
                process.wait.assert_called_once_with(timeout=5)
                process.kill.assert_not_called()

    def test_second_wait_timeout_preserves_unknown_exit(self):
        runtime = self.root / 'unreaped'
        runtime.mkdir()
        (runtime / 'batond.sock').touch()
        self.evidence.mkdir()
        process = mock.Mock(pid=999999999)
        process.poll.return_value = None
        process.wait.side_effect = subprocess.TimeoutExpired('owned-child', 5)
        result = {'tasks': [], 'sequence': 0, 'api_version': 1, 'database_schema_version': 19}
        cli = subprocess.CompletedProcess([], 0, json.dumps({'result': result}).encode(), b'')
        with mock.patch.object(TOOLS.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(str(runtime))), mock.patch.object(TOOLS.subprocess, 'Popen', return_value=process), mock.patch.object(TOOLS.subprocess, 'run', return_value=cli), mock.patch.object(TOOLS, 'observe_database', return_value=CURRENT_PROBE):
            with self.assertRaises(subprocess.TimeoutExpired):
                TOOLS.probe(self.source, self.evidence)
        self.assertIsNone(json.loads((self.evidence / 'probe-cleanup.json').read_text())['wait_exit'])
        process.kill.assert_called_once_with()

    def test_prepare_rejects_same_byte_source_or_frozen_replacement(self):
        for directory in (self.source, self.output):
            output = self.root / ('frozen' if directory == self.output else 'source-swap-tools')
            evidence = self.root / (output.name + '-evidence')

            def replace_after_probe(*args):
                candidate = directory / 'baton'
                replacement = self.root / 'replacement'
                replacement.write_bytes(candidate.read_bytes())
                replacement.chmod(candidate.stat().st_mode & 0o777)
                replacement.replace(candidate)
                return CURRENT_PROBE

            with self.subTest(directory=directory), mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY), mock.patch.object(TOOLS, 'probe', side_effect=replace_after_probe):
                with self.assertRaisesRegex(ValueError, 'identity changed'):
                    TOOLS.prepare(ROOT, self.source, output, evidence)
                self.assertFalse((output / 'manifest.json').exists())

    def test_current_verify_rejects_legacy_and_incomplete_or_wrong_current_probe_and_pair(self):
        prepared = self.prepare()
        path = self.output / 'manifest.json'
        with mock.patch.object(TOOLS, 'source_identity', return_value=IDENTITY):
            self.assertEqual(TOOLS.verify(ROOT, self.output), prepared)
            for key, value in [
                ('schema', 18), ('schema', 19.0), ('schema', 20),
                ('probe', {'schema': 18}), ('probe', {**CURRENT_PROBE, 'sigterm_sent': False}),
                ('probe', {**CURRENT_PROBE, 'forced_kill': True}),
                ('probe', {**CURRENT_PROBE, 'snapshot_api_version': True}),
                ('probe', {**CURRENT_PROBE, 'original_daemon_SIGTERM_wait_exit': None}),
                ('probe', {**CURRENT_PROBE, 'snapshot_database_schema_version': 18}),
                ('probe', {**CURRENT_PROBE, 'zero_tables': {**CURRENT_PROBE['zero_tables'], 'host_registrations': False}}),
                ('source_binaries_after', {'baton': prepared['binaries']['baton']}),
                ('frozen_binaries_after', {**prepared['binaries'], 'baton': prepared['binaries']['batond']}),
            ]:
                with self.subTest(key=key, value=value):
                    path.write_text(json.dumps({**prepared, key: value}))
                    with self.assertRaises(ValueError):
                        TOOLS.verify(ROOT, self.output)
            path.write_text(json.dumps(prepared))
            baton = self.output / 'baton'
            replacement = self.root / 'replacement'
            replacement.write_bytes(baton.read_bytes())
            replacement.chmod(0o500)
            replacement.replace(baton)
            with self.assertRaisesRegex(ValueError, 'entry identity changed'):
                TOOLS.verify(ROOT, self.output)


if __name__ == '__main__':
    unittest.main()
