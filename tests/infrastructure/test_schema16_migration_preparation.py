"""Finite genuine16 producer contracts. No Git, compiler, daemon or migration runs."""
import copy
import importlib.util
import json
from pathlib import Path
import platform
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]

def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

H = load('build16_controls', 'scripts/prepare-schema16-migration-fixture.py')
G = load('generate16_controls', 'scripts/generate-schema16-migration-fixture.py')
P = load('project16_controls', 'scripts/record-legacy-migration-ci-evidence.py')


def build16():
    return {'contract': H.CONTRACT, 'status': 'compiled', 'schema': 16,
            'platform': sys.platform, 'architecture': platform.machine(), 'os_release': '25.0.0',
            'source': {'commit': H.SOURCE, 'tree': H.TREE, 'repository': H.REPOSITORY,
                       'archive_sha256': 'a' * 64, 'files_sha256': {'Cargo.toml': 'b' * 64, 'Cargo.lock': 'c' * 64},
                       'compiled_source_clean': True},
            'build': {'argv': ['/never-executed/cargo', 'build', '--offline', '--locked', '-j', '2', '--bin', 'batond', '--bin', 'baton'],
                      'default_features': True, 'features': [], 'exit_code': 0, 'lockfile_sha256': 'c' * 64},
            'toolchain': {'cargo': 'cargo 1.99.1 (abcdef12 2026-10-01)', 'rustc': 'rustc 1.99.1 (abcdef12 2026-10-01)'},
            'builder_sha256': 'd' * 64, 'common_helper_sha256': 'e' * 64,
            'binaries': {n: {'bytes': 128, 'sha256': 'f' * 64, 'mode': '0o500', 'nlink': 1,
                             'format': 'mach-o-64' if sys.platform == 'darwin' else 'elf-64',
                             'architecture': platform.machine()} for n in ('batond', 'baton')}}


def fixture16():
    tools = build16()['binaries']
    counts = {n: 0 for n in P.SCHEMA16_TABLES}
    initial = {'schema': 16, 'query_only': True, 'foreign_key_errors': 0, 'counts': counts, 'sequence': 0}
    final = copy.deepcopy(initial)
    final['counts'].update(tasks=3, events=7, idempotency=5, managed_workspaces=1)
    final['sequence'] = 7
    return {'contract': 'agentbaton.actual-task-schema16.v1', 'status': 'passed', 'schema': 16,
            'origin': 'actual_source16_core_rpc', 'source_commit': H.SOURCE, 'source_tree': H.TREE,
            'provenance_kind': 'reviewed_source_build', 'platform': sys.platform, 'architecture': platform.machine(), 'os_release': '25.0.0',
            'tool_metadata': tools, 'tool_metadata_after': copy.deepcopy(tools),
            'binaries': {n: tools[n]['sha256'] for n in tools},
            'daemon': {'sigterm_sent': True, 'forced_kill': False, 'identity_gone_after_join': True,
                       'leader_unreaped_until_signalling': True, 'group_quiescent_before_reaping': True,
                       'cleanup_errors': [], 'daemon_wait_exit': 0},
            'backup_logical_exact': True, 'source_git_unchanged': True, 'prepared_task_count': 1,
            'runtime_mode': '0o700', 'socket_utf8_bytes': 37,
            'actual_cli_probe': {'exit': 0, 'api_version': 1, 'sequence': 0},
            'initial_probe': initial, 'final_probe': final, 'final_api_sequence': 7,
            'database_sha256': 'a' * 64, 'database_bytes': 634880,
            **{n: 'b' * 64 for n in ['generator_sha256', 'helper_sha256', 'common_helper_sha256', 'native_helper_sha256', 'tool_provenance_sha256']}}


