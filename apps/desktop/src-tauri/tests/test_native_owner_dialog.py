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
    value = {'read_only': True, 'schema_version': 14, 'sequence': 9, 'proposal': proposal,
             'proposal_result': None, 'grants': [], 'events': [], 'counts': COUNTS.copy()}
    return proposal, value


def approved():
    proposal, before = pending()
    after = copy.deepcopy(before)
    after['sequence'] = 11
    after['proposal']['revision'] = 2
    after['proposal']['consumed'] = True
    grant = {'id': 'grant', 'approval_id': 'independent-grant-approval', 'project_id': 'project', 'root_grant_id': 'grant',
             'parent_id': None, 'depth': 0, 'revision': 1, 'revoked': False,
             'scope': {'workspace_ids': ['workspace'], 'connection_ids': ['connection'], 'max_attempts': 1,
                       'max_concurrency': 1, 'max_duration_ms': 1000, 'host_ids': ['local'],
                       'recipients': [], 'network': {'mode': 'deny'}, 'max_depth': 0, 'max_children': 0},
             'enforcement': runner.grant_enforcement(),
             'workspace_bindings': {'workspace': proposal['snapshot']['refs']['workspace_identity']},
             'connection_bindings': {'connection': proposal['snapshot']['refs']['connection']}}
    after['grants'] = [grant]
    after['proposal_result'] = grant
    after['events'] = [{'aggregate_id': 'grant', 'kind': 'execution_grant.approved', 'source': 'user_action', 'revision': 1, 'sequence': 10,
                        'payload': {key: grant[field] for key, field in [('grant_id', 'id'), ('project_id', 'project_id'), ('root_grant_id', 'root_grant_id'), ('approval_id', 'approval_id'), ('scope', 'scope'), ('enforcement', 'enforcement')]}},
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
        self.assertTrue(daemon.is_file(), 'Provide the actual frozen/schema14 batond; never substitute test DDL')
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
                    factory_tables = {row[0] for row in actual.execute("SELECT name FROM sqlite_master WHERE type='table'") if row[0].startswith('factory_')}
                    self.assertEqual(factory_tables, set(runner.FACTORY_TABLES))
                    clone = output / 'real-core-copy'
                    clone.mkdir()
                    with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                        actual.backup(copy_db)
                self.assertEqual(set(observed['counts']), set(runner.ZERO_TABLES))
                self.assertEqual(len(observed['counts']), 30)
                for table in runner.FACTORY_TABLES:
                    missing_copy = output / ('missing-' + table)
                    missing_copy.mkdir()
                    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as actual:
                        actual.execute('PRAGMA query_only=ON')
                        with sqlite3.connect(missing_copy / 'baton.sqlite3') as copy_db:
                            actual.backup(copy_db)
                    with sqlite3.connect(missing_copy / 'baton.sqlite3') as copy_db:
                        copy_db.execute('DROP TABLE ' + table)
                    with self.assertRaisesRegex(sqlite3.OperationalError, table):
                        runner.database(missing_copy, proposal['id'])
                # Negative controls mutate only an isolated backup of the REAL
                # migration DB. The running Core database/schema is untouched.
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute("INSERT INTO goal_graph_jobs(id,run_id,node_id,plan_revision,state,attempt_id,workspace_id,document) VALUES('negative-job','absent-run','node',1,'running',NULL,'absent-workspace','{}')")
                self.assertIsNotNone(runner.pending_error(runner.database(clone, proposal['id']), proposal, observed['sequence']))
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute('DELETE FROM goal_graph_jobs')
                    copy_db.execute('PRAGMA user_version=15')
                self.assertIsNotNone(runner.pending_error(runner.database(clone, proposal['id']), proposal, observed['sequence']))
                with sqlite3.connect(clone / 'baton.sqlite3') as copy_db:
                    copy_db.execute('PRAGMA user_version=14')
                    copy_db.execute('DROP TABLE goal_plan_heads')
                with self.assertRaisesRegex(sqlite3.OperationalError, 'goal_plan_heads'):
                    runner.database(clone, proposal['id'])
                self.assertIsNone(runner.pending_error(runner.database(runtime, proposal['id']), proposal, observed['sequence']))
                with self.assertRaisesRegex(ValueError, 'never a Grant'):
                    fixture['client'].call('approvals.finalize', {'proposal_id': proposal['id']})
            self.assertTrue(runner.json.loads((output / 'daemon-cleanup.json').read_text())['ok'])

    def test_real_core_grant_contract_binds_independent_approval_and_owner_proposal(self):
        daemon = Path(os.environ['AGENTBATON_OWNER_FIXTURE_DAEMON']).resolve()
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with runner.bootstrap(daemon, output) as fixture:
                runtime, proposal = fixture['runtime'], fixture['proposal']
                before = runner.database(runtime, proposal['id'])
                self.assertIsNone(runner.pending_error(before, proposal, before['sequence']))
                # Private test-only Owner fixture. The host bootstrap client
                # STILL cannot finalize a Grant; no OS/human approval is claimed.
                frame = {'api_version': 1, 'request_id': str(runner.uuid.uuid4()),
                         'token': (runtime / 'session.token').read_text(),
                         'method': 'approvals.finalize', 'params': {'proposal_id': proposal['id']},
                         'expected_revision': proposal['revision'],
                         'idempotency_key': 'proposal:' + proposal['id']}
                with runner.socket.socket(runner.socket.AF_UNIX) as stream:
                    stream.settimeout(20)
                    stream.connect(str(runtime / 'batond.sock'))
                    stream.sendall(runner.json.dumps(frame).encode() + b'\n')
                    raw = bytearray()
                    while not raw.endswith(b'\n'):
                        chunk = stream.recv(65536)
                        self.assertTrue(chunk)
                        self.assertLessEqual(len(raw) + len(chunk), 4 * 1024 * 1024)
                        raw.extend(chunk)
                response = runner.json.loads(raw)
                after = runner.database(runtime, proposal['id'])
                print('REAL_CORE_GRANT_CONTRACT=' + runner.json.dumps({'proposal': proposal, 'after': after, 'response': response}))
                self.assertIsNone(runner.approved_error(after, proposal, before['sequence'], response))
            self.assertTrue(runner.json.loads((output / 'daemon-cleanup.json').read_text())['ok'])

    def test_exact_root_grant_commit_event_and_reply_bindings_are_required(self):
        proposal, after, response = approved()
        self.assertIsNone(runner.approved_error(after, proposal, 9, response))
        for field, wrong in [('counts', {}), ('sequence', 10), ('schema_version', 12), ('proposal_result', {}), ('grants', [])]:
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

    def test_schema14_requires_original19_plus_all_eleven_factory_zero_tables(self):
        proposal, before = pending()
        _, after, response = approved()
        self.assertEqual(len(runner.ZERO_TABLES), 30)
        self.assertEqual(len(runner.FACTORY_TABLES), 11)
        self.assertEqual(set(runner.ZERO_TABLES), {'attempts', 'sessions', *runner.GOAL_TABLES, *runner.FACTORY_TABLES})
        for schema in [11, 12, 13, 15, 13.0, 14.0, True, '13', '14', None]:
            changed = copy.deepcopy(before); changed['schema_version'] = schema
            self.assertIsNotNone(runner.pending_error(changed, proposal, 9), schema)
            changed = copy.deepcopy(after); changed['schema_version'] = schema
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response), schema)
        for table in runner.ZERO_TABLES:
            for wrong in [1, -1, None, True, False, '0', 0.0]:
                changed = copy.deepcopy(before); changed['counts'][table] = wrong
                self.assertIsNotNone(runner.pending_error(changed, proposal, 9), table)
                changed = copy.deepcopy(after); changed['counts'][table] = wrong
                self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response), table)
            changed = copy.deepcopy(before); del changed['counts'][table]
            self.assertIsNotNone(runner.pending_error(changed, proposal, 9), table)
            changed = copy.deepcopy(after); del changed['counts'][table]
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response), table)
        for value in [before, after]:
            value['counts']['unknown_factory_authority'] = 0
        self.assertIsNotNone(runner.pending_error(before, proposal, 9))
        self.assertIsNotNone(runner.approved_error(after, proposal, 9, response))

    def test_foreign_approval_and_expanded_scope_are_never_accepted(self):
        proposal, after, response = approved()
        changed = copy.deepcopy(after)
        changed['grants'][0]['scope']['workspace_ids'] = ['foreign']
        changed['proposal_result'] = changed['grants'][0]
        self.assertIsNotNone(runner.approved_error(changed, proposal, 9, {'result': changed['grants'][0]}))
        response = copy.deepcopy(response)
        response['result']['approval_id'] = 'other-proposal'
        self.assertIsNotNone(runner.approved_error(after, proposal, 9, response))

    def test_opaque_approval_id_is_bound_across_complete_records_and_event_payload(self):
        proposal, after, response = approved()
        self.assertNotEqual(after['grants'][0]['approval_id'], proposal['id'])
        for bad in [None, '', ' ', 'x' * 513, '批' * 171, True]:
            changed = copy.deepcopy(after)
            changed['grants'][0]['approval_id'] = bad
            changed['events'][0]['payload']['approval_id'] = bad
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, {'result': changed['grants'][0]}))
        # Opaque role IDs are not required to be different either: consistency
        # with the persisted Grant, not an invented equality/inequality, binds it.
        changed = copy.deepcopy(after)
        changed['grants'][0]['approval_id'] = proposal['id']
        changed['events'][0]['payload']['approval_id'] = proposal['id']
        self.assertIsNone(runner.approved_error(changed, proposal, 9, {'result': changed['grants'][0]}))

    def test_each_grant_result_and_unique_event_association_is_exact(self):
        proposal, after, response = approved()
        for layer in ['database_grant', 'proposal_result', 'response']:
            changed = copy.deepcopy(after)
            changed['proposal_result'] = copy.deepcopy(changed['proposal_result'])
            reply = copy.deepcopy(response)
            target = {'database_grant': changed['grants'][0], 'proposal_result': changed['proposal_result'], 'response': reply['result']}[layer]
            target['approval_id'] = 'foreign-opaque-id'
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, reply), layer)
        for field in ['grant_id', 'project_id', 'root_grant_id', 'approval_id', 'scope', 'enforcement']:
            changed = copy.deepcopy(after)
            changed['events'][0]['payload'][field] = 'foreign'
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response), field)
        changed = copy.deepcopy(after)
        extra = copy.deepcopy(changed['events'][0])
        extra['aggregate_id'] = 'foreign-grant'
        changed['events'].append(extra)
        self.assertIsNotNone(runner.approved_error(changed, proposal, 9, response))

    def test_self_consistent_expanded_scope_or_false_enforcement_is_not_accepted(self):
        proposal, after, _ = approved()
        for container, field, wrong in [('scope', 'host_ids', ['foreign']), ('scope', 'recipients', ['new']),
                                         ('scope', 'network', {'mode': 'unrestricted'}), ('scope', 'max_depth', 1),
                                         ('scope', 'max_attempts', True), ('enforcement', 'os_filesystem_sandbox', True),
                                         ('enforcement', 'baton_mediated_scope', 1)]:
            changed = copy.deepcopy(after)
            changed['grants'][0][container][field] = wrong
            self.assertIsNotNone(runner.approved_error(changed, proposal, 9, {'result': changed['grants'][0]}), (container, field))

    def test_independent_json_records_reject_four_nested_boolean_number_tampers(self):
        # JSON decode gives distinct records, unlike deepcopy of an aliased
        # synthetic fixture. Native response/DB/event records are independent.
        cases = [('grant_event_scope', True), ('grant_event_enforcement', 1),
                 ('response_scope', True), ('proposal_result_scope', True)]
        for mode, wrong in cases:
            with self.subTest(mode=mode):
                proposal, after, response = runner.json.loads(runner.json.dumps(approved()))
                if mode == 'grant_event_scope':
                    after['events'][0]['payload']['scope']['max_attempts'] = wrong
                elif mode == 'grant_event_enforcement':
                    after['events'][0]['payload']['enforcement']['baton_mediated_scope'] = wrong
                elif mode == 'response_scope':
                    response['result']['scope']['max_attempts'] = wrong
                else:
                    after['proposal_result']['scope']['max_attempts'] = wrong
                self.assertIsNotNone(runner.approved_error(after, proposal, 9, response))

    def test_typed_json_equality_preserves_json_types_and_nested_values(self):
        for left, right in [(True, 1), (1, 1.0), (False, 0.0), ({'x': True}, {'x': 1}),
                            ([1, {'x': 2}], [1, {'x': 2.0}]), (None, False), ({'x': 1}, {'x': 1, 'y': None})]:
            self.assertFalse(runner.json_exact(left, right))
            self.assertFalse(runner.json_exact(right, left))
        self.assertTrue(runner.json_exact({'id': '你好', 'values': [True, 1, 1.0, None]},
                                          {'values': [True, 1, 1.0, None], 'id': '你好'}))
        self.assertFalse(runner.json_exact(float('nan'), float('nan')))
        self.assertFalse(runner.json_exact(float('inf'), float('inf')))

    def test_independent_snapshots_refs_and_float_payload_tampers_are_rejected(self):
        cases = ['snapshot_device', 'workspace_binding', 'connection_binding', 'response_float',
                 'event_float', 'confirmation_bool']
        for mode in cases:
            with self.subTest(mode=mode):
                proposal, after, response = runner.json.loads(runner.json.dumps(approved()))
                if mode == 'snapshot_device':
                    after['proposal']['snapshot']['refs']['workspace_identity']['device'] = True
                elif mode == 'workspace_binding':
                    for record in [after['grants'][0], after['proposal_result'], response['result']]:
                        record['workspace_bindings']['workspace']['device'] = True
                elif mode == 'connection_binding':
                    for record in [after['grants'][0], after['proposal_result'], response['result']]:
                        record['connection_bindings']['connection']['revision'] = 2.0
                elif mode == 'response_float':
                    response['result']['scope']['max_duration_ms'] = 1000.0
                elif mode == 'event_float':
                    after['events'][0]['payload']['scope']['max_duration_ms'] = 1000.0
                else:
                    after['events'][1]['payload']['consumed'] = 1
                self.assertIsNotNone(runner.approved_error(after, proposal, 9, response))
        proposal, pending_value = pending()
        pending_value = runner.json.loads(runner.json.dumps(pending_value))
        pending_value['proposal']['snapshot']['refs']['workspace_identity']['device'] = True
        self.assertIsNotNone(runner.pending_error(pending_value, proposal, 9))

    def test_scalar_schema_and_event_cas_types_never_coerce(self):
        cases = ['confirmed_revision_float', 'approved_schema_float', 'pending_schema_float',
                 'grant_revision_bool', 'grant_revision_float', 'reference_revision_bool',
                 'reference_revision_float']
        for mode in cases:
            with self.subTest(mode=mode):
                proposal, after, response = runner.json.loads(runner.json.dumps(approved()))
                if mode == 'pending_schema_float':
                    reference, value = pending()
                    value['schema_version'] = 12.0
                    self.assertIsNotNone(runner.pending_error(value, reference, 9))
                    continue
                if mode == 'confirmed_revision_float':
                    after['events'][1]['revision'] = 2.0
                elif mode == 'approved_schema_float':
                    after['schema_version'] = 12.0
                elif mode == 'grant_revision_bool':
                    after['events'][0]['revision'] = True
                elif mode == 'grant_revision_float':
                    after['events'][0]['revision'] = 1.0
                elif mode == 'reference_revision_bool':
                    proposal['revision'] = True
                else:
                    proposal['revision'] = 1.0
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
        self.assertTrue(daemon.is_file(), 'Use the real frozen schema14 daemon')
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


