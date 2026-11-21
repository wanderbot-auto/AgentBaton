"""Fixed inputs, private schema and preserved Factory compatibility controls."""
import ast
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SUBJECT = Path(__file__).with_name('factory-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('_factory_schema19_controls', SUBJECT)
F = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = F
SPEC.loader.exec_module(F)
HEAD, TREE = 'a' * 40, 'b' * 40


class CurrentFactoryInputs(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.frontend = self.root / 'frontend'
        self.frontend.mkdir()
        (self.frontend / 'index.html').write_bytes(b'fixed frontend input')
        (self.frontend / 'entry.js').write_bytes(b'fixed module input')
        self.daemon, self.companion = self.tools / 'batond', self.tools / 'baton'
        for path in (self.daemon, self.companion):
            path.write_bytes(path.name.encode())
            path.chmod(0o500)
        self.candidate = self.root / 'candidate.json'
        self.manifest = {'schema': 19, 'git_head': HEAD, 'git_tree': TREE, 'compiled_core_source_head': HEAD,
            'dirty': False, 'tracked_clean': True,
            'binaries': {path.name: F.sha(path.read_bytes()) for path in (self.daemon, self.companion)},
            'frontend_assets': {path.name: F.sha(path.read_bytes()) for path in self.frontend.iterdir()}}
        self.args = SimpleNamespace(batond=self.daemon, candidate_manifest=self.candidate, frontend=self.frontend)
        self.write_manifest()
        git = patch.object(F, 'factory_current_source', return_value=(HEAD, TREE))
        self.git = git.start()
        self.addCleanup(git.stop)

    def write_manifest(self, **fields):
        self.candidate.write_text(json.dumps({**self.manifest, **fields}) + '\n')

    def prepare(self):
        is_file = Path.is_file
        with patch.object(F.sys, 'platform', 'linux'), patch.object(Path, 'is_file',
                lambda path: path == Path('/proc/self/stat') or is_file(path)):
            return F.current_factory_preflight(self.args)

    def test_exact_manifest_bindings_and_final_input_pins(self):
        prepared = self.prepare()
        self.assertEqual((prepared.source_head, prepared.source_tree), (HEAD, TREE))
        self.assertEqual(prepared.manifest, self.manifest)
        self.assertEqual(prepared.tool_paths, {'batond': self.daemon, 'baton': self.companion})
        self.assertEqual({key: pin['sha256'] for key, pin in prepared.asset_pins.items()}, self.manifest['frontend_assets'])
        F.verify_factory_run_inputs(prepared, F.sha(SUBJECT.read_bytes()))

    def test_wrong_schema_and_source_refused_before_tools_open(self):
        variants = [{'schema': value} for value in (False, True, '19', 19.0, 18, 20, None)]
        variants.extend(({'git_head': 'c' * 40}, {'git_tree': 'c' * 40}, {'source_head': 'c' * 40},
            {'compiled_core_source_head': 'c' * 40}, {'frontend_source_head': 'c' * 40}, {'dirty': 0},
            {'tracked_clean': 1}, {'binaries': {'batond': self.manifest['binaries']['batond']}}))
        for fields in variants:
            self.write_manifest(**fields)
            with self.subTest(fields=fields), patch.object(F, 'factory_tools_pins') as tools:
                with self.assertRaises(F.BoundaryRefusal):
                    self.prepare()
                tools.assert_not_called()

    def test_both_swapped_binaries_are_refused(self):
        for path in (self.daemon, self.companion):
            with self.subTest(path=path.name):
                path.chmod(0o700)
                path.write_bytes(b'wrong same-build executable')
                with self.assertRaisesRegex(F.BoundaryRefusal, 'tool digest mismatch'):
                    self.prepare()
                path.write_bytes(path.name.encode())
                path.chmod(0o500)

    def test_links_and_untrusted_tool_modes_are_refused(self):
        for mode in (0o600, 0o520, 0o4500):
            with self.subTest(mode=mode):
                self.companion.chmod(mode)
                with self.assertRaisesRegex(F.BoundaryRefusal, 'safely executable'):
                    self.prepare()
        self.companion.chmod(0o500)
        alias = self.tools / 'alias'
        alias.hardlink_to(self.companion)
        with self.assertRaisesRegex(F.BoundaryRefusal, 'single-link'):
            self.prepare()
        alias.unlink()
        self.companion.rename(alias)
        self.companion.symlink_to(alias.name)
        with self.assertRaisesRegex(F.BoundaryRefusal, 'regular file'):
            self.prepare()

    def test_full_frontend_and_companion_budget_are_enforced(self):
        (self.frontend / 'extra').write_bytes(b'unpinned')
        with self.assertRaisesRegex(F.BoundaryRefusal, 'frontend assets'):
            self.prepare()
        self.companion.chmod(0o700)
        with self.companion.open('wb') as stream:
            stream.truncate(128 * 1024 * 1024 + 1)
        with self.assertRaisesRegex(F.BoundaryRefusal, 'bounded regular file'):
            self.prepare()

    def test_source_manifest_assets_and_same_bytes_new_inode_drift(self):
        initial = self.prepare()
        harness = F.sha(SUBJECT.read_bytes())
        self.git.return_value = ('c' * 40, TREE)
        with self.assertRaisesRegex(F.BoundaryRefusal, 'source changed'):
            F.verify_factory_run_inputs(initial, harness)
        self.git.return_value = (HEAD, TREE)
        self.write_manifest(scope='changed')
        with self.assertRaisesRegex(F.BoundaryRefusal, 'manifest changed'):
            F.verify_factory_run_inputs(initial, harness)
        self.write_manifest()
        initial = self.prepare()
        asset = self.frontend / 'entry.js'
        asset.write_bytes(b'drifted contents')
        with self.assertRaisesRegex(F.BoundaryRefusal, 'frontend changed'):
            F.verify_factory_run_inputs(initial, harness)
        asset.write_bytes(b'fixed module input')
        initial = self.prepare()
        replacement = self.tools / 'new-baton'
        replacement.write_bytes(self.companion.read_bytes())
        replacement.chmod(0o500)
        replacement.replace(self.companion)
        with self.assertRaisesRegex(F.BoundaryRefusal, 'tool identity changed'):
            F.verify_factory_run_inputs(initial, harness)

    def test_cli_preflight_does_not_create_process_in_any_case(self):
        self.write_manifest(schema=18)
        for case in ('normal', 'cancel', 'unknown'):
            output = self.root / case
            args = ['factory-real-ipc.py', '--case', case, '--batond', str(self.daemon), '--frontend',
                str(self.frontend), '--candidate-manifest', str(self.candidate), '--output', str(output)]
            with self.subTest(case=case), patch.object(F.sys, 'argv', args), patch.object(F.subprocess, 'Popen') as spawn, \
                    patch.object(F, 'PrivateHost') as host, patch.object(F.http.server, 'ThreadingHTTPServer') as listener, \
                    patch('builtins.print'):
                self.assertEqual(F.main(), 2)
                spawn.assert_not_called()
                host.assert_not_called()
                listener.assert_not_called()
                self.assertEqual({path.name for path in output.iterdir()}, {'preflight-refusal.json'})
                report = json.loads((output / 'preflight-refusal.json').read_bytes())
                self.assertEqual(report['actual_runtime'], 'not_run')
                self.assertEqual(report['schema_required'], 19)
                self.assertIs(report['no_process_token_read_or_listener'], True)

    def test_legacy18_preflight_keeps_original_manifest_shape(self):
        self.write_manifest(schema=18)
        self.assertEqual(F.preflight(self.args)[:3], (self.daemon, self.companion, self.frontend))
        self.write_manifest()
        with self.assertRaisesRegex(AssertionError, 'integer schema18'):
            F.preflight(self.args)


class PrivateSchema(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.runtime = Path(temp.name).resolve()
        self.db = sqlite3.connect(self.runtime / 'baton.sqlite3')
        self.addCleanup(self.db.close)
        self.db.execute('PRAGMA user_version=19')
        self.host = Mock()

    def observe(self, version, api=1, **extra):
        response = {'result': {'api_version': api, 'database_schema_version': version}, **extra}
        raw = json.dumps(response).encode() + b'\n'
        self.host.wire.return_value = (response, raw)
        return F.observe_factory_schema(self.host, self.runtime), raw

    def test_actual_contract_reads_only_snapshot_and_read_only_database(self):
        result, raw = self.observe(19)
        self.assertEqual(result, {'ipc_api_version': 1, 'ipc_database_schema_version': 19,
            'sqlite_user_version': 19, 'snapshot_reply_sha256': F.sha(raw)})
        self.host.wire.assert_called_once_with(F.FixedCall('system.snapshot', {}))

    def test_wrong_snapshot_refused_without_sqlite(self):
        for version in (False, True, '19', 19.0, 18, 20, None):
            with self.subTest(version=version), patch.object(F.support, 'graph_database') as db:
                with self.assertRaisesRegex(F.BoundaryRefusal, 'snapshot'):
                    self.observe(version)
                db.assert_not_called()
        with self.assertRaises(F.BoundaryRefusal):
            self.observe(19, error={})
        with self.assertRaises(F.BoundaryRefusal):
            self.observe(19, api=True)

    def test_old_or_future_database_cannot_match_current_ipc(self):
        for version in (18, 20):
            self.db.execute(f'PRAGMA user_version={version}')
            with self.subTest(version=version), self.assertRaisesRegex(F.BoundaryRefusal, 'database'):
                self.observe(19)


class HistoricalContract(unittest.TestCase):
    def test_historical_factory_and_child_helper_source_is_byte_exact(self):
        source = SUBJECT.read_text()
        functions = {node.name: node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef)}
        names = ('preflight', 'install_private_tools', 'bounded_file', 'collect_started_records',
            'inspect_owned_records', 'verify_legacy_factory_shared_budget', 'note_failure', 'cleanup_call', 'runtime_exit')
        combined = b'\0'.join(ast.get_source_segment(source, functions[name]).encode() for name in names)
        self.assertEqual(F.sha(combined), 'b2aa782f3ed1e27726706eb0fe01fb5e6e76e05fd1e94d09d3afb75da6679a4b')

    def test_reviewed_support_uses_exact_checked_bytes(self):
        self.assertEqual(F.sha(F.SUPPORT_SOURCE), '550256ce728cbc26c5873237e09888195cfe01726b697d46d5e2b881e46b4d18')
        self.assertEqual(F.SUPPORT_SOURCE, F.SUPPORT_PATH.read_bytes())
        with patch.object(F, 'SUPPORT_SOURCE', b'wrong helper'):
            with self.assertRaisesRegex(F.BoundaryRefusal, 'reviewed'):
                F.factory_current_source()


if __name__ == '__main__':
    unittest.main()