class Genuine16ProducerControls(unittest.TestCase):
    def test_fixed_source_tree_and_compiler_contract_are_independent_of_current17(self):
        self.assertEqual(H.SOURCE, '0644ea9d8c10d075fef6b139bd9d7d363d5cc4fb')
        self.assertEqual(H.TREE, '107895dfd43c996c668017d556f938d01fa17d3d')
        self.assertEqual(len(H.SCHEMA_TABLES), 55)
        self.assertFalse(any(n.startswith('integration') for n in H.SCHEMA_TABLES))
        with mock.patch.object(H.subprocess, 'run', side_effect=AssertionError('no compiler/product')):
            H.validate_compiled(build16())
        for path, bad in [(('schema',), 15), (('schema',), 17), (('schema',), 16.0), (('schema',), True),
                          (('source', 'commit'), 'a' * 40), (('source', 'tree'), 'a' * 40),
                          (('source', 'compiled_source_clean'), False), (('build', 'exit_code'), True),
                          (('build', 'features'), ['different']), (('build', 'lockfile_sha256'), 'e' * 64)]:
            value = build16(); target = value
            for key in path[:-1]: target = target[key]
            target[path[-1]] = bad
            with self.subTest(path=path, bad=bad), self.assertRaises(ValueError): H.validate_compiled(value)

    def test_no_unpinned_frozen_pair_provenance_fallback(self):
        with mock.patch.object(H, 'tool_records', return_value={}), self.assertRaisesRegex(ValueError, 'source-build manifest'):
            H.provenance(Path('/never-executed'))

    def test_public16_build_and_fixture_only_publish_finite_whitelist(self):
        build = build16(); fixture = fixture16()
        H.validate_compiled(build)
        for value in [build, fixture]: value['raw_rpc'] = {'token': 'PRIVATE_CANARY'}
        fixture['tasks'] = [{'id': 'PRIVATE_TASK_PATH_ID'}]
        self.assertEqual(P.public16_build(build)['schema'], 16)
        result = P.public16_fixture(fixture)
        self.assertEqual(result['prepared_task_count'], 1)
        self.assertNotIn('PRIVATE_', json.dumps(result))
        self.assertEqual(result['origin'], 'actual_source16_core_rpc')
        self.assertNotIn('tasks', result)
        self.assertNotIn('source_git_path', result)

    def test_old15_and_current17_cannot_be_relabelled_as16_inputs(self):
        for projector, make in [(P.public16_build, build16), (P.public16_fixture, fixture16)]:
            for bad in [14, 15, 17, 18, 16.0, True, '16']:
                value = make(); value['schema'] = bad
                with self.subTest(projector=projector.__name__, bad=bad), self.assertRaises(ValueError): projector(value)
        for bad in ['actual_source15_core_rpc', 'actual_source17_core_rpc', {'token': 'PRIVATE_CANARY'}]:
            value = fixture16(); value['origin'] = bad
            with self.assertRaises(ValueError): P.public16_fixture(value)

    def test_bound_workspace_no_grant_and_join_proofs_remain_strict(self):
        for field, bad in [('source_git_unchanged', 1), ('prepared_task_count', True), ('prepared_task_count', 0),
                           ('backup_logical_exact', False)]:
            value = fixture16(); value[field] = bad
            with self.subTest(field=field), self.assertRaises(ValueError): P.public16_fixture(value)
        for field, bad in [('sigterm_sent', 1), ('daemon_wait_exit', False), ('forced_kill', True),
                           ('identity_gone_after_join', False), ('group_quiescent_before_reaping', False)]:
            value = fixture16(); value['daemon'][field] = bad
            with self.subTest(field=field), self.assertRaises(ValueError): P.public16_fixture(value)
        for table in P.AUTHORITY16 + P.FACTORY16:
            value = fixture16(); value['final_probe']['counts'][table] = 1
            with self.subTest(table=table), self.assertRaises(ValueError): P.public16_fixture(value)

    def test_exact_55_tables_refuse_missing_extra_typed_counts_and_new17_domains(self):
        for mode in ['missing', 'extra', 'bool', 'float', 'integration']:
            value = fixture16(); counts = value['final_probe']['counts']
            if mode == 'missing': del counts['sessions']
            elif mode == 'extra': counts['PRIVATE_CANARY'] = 0
            elif mode == 'integration': counts['integration_policies'] = 0
            else: counts['sessions'] = False if mode == 'bool' else 0.0
            with self.subTest(mode=mode), self.assertRaises(ValueError): P.public16_fixture(value)
        for projector in [P.public16_build, P.public16_fixture]:
            self.assertEqual(projector({'status': 'failed_or_interrupted', 'raw_stderr': 'PRIVATE_CANARY'}), {'status': 'failed_or_interrupted'})

    def test_generated_snapshot_queries_real_sqlite_and_never_runs_migration(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'inert.sqlite3'
            with sqlite3.connect(path) as db:
                db.execute('PRAGMA user_version=16')
                for table in H.SCHEMA_TABLES:
                    if table == 'sqlite_sequence': continue
                    db.execute('CREATE TABLE '+table+'(sequence INTEGER PRIMARY KEY AUTOINCREMENT)' if table == 'events' else 'CREATE TABLE '+table+'(id INTEGER)')
            before = path.read_bytes()
            with mock.patch.object(G.subprocess, 'Popen', side_effect=AssertionError('no daemon')):
                observed = G.snapshot(path)
            self.assertEqual(observed['schema'], 16)
            self.assertEqual(path.read_bytes(), before)
            for schema in [15, 17, 18]:
                with sqlite3.connect(path) as db: db.execute(f'PRAGMA user_version={schema}')
                with self.assertRaises(ValueError): G.snapshot(path)
            with sqlite3.connect(path) as db:
                db.execute('PRAGMA user_version=16'); db.execute('CREATE TABLE integration_policies(id TEXT)')
            with self.assertRaises(ValueError): G.snapshot(path)


if __name__ == '__main__':
    unittest.main()
