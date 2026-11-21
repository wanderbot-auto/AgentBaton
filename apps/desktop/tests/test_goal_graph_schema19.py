"""Candidate, startup and read-only schema controls for the current Graph fixture."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


SOURCE = Path(__file__).with_name('goal-graph-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('_current_graph_schema19', SOURCE)
G = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = G
SPEC.loader.exec_module(G)
HEAD, TREE = 'a' * 40, 'b' * 40
BINARY = b'inert test-only candidate bytes'
DIGEST = G.sha(BINARY)


def candidate(**changes):
    return {'schema': 19, 'git_head': HEAD, 'git_tree': TREE, 'dirty': False, 'binaries': {'batond': DIGEST}, **changes}


def raw(value):
    return json.dumps(value).encode()


class FrozenGraphCandidate(unittest.TestCase):
    def test_workflow_manifest_and_matching_source_aliases(self):
        for manifest in (candidate(), candidate(source_head=HEAD, compiled_core_source_head=HEAD,
                tracked_clean=True, binaries={'batond': {'sha256': DIGEST}})):
            with self.subTest(manifest=manifest):
                binary = Mock()
                binary.read_bytes.return_value = BINARY
                self.assertEqual(G.verify_graph_candidate(binary, raw(manifest), HEAD, TREE), (manifest, DIGEST))
                binary.read_bytes.assert_called_once_with()

    def test_wrong_manifest_schema_type_source_or_dirtiness_refused_before_binary_read(self):
        invalid = [candidate(schema=schema) for schema in (17, 18, 20, True, False, 19.0, '19', None)]
        invalid += [candidate(**values) for values in (
            {'git_head': 'f' * 40}, {'git_tree': 'f' * 40}, {'source_head': 'f' * 40},
            {'compiled_core_source_head': 'f' * 40}, {'dirty': True}, {'dirty': 0},
            {'tracked_clean': False}, {'tracked_clean': 1}, {'binaries': []}, {'binaries': {'batond': True}},
        )]
        for manifest in invalid + [{}, [], 19]:
            with self.subTest(manifest=manifest):
                binary = Mock()
                with self.assertRaises(G.BoundaryRefusal):
                    G.verify_graph_candidate(binary, raw(manifest), HEAD, TREE)
                binary.read_bytes.assert_not_called()

    def test_duplicate_schema_and_binary_substitution_refused(self):
        binary = Mock()
        with self.assertRaisesRegex(G.BoundaryRefusal, 'Duplicate'):
            G.verify_graph_candidate(binary, b'{"schema":18,"schema":19}', HEAD, TREE)
        binary.read_bytes.assert_not_called()
        binary.read_bytes.return_value = BINARY + b' substituted'
        with self.assertRaisesRegex(G.BoundaryRefusal, 'daemon changed'):
            G.verify_graph_candidate(binary, raw(candidate()), HEAD, TREE)

    def test_startup_checks_tracked_source_before_attributing_commit_and_tree(self):
        with patch.object(G.subprocess, 'check_output', return_value=b'M modified') as git:
            with self.assertRaisesRegex(G.BoundaryRefusal, 'tracked changes'):
                G.graph_source()
            git.assert_called_once()
        with patch.object(G.subprocess, 'check_output', side_effect=[b'', HEAD + '\n' + TREE + '\n']):
            self.assertEqual(G.graph_source(), (HEAD, TREE))

    def test_all_explicit_modes_refuse_old_candidate_before_process_token_or_listener(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary, manifest = root / 'batond', root / 'candidate.json'
            binary.write_bytes(BINARY)
            manifest.write_bytes(raw(candidate(schema=18)))
            for index, case in enumerate((None, *G.VALIDATION_CASES)):
                with self.subTest(case=case), patch.object(G, 'graph_source', return_value=(HEAD, TREE)), \
                        patch.object(G.subprocess, 'Popen') as launch, patch.object(G, 'PrivateUnixFixture') as host, \
                        patch.object(G.http.server, 'ThreadingHTTPServer') as server, patch('builtins.print'):
                    out = root / str(index)
                    out.mkdir()
                    args = SimpleNamespace(validation_case=case, batond=binary, candidate_manifest=manifest, frontend=root)
                    self.assertEqual(G.owner_only_driver(args, out), 2)
                    report = json.loads((out / 'preflight-refusal.json').read_bytes())
                    self.assertEqual(set(p.name for p in out.iterdir()), {'preflight-refusal.json'})
                    self.assertEqual((report['status'], report['actual_runtime'], report['schema_required']),
                        ('preflight_refused', 'not_run', 19))
                    self.assertIs(report['no_process_token_read_or_listener'], True)
                    launch.assert_not_called()
                    host.assert_not_called()
                    server.assert_not_called()


class GraphRunResult(unittest.TestCase):
    def test_final_input_check_keeps_git_identity_distinct_from_private_worktree(self):
        with tempfile.TemporaryDirectory() as temp:
            fixture_path = Path(temp) / 'source'
            fixture_path.mkdir()
            manifest = fixture_path / 'manifest.json'
            manifest.write_bytes(raw(candidate()))
            binary = fixture_path / 'batond'
            binary.write_bytes(BINARY)
            report = {'source': HEAD, 'git_tree': TREE, 'daemon_sha256': DIGEST,
                'manifest_sha256': G.sha(manifest.read_bytes()), 'harness_sha256': G.sha(SOURCE.read_bytes())}

            with patch.object(G, 'graph_source', return_value=(HEAD, TREE)):
                G.verify_graph_run_inputs(report, manifest, binary)
                with self.assertRaisesRegex(G.BoundaryRefusal, 'inputs changed'):
                    G.verify_graph_run_inputs({**report, 'source': fixture_path}, manifest, binary)
                binary.write_bytes(BINARY + b' changed')
                with self.assertRaisesRegex(G.BoundaryRefusal, 'inputs changed'):
                    G.verify_graph_run_inputs(report, manifest, binary)
            binary.write_bytes(BINARY)
            with patch.object(G, 'graph_source', return_value=('f' * 40, TREE)):
                with self.assertRaisesRegex(G.BoundaryRefusal, 'inputs changed'):
                    G.verify_graph_run_inputs(report, manifest, binary)

    def test_close_exception_revokes_a_previously_successful_run(self):
        report = {'status': 'passed'}
        def clean(value):
            return str(value).replace('generated-secret', '[redacted]')
        secret = 'generated-secret'
        try:
            raise RuntimeError('browser close failed with ' + secret)
        except RuntimeError as error:
            G.record_graph_failure(report, error, clean)
        self.assertEqual(report['status'], 'failed_or_interrupted')
        self.assertEqual(report['failure']['type'], 'RuntimeError')
        self.assertIn('browser close failed with [redacted]', report['failure']['message'])
        self.assertNotIn('generated-secret', report['failure']['traceback'])


class PrivateGraphSchema(unittest.TestCase):
    def setUp(self):
        runtime = tempfile.TemporaryDirectory()
        self.addCleanup(runtime.cleanup)
        self.runtime = Path(runtime.name)
        self.db = sqlite3.connect(self.runtime / 'baton.sqlite3')
        self.addCleanup(self.db.close)
        self.db.execute('PRAGMA user_version=19')
        self.host = Mock()

    def observe(self, snapshot):
        self.host.call.return_value = {'result': snapshot}
        return G.graph_schema_evidence(self.host, self.runtime)

    def test_three_finite_runtime_fields_from_snapshot_and_readonly_sql(self):
        evidence = self.observe({'api_version': 1, 'database_schema_version': 19, 'private': 'omitted'})
        self.assertEqual(evidence, {'ipc_api_version': 1, 'ipc_database_schema_version': 19, 'sqlite_user_version': 19})
        self.host.call.assert_called_once_with(G.FixedCall('system.snapshot', {}))
        with G.graph_database(self.runtime) as db:
            self.assertEqual(db.execute('PRAGMA query_only').fetchone(), (1,))
            db.execute('PRAGMA query_only=OFF')
            with self.assertRaisesRegex(sqlite3.OperationalError, 'readonly'):
                db.execute('PRAGMA user_version=20')
        self.assertEqual(self.db.execute('PRAGMA user_version').fetchone(), (19,))

    def test_untrusted_snapshot_replies_refuse_before_sqlite(self):
        for version in (18, 20, True, False, 19.0, '19', None):
            with self.subTest(version=version), patch.object(G, 'graph_database') as db:
                with self.assertRaisesRegex(G.BoundaryRefusal, 'snapshot'):
                    self.observe({'api_version': 1, 'database_schema_version': version})
                db.assert_not_called()
        for api in (True, 1.0, 2, '1', None):
            with self.subTest(api=api), self.assertRaises(G.BoundaryRefusal):
                self.observe({'api_version': api, 'database_schema_version': 19})
        self.host.call.return_value = {'result': {'api_version': 1, 'database_schema_version': 19}, 'error': {}}
        with self.assertRaises(G.BoundaryRefusal):
            G.graph_schema_evidence(self.host, self.runtime)

    def test_correct_snapshot_cannot_mask_wrong_real_sqlite_schema(self):
        for version in (0, 18, 20):
            self.db.execute(f'PRAGMA user_version={version}')
            with self.subTest(version=version), self.assertRaisesRegex(G.BoundaryRefusal, 'Read-only Graph database'):
                self.observe({'api_version': 1, 'database_schema_version': 19})


if __name__ == '__main__':
    unittest.main()
