"""Schema and tool-identity regression controls for the current Goal Execution driver."""
from contextlib import closing
import importlib.util
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, call, patch


spec = importlib.util.spec_from_file_location(
    'goal_execution_real_ipc', Path(__file__).with_name('goal-execution-real-ipc.py')
)
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class CurrentCandidateTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.source = self.root / 'source'
        self.source.mkdir()
        self.git('init', '--quiet')
        (self.source / 'tracked').write_text('fixture source\n')
        self.git('add', 'tracked')
        self.git('commit', '--quiet', '-m', 'candidate source')
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.staged = self.root / 'staged'
        self.manifest = {
            'schema': 19,
            'git_head': self.git('rev-parse', 'HEAD'),
            'git_tree': self.git('rev-parse', 'HEAD^{tree}'),
            'dirty': False,
            'binaries': {},
        }
        for name in ('batond', 'baton'):
            file = self.tools / name
            payload = (name + '\n').encode()
            file.write_bytes(payload)
            file.chmod(0o755)
            self.manifest['binaries'][name] = driver.sha(payload)

    def git(self, *args):
        env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null'}
        return subprocess.check_output(
            ['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
             '-c', 'core.hooksPath=/dev/null', *args], cwd=self.source, env=env, text=True
        ).strip()

    def admit(self, raw=None, tool='batond'):
        with patch.object(driver, 'ROOT', self.source):
            return driver.candidate_tools(
                self.tools / tool, raw if raw is not None else json.dumps(self.manifest).encode(), self.staged
            )

    def test_current_clean_source_copies_only_the_witnessed_sibling_tools(self):
        self.manifest['binaries']['batond'] = {'sha256': self.manifest['binaries']['batond']}
        manifest, tools, hashes = self.admit()
        self.assertEqual(manifest, self.manifest)
        self.assertEqual(set(tools), {'batond', 'baton'})
        for name, staged in tools.items():
            with self.subTest(name=name):
                self.assertEqual(staged, self.staged / name)
                self.assertEqual(staged.read_bytes(), (self.tools / name).read_bytes())
                self.assertEqual(staged.stat().st_mode & 0o777, 0o500)
                self.assertEqual(hashes[name], driver.sha(staged.read_bytes()))
        (self.tools / 'baton').write_bytes(b'replacement')
        self.assertEqual(driver.sha(tools['baton'].read_bytes()), hashes['baton'])

    def test_manifest_schema_requires_integer_19_and_unique_fields(self):
        for value in (None, False, True, 16, 17, 18, 20, '19', 19.0):
            with self.subTest(value=value), self.assertRaisesRegex(AssertionError, 'schema19'):
                self.manifest['schema'] = value
                self.admit()
            self.assertFalse(self.staged.exists())
        self.manifest['schema'] = 19
        raw = json.dumps(self.manifest).removesuffix('}') + ', "schema": 19}'
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            self.admit(raw.encode())

    def test_wrong_source_tree_or_dirty_checkout_is_refused(self):
        for key, value in (('git_head', '0' * 40), ('git_tree', '0' * 40), ('dirty', 0)):
            original = self.manifest[key]
            self.manifest[key] = value
            with self.subTest(key=key), self.assertRaisesRegex(AssertionError, 'tracked checkout'):
                self.admit()
            self.manifest[key] = original
        (self.source / 'tracked').write_text('different source\n')
        with self.assertRaisesRegex(AssertionError, 'tracked checkout'):
            self.admit()

    def test_missing_companion_digest_or_wrong_daemon_role_is_refused(self):
        with self.assertRaisesRegex(AssertionError, 'sibling build'):
            self.admit(tool='baton')
        del self.manifest['binaries']['baton']
        with self.assertRaisesRegex(AssertionError, 'Both Core tool identities'):
            self.admit()

    def test_mixed_companion_bytes_are_refused(self):
        (self.tools / 'baton').write_bytes(b'different build')
        with self.assertRaisesRegex(AssertionError, 'bytes or identity'):
            self.admit()

    def test_duplicate_tool_images_are_refused_even_if_the_hashes_match(self):
        (self.tools / 'baton').write_bytes((self.tools / 'batond').read_bytes())
        self.manifest['binaries']['baton'] = self.manifest['binaries']['batond']
        with self.assertRaisesRegex(AssertionError, 'same bytes'):
            self.admit()

    def test_symlinked_companion_from_another_directory_is_refused(self):
        alt = self.root / 'same-hash-companion'
        (self.tools / 'baton').rename(alt)
        (self.tools / 'baton').symlink_to(alt)
        with self.assertRaises(OSError):
            self.admit()


class CurrentCoreWitnessTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.database = self.root / 'baton.sqlite3'
        self.snapshot = {'api_version': 1, 'database_schema_version': 19, 'sequence': 0, 'tasks': []}

    def set_schema(self, schema):
        with closing(sqlite3.connect(self.database)) as db:
            db.execute(f'PRAGMA user_version={schema:d}')

    def test_snapshot_numeric_fields_have_exact_types(self):
        self.assertIs(driver.require_current_snapshot(self.snapshot), self.snapshot)
        for key, values in {
            'api_version': (True, 1.0, '1', 2, None),
            'database_schema_version': (18, 20, '19', 19.0, True, None),
            'sequence': (False, '0', 0.0, -1, 1 << 53, None),
        }.items():
            for value in values:
                with self.subTest(key=key, value=value), self.assertRaisesRegex(AssertionError, 'Core snapshot'):
                    driver.require_current_snapshot({**self.snapshot, key: value})

    def test_sqlite_uses_existing_read_only_schema_19(self):
        for schema in (0, 17, 18, 20):
            self.set_schema(schema)
            with self.subTest(schema=schema), self.assertRaisesRegex(AssertionError, 'SQLite schema 19'):
                with driver.readonly_database(self.root):
                    self.fail('unexpected writable or wrong-version connection')
        self.set_schema(19)
        before = self.database.read_bytes()
        with driver.readonly_database(self.root) as db:
            with self.assertRaises(sqlite3.OperationalError):
                db.execute('PRAGMA user_version=20')
        self.assertEqual(self.database.read_bytes(), before)

    def test_missing_database_is_not_created(self):
        with self.assertRaises(sqlite3.OperationalError):
            with driver.readonly_database(self.root):
                self.fail('unexpected connection')
        self.assertFalse(self.database.exists())

    def test_cli_snapshot_and_sql_sequence_must_match_private_ipc(self):
        companion = self.root / 'baton'
        for snapshot in ({**self.snapshot, 'tasks': ['another daemon']}, {**self.snapshot, 'sequence': False}):
            with self.subTest(snapshot=snapshot), patch.object(
                driver.subprocess, 'run', return_value=subprocess.CompletedProcess(
                    [], 0, stdout=json.dumps({'result': snapshot}).encode()
                )
            ), patch.object(driver, 'database_observation') as database:
                with self.assertRaises(AssertionError):
                    driver.verify_private_core(self.root, companion, self.snapshot)
                database.assert_not_called()
        reply = subprocess.CompletedProcess([], 0, stdout=json.dumps({'result': self.snapshot}).encode())
        with patch.object(driver.subprocess, 'run', return_value=reply) as invoked:
            with patch.object(driver, 'database_observation', return_value={'sequence': 1}):
                with self.assertRaisesRegex(AssertionError, 'sequences disagree'):
                    driver.verify_private_core(self.root, companion, self.snapshot)
            with patch.object(driver, 'database_observation', return_value={'sequence': 0}):
                self.assertEqual(driver.verify_private_core(self.root, companion, self.snapshot),
                                 {'api_version': 1, 'sqlite_schema_version': 19, 'sequence': 0,
                                  'matched_cli_snapshot': True, 'read_only': True})
        self.assertEqual(invoked.call_args.args[0], [str(companion), 'snapshot'])
        self.assertEqual(invoked.call_args.kwargs['env']['BATON_RUNTIME_DIR'], str(self.root))