class PresentationControls(unittest.TestCase):
    def test_lossless_utf8_pages_preserve_whitespace_and_unicode(self):
        original = '  first\n\t你好🔒\r\n\0\x7f\u202elast  '
        prompts = runner.presentation_prompts(original)
        restored = ''.join(runner.json.loads(line.split(' ', 1)[1])
                           for page in prompts[:-1] for line in page.splitlines() if line.startswith('L'))
        self.assertEqual(restored, original)
        self.assertTrue(prompts[-1].startswith('FINAL display confirmation'))
        self.assertIn('你好🔒', ''.join(prompts))
        for prompt in prompts:
            self.assertLess(len(prompt), 1024)
            self.assertLessEqual(len(prompt.splitlines()), 12)
            self.assertTrue(all(line and not line[0].isspace() for line in prompt.splitlines()))

    def test_all_twelve_unicode_bidi_controls_are_visible_escaped_and_lossless(self):
        controls = [0x061c, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c,
                    0x202d, 0x202e, 0x2066, 0x2067, 0x2068, 0x2069]
        self.assertEqual(len(controls), 12)
        for code in controls:
            with self.subTest(code=hex(code)):
                original = f'recipient: abc{chr(code)}123 你好🔒'
                prompts = runner.presentation_prompts(original)
                displayed = ''.join(prompts)
                self.assertNotIn(chr(code), displayed)
                self.assertIn(chr(92) + f'u{code:04x}', displayed)
                self.assertIn('你好🔒', displayed)
                restored = ''.join(runner.json.loads(line.split(' ', 1)[1])
                                   for page in prompts[:-1] for line in page.splitlines() if line.startswith('L'))
                self.assertEqual(restored.encode('utf-8'), original.encode('utf-8'))

    def test_canonical_json_row_matches_production_rust_vector(self):
        self.assertEqual(runner.presentation_prompts(' \t你好🔒\n')[0],
            'Native review page 1/1\nOriginal UTF-8 bytes 0..13 of 13.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\nL00001 " \\t你好🔒\\n"')

    def test_existing_snapshot_limits_roundtrip_without_summary_or_truncation(self):
        for size in [0, 16_384, 1_048_576, 1_048_576 + 32_768]:
            original = 'x' * size
            prompts = runner.presentation_prompts(original)
            restored = ''.join(runner.json.loads(line.split(' ', 1)[1])
                               for page in prompts[:-1] for line in page.splitlines() if line.startswith('L'))
            self.assertEqual(restored, original)
            self.assertTrue(all(len(prompt) < 1024 for prompt in prompts))
        with self.assertRaises(ValueError):
            runner.presentation_prompts('x' * (1_048_576 + 32_769))

    def test_page_cas_and_index_counts_do_not_coerce_boolean_or_float(self):
        proposal = {'id': 'proposal', 'revision': 1}
        prompts = runner.presentation_prompts('complete message')
        value = {'phase': 'approve', 'presentation_index': 1, 'presentation_count': len(prompts),
                 'proposal_id': 'proposal', 'proposal_revision': 1, 'message': prompts[1],
                 'title': 'AgentBaton — Native owner approval', 'parent_matches': True,
                 'visible': True, 'complete_text_visible': True}
        self.assertIsNone(runner.presentation_error(value, proposal, prompts, 'approve', 1))
        for field, wrong in [('presentation_index', True), ('presentation_index', 1.0),
                             ('presentation_count', float(len(prompts))), ('proposal_revision', True),
                             ('proposal_revision', 1.0)]:
            changed = {**value, field: wrong}
            self.assertIsNotNone(runner.presentation_error(changed, proposal, prompts, 'approve', 1))

    def test_partial_or_reordered_foreign_or_offscreen_page_has_no_host_permit(self):
        proposal = {'id': 'proposal', 'revision': 3}
        prompts = runner.presentation_prompts('complete exact message')
        value = {'phase': 'approve', 'presentation_index': 0, 'presentation_count': len(prompts),
                 'proposal_id': 'proposal', 'proposal_revision': 3, 'message': prompts[0],
                 'title': 'AgentBaton — Native owner approval', 'parent_matches': True,
                 'visible': True, 'complete_text_visible': True}
        self.assertIsNone(runner.presentation_error(value, proposal, prompts, 'approve', 0))
        for field, wrong in [('message', prompts[0].lstrip()[:-1]), ('presentation_index', 1),
                             ('presentation_count', 1), ('proposal_id', 'foreign'), ('proposal_revision', 4),
                             ('visible', False), ('complete_text_visible', False), ('parent_matches', False)]:
            changed = {**value, field: wrong}
            self.assertIsNotNone(runner.presentation_error(changed, proposal, prompts, 'approve', 0))




