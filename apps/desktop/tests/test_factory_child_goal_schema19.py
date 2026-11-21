"""Current Child Goal source, typed schema and finite evidence controls."""
from contextlib import closing
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from tests.infrastructure.host19_child_goal_source_fixture import (
    LAYERS, restore_child_goal_before_schema19, sha,
)

DRIVER = Path(__file__).resolve().with_name('factory-child-goal-real-ipc.py')
REPO = DRIVER.parents[3]
SPEC = importlib.util.spec_from_file_location('_child_goal_schema19_controls', DRIVER)
G = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = G
SPEC.loader.exec_module(G)
P = G.finite_projection
HEAD, TREE = 'a' * 40, 'b' * 40
PRIVATE = 'PRIVATE_FIXTURE_TOKEN_PATH_AND_RAW_WIRE'
HARNESS_SHA, PROJECTOR_SHA = P.sha(DRIVER.read_bytes()), P.sha(G.PROJECTOR_SOURCE)


class CurrentChildGoal(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.private = self.root / 'private'
        self.private.mkdir(mode=0o700)
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        for name in ('batond', 'baton'):
            tool = self.tools / name
            tool.write_bytes(name.encode())
            tool.chmod(0o500)
        self.frontend = self.root / 'frontend'
        self.frontend.mkdir()
        self.asset = self.frontend / 'index.html'
        self.asset.write_bytes(b'fixed frontend')
        self.manifest = self.root / 'manifest.json'
        self.candidate = {'schema': 19, 'git_head': HEAD, 'git_tree': TREE, 'source_head': HEAD,
            'compiled_core_source_head': HEAD, 'frontend_source_head': HEAD, 'dirty': False, 'tracked_clean': True,
            'binaries': {name: P.sha((self.tools / name).read_bytes()) for name in ('batond', 'baton')},
            'frontend_assets': {'index.html': P.sha(self.asset.read_bytes())}}
        self.write_manifest()
        self.args = SimpleNamespace(tools=self.tools, manifest=self.manifest, frontend=self.frontend)

    def write_manifest(self, **changes):
        self.manifest.write_text(json.dumps({**self.candidate, **changes}))

    def write_private(self, name, raw):
        path = self.private / name
        path.write_bytes(raw)
        path.chmod(0o600)

    def prepare(self):
        is_file = Path.is_file
        with patch.object(sys, 'platform', 'linux'), patch.object(G.support, 'graph_source', return_value=(HEAD, TREE)), \
                patch.object(Path, 'is_file', lambda path: path == Path('/proc/self/stat') or is_file(path)):
            return G.current_child_goal_preflight(self.args, self.private)

    def test_candidate_pin_and_preflight_schema_types(self):
        prepared, free, tool_bytes = self.prepare()
        self.assertEqual(prepared.manifest, self.candidate)
        self.assertEqual((prepared.source_head, prepared.source_tree), (HEAD, TREE))
        self.assertEqual(tool_bytes, len(b'batondbaton'))
        self.assertGreaterEqual(free, 232 * 1024 * 1024 + tool_bytes)
        for schema in (18, 20, 19.0, True, False, '19'):
            self.write_manifest(schema=schema)
            with self.subTest(schema=schema), patch.object(G.base, 'factory_tools_pins') as tools:
                with self.assertRaises((G.Refusal, P.Refused)):
                    self.prepare()
                tools.assert_not_called()
                with self.assertRaises(P.Refused):
                    P.current_candidate_summary(self.manifest.read_bytes(), HEAD, TREE)

    def test_preflight_wrong_source_tool_asset_or_projector(self):
        for field in ('git_head', 'source_head', 'compiled_core_source_head', 'frontend_source_head', 'git_tree'):
            self.write_manifest(**{field: 'c' * 40})
            with self.subTest(field=field), self.assertRaises((G.Refusal, P.Refused)):
                self.prepare()
        self.write_manifest()
        with patch.object(G, 'PROJECTOR_SOURCE', b'other projector'), self.assertRaises(G.Refusal):
            self.prepare()
        self.asset.write_text('other frontend')
        with self.assertRaisesRegex(G.Refusal, 'frontend assets'):
            self.prepare()
        self.asset.write_bytes(b'fixed frontend')
        tool = self.tools / 'baton'
        tool.chmod(0o700)
        tool.write_bytes(b'other executable')
        with self.assertRaisesRegex(G.Refusal, 'tool digest mismatch'):
            self.prepare()

    def bind_snapshot(self, report, schema=19):
        reply = json.dumps({'result': {'api_version': 1, 'database_schema_version': schema},
            'request_id': PRIVATE}).encode() + b'\n'
        self.write_private('wire-reply-0.json', reply)
        report['observations']['schema_evidence'] = {'manifest_schema': 19, 'ipc_api_version': 1,
            'ipc_database_schema_version': 19, 'sqlite_user_version': 19, 'snapshot_reply_sha256': P.sha(reply)}
        report['actual_core_wires'] = [{'method': 'system.snapshot', 'sequence': 0, 'params': {},
            'key': None, 'revision': None, 'capture_state': 'captured', 'capture_complete': True,
            'bytes': len(reply), 'sha256': P.sha(reply)}]

    def bind_database(self, report, case, schema=19, mixed=False):
        path = self.private / 'runtime.raw.sqlite3'
        path.unlink(missing_ok=True)
        sessions = [f'generated-{i}' for i in range(2 if case == 'cancel' else 3)]
        with closing(sqlite3.connect(path)) as db, db:
            db.execute(f'PRAGMA user_version={schema}')
            db.execute('CREATE TABLE sessions (document TEXT)')
            for name in sessions:
                db.execute('INSERT INTO sessions VALUES (?)', (json.dumps({
                    'id': name + '-other' if mixed else name, 'state': 'stopped', 'private': PRIVATE,
                }),))
        path.chmod(0o600)
        raw = path.read_bytes()
        report['observations']['retained_generated_private_database'] = {'bytes': len(raw),
            'sha256': P.sha(raw), 'mode': '0o600', 'contains_only_generated_test_resources': True}
        return sessions

    def report(self, case):
        report = {'status': 'passed', 'schema_required': 19, 'case': case,
            'source_head': HEAD, 'git_tree': TREE, 'candidate': deepcopy(self.candidate),
            'manifest_sha256': P.sha(self.manifest.read_bytes()), 'harness_sha256': HARNESS_SHA,
            'projector_sha256': PROJECTOR_SHA, 'frontend_assets': deepcopy(self.candidate['frontend_assets']),
            'finite_diagnostics': P.FiniteRuntimeDiagnosticsV5().snapshot(),
            'checks': [{'id': item, 'status': 'passed', 'evidence': PRIVATE} for item in sorted(P.EXPECTED[case])],
            'observations': {'final_snapshot': {'schema': 19}, 'final_diagnostics': {'complete': True, 'schema': 19}},
            'cleanup_complete': True, 'http_unexpected_errors': [],
            'screenshots': [PRIVATE], 'private_owner_reviews': [{'token': PRIVATE}]}
        self.bind_snapshot(report)
        sessions = self.bind_database(report, case)
        records = len(sessions) - 1
        identities = [{'role': role, 'expected': {'pid': 5 + 2*i + j, 'start_ticks': '100'},
            'original_identity_gone': True, 'signal_sent': False} for i in range(records)
            for j, role in enumerate(('owned_provider', 'owned_companion'))]
        report['cleanup'] = [
            {'action': 'browser_and_playwright_closed', 'confirmed': True},
            {'action': 'frozen_inputs_rechecked', 'confirmed': True},
            {'action': 'private_fixture_directory_cleanup_after_all_confirmed', 'confirmed': True},
            {'action': 'HTTP_serve_and_handler_join', 'confirmed': True, 'active': {}},
            {'action': 'actual_private_daemon_wait', 'pid': 2, 'exitcode': 0},
            {'action': 'fixed previously verified generated identities inspected', 'complete': True,
                'workflow_successful_before_cleanup': True, 'required_records_for_success': records,
                'actual_bound_records': records, 'verified_records': records, 'required_identities_for_success': records*2,
                'actual_bound_identities': records*2, 'inspected_identities': records*2, 'observations': identities},
            {'action': 'bound_Graph_provider_identity_gone', 'expected': {'pid': 20, 'start_ticks': '150'},
                'confirmed': True, 'signal_sent': False},
            {'action': 'all_persisted_sessions_match_bound_stopped_processes', 'confirmed': True, 'session_ids': sessions}]
        pins, copies = {}, {}
        for name, digest in self.candidate['binaries'].items():
            pins[name] = {'bytes': len(name), 'sha256': digest}
            copies[name] = {'original_sha256': digest, 'original_after_sha256': digest, 'copy_sha256': digest,
                'original_bytes': len(name), 'installed_sha256': digest, 'installed_bytes': len(name)}
        report['frozen_tool_pins'] = pins
        report['private_installation'] = {'tools': copies, 'installed_file_pins': deepcopy(pins), 'strip': {'exitcode': 0}}
        return report

    def publish(self, report, exit_code='0'):
        self.write_private('report.json', json.dumps(report).encode())
        output = Path(tempfile.mkdtemp(dir=self.root)) / 'out'
        args = ['projector', '--private-dir', str(self.private), '--output', str(output),
            '--manifest', str(self.manifest), '--case', report['case'], f'--exit-code={exit_code}']
        with patch.object(P, 'current_checkout', return_value=(HEAD, TREE)), patch.object(sys, 'argv', args):
            result = P.main()
        raw = (output / 'summary.json').read_text()
        self.assertNotIn(PRIVATE, raw)
        self.assertNotIn(str(self.private), raw)
        return result, json.loads(raw)

    def test_all_cases_restrict_output_and_require_actual_return(self):
        for case in P.CASES:
            with self.subTest(case=case):
                report = self.report(case)
                code, summary = self.publish(report)
                self.assertEqual(code, 0)
                self.assertEqual(summary['status'], 'passed')
                self.assertEqual(summary['schema'], 19)
                self.assertEqual(summary['git_tree'], TREE)
                self.assertEqual({row['id'] for row in summary['checks']}, P.EXPECTED[case])
                for unexpected in ('1', '-9', 'unobserved', '256', '19.0', PRIVATE):
                    code, summary = self.publish(report, unexpected)
                    self.assertEqual((code, summary['status']), (1, 'failed'))
                    self.assertEqual(summary['failure_kind'],
                        'runtime_failed' if unexpected in ('1', '-9', 'unobserved') else 'runtime_exit_invalid')

    def test_source_type_digest_and_cleanup_mismatches_are_finite(self):
        report = self.report('normal')
        variants = [
            (('schema_required',), 19.0), (('source_head',), '0' * 40), (('git_tree',), '0' * 40),
            (('projector_sha256',), '0' * 64), (('harness_sha256',), '0' * 64), (('manifest_sha256',), '0' * 64),
            (('frontend_assets', 'index.html'), '0' * 64),
            (('private_installation', 'tools', 'baton', 'original_sha256'), '0' * 64),
            (('frozen_tool_pins', 'batond', 'bytes'), True),
            (('actual_core_wires', 0, 'sequence'), False), (('actual_core_wires', 0, 'sha256'), '0' * 64),
            (('observations', 'schema_evidence', 'ipc_api_version'), True),
            (('observations', 'final_diagnostics', 'schema'), '19'),
            (('cleanup', 0, 'confirmed'), 1), (('cleanup', 4, 'exitcode'), False),
            (('cleanup', 5, 'inspected_identities'), 4.0), (('cleanup', 6, 'signal_sent'), True),
            (('cleanup', 7, 'session_ids'), ['one', 'two', 'two'])]
        for path, replacement in variants:
            changed = deepcopy(report)
            parent = changed
            for part in path[:-1]:
                parent = parent[part]
            parent[path[-1]] = replacement
            with self.subTest(path=path):
                code, summary = self.publish(changed)
                self.assertEqual((code, summary['failure_kind']), (1, 'report_invalid'))

    def test_ipc_and_retained_readonly_database_cannot_be_relabelled(self):
        report = self.report('cancel')
        for schema in (18, 19.0, 20, True, '19'):
            self.bind_snapshot(report, schema)
            with self.subTest(ipc_schema=schema):
                self.assertEqual(self.publish(report)[1]['failure_kind'], 'report_invalid')
        self.bind_snapshot(report)
        for schema, mixed in ((18, False), (20, False), (19, True)):
            self.bind_database(report, 'cancel', schema=schema, mixed=mixed)
            with self.subTest(db_schema=schema, mixed_sessions=mixed):
                self.assertEqual(self.publish(report)[1]['failure_kind'], 'report_invalid')

    def test_current_failure_keeps_closed_set_diagnostics(self):
        report = self.report('cancel')
        diagnostics = P.FiniteRuntimeDiagnosticsV5()
        diagnostics.failure(OSError(5, PRIVATE), 'runtime', str(DRIVER), G.Refusal)
        report.update(status='failed_or_interrupted', cleanup_complete=False,
            failure={'exception': PRIVATE}, finite_diagnostics=diagnostics.snapshot())
        report.pop('private_installation')
        code, summary = self.publish(report, '1')
        self.assertEqual((code, summary['failure_kind']), (1, 'runtime_failed'))
        self.assertEqual(summary['finite_diagnostics']['failure_class'], 'os_error')

    def test_historical_outer_inverse_checks_every_byte_and_rejects_tampering(self):
        for path, (current_pin, historical_pin, _) in LAYERS.items():
            source = (REPO / path).read_text()
            with self.subTest(source=path):
                self.assertEqual(sha(source), current_pin)
                restored = restore_child_goal_before_schema19(path, source)
                self.assertEqual(sha(restored), historical_pin)
                self.assertEqual(restore_child_goal_before_schema19(path, restored), restored)
                for unknown in (source + '# unreviewed\n', source[1:]):
                    with self.assertRaises(AssertionError):
                        restore_child_goal_before_schema19(path, unknown)


if __name__ == '__main__':
    unittest.main()
