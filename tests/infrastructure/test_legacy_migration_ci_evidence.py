import copy
import importlib.util
import json
from pathlib import Path
import unittest

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


if __name__ == '__main__':
    unittest.main()