# Explicit synthetic JSON fixtures below. They do not claim a Core/OS approval.
def factory_pending():
    grant_proposal, _, grant_response = approved()
    grant = copy.deepcopy(grant_response['result'])
    grant['scope']['max_duration_ms'] = 300000
    artifact = {'artifact_id': 'report', 'relative_path': 'report.txt', 'max_bytes': 1024, 'expected_sha256': None}
    policy = {'max_nodes': 1, 'max_revisions': 2, 'max_plan_bytes': 1024,
              'outputs': [{'artifact': artifact, 'validators': []}], 'writable_prefixes': ['report.txt'],
              'owner_acceptance_criteria': 'Complete 原文 / emoji😀 / {page} / no execution'}
    selection = {'root_task_id': 'task', 'expected_task_revision': 1, 'grant_id': grant['id'],
                 'expected_grant_revision': 1, 'connection_ids': ['connection'], 'workspace_ids': ['workspace'],
                 'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'max_bytes': 1024, 'expected_sha256': 'a' * 64}],
                 'outputs': [artifact], 'recipients': [], 'max_depth': 1, 'max_children': 1, 'max_total_nodes': 2,
                 'root_budget': {'attempts': 1, 'parallel': 1, 'runtime_ms': 1000, 'output_bytes': 1024, 'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None},
                 'project_budget': {'attempts': 1, 'parallel': 1, 'runtime_ms': 1000, 'output_bytes': 1024, 'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None},
                 'ttl_ms': 300000, 'capabilities': {'delegate_task': True, 'resume_parent_as_new_attempt': True, 'child_planning': True, 'child_goal_execution': False, 'create_workspace': False},
                 'child_planning_policy': policy}
    bindings = {'root_task': {'id': 'task', 'project_id': 'project', 'goal': 'Generated task', 'state': 'draft'},
                'approved_grant': grant, 'child_planning': {'schema_version': 1, 'authority_kind': 'factory_child_planning_only',
                  'policy': copy.deepcopy(policy), 'owner_rule_id': 'factory-child-owner-review', 'input_rule_id': 'factory-child-input-sha256',
                  'validator_definitions': {}, 'definition_digest': 'd' * 64, 'goal_execution_authorized': False, 'new_workspace_authorized': False},
                'unknown_preserved': {'controls': '\u061c\u2028\n', 'list': [False, 0, 1.5]}}
    proposal = {'id': 'factory-proposal', 'kind': 'factory', 'consumed': False, 'revision': 1,
                'snapshot': {'schema_version': 1, 'selection': selection, 'bindings': bindings, 'contract_digest': 'c' * 64},
                'created_at': 'generated', 'expires_at_ms': 9999999999999}
    before = {'read_only': True, 'schema_version': 14, 'sequence': 20, 'proposal': copy.deepcopy(proposal),
              'proposal_result': None, 'grants': [copy.deepcopy(grant)], 'events': [], 'counts': COUNTS.copy(),
              'grant_documents': {'grant': runner.json.dumps(grant)}, 'other_proposals': {'connection': 'original raw connection'},
              'task_documents': {'task': 'original raw task'}, 'scopes': [], 'dispatch_intents': 0}
    fixture = {'kind': runner.FACTORY_KIND, 'proposal': proposal, 'phase': 'cancel', 'review_nonce': 'f' * 32}
    return fixture, before


def factory_approved():
    fixture, before = factory_pending()
    proposal = fixture['proposal']
    scope = {'id': proposal['id'], 'approval_id': proposal['id'], 'root_task_id': 'task', 'project_id': 'project',
             'root_grant_id': 'grant', 'root_grant_revision': 1, 'revision': 1, 'revoked': False,
             'selection': copy.deepcopy(proposal['snapshot']['selection']), 'bindings': copy.deepcopy(proposal['snapshot']['bindings']),
             'contract_digest': proposal['snapshot']['contract_digest'], 'created_at': 'generated', 'expires_at_ms': 9999999999999}
    after = copy.deepcopy(before)
    after['sequence'] = 21
    after['proposal']['consumed'] = True
    after['proposal']['revision'] = 2
    after['proposal_result'] = copy.deepcopy(scope)
    after['scopes'] = [{'document': copy.deepcopy(scope), 'owner_sequence': 21}]
    after['counts']['factory_scopes'] = 1
    after['events'].append({'aggregate_id': proposal['id'], 'kind': 'approval.confirmed', 'source': 'local_owner_confirmation',
        'revision': 2, 'sequence': 21, 'payload': {'kind': 'factory', 'factory_scope_id': proposal['id'],
        'contract_digest': scope['contract_digest'], 'root_task_id': 'task', 'root_grant_id': 'grant',
        'original_grant_revision_unchanged': 1, 'spawned': False, 'authority': 'independent_task_delegation_only'}})
    return fixture, before, after, {'result': copy.deepcopy(scope)}


class FactoryPlanningControls(unittest.TestCase):
    @staticmethod
    def source():
        return (ROOT / 'apps/desktop/src-tauri/src/approval.rs').read_text()

    def test_complete_factory_outer_messages_contain_exact_snapshot_all_fields_and_utf8(self):
        fixture, _ = factory_pending()
        p = fixture['proposal']
        # Force a UTF8 split across the original12KiB outer page boundary.
        p['snapshot']['bindings']['unknown_long'] = '😀中{page}' * 1800
        messages = runner.factory_messages(p, self.source())
        segments = [message.split('\n\n', 2)[2] for message in messages[:-1]]
        self.assertEqual(''.join(segments).encode(), runner.compact(p['snapshot']))
        self.assertGreater(len(messages), 2)
        self.assertTrue(all('PLAN STORAGE ONLY' in m and '不授予子Goal执行' in m for m in messages))
        self.assertIn('FINAL independent Factory approval', messages[-1])
        self.assertNotIn('{}', messages[0].split('\n\n', 2)[1])

    def test_nested_fulltext_reconstruction_keeps_every_outer_callback_and_inner_final(self):
        fixture, _ = factory_pending()
        messages, pages, contexts = runner.factory_presentation(fixture['proposal'], self.source())
        for outer, message in enumerate(messages):
            group = [(p, c) for p, c in zip(pages, contexts) if c['outer_index'] == outer]
            rows = []
            for page, context in group:
                self.assertEqual(context['outer_count'], len(messages))
                for line in page.splitlines():
                    if line.startswith('L'):
                        rows.append(runner.json.loads(line.split(' ', 1)[1]))
            self.assertEqual(''.join(rows).encode(), message.encode())
            self.assertTrue(group[-1][1]['inner_final'])
            self.assertIn('FINAL display confirmation', group[-1][0])
        self.assertTrue(contexts[-1]['outer_final'] and contexts[-1]['inner_final'])

    def test_factory_compact_fixture_preserves_proposal_and_does_not_raise64k(self):
        fixture, before = factory_pending()
        p = fixture['proposal']
        value, pages, contexts = runner.factory_fixture(p, {'selection': p['snapshot']['selection'], 'idempotencyKey': runner.FACTORY_KEY}, Path('/generated'), Path('/evidence'), before, self.source(), 'cancel')
        self.assertTrue(runner.json_exact(value['proposal'], p))
        self.assertNotIn('presentation', value)
        self.assertNotIn('message', value)
        self.assertEqual(value['presentation_sha256'], runner.digest(pages))
        self.assertEqual(value['presentation_count'], len(contexts))
        p['snapshot']['bindings']['large_complete_field'] = 'x' * 65536
        with self.assertRaisesRegex(ValueError, '64KiB'):
            runner.factory_fixture(p, {}, Path('/g'), Path('/e'), before, self.source(), 'cancel')

    def test_each_factory_display_page_requires_original_grant_and_zero_finalize(self):
        fixture, before = factory_pending()
        self.assertIsNone(runner.factory_pending_error(before, fixture, before))
        for table in runner.ZERO_TABLES:
            changed = copy.deepcopy(before)
            changed['counts'][table] = 1
            self.assertIsNotNone(runner.factory_pending_error(changed, fixture, before), table)
        for key, wrong in [('sequence', 21), ('schema_version', 14.0), ('dispatch_intents', True),
                           ('grant_documents', {'grant': 'changed raw bytes'}), ('other_proposals', {}),
                           ('task_documents', {}), ('proposal_result', {'id': 'factory-proposal'})]:
            changed = copy.deepcopy(before)
            changed[key] = wrong
            self.assertIsNotNone(runner.factory_pending_error(changed, fixture, before), key)

    def test_exact_factory_commit_is_only_one_scope_one_event_no_other_mutation(self):
        fixture, before, after, reply = factory_approved()
        self.assertIsNone(runner.factory_approved_error(after, fixture, before, reply))
        for key, wrong in [('sequence', 22), ('sequence', 21.0), ('schema_version', 13),
                           ('dispatch_intents', 1), ('grant_documents', {}), ('other_proposals', {}), ('task_documents', {}),
                           ('scopes', []), ('events', []), ('counts', {})]:
            changed = copy.deepcopy(after)
            changed[key] = wrong
            self.assertIsNotNone(runner.factory_approved_error(changed, fixture, before, reply), key)
        for table in runner.ZERO_TABLES:
            changed = copy.deepcopy(after)
            changed['counts'][table] += 1
            self.assertIsNotNone(runner.factory_approved_error(changed, fixture, before, reply), table)

    def test_factory_receipt_policy_scope_digest_and_three_independent_records_exact(self):
        fixture, before, after, reply = factory_approved()
        for field, bad in [('id', 'foreign'), ('approval_id', 'foreign'), ('revision', True), ('root_grant_revision', 1.0),
                           ('revoked', 0), ('selection', {}), ('bindings', {}), ('contract_digest', 'e' * 64)]:
            changed = runner.json.loads(runner.json.dumps(reply))
            changed['result'][field] = bad
            self.assertIsNotNone(runner.factory_approved_error(after, fixture, before, changed), field)
        # Self-consistent expanded receipts still cannot exceed approved selection.
        for capability in ['child_goal_execution', 'create_workspace']:
            changed = copy.deepcopy(after); response = copy.deepcopy(reply)
            for value in [changed['proposal_result'], changed['scopes'][0]['document'], response['result']]:
                value['selection']['capabilities'][capability] = True
            self.assertIsNotNone(runner.factory_approved_error(changed, fixture, before, response))

    def test_outer_inner_nonce_title_and_exact_page_text_are_all_required(self):
        fixture, before = factory_pending()
        value, pages, contexts = runner.factory_fixture(fixture['proposal'], {}, Path('/g'), Path('/e'), before, self.source(), 'cancel')
        observed = {'phase': 'cancel', 'presentation_index': 0, 'presentation_count': len(pages),
                    'proposal_id': value['proposal']['id'], 'proposal_revision': 1, 'message': pages[0],
                    'title': runner.FACTORY_TITLE, 'parent_matches': True, 'visible': True, 'complete_text_visible': True,
                    'review_nonce': value['review_nonce'], 'factory_page': contexts[0]}
        self.assertIsNone(runner.factory_presentation_error(observed, value, pages, contexts, 0))
        for field, wrong in [('review_nonce', '0' * 32), ('factory_page', contexts[-1]), ('title', 'AgentBaton — Native owner approval'),
                             ('message', pages[0][:-1]), ('presentation_index', False), ('presentation_count', float(len(pages))),
                             ('complete_text_visible', False), ('proposal_revision', True)]:
            changed = copy.deepcopy(observed); changed[field] = wrong
            self.assertIsNotNone(runner.factory_presentation_error(changed, value, pages, contexts, 0), field)
        changed = copy.deepcopy(observed)
        changed['factory_page']['outer_index'] = 0.0
        self.assertIsNotNone(runner.factory_presentation_error(changed, value, pages, contexts, 0))

    def test_target_factory_finalization_is_never_available_to_bootstrap_host(self):
        class Client:
            runtime = Path('/no-runtime')
        grant, _ = pending()
        client = runner.FactoryBootstrapClient(Client(), grant)
        for params, key, revision in [({'proposal_id': 'factory-proposal'}, 'proposal:factory-proposal', 1),
                                       ({'proposal_id': grant['id']}, 'wrong-key', 1),
                                       ({'proposal_id': grant['id']}, 'proposal:' + grant['id'], True)]:
            with self.assertRaisesRegex(ValueError, 'NEVER Factory'):
                client.request('approvals.finalize', params, key, revision)
        for method in ['factory_runs.start', 'factory_child_plans.submit', 'sessions.start', 'grants.delegate']:
            with self.assertRaisesRegex(ValueError, 'fixed Factory'):
                client.request(method, {}, 'key')

    def test_factory_linux_is_not_run_and_creates_no_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'not-created'
            with mock.patch.object(runner.sys, 'platform', 'linux'):
                with self.assertRaisesRegex(RuntimeError, 'NOT_RUN'):
                    runner.run_factory(Path('/missing'), Path('/missing'), Path('/missing'), output)
            self.assertFalse(output.exists())

    def test_invalid_phase_cancel_index_unsupported_kind_cannot_become_grant(self):
        fixture, before = factory_pending()
        for phase, index in [('unknown', 0), ('cancel', True), ('cancel', 1.0), ('cancel', -1), ('cancel', 10000)]:
            with self.assertRaises(ValueError):
                runner.factory_fixture(fixture['proposal'], {}, Path('/g'), Path('/e'), before, self.source(), phase, index)
        source = (ROOT / 'apps/desktop/src-tauri/src/native_owner_dialog_test/macos.rs').read_text()
        self.assertIn('controller::prepare_factory(&mut expected)?', source)
        self.assertIn('expected["kind"] != "native_grant_os_dialog_fixture"', source)
        self.assertIn('factory_scope_approve', source)

    def test_presentation_mirror_and_native_production_sources_are_mechanically_bound(self):
        parent = (ROOT / 'apps/desktop/src-tauri/src/native_parent.rs').read_text()
        controller = (ROOT / 'apps/desktop/src-tauri/src/native_owner_dialog_test/controller.rs').read_text()
        original = parent.split('fn review_prompts(', 1)[1].split('\nfn present_complete(', 1)[0]
        mirror = controller.split('pub fn mirrored_review_prompts(', 1)[1].split('\npub fn validate(', 1)[0]
        self.assertEqual(''.join(original.split()), ''.join(mirror.split()))
        shipping = self.source().split('pub fn confirm_factory_and_finalize(', 1)[1].split('\n#[cfg(test)]', 1)[0]
        mirror_factory = controller.split('pub fn factory_messages(', 1)[1].split('pub fn prepare_factory(', 1)[0]
        # The three production action/page/final literal strings must all appear
        # byte-for-byte in the Rust test mirror. Full byte reconstruction tested above.
        literals = [runner.re.search(pattern, shipping, runner.re.S).group(1) for pattern in [
            r'let action = if .*?\{\s*("(?:[^"\\]|\\.)*")',
            r'let message\s*=\s*format!\(\s*("(?:[^"\\]|\\.)*")',
            r'if !confirm_native\(&format!\(\s*("(?:[^"\\]|\\.)*")']]
        for literal in literals:
            self.assertIn(literal, mirror_factory)

    def test_old_or_future_schema_refused_before_any_factory_resource_approval(self):
        proposal, initial = pending()
        for schema in [12, 13, 14.0, 15]:
            changed = copy.deepcopy(initial)
            changed['schema_version'] = schema
            with mock.patch.object(runner, 'database', return_value=changed), mock.patch.object(runner, 'FactoryBootstrapClient') as host:
                with self.assertRaisesRegex(ValueError, 'schema'):
                    runner.prepare_factory({'runtime': Path('/generated'), 'proposal': proposal, 'client': object()})
                host.assert_not_called()

    def test_real_core_factory_preparation_is_schema14_complete_and_never_finalizes_target(self):
        daemon = Path(os.environ['AGENTBATON_OWNER_FIXTURE_DAEMON']).resolve()
        self.assertTrue((daemon.parent / 'baton').is_file(), 'Use matching same-head14 tools, not an old companion')
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with runner.bootstrap(daemon, output, factory_planning=True) as generated:
                proposal, invoke = runner.prepare_factory(generated)
                before = runner.factory_database(generated['runtime'], proposal['id'])
                fixture, pages, contexts = runner.factory_fixture(proposal, invoke, generated['runtime'], output, before, self.source(), 'cancel')
                self.assertIsNone(runner.factory_pending_error(before, fixture, before))
                self.assertEqual(before['grants'][0]['scope']['max_duration_ms'], 300000)
                self.assertEqual(before['grants'][0]['scope']['max_children'], 0)
                self.assertEqual(before['counts']['factory_scopes'], 0)
                self.assertEqual(len(before['counts']), 30)
                self.assertGreater(len(pages), 2)
                self.assertEqual(contexts[-1]['outer_final'], True)
                self.assertEqual(proposal['snapshot']['bindings']['child_planning']['policy'], invoke['selection']['child_planning_policy'])
                self.assertFalse(any(row['method'] == 'approvals.finalize' and row['params'] == {'proposal_id': proposal['id']} for row in generated['client'].transcript))
                print('REAL_CORE_FACTORY_NATIVE_FIXTURE=' + runner.json.dumps({'fixture': fixture, 'before': before, 'dimension': 'actual Core preparation only; NO Native OS and NO Factory finalize'}))
            self.assertTrue(runner.json.loads((output / 'daemon-cleanup.json').read_text())['ok'])


if __name__ == '__main__':
    unittest.main()
