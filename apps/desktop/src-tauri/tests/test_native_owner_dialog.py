"""Runner/controller unit controls. No OS dialog or real Core success claimed."""
import copy
import importlib.util
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('owner_runner', ROOT / 'scripts/native-owner-dialog-smoke.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
COUNTS = {name: 0 for name in ['attempts', 'sessions', 'goal_scopes', 'goal_plans', 'goal_runs', 'goal_run_nodes', 'goal_run_reservations']}


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
    after['events'] = [{'aggregate_id': 'grant', 'kind': 'execution_grant.approved', 'source': 'user_action', 'revision': 1},
                       {'aggregate_id': proposal['id'], 'kind': 'approval.confirmed', 'source': 'local_owner_confirmation', 'revision': 2,
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
        proposal, _ = pending()
        with tempfile.TemporaryDirectory() as temporary:
            runtime = Path(temporary)
            with sqlite3.connect(runtime / 'baton.sqlite3') as conn:
                conn.execute('PRAGMA user_version=12')
                conn.execute('CREATE TABLE events(sequence INTEGER PRIMARY KEY, document TEXT)')
                conn.execute('CREATE TABLE approval_proposals(id TEXT, document TEXT, result TEXT)')
                conn.execute('CREATE TABLE execution_grants(document TEXT)')
                for table in COUNTS:
                    conn.execute(f'CREATE TABLE {table}(id TEXT)')
                conn.execute('INSERT INTO approval_proposals VALUES(?,?,NULL)', (proposal['id'], runner.json.dumps(proposal)))
            observed = runner.database(runtime, proposal['id'])
            self.assertIsNone(runner.pending_error(observed, proposal, 0))
            with sqlite3.connect(runtime / 'baton.sqlite3') as conn:
                conn.execute('INSERT INTO execution_grants VALUES(?)', ('{"id":"changed"}',))
            self.assertIsNotNone(runner.pending_error(runner.database(runtime, proposal['id']), proposal, 0))

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

    @unittest.skipIf(sys.platform == 'darwin', 'Linux-only negative; no fake macOS run')
    def test_linux_is_explicitly_not_run_before_creating_mutation_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'not-created'
            with self.assertRaisesRegex(RuntimeError, 'NOT_RUN'):
                runner.run(Path('/no-app'), Path('/no-daemon'), output)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
