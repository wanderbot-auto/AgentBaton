"""Schema 19 candidate and read-only database boundary controls."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SUBJECT = Path(__file__).with_name('factory-child-planning-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('_child_planning_schema19_controls', SUBJECT)
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
HEAD, TREE = 'a' * 40, 'b' * 40


class FrozenPlanningInputs(unittest.TestCase):
    def setUp(self):
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.root = Path(root.name).resolve()
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.frontend = self.root / 'frontend'
        self.frontend.mkdir()
        self.asset = self.frontend / 'index.html'
        self.asset.write_bytes(b'fixed frontend')
        for name in ('batond', 'baton'):
            (self.tools / name).write_bytes(name.encode())
            (self.tools / name).chmod(0o500)
        self.manifest_path = self.root / 'manifest.json'
        self.fields = {'schema': 19, 'git_head': HEAD, 'git_tree': TREE, 'source_head': HEAD,
            'compiled_core_source_head': HEAD, 'frontend_source_head': HEAD, 'dirty': False,
            'binaries': {name: P.sha((self.tools/name).read_bytes()) for name in ('batond', 'baton')},
            'frontend_assets': {'index.html': P.sha(self.asset.read_bytes())}}
        self.write_manifest()
        self.args = SimpleNamespace(tools=self.tools, frontend=self.frontend, manifest=self.manifest_path)
        git = patch.object(P.base.support, 'graph_source', return_value=(HEAD, TREE))
        self.git = git.start()
        self.addCleanup(git.stop)

    def write_manifest(self, **changes):
        self.manifest_path.write_text(json.dumps({**self.fields, **changes}))

    def prepare(self):
        is_file = Path.is_file
        with patch.object(sys, 'platform', 'linux'), patch.object(Path, 'is_file',
                lambda path: path == Path('/proc/self/stat') or is_file(path)):
            return P.current_planning_preflight(self.args)

    def test_supported_factory_and_graph_source_and_inputs(self):
        self.assertEqual(P.sha(P.BASE_SOURCE), P.FACTORY_SUPPORT_SHA256)
        self.assertEqual(P.BASE_SOURCE, P.BASE_PATH.read_bytes())
        self.assertEqual(P.base.FACTORY_SCHEMA, 19)
        prepared = self.prepare()
        self.assertEqual((prepared.source_head, prepared.source_tree), (HEAD,TREE))
        self.assertEqual(prepared.manifest, self.fields)
        self.assertEqual(prepared.tool_paths, {name:self.tools/name for name in ('batond','baton')})
        P.verify_planning_run_inputs(prepared, P.sha(SUBJECT.read_bytes()))

    def test_schema_and_wrong_source_rejected_before_opening_tools(self):
        variants = [{'schema': item} for item in (18, 20, True, False, 19.0, '19', None)]
        variants += [{'git_head': 'c' * 40}, {'git_tree': 'c' * 40},
            {'compiled_core_source_head': 'c' * 40}, {'frontend_source_head': 'c' * 40}, {'dirty': True}]
        for fields in variants:
            self.write_manifest(**fields)
            with self.subTest(fields=fields), patch.object(P.base, 'factory_tools_pins') as tools:
                with self.assertRaises(P.Refusal):
                    self.prepare()
                tools.assert_not_called()

    def test_preflight_rejects_replacement_tool_and_extra_asset(self):
        other = self.tools / 'baton'
        other.chmod(0o700)
        other.write_bytes(b'different executable')
        with self.assertRaisesRegex(P.Refusal, 'tool digest mismatch'):
            self.prepare()
        other.write_bytes(b'baton')
        other.chmod(0o500)
        (self.frontend / 'wrong').write_bytes(b'unlisted')
        with self.assertRaisesRegex(P.Refusal, 'frontend assets'):
            self.prepare()

    def test_final_recheck_refuses_source_tool_asset_and_manifest_drift(self):
        digest = P.sha(SUBJECT.read_bytes())
        prepared = self.prepare()
        self.git.return_value=('c'*40,TREE)
        with self.assertRaisesRegex(P.Refusal, 'source changed'):
            P.verify_planning_run_inputs(prepared, digest)
        self.git.return_value=(HEAD,TREE)
        with self.assertRaisesRegex(P.Refusal, 'driver changed'):
            P.verify_planning_run_inputs(prepared,'f'*64)
        self.write_manifest(scope='changed')
        with self.assertRaisesRegex(P.Refusal, 'manifest changed'):
            P.verify_planning_run_inputs(prepared,digest)
        self.write_manifest()
        prepared=self.prepare()
        self.asset.write_bytes(b'changed asset')
        with self.assertRaisesRegex(P.Refusal, 'frontend changed'):
            P.verify_planning_run_inputs(prepared,digest)
        self.asset.write_bytes(b'fixed frontend')
        prepared=self.prepare()
        replacement=self.tools/'replacement'
        replacement.write_bytes(b'baton')
        replacement.chmod(0o500)
        replacement.replace(self.tools/'baton')
        with self.assertRaisesRegex(P.Refusal, 'tool identity changed'):
            P.verify_planning_run_inputs(prepared,digest)
        with patch.object(P,'BASE_SOURCE',b'unreviewed implementation'):
            with self.assertRaisesRegex(P.Refusal,'reviewed schema19 Factory'):
                self.prepare()

    def test_cli_refuses_transparently_without_runtime_calls(self):
        output = self.root / 'refusal'
        self.write_manifest(schema=True)
        argv = [str(SUBJECT),'--run','--tools',str(self.tools),'--frontend',str(self.frontend),
            '--manifest',str(self.manifest_path),'--output',str(output)]
        with patch.object(sys,'argv',argv), patch.object(P,'runtime_driver') as runtime, \
                patch.object(P.subprocess,'Popen') as spawn:
            self.assertEqual(P.main(),2)
        runtime.assert_not_called()
        spawn.assert_not_called()
        self.assertEqual({path.name for path in output.iterdir()},{'preflight-refusal.json'})
        report=json.loads((output/'preflight-refusal.json').read_bytes())
        self.assertEqual((report['status'],report['actual_runtime'],report['schema_required']),('preflight_refused','not_run',19))
        self.assertIs(report['no_process_token_read_or_listener'],True)


class PlanningSqliteVersions(unittest.TestCase):
    def setUp(self):
        root=tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.runtime=Path(root.name).resolve()
        self.db=sqlite3.connect(self.runtime/'baton.sqlite3')
        self.addCleanup(self.db.close)
        fixed={'execution_grants','managed_workspaces','tasks','attempts','sessions','factory_attempts','factory_runs',
            'factory_child_plan_heads','factory_child_plan_revisions','budgets','workspace_leases',
            'factory_budget_reservations','goal_ledgers','idempotency','events'}
        G=P.base.support
        self.tables=fixed | G.LEGACY_GOAL_TABLES | set(G.FACTORY_RECORD_TABLES.values()) | set(G.INTEGRATION_RECORD_TABLES.values())
        for name in self.tables:
            self.db.execute(f'CREATE TABLE "{name}"(id INTEGER)')

    def test_current_and_historical_fingerprints_use_exact_distinct_schema(self):
        self.db.execute('PRAGMA user_version=18')
        old=P.fingerprint_db(self.runtime)
        self.assertEqual(set(old),self.tables)
        with self.assertRaises(P.Refusal):
            P.current_fingerprint_db(self.runtime)
        self.db.execute('PRAGMA user_version=19')
        current=P.current_fingerprint_db(self.runtime)
        self.assertEqual(current,old)
        with self.assertRaises(AssertionError):
            P.fingerprint_db(self.runtime)
        self.db.execute('PRAGMA user_version=20')
        with self.assertRaises(P.Refusal):
            P.current_fingerprint_db(self.runtime)

    def test_schema19_fingerprint_detects_rows_and_rejects_unplanned_authority(self):
        self.db.execute('PRAGMA user_version=19')
        before=P.current_fingerprint_db(self.runtime)
        self.db.execute('INSERT INTO factory_child_plan_revisions VALUES (1)')
        self.db.commit()
        after=P.current_fingerprint_db(self.runtime)
        self.assertEqual(after['factory_child_plan_revisions']['count'],1)
        self.assertNotEqual(after['factory_child_plan_revisions']['sha256'],before['factory_child_plan_revisions']['sha256'])
        self.db.execute('INSERT INTO factory_child_goal_runs VALUES (1)')
        self.db.commit()
        with self.assertRaisesRegex(AssertionError,'cannot dispatch'):
            P.current_fingerprint_db(self.runtime)


if __name__ == '__main__':
    unittest.main()
