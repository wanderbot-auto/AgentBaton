"""Finite18-input evidence rejection controls. No19 migration/native claim."""
import copy
from contextlib import closing
import importlib.util
import json
import os
from pathlib import Path
import platform
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('actual18_evidence_controls', ROOT / 'scripts/generate-schema18-migration-fixture.py')
G = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(G)
H = G.H


def synthetic_report():
    first = dict(schema=18, query_only=True, foreign_key_errors=0,
                 counts={name: 0 for name in H.SCHEMA_TABLES}, sequence=0,
                 logical_sha256='a' * 64, schema_sha256='b' * 64)
    last = copy.deepcopy(first)
    last['counts'].update(projects=1, tasks=3, events=5, idempotency=4, sqlite_sequence=1)
    last['sequence'] = 5
    tools = {name: dict(sha256='c' * 64) for name in ('batond', 'baton')}
    return dict(contract=G.CONTRACT, status='passed', schema=18, source_commit=H.SOURCE,
        source_tree=H.TREE, origin='normal_accepted18_production_rpc', platform=sys.platform,
        architecture=platform.machine(), generator_sha256=H.sha(Path(G.__file__).read_bytes()),
        helper_sha256=H.sha(Path(H.__file__).read_bytes()), helpers_sha256=H.HELPERS.copy(),
        actual_cli_probe=dict(exit_code=0, api_version=1, database_schema_version=18),
        owner_rejection='AUTH_REQUIRED', conflict_rejection='CONFIG_CONFLICT', receipt_replay_exact=True,
        daemon=dict(sigterm_sent=True, forced_kill=False, daemon_wait_exit=0,
                    identity_gone_after_join=True, group_quiescent_before_reaping=True,
                    leader_unreaped_until_signalling=True, cleanup_errors=[]),
        initial_probe=first, final_probe=last, final_api_sequence=5,
        backup_logical_exact=True, source_content_unchanged=True, linked_journal_verified=True,
        tool_metadata=tools, tool_metadata_after=copy.deepcopy(tools),
        binaries={name: value['sha256'] for name, value in tools.items()},
        database_sha256='d' * 64, database_bytes=4096, tool_provenance_sha256=H.sha(b'synthetic build'),
        rpc_transcript_sha256=H.sha(b'synthetic RPC'))