class OwnedTransportCleanupTest(unittest.TestCase):
    def setUp(self):
        self.cleanup = {'browser_closed': False, 'playwright_stopped': False,
                        'http_server_closed': False, 'http_thread_joined': False,
                        'daemon_joined': False, 'daemon_exit': None, 'daemon_forced_kill': False, 'errors': []}
        self.browser = Mock()
        self.browser.is_connected.return_value = False
        self.server = Mock()
        self.thread = Mock()
        self.thread.is_alive.side_effect = [True, False]
        self.daemon = Mock(pid=4321)
        self.daemon.wait.return_value = 0

    def finish(self):
        with driver.owned_browser(self.browser, self.cleanup):
            pass
        self.cleanup['playwright_stopped'] = True
        driver.stop_transports(self.server, self.thread, self.daemon, self.cleanup)

    def test_pass_requires_confirmed_browser_http_and_daemon_cleanup(self):
        self.finish()
        self.assertTrue(driver.clean_exit(self.cleanup))
        self.browser.close.assert_called_once_with()
        self.server.shutdown.assert_called_once_with()
        self.server.server_close.assert_called_once_with()
        self.thread.join.assert_called_once_with(timeout=5)
        self.daemon.terminate.assert_called_once_with()
        self.daemon.wait.assert_called_once_with(timeout=5)
        for key in ('browser_closed', 'playwright_stopped', 'http_server_closed', 'http_thread_joined', 'daemon_joined'):
            with self.subTest(key=key):
                self.assertFalse(driver.clean_exit({**self.cleanup, key: False}))

    def test_daemon_wait_nonzero_is_failure_even_when_joined(self):
        self.daemon.wait.return_value = 1
        self.finish()
        self.assertTrue(self.cleanup['daemon_joined'])
        self.assertEqual(self.cleanup['daemon_exit'], 1)
        self.assertFalse(driver.clean_exit(self.cleanup))

    def test_forced_daemon_kill_cannot_pass_even_if_wait_returns_zero(self):
        self.daemon.wait.side_effect = [subprocess.TimeoutExpired('batond', 5), 0]
        with patch.object(driver.os, 'killpg') as kill:
            self.finish()
        kill.assert_called_once_with(4321, signal.SIGKILL)
        self.assertEqual(self.daemon.wait.call_args_list, [call(timeout=5), call()])
        self.assertTrue(self.cleanup['daemon_joined'])
        self.assertTrue(self.cleanup['daemon_forced_kill'])
        self.assertFalse(driver.clean_exit(self.cleanup))

    def test_lingering_http_thread_and_still_connected_browser_cannot_pass(self):
        self.thread.is_alive.side_effect = [True, True]
        self.browser.is_connected.return_value = True
        self.finish()
        self.assertFalse(self.cleanup['http_thread_joined'])
        self.assertFalse(self.cleanup['browser_closed'])
        self.assertTrue(self.cleanup['daemon_joined'])
        self.assertFalse(driver.clean_exit(self.cleanup))

    def test_close_exception_preserves_original_failure_and_other_cleanup(self):
        self.browser.close.side_effect = OSError('test browser close failure')
        with self.assertRaisesRegex(RuntimeError, 'primary scenario'):
            with driver.owned_browser(self.browser, self.cleanup):
                raise RuntimeError('primary scenario')
        self.server.shutdown.side_effect = OSError('test HTTP shutdown failure')
        driver.stop_transports(self.server, self.thread, self.daemon, self.cleanup)
        self.server.server_close.assert_called_once_with()
        self.daemon.wait.assert_called_once_with(timeout=5)
        self.assertEqual(self.cleanup['errors'], [
            {'stage': 'browser_close', 'type': 'OSError'},
            {'stage': 'http_shutdown', 'type': 'OSError'},
        ])
        self.assertFalse(driver.clean_exit(self.cleanup))


if __name__ == '__main__':
    unittest.main()
