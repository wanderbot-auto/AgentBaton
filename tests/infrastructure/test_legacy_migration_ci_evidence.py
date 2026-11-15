import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('legacy_ci_projection', ROOT / 'scripts/record-legacy-migration-ci-evidence.py')
PROJECTION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROJECTION)


def build():
    return {'contract': 'agentbaton.schema14-source-build.v1', 'status': 'passed',
            'schema': 14, 'platform': 'linux', 'builder_sha256': 'a' * 64,
            'source': {'commit': PROJECTION.SOURCE, 'tree': PROJECTION.TREE,
                       'archive_sha256': 'b' * 64, 'compiled_source_clean': True},
            'binaries': {name: {'sha256': 'c' * 64, 'bytes': 123, 'mode': '0o500', 'nlink': 1}
                         for name in ('batond', 'baton')},
            'toolchain': {name: name + ' 1.99.1 (abcdef12 2026-10-01)' for name in ('cargo', 'rustc')},
            'build': {'exit_code': 0}}


def fixture():
    return {'status': 'passed', 'schema': 14, 'origin': 'actual_frozen_schema14_core_rpc',
            'source_commit': PROJECTION.SOURCE, 'provenance_kind': 'reviewed_source_build',
            'daemon_exit': 0, 'forced_kill': False,
            'runtime_cleanup': 'removed_after_successful_owned_join_and_backup',
            'database_sha256': 'd' * 64, 'counts': {name: 1 for name in PROJECTION.TABLES}}


def build15(platform='linux', architecture='x86_64'):
    record = build()
    record.update(contract='agentbaton.schema15-source-build.v1', status='compiled', schema=15,
                  platform=platform, architecture=architecture, os_release='25.0.0', common_helper_sha256='e' * 64)
    record['source'].update(commit=PROJECTION.SOURCE15, tree=PROJECTION.TREE15,
                            repository='https://github.com/wanderbot-auto/AgentBaton.git')
    record['build'].update(default_features=True, features=[],
                          argv=['/private/inert/cargo', 'build', '--offline', '--locked', '-j', '2', '--bin', 'batond', '--bin', 'baton'])
    for binary in record['binaries'].values():
        binary.update(format='mach-o-64' if platform == 'darwin' else 'elf-64', architecture=architecture)
    return record


def fixture15(platform='linux', architecture='x86_64'):
    tools = build15(platform, architecture)['binaries']
    counts = {name: 0 for name in ('tasks', 'events', 'idempotency') + PROJECTION.FACTORY15 + PROJECTION.AUTHORITY15}
    initial = {'schema': 15, 'query_only': True, 'foreign_key_errors': 0, 'counts': counts, 'sequence': 0}
    final = copy.deepcopy(initial)
    final['counts'].update(tasks=3, events=5, idempotency=4)
    final['sequence'] = 5
    return {'contract': 'agentbaton.actual-task-schema15.v1', 'status': 'passed', 'schema': 15,
            'origin': 'actual_source15_core_rpc', 'source_commit': PROJECTION.SOURCE15, 'source_tree': PROJECTION.TREE15,
            'provenance_kind': 'reviewed_source_build', 'platform': platform, 'architecture': architecture, 'os_release': '25.0.0',
            'tool_metadata': tools, 'tool_metadata_after': copy.deepcopy(tools),
            'binaries': {name: tools[name]['sha256'] for name in tools},
            'daemon': {'sigterm_sent': True, 'forced_kill': False, 'identity_gone_after_join': True,
                       'leader_unreaped_until_signalling': True, 'group_quiescent_before_reaping': True,
                       'cleanup_errors': [], 'daemon_wait_exit': 0},
            'backup_logical_exact': True, 'runtime_mode': '0o700', 'socket_utf8_bytes': 37,
            'actual_cli_probe': {'exit': 0, 'api_version': 1, 'sequence': 0},
            'initial_probe': initial, 'final_probe': final, 'final_api_sequence': 5,
            'database_sha256': 'd' * 64, 'database_bytes': 634880,
            **{field: 'a' * 64 for field in ('generator_sha256', 'helper_sha256', 'common_helper_sha256',
                                          'native_helper_sha256', 'tool_provenance_sha256')}}


