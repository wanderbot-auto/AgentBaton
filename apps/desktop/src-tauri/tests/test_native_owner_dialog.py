"""Runner/controller unit controls. No OS dialog or real Core success claimed."""
import copy
import importlib.util
import os
import subprocess
import select
import time
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('owner_runner', ROOT / 'scripts/native-owner-dialog-smoke.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
COUNTS = {name: 0 for name in runner.ZERO_TABLES}


def pending():
    proposal = {'id': 'grant-proposal', 'kind': 'grant', 'consumed': False, 'revision': 1,
                'snapshot': {'intent': {'project_id': 'project', 'workspace_id': 'workspace', 'connection_id': 'connection',
                                        'max_attempts': 1, 'max_concurrency': 1, 'max_duration_ms': 1000},
                             'refs': {'workspace_identity': {'canonical_root': '/fixture', 'device': 1, 'inode': 2}, 'connection': {'revision': 2}}}}
    value = {'read_only': True, 'schema_version': 12, 'sequence': 9, 'proposal': proposal,
             'proposal_result': None, 'grants': [], 'events': [], 'counts': COUNTS.copy()}
    return proposal, value


def approved():
    proposal, before = pending()
    after = copy.deepcopy(before)
    after['sequence'] = 11
    after['proposal']['revision'] = 2
    after['proposal']['consumed'] = True
    grant = {'id': 'grant', 'approval_id': proposal['id'], 'project_id': 'project', 'root_grant_id': 'grant',
             'parent_id': None, 'depth': 0, 'revision': 1, 'revoked': False,
             'scope': {'workspace_ids': ['workspace'], 'connection_ids': ['connection'], 'max_attempts': 1,
                       'max_concurrency': 1, 'max_duration_ms': 1000},
             'workspace_bindings': {'workspace': proposal['snapshot']['refs']['workspace_identity']},
             'connection_bindings': {'connection': proposal['snapshot']['refs']['connection']}}
    after['grants'] = [grant]
    after['proposal_result'] = grant
    after['events'] = [{'aggregate_id': 'grant', 'kind': 'execution_grant.approved', 'source': 'user_action', 'revision': 1, 'sequence': 10},
                       {'aggregate_id': proposal['id'], 'kind': 'approval.confirmed', 'source': 'local_owner_confirmation', 'revision': 2, 'sequence': 11,
                        'payload': {'kind': 'grant', 'proposal_id': proposal['id'], 'consumed': True}}]
    return proposal, after, {'result': grant}


class OwnerRunnerControls(unittest.TestCase):
    def test_pending_counts_schema_sequence_and_real_proposal_are_strict(self):
        proposal, value = pending()
        self.assertIsNone(runner.pending_error(value, proposal, 9))
        for field, bad in [('sequence', True), ('schema_version', 11), ('read_only', 'true'), ('grants', [{}]), ('counts', {}), ('proposal_result', {'id': 'grant'})]:
            changed = copy.deepcopy(value)
            changed[field] = bad
            self.assertIsNotNone(runner.pending_error(changed, proposal, 9), field)
        changed = copy.deepcopy(value)
        changed['counts']['attempts'] = True
        self.assertIsNotNone(runner.pending_error(changed, proposal, 9))

    def test_real_readonly_sqlite_observation_rejects_authority_change(self):
        daemon = Path(os.environ['AGENTBATON_OWNER_FIXTURE_DAEMON']).resolve()
        self.assertTrue(daemon.is_file(), 'Provide the actual frozen/schema12 batond; never substitute test DDL')
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with runner.bootstrap(daemon, output) as fixture:
                runtime, proposal = fixture['runtime'], fixture['proposal']
                observed = runner.database(runtime, proposal['id'])
                self.assertIsNone(runner.pending_error(observed, proposal, observed['sequence']))
                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as actual:
                    actual.execute('PRAGMA query_only=ON')
                    tables = {row[0] for row in actual.execute("SELECT name FROM sqlite_master WHERE type='table'") if row[0].startswith('goal_')}
                    self.assertEqual(tables, set(runner.GOAL_TABLES))
                    clone = output / 'real-core-copy'
                    clone.mkdir()
                    with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                        actual.backup(copy_db)
                self.assertEqual(set(observed['counts']), set(runner.ZERO_TABLES))
                self.assertEqual(len(observed['counts']), 19)
                # Negative controls mutate only an isolated backup of the REAL
                # migration DB. The running Core database/schema is untouched.
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute("INSERT INTO goal_graph_jobs(id,run_id,node_id,plan_revision,state,attempt_id,workspace_id,document) VALUES('negative-job','absent-run','node',1,'running',NULL,'absent-workspace','{}')")
                self.assertIsNotNone(runner.pending_error(runner.database(clone, proposal['id']), proposal, observed['sequence']))
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute('DELETE FROM goal_graph_jobs')
                    copy_db.execute('PRAGMA user_version=13')
                self.assertIsNotNone(runner.pending_error(runner.database(clone, proposal['id']), proposal, observed['sequence']))
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute('PRAGMA user_version=12')
                    copy_db.execute('DROP TABLE goal_plan_heads')
                with self.assertRaisesRegex(sqlite3.OperationalError, 'goal_plan_heads'):
                    runner.database(clone, proposal['id'])
                self.assertIsNone(runner.pending_error(runner.database(runtime, proposal['id']), proposal, observed['sequence']))
                with self.assertRaisesRegex(ValueError, 'never a Grant'):
                    fixture['client'].call('approvals.finalize', {'proposal_id': proposal['id']})
            self.assertTrue(runner.json.loads((output / 'daemon-cleanup.json').read_text())['ok'])

    def test_exact_root_grant_commit_event_and_reply_bindings_are_required(self):
        proposal, after, response = approved()
        self.assertIsNone(runner.approved_error(after, proposal, 9, response))
        for field, wrong in [('counts', {}), ('sequence', 10), ('schema_version', 13), ('proposal_result', {}), ('grants', [])]:
            changed = copy.deepcopy(after)
            changed[field] = wrong
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response), field)
        changed = copy.deepcopy(after)
        changed['events'][1]['source'] = 'agent_output'
        self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response))
        changed = copy.deepcopy(after)
        changed['events'].append(changed['events'][1])
        self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response))

    def test_exact_event_order_cannot_be_missing_old_swapped_or_faked(self):
        proposal, after, response = approved()
        for first, second in [(None, 11), (10, None), (8, 11), (11, 10), (True, 11)]:
            changed = copy.deepcopy(after)
            changed['events'][0]['sequence'] = first
            changed['events'][1]['sequence'] = second
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response))

    def test_foreign_approval_and_expanded_scope_are_never_accepted(self):
        proposal, after, response = approved()
        changed = copy.deepcopy(after)
        changed['grants'][0]['scope']['workspace_ids'] = ['foreign']
        changed['proposal_result'] = changed['grants'][0]
        self.assertIsNotNone(runner.approved_error(changed, proposal, 9, {'result': changed['grants'][0]}))
        response = copy.deepcopy(response)
        response['result']['approval_id'] = 'other-proposal'
        self.assertIsNotNone(runner.approved_error(after, proposal, 9, response))

    def test_host_client_cannot_finalize_grant_or_generic_mutation(self):
        client = runner.NativeCoreClient(Path('/does-not-exist'))
        client.connection_proposal = 'generated-connection-only'
        with self.assertRaisesRegex(ValueError, 'never a Grant'):
            client.call('approvals.finalize', {'proposal_id': 'grant-proposal'})
        for method in ['sessions.create', 'goal_graph_runs.dispatch', 'grants.create']:
            with self.assertRaisesRegex(ValueError, 'Not a fixed'):
                client.call(method, {})

    def test_expected_message_keeps_full_production_formatter_and_utf8_snapshot(self):
        proposal, _ = pending()
        proposal['snapshot']['unicode'] = '你好\nexact'
        message = runner.expected_message(proposal, (ROOT / 'apps/desktop/src-tauri/src/approval.rs').read_text())
        self.assertIn('Grant this exact recipient', message)
        self.assertIn('你好\\nexact', message)
        self.assertIn('Cancel denies this action.', message)
        self.assertIn('"workspace_identity"', message)
        with self.assertRaises(ValueError):
            runner.expected_message(proposal, 'changed formatter')

    def test_cleanup_preserves_unexpected_exit_and_forced_kill_as_failure(self):
        for code in [0, 1]:
            child = subprocess.Popen([sys.executable, '-c', f'raise SystemExit({code})'], start_new_session=True)
            deadline = time.monotonic() + 3
            while not runner.exited_without_reaping(child):
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.02)
            status = runner.stop_owned(child, 'daemon')
            self.assertFalse(status['ok'])
            self.assertEqual(status['returncode'], code)
        child = subprocess.Popen([sys.executable, '-c', 'import time;time.sleep(30)'], start_new_session=True)
        status = runner.stop_owned(child, 'daemon')
        self.assertTrue(status['ok'])
        self.assertEqual(status['signals'], [{'signal': 'SIGTERM', 'result': 'signalled'}])
        self.assertEqual(status['returncode'], -runner.signal.SIGTERM)
        child = subprocess.Popen([sys.executable, '-c', "import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);print('ready',flush=True);time.sleep(30)"], start_new_session=True, stdout=subprocess.PIPE)
        try:
            self.assertTrue(select.select([child.stdout], [], [], 3)[0])
            self.assertEqual(child.stdout.readline(), b'ready\n')
            status = runner.stop_owned(child, 'daemon')
            self.assertFalse(status['ok'])
            self.assertEqual(status['returncode'], -runner.signal.SIGKILL)
            self.assertIn({'signal': 'SIGKILL', 'result': 'signalled'}, status['signals'])
        finally:
            child.stdout.close()
            if child.returncode is None:
                runner.stop_owned(child, 'daemon')


    def test_exited_app_live_owned_descendant_is_reaped_and_never_green(self):
        runner.enable_owned_subreaper()
        child = subprocess.Popen([sys.executable, '-c', "import os,time;pid=os.fork();time.sleep(30) if pid==0 else None"], start_new_session=True)
        deadline = time.monotonic() + 3
        while not runner.exited_without_reaping(child):
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.02)
        status = runner.stop_owned(child, 'app')
        self.assertFalse(status['ok'])
        self.assertFalse(status['initial_group_quiescent'])
        self.assertTrue(status['group_quiescent_before_reaping'])
        self.assertEqual(status['returncode'], 0)
        self.assertEqual(status['signals'], [{'signal': 'SIGTERM', 'result': 'signalled'}])
        if sys.platform.startswith('linux'):
            self.assertGreaterEqual(status['adopted_owned_group_children_reaped'], 1)

    def test_observation_failure_still_reaps_and_reaped_identity_is_not_signalled(self):
        child = subprocess.Popen([sys.executable, '-c', 'import time;time.sleep(30)'], start_new_session=True)
        actual = runner.group_helpers()
        class FirstObservationFails:
            def __init__(self):
                self.first = True
            def owned_child_exited(self, pid):
                if self.first:
                    self.first = False
                    raise OSError('retained injected observation failure')
                return actual.owned_child_exited(pid)
            owned_group_is_quiescent = staticmethod(actual.owned_group_is_quiescent)
        controls = FirstObservationFails()
        with mock.patch.object(runner, 'group_helpers', return_value=controls):
            status = runner.stop_owned(child, 'daemon')
        self.assertFalse(status['ok'])
        self.assertEqual(status['returncode'], -runner.signal.SIGTERM)
        self.assertTrue(status['errors'])
        # Leader was already reaped by the completed cleanup; never signal an
        # unknown group which could have reused that PID numerically.
        with mock.patch.object(runner.os, 'killpg') as signal_group:
            self.assertFalse(runner.stop_owned(child, 'daemon')['ok'])
            signal_group.assert_not_called()

    @unittest.skipIf(sys.platform == 'darwin', 'Linux-only negative; no fake macOS run')
    def test_linux_is_explicitly_not_run_before_creating_mutation_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'not-created'
            with self.assertRaisesRegex(RuntimeError, 'NOT_RUN'):
                runner.run(Path('/no-app'), Path('/no-daemon'), output)
            self.assertFalse(output.exists())


