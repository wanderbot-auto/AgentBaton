#!/usr/bin/env python3
"""Current schema and candidate checks with inert private files and SQLite fixtures."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest


PATH = Path(__file__).with_name('data-workspace-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('data_workspace_driver', PATH)
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class SchemaTests(unittest.TestCase):
    def test_only_the_exact_integer_is_current(self):
        for source in ('system.snapshot', 'read-only SQLite', 'candidate manifest'):
            self.assertEqual(DRIVER.require_current_schema(19, source), 19)
            for invalid in (18, 20, False, True, None, '19', 19.0):
                with self.subTest(source=source, invalid=invalid), self.assertRaisesRegex(ValueError, 'integer schema 19'):
                    DRIVER.require_current_schema(invalid, source)

    def test_actual_private_readonly_database_preserves_transaction_and_file(self):
        with tempfile.TemporaryDirectory(prefix='data schema # ') as directory:
            runtime = Path(directory)
            path = runtime / 'baton.sqlite3'
            for version in (18, 19, 20):
                with sqlite3.connect(path) as store:
                    store.execute(f'PRAGMA user_version={version}')
                original = path.read_bytes()
                with DRIVER.readonly_database(runtime) as store:
                    self.assertTrue(store.in_transaction)
                    self.assertEqual(store.execute('PRAGMA query_only').fetchone(), (1,))
                    observed, = store.execute('PRAGMA user_version').fetchone()
                    self.assertIs(type(observed), int)
                    if version == 19:
                        self.assertEqual(DRIVER.require_current_schema(observed, 'read-only SQLite'), 19)
                    else:
                        with self.assertRaises(ValueError):
                            DRIVER.require_current_schema(observed, 'read-only SQLite')
                    with self.assertRaises(sqlite3.OperationalError):
                        store.execute('CREATE TABLE must_never_persist(value TEXT)')
                self.assertEqual(path.read_bytes(), original)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.manifest = {'schema': 19, 'git_head': 'a' * 40, 'git_tree': 'b' * 40, 'dirty': False,
                         'cargo_features': [DRIVER.PREVIEW_FEATURE],
                         'binaries': {'batond': 'c' * 64, 'baton': {'sha256': 'd' * 64}}}
        self.tools = {name: {'sha256': digest} for name, digest in
                      (('batond', 'c' * 64), ('baton', 'd' * 64))}

    def read(self, **changes):
        return DRIVER.candidate_attribution(json.dumps(self.manifest | changes).encode(), self.tools, True)

    def test_current_manifest_attribution_preserves_both_verified_tools_and_source(self):
        facts = self.read()
        self.assertEqual(facts['schema'], 19)
        self.assertEqual(facts['git_head'], self.manifest['git_head'])
        self.assertEqual(facts['git_tree'], self.manifest['git_tree'])
        self.assertEqual(facts['matched_core_tools'], ['batond', 'baton'])
        self.assertTrue(facts['not_final_review_or_ci_acceptance'])
        self.assertEqual(self.read(source_tree_sha256='e' * 64)['source_tree_sha256'], 'e' * 64)

    def test_other_schema_or_ambiguous_source_is_rejected(self):
        for schema in (18, 20, True, '19', 19.0):
            with self.subTest(schema=schema), self.assertRaisesRegex(ValueError, 'candidate manifest'):
                self.read(schema=schema)
        for changes in ({'git_head': 'not-a-commit'}, {'source_head': 'f' * 40},
                        {'git_tree': None}, {'source_tree_sha256': 'bad'}, {'dirty': True},
                        {'dirty': 0}, {'tracked_clean': 1}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.read(**changes)
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            DRIVER.candidate_attribution(b'{"schema":18,"schema":19}', self.tools, True)

    def test_companion_and_daemon_hashes_are_independently_required(self):
        for tool in ('batond', 'baton'):
            for digest in (None, '0' * 64):
                binaries = self.manifest['binaries'].copy()
                if digest is None:
                    del binaries[tool]
                else:
                    binaries[tool] = digest
                with self.subTest(tool=tool, digest=digest), self.assertRaisesRegex(ValueError, f'Candidate {tool}'):
                    self.read(binaries=binaries)
        del self.tools['baton']
        with self.assertRaisesRegex(ValueError, 'Candidate baton '):
            self.read()

    def test_supplied_features_cannot_claim_gated_evidence_from_an_unrelated_build(self):
        for features in (None, [], [False], ['unrelated'], DRIVER.PREVIEW_FEATURE):
            with self.subTest(features=features), self.assertRaisesRegex(ValueError, 'feature declaration'):
                self.read(cargo_features=features)

    def test_tool_pin_requires_safe_executable_and_hashes_actual_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'synthetic-tool'
            path.write_bytes(b'generated fixture, never executed')
            path.chmod(0o700)
            facts = DRIVER.pinned_tool(path)
            self.assertEqual(facts['sha256'], DRIVER.sha(path.read_bytes()))
            self.assertEqual(facts['inode'], path.stat().st_ino)
            for mode in (0o600, 0o770):
                path.chmod(mode)
                with self.assertRaisesRegex(ValueError, 'Unsafe executable'):
                    DRIVER.pinned_tool(path)


class CleanupTests(unittest.TestCase):
    def test_only_normal_joined_shutdown_preserves_success(self):
        good = {'status': 'passed', 'browser_closed': True}
        DRIVER.finalize_fixture_cleanup(good, 0, False, True)
        self.assertEqual(good['status'], 'passed')
        self.assertEqual(good['cleanup'], {'daemon_exit_code': 0, 'daemon_forced': False,
                                          'http_thread_joined': True})
        for exit_code, forced, http_joined, browser_closed in (
                (1, False, True, True), (False, False, True, True),
                (0, True, True, True), (0, False, False, True), (0, False, True, False)):
            with self.subTest(exit_code=exit_code, forced=forced, http_joined=http_joined,
                              browser_closed=browser_closed):
                report = {'status': 'passed', 'browser_closed': browser_closed}
                DRIVER.finalize_fixture_cleanup(report, exit_code, forced, http_joined)
                self.assertEqual(report['status'], 'failed_or_interrupted')
                self.assertEqual(report['failure']['stage'], 'cleanup')

    def test_cleanup_refusal_preserves_an_earlier_failure(self):
        earlier = {'stage': 'browser', 'type': 'OriginalFixtureFailure'}
        for status in ('passed', 'failed_or_interrupted'):
            report = {'status': status, 'browser_closed': False, 'failure': earlier}
            DRIVER.finalize_fixture_cleanup(report, -9, True, False)
            self.assertEqual(report['status'], 'failed_or_interrupted')
            self.assertIs(report['failure'], earlier)
            self.assertTrue(report['cleanup']['daemon_forced'])


if __name__ == '__main__':
    unittest.main()