class Genuine18EvidenceControls(unittest.TestCase):
    def test_synthetic_metadata_is_only_finite_contract_not_real_evidence(self):
        with mock.patch.object(G.subprocess, 'Popen', side_effect=AssertionError('no product')):
            G.validate_report(synthetic_report())
        # There is deliberately no Rust migration consumer or19 output contract.
        self.assertEqual(G.CONTRACT, 'agentbaton.actual-project-task-schema18.v1')

    def test_wrong_source_schema_protocol_helpers_and_unconfirmed_outcomes_refuse(self):
        original = synthetic_report()
        cases = [(('schema',), value) for value in [16, 17, 19, 20, 18.0, True, '18', None]] + [
            (('source_commit',), 'a' * 40), (('source_tree',), 'b' * 40),
            (('origin',), 'schema_stamped19'), (('generator_sha256',), 'e' * 64),
            (('helper_sha256',), 'f' * 64), (('helpers_sha256',), {}),
            (('actual_cli_probe', 'api_version'), True),
            (('actual_cli_probe', 'database_schema_version'), 19),
            (('owner_rejection',), None), (('conflict_rejection',), None),
            (('receipt_replay_exact',), 1), (('source_content_unchanged',), False),
            (('backup_logical_exact',), False), (('linked_journal_verified',), False),
            (('daemon', 'forced_kill'), True), (('daemon', 'daemon_wait_exit'), False),
            (('daemon', 'identity_gone_after_join'), False), (('daemon', 'cleanup_errors'), ['unknown']),
            (('initial_probe', 'sequence'), False),
            (('final_probe', 'sequence'), 5.0), (('final_api_sequence',), 5.0),
            (('final_probe', 'sequence'), True), (('final_api_sequence',), True)]
        for path, bad in cases:
            value = copy.deepcopy(original)
            target = value
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = bad
            with self.subTest(path=path, bad=bad), self.assertRaises(ValueError):
                G.validate_report(value)

    def test_missing_extra_boolean_counts_unexpected_authority_and_tools_refuse(self):
        for mode in ['missing', 'extra', 'boolean', 'float', 'host', 'authority', 'tools']:
            value = synthetic_report()
            counts = value['final_probe']['counts']
            if mode == 'missing':
                counts.pop('events')
            elif mode == 'extra':
                counts['unexpected'] = 0
            elif mode == 'host':
                counts['host_registrations'] = 0
            elif mode == 'boolean':
                counts['connections'] = False
            elif mode == 'float':
                counts['connections'] = 0.0
            elif mode == 'authority':
                counts['execution_grants'] = 1
            else:
                value['tool_metadata_after']['baton']['sha256'] = 'e' * 64
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                G.validate_report(value)

    def test_sqlite_read_probe_is_inert_and_refuses19_stamped_or_corrupt_disposable_shapes(self):
        # These controlled empty shapes test the reader only. They are NOT
        # generated predecessors, tool evidence or successful lifecycle data.
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve() / 'synthetic.sqlite3'
            with closing(sqlite3.connect(path)) as db:
                for table in H.SCHEMA_TABLES:
                    if table == 'sqlite_sequence':
                        continue
                    db.execute('CREATE TABLE ' + table + ('(sequence INTEGER PRIMARY KEY AUTOINCREMENT)' if table == 'events' else '(id INTEGER)'))
                db.execute('PRAGMA user_version=18')
            path.chmod(0o600)
            before = path.read_bytes()
            opened = []
            connect = sqlite3.connect
            def tracked_connect(*args, **kwargs):
                connection = connect(*args, **kwargs)
                opened.append(connection)
                return connection
            with mock.patch.object(G.subprocess, 'Popen', side_effect=AssertionError('no daemon')), \
                 mock.patch.object(G.sqlite3, 'connect', side_effect=tracked_connect):
                self.assertEqual(G.probe(path)['schema'], 18)
            for connection in opened:
                with self.assertRaises(sqlite3.ProgrammingError):
                    connection.execute('SELECT 1')
            self.assertEqual(path.read_bytes(), before)
            for schema in [17, 19, 20]:
                with closing(sqlite3.connect(path)) as db:
                    db.execute('PRAGMA user_version=' + str(schema))
                before = path.read_bytes()
                with self.assertRaises(ValueError):
                    G.probe(path)
                self.assertEqual(path.read_bytes(), before)
            path.write_bytes(b'corrupt disposable bytes')
            with self.assertRaises(sqlite3.DatabaseError):
                G.probe(path)

    def test_identity_sampling_failure_records_failed_daemon_and_reaps_direct_child(self):
        class Child:
            pid = 4321002
            killed = False
            joined = False
            def kill(self):
                self.killed = True
            def wait(self, timeout=None):
                self.joined = True
                return -9
        child = Child()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            output = root / 'output'
            frozen = root / 'frozen'
            frozen.mkdir()
            argv = ['generator', '--frozen', str(frozen), '--build-manifest', str(frozen / 'source-build-manifest.json'), '--output', str(output)]
            with mock.patch.object(G.sys, 'argv', argv), \
                 mock.patch.object(H, 'provenance', return_value=({}, b'synthetic build', {})), \
                 mock.patch.object(G.subprocess, 'Popen', return_value=child), \
                 mock.patch.object(H.COMMON, 'darwin_owned_identity', side_effect=ValueError('identity unavailable')), \
                 mock.patch.object(H.COMMON.os, 'killpg', side_effect=AssertionError('unknown group signal')) as group:
                with self.assertRaises(ValueError):
                    G.main()
            self.assertTrue(child.killed)
            self.assertTrue(child.joined)
            value = json.loads((output / 'manifest.json').read_text())
            self.assertEqual(value['status'], 'failed_or_interrupted')
            self.assertTrue(value['daemon']['forced_kill'])
            self.assertFalse(value['daemon']['identity_gone_after_join'])
            group.assert_not_called()

    def test_tampered_database_rejects_before_sqlite_or_process(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            record = synthetic_report()
            for name, raw in [('manifest.json', json.dumps(record).encode()),
                              ('tool-provenance.json', b'synthetic build'),
                              ('rpc-private.jsonl', b'synthetic RPC'),
                              ('actual-schema18.sqlite3', b'changed DB')]:
                path = output / name
                path.write_bytes(raw)
                path.chmod(0o600)
            with mock.patch.object(H, 'provenance', return_value=({}, b'synthetic build', record['tool_metadata'])), \
                 mock.patch.object(G, 'probe', side_effect=AssertionError('must reject bytes first')), \
                 mock.patch.object(G.subprocess, 'Popen', side_effect=AssertionError('no daemon')):
                with self.assertRaisesRegex(ValueError, 'predecessor bytes'):
                    G.verify_fixture(output, Path('/never'), Path('/never'))


if __name__ == '__main__':
    unittest.main()