class BootstrapPathControls(unittest.TestCase):
    def daemon(self):
        daemon = Path(os.environ['AGENTBATON_OWNER_FIXTURE_DAEMON']).resolve()
        self.assertTrue(daemon.is_file(), 'Use the real frozen schema12 daemon')
        return daemon

    def test_alias_parent_real_core_refuses_raw_alias_but_canonical_bootstrap_is_ready(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            real = base / 'real'
            real.mkdir(mode=0o700)
            alias = base / 'alias'
            alias.symlink_to(real, target_is_directory=True)
            denied = real / 'denied-runtime'
            denied.mkdir(mode=0o700)
            env = {**os.environ, 'BATON_RUNTIME_DIR': str(alias / 'denied-runtime')}
            child = subprocess.Popen([str(self.daemon())], env=env, start_new_session=True,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 3
                while not runner.exited_without_reaping(child):
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                cleanup = runner.stop_owned(child, 'daemon')
                _, stderr = child.communicate(timeout=1)
                self.assertEqual(cleanup['returncode'], 1)
                self.assertFalse(cleanup['ok'])
                self.assertEqual(cleanup['signals'], [])
                self.assertIn(b'Runtime paths must not contain symbolic links', stderr)
            finally:
                if child.returncode is None:
                    runner.stop_owned(child, 'daemon')
                child.stderr.close()
            output = base / 'evidence'
            output.mkdir(mode=0o700)
            with mock.patch.object(runner.tempfile, 'tempdir', str(alias)):
                with runner.bootstrap(self.daemon(), output) as fixture:
                    runtime = fixture['runtime']
                    self.assertEqual(runtime, runtime.resolve(strict=True))
                    observed = runner.database(runtime, fixture['proposal']['id'])
                    self.assertIsNone(runner.pending_error(observed, fixture['proposal'], observed['sequence']))
                    paths = runner.json.loads((output / 'bootstrap-paths.json').read_text())
                    self.assertIn('/alias/', paths['requested_root'])
                    self.assertNotIn('/alias/', paths['root']['path'])
                    for name in ['root', 'runtime']:
                        self.assertEqual(paths[name]['uid'], os.geteuid())
                        self.assertEqual(paths[name]['mode'], 0o700)
            state = runner.json.loads((output / 'bootstrap-state.json').read_text())
            self.assertEqual(state['status'], 'completed')
            self.assertTrue(state['cleanup_ok'])
            for name in ['stdout', 'stderr']:
                self.assertEqual(state[name]['mode'], 0o600)

    def test_long_private_temp_parent_uses_short_canonical_real_core_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            long_parent = base / ('long-parent-' + 'x' * 120)
            long_parent.mkdir(mode=0o700)
            output = base / 'evidence'
            output.mkdir(mode=0o700)
            with mock.patch.object(runner.tempfile, 'tempdir', str(long_parent)):
                with runner.bootstrap(self.daemon(), output) as fixture:
                    paths = runner.json.loads((output / 'bootstrap-paths.json').read_text())
                    self.assertGreater(paths['initial_socket_utf8_bytes'], 103)
                    self.assertTrue(paths['private_short_fallback'])
                    self.assertLessEqual(paths['socket']['utf8_bytes'], 103)
                    self.assertEqual(fixture['runtime'], fixture['runtime'].resolve(strict=True))
                    self.assertIsInstance(fixture['client'].call('system.snapshot', {}), dict)
            self.assertTrue(runner.json.loads((output / 'bootstrap-state.json').read_text())['cleanup_ok'])

    def test_bootstrap_retains_original_exit_and_private_stderr_without_yielding(self):
        for code in [0, 23]:
            with self.subTest(code=code), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                output = base / 'evidence'
                output.mkdir(mode=0o700)
                failing = base / 'own-exit-fixture'
                failing.write_text(f'#!/bin/sh\nprintf "owned stdout marker\\n"\nprintf "owned stderr marker\\n" >&2\nexit {code}\n')
                failing.chmod(0o700)
                with self.assertRaisesRegex(RuntimeError, 'Fixture daemon exited before readiness'):
                    with runner.bootstrap(failing, output):
                        self.fail('An exited daemon must never yield a ready fixture')
                state = runner.json.loads((output / 'bootstrap-state.json').read_text())
                cleanup = runner.json.loads((output / 'daemon-cleanup.json').read_text())
                self.assertEqual(state['status'], 'failed')
                self.assertEqual(state['phase'], 'daemon_readiness')
                self.assertEqual(state['original_failure']['message'], 'Fixture daemon exited before readiness')
                self.assertEqual(state['daemon_exitcode'], code)
                self.assertFalse(state['cleanup_ok'])
                self.assertFalse(cleanup['ok'])
                self.assertEqual(cleanup['signals'], [])
                self.assertEqual((output / 'daemon-stderr.log').read_bytes(), b'owned stderr marker\n')
                self.assertEqual((output / 'daemon-stdout.log').read_bytes(), b'owned stdout marker\n')
                self.assertEqual(state['stderr']['mode'], 0o600)
                self.assertEqual(state['stderr']['sha256'], runner.hashlib.sha256(b'owned stderr marker\n').hexdigest())
                self.assertEqual(runner.json.loads((output / 'core-bootstrap-transcript.json').read_text()), [])

    def test_bootstrap_refuses_nonprivate_or_nonempty_evidence_without_overwriting(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'evidence'
            output.mkdir(mode=0o700)
            marker = output / 'unknown-existing.txt'
            marker.write_bytes(b'keep existing evidence')
            with mock.patch.object(runner.subprocess, 'Popen') as spawn:
                with self.assertRaisesRegex(ValueError, 'fresh empty'):
                    with runner.bootstrap(Path('/not-started'), output):
                        self.fail('Nonempty evidence must be refused')
                spawn.assert_not_called()
            self.assertEqual(marker.read_bytes(), b'keep existing evidence')
            marker.unlink()
            output.chmod(0o755)
            with self.assertRaisesRegex(ValueError, 'private and owned'):
                with runner.bootstrap(Path('/not-started'), output):
                    self.fail('Nonprivate evidence must be refused')


if __name__ == '__main__':
    unittest.main()