class PublicMigrationEvidenceTests(unittest.TestCase):
    def test_valid_metadata_preserves_hashes_counts_and_actual_source_kind(self):
        expected = build()
        del expected['build']
        expected['build_exit'] = 0
        self.assertEqual(PROJECTION.public_build(build()), expected)
        self.assertEqual(PROJECTION.public_fixture(fixture()), fixture())
        self.assertEqual(PROJECTION.public_build(build())['binaries']['baton']['sha256'], 'c' * 64)
        self.assertNotIn('source_mode', PROJECTION.public_fixture(fixture()))

    def test_nested_unknown_private_fields_are_never_forwarded(self):
        value = build()
        value['binaries']['baton']['raw_capability'] = 'PRIVATE_SENTINEL_CAP'
        value['toolchain']['raw_rpc'] = {'token': 'PRIVATE_SENTINEL_RPC'}
        value['source']['raw_request'] = 'PRIVATE_SENTINEL_SOURCE'
        result = json.dumps(PROJECTION.public_build(value))
        self.assertNotIn('PRIVATE_SENTINEL', result)
        self.assertEqual(set(PROJECTION.public_build(value)['binaries']['baton']), {'sha256', 'bytes', 'mode', 'nlink'})

    def test_wrong_scalar_shapes_cannot_relay_nested_private_history(self):
        for field in ('origin', 'provenance_kind', 'status', 'runtime_cleanup', 'source_commit'):
            value = fixture()
            value[field] = {'raw_capability': 'PRIVATE_SENTINEL'}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'finite public schema'):
                PROJECTION.public_fixture(value)

    def test_counts_must_have_only_seven_known_nonempty_integer_tables(self):
        for change in ('extra', 'missing', 'bool', 'float', 'string', 'zero'):
            value = fixture()
            if change == 'extra':
                value['counts']['PRIVATE_SENTINEL'] = 1
            elif change == 'missing':
                del value['counts'][PROJECTION.TABLES[0]]
            else:
                value['counts'][PROJECTION.TABLES[0]] = {'bool': True, 'float': 1.0, 'string': 'PRIVATE_SENTINEL', 'zero': 0}[change]
            with self.subTest(change=change), self.assertRaises(ValueError):
                PROJECTION.public_fixture(value)

    def test_known_toolchain_values_follow_version_grammar_and_length(self):
        for value in ({'raw_rpc': 'PRIVATE_SENTINEL'}, 'PRIVATE_SENTINEL', 'cargo 1.99.1 ' + 'x' * 200):
            record = build()
            record['toolchain']['cargo'] = value
            with self.subTest(kind=type(value).__name__), self.assertRaises(ValueError):
                PROJECTION.public_build(record)

    def test_binary_fields_and_source_are_finite(self):
        for field, value in [('bytes', True), ('bytes', 0), ('sha256', 'PRIVATE_SENTINEL'), ('mode', '0o700'), ('nlink', True), ('nlink', 2)]:
            record = build()
            record['binaries']['baton'][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                PROJECTION.public_build(record)
        record = build()
        record['source']['commit'] = '0' * 40
        with self.assertRaises(ValueError):
            PROJECTION.public_build(record)

    def test_failed_records_expose_only_fixed_status(self):
        for project in (PROJECTION.public_build, PROJECTION.public_fixture):
            result = project({'status': 'failed_or_interrupted', 'binaries': {'raw_rpc': 'PRIVATE_SENTINEL'}, 'origin': {'token': 'PRIVATE_SENTINEL'}})
            self.assertEqual(result, {'status': 'failed_or_interrupted'})

    def test_public_projection_does_not_mutate_private_input(self):
        for value, project in ((build(), PROJECTION.public_build), (fixture(), PROJECTION.public_fixture)):
            before = copy.deepcopy(value)
            project(value)
            self.assertEqual(value, before)


class PublicSchema15MigrationEvidenceTests(unittest.TestCase):
    def test_native_platform_shapes_are_finite_without_claiming_native_execution(self):
        for platform, arch in [('linux', 'x86_64'), ('linux', 'aarch64'), ('darwin', 'arm64'), ('darwin', 'x86_64')]:
            with self.subTest(platform=platform, architecture=arch):
                b = PROJECTION.public15_build(build15(platform, arch))
                f = PROJECTION.public15_fixture(fixture15(platform, arch))
                self.assertEqual(b['status'], 'compiled')
                self.assertEqual(f['status'], 'passed')
                self.assertEqual(b['source']['commit'], PROJECTION.SOURCE15)
                self.assertEqual(f['source_tree'], PROJECTION.TREE15)
                self.assertEqual(f['final_probe']['counts']['tasks'], 3)
                self.assertEqual(f['daemon_stop']['cleanup_error_count'], 0)
                self.assertNotIn('process_proof', b)  # build-only producer never probes a daemon
                self.assertNotIn('consumer', f)

    def test_wrong_version_source_contract_status_or_provenance_is_rejected(self):
        for make, project in ((build15, PROJECTION.public15_build), (fixture15, PROJECTION.public15_fixture)):
            for key, value in [('schema', 15.0), ('schema', True), ('schema', 14), ('status', 'unknown'),
                               ('status', {'raw': 'PRIVATE_SENTINEL'}), ('contract', 'PRIVATE_SENTINEL')]:
                record = make(); record[key] = value
                with self.subTest(project=project.__name__, key=key, value=value), self.assertRaises(ValueError):
                    project(record)
        for path, value in [(('source', 'commit'), PROJECTION.SOURCE), (('source', 'tree'), PROJECTION.TREE),
                             (('build', 'exit_code'), False), (('build', 'exit_code'), 1),
                             (('build', 'default_features'), 1), (('build', 'features'), ['preview-evidence']),
                             (('build', 'argv'), ['/private/inert/cargo', 'build'])]:
            record = build15(); record[path[0]][path[1]] = value
            with self.subTest(path=path), self.assertRaises(ValueError): PROJECTION.public15_build(record)
        record = build15(); record['status'] = 'passed'
        with self.assertRaises(ValueError): PROJECTION.public15_build(record)
        for key, value in [('source_commit', PROJECTION.SOURCE), ('source_tree', PROJECTION.TREE),
                           ('provenance_kind', 'immutable_frozen_pair')]:
            record = fixture15(); record[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)

    def test_no_private_history_path_identity_argv_or_nested_diagnostics_escape(self):
        for make, project in ((build15, PROJECTION.public15_build), (fixture15, PROJECTION.public15_fixture)):
            value = make()
            value.update(raw_rpc={'token': 'PRIVATE_SENTINEL'}, argv=['PRIVATE_SENTINEL'],
                         tasks=[{'id': 'PRIVATE_SENTINEL'}], project_id='PRIVATE_SENTINEL',
                         retained_private_root='PRIVATE_SENTINEL', error={'traceback': 'PRIVATE_SENTINEL'})
            if make is build15:
                for binary in value['binaries'].values(): binary['inode'] = 'PRIVATE_SENTINEL'
                value['build']['argv'][0] = '/PRIVATE_SENTINEL/cargo'
                value['source']['files_sha256'] = {'PRIVATE_SENTINEL': 'PRIVATE_SENTINEL'}
                value['toolchain']['private'] = {'rpc': 'PRIVATE_SENTINEL'}
            else:
                value['daemon']['owned_identity'] = {'pid': 'PRIVATE_SENTINEL'}
                value['final_probe']['counts']['PRIVATE_SENTINEL'] = 0
                value['final_probe']['unknown'] = {'rpc': 'PRIVATE_SENTINEL'}
                value['tool_metadata']['baton']['inode'] = 'PRIVATE_SENTINEL'
            before = copy.deepcopy(value)
            public = project(value)
            self.assertNotIn('PRIVATE_SENTINEL', json.dumps(public))
            self.assertEqual(value, before)

    def test_numeric_bounds_and_integer_types_are_exact(self):
        for field, low, high in [('tasks', 3, 3), ('events', 5, 100000), ('idempotency', 4, 100000)]:
            for value in [True, float(low), str(low), low - 1, high + 1, float('inf')]:
                record = fixture15(); record['final_probe']['counts'][field] = value
                with self.subTest(field=field, value=value), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        for field, value in [('database_bytes', True), ('database_bytes', 16777217),
                             ('final_api_sequence', 5.0), ('final_api_sequence', 6), ('socket_utf8_bytes', 101)]:
            record = fixture15(); record[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        for table in PROJECTION.FACTORY15 + PROJECTION.AUTHORITY15:
            record = fixture15(); record['final_probe']['counts'][table] = 1
            with self.subTest(table=table), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)

    def test_stopped_joined_no_force_and_clean_backup_proof_cannot_be_weakened(self):
        for field, value in [('sigterm_sent', 1), ('forced_kill', 0), ('identity_gone_after_join', False),
                             ('leader_unreaped_until_signalling', False), ('group_quiescent_before_reaping', False),
                             ('daemon_wait_exit', False), ('daemon_wait_exit', -9), ('cleanup_errors', ['PRIVATE_SENTINEL'])]:
            record = fixture15(); record['daemon'][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        for field in ('backup_logical_exact',):
            record = fixture15(); record[field] = 1
            with self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        for probe in ('initial_probe', 'final_probe'):
            for field, value in [('query_only', 1), ('foreign_key_errors', 1), ('schema', 16)]:
                record = fixture15(); record[probe][field] = value
                with self.subTest(probe=probe, field=field), self.assertRaises(ValueError): PROJECTION.public15_fixture(record)

    def test_tools_are_bounded_native_single_link_and_sha_stable(self):
        for field, value in [('bytes', 268435457), ('bytes', True), ('nlink', True), ('nlink', 2),
                             ('mode', '0o700'), ('architecture', 'PRIVATE_SENTINEL'),
                             ('format', 'mach-o-64'), ('sha256', 'PRIVATE_SENTINEL')]:
            record = build15(); record['binaries']['baton'][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): PROJECTION.public15_build(record)
        record = fixture15(); record['tool_metadata_after']['baton']['sha256'] = 'e' * 64
        with self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        record = fixture15(); record['binaries']['baton'] = 'e' * 64
        with self.assertRaises(ValueError): PROJECTION.public15_fixture(record)
        record = build15(); record['toolchain']['cargo'] = 'PRIVATE_SENTINEL'
        with self.assertRaises(ValueError): PROJECTION.public15_build(record)

    def test_failure_records_publish_only_fixed_status(self):
        for project in (PROJECTION.public15_build, PROJECTION.public15_fixture):
            self.assertEqual(project({'status': 'failed_or_interrupted', 'error': {'token': 'PRIVATE_SENTINEL'}}),
                             {'status': 'failed_or_interrupted'})

    def test_loader_refuses_duplicates_nonfinite_overflow_and_unsafe_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'metadata.json'
            for raw in ['{"status":"passed","status":"failed_or_interrupted"}',
                        '{"ignored":{"same":1,"same":2}}', '{"ignored":NaN}',
                        '{"ignored":Infinity}', '{"ignored":-Infinity}', '{"ignored":1e999}']:
                path.write_text(raw)
                with self.subTest(raw=raw), self.assertRaises(ValueError): PROJECTION.load(path)
            path.write_text('{}'); link = Path(tmp) / 'link.json'; link.symlink_to(path)
            with self.assertRaises(OSError): PROJECTION.load(link)
            broken = Path(tmp) / 'broken.json'; broken.symlink_to(Path(tmp) / 'missing-target.json')
            with self.assertRaises(ValueError): PROJECTION.load(broken)
            fifo = Path(tmp) / 'fifo'; os.mkfifo(fifo, 0o600)
            with self.assertRaises(ValueError): PROJECTION.load(fifo)
            path.write_text(' ' * 4194305)
            with self.assertRaises(ValueError): PROJECTION.load(path)
            self.assertIsNone(PROJECTION.load(Path(tmp) / 'missing.json'))

    def test_main_keeps_missing_honest_and_collects_all_four_inputs_after_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            temporary = Path(tmp) / 'private'; output = Path(tmp) / 'public'
            def run():
                with mock.patch.object(sys, 'argv', ['projection', '--temporary', str(temporary), '--output', str(output)]):
                    return PROJECTION.main()
            self.assertEqual(run(), 0)
            missing = json.loads((output / 'summary.json').read_text())
            for key in ('source_build', 'fixture', 'schema15_source_build', 'schema15_fixture'):
                self.assertEqual(missing[key], {'status': 'missing_or_unpublished'})
            self.assertIs(missing['metadata_only'], True)
            def write(name, record):
                path = temporary / name; path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(record))
            write('agentbaton-legacy14-build/frozen/source-build-manifest.json', build())
            write('agentbaton-legacy14-fixture/manifest.json', fixture())
            write('agentbaton-legacy15-build/frozen/source-build-manifest.json', build15())
            write('agentbaton-legacy15-fixture/manifest.json', fixture15())
            self.assertEqual(run(), 0)
            valid = json.loads((output / 'summary.json').read_text())
            self.assertEqual(valid['source_build'], PROJECTION.public_build(build()))
            self.assertEqual(valid['fixture'], PROJECTION.public_fixture(fixture()))
            self.assertEqual(valid['schema15_source_build']['status'], 'compiled')
            self.assertEqual(valid['schema15_fixture']['status'], 'passed')
            write('agentbaton-legacy14-build/frozen/source-build-manifest.json', {'status': 'PRIVATE_SENTINEL'})
            invalid = build15(); invalid['toolchain']['cargo'] = {'token': 'PRIVATE_SENTINEL'}
            write('agentbaton-legacy15-build/frozen/source-build-manifest.json', invalid)
            self.assertEqual(run(), 1)
            published = (output / 'summary.json').read_text(); result = json.loads(published)
            self.assertNotIn('PRIVATE_SENTINEL', published)
            self.assertEqual(result['source_build'], {'status': 'invalid_private_metadata_not_published'})
            self.assertEqual(result['schema15_source_build'], {'status': 'invalid_private_metadata_not_published'})
            self.assertEqual(result['fixture'], PROJECTION.public_fixture(fixture()))
            self.assertEqual(result['schema15_fixture'], PROJECTION.public15_fixture(fixture15()))


if __name__ == '__main__':
    unittest.main()
