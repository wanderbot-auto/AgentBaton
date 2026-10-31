"""Fresh generated SQLite fee-kind controls; never retained product databases."""
import ast
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sqlite3
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_rework_fee_kind_generated_fixtures',
    ROOT / 'tests/infrastructure/test_factory_rework_ci_evidence.py')
F = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F)
P, SECRET = F.P, F.SECRET
ORIGINAL_BUDGET_SHA256 = '78feade13b1c0368714cb2eea24555f31ef4607e02e08bf2a71b4c561696fe68'
ADDITIONS = (
    "    out['fee_kind_observations'] = fee_kind_unavailable()\n",
    "        fee_records = []\n",
    "            fee_records.append(('graph', attempt, value))\n",
    "                fee_records.append(('factory', attempt, value))\n",
    "        out['fee_kind_observations'] = fee_kind_observations(conn, goal, scope, child, record, fee_records, totals)\n",
)


def original_budget_source(text):
    tree = ast.parse(text)
    node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'budget_usage')
    source = ast.get_source_segment(text, node)
    for addition in ADDITIONS:
        if source.count(addition) != 1: raise ValueError('Fee observation insertion count changed')
        source = source.replace(addition, '', 1)
    if hashlib.sha256(source.encode()).hexdigest() != ORIGINAL_BUDGET_SHA256:
        raise ValueError('Original aggregate/seen/guards were changed')
    return source


def original_budget_function():
    source = original_budget_source((ROOT / 'scripts/record-factory-rework-ci-evidence.py').read_text())
    namespace = vars(P).copy(); exec(compile(source, '<original-budget-aggregate>', 'exec'), namespace)
    return namespace['budget_usage']


def seed(test):
    legacy = F.BudgetProjection(); legacy.setUp(); test.addCleanup(legacy.doCleanups)
    f = legacy.f
    factory_run, session = SECRET + '-factory-run', SECRET + '-recovery-session'
    conn = sqlite3.connect(f.db)
    conn.executescript('''ALTER TABLE goal_run_nodes ADD COLUMN node_id TEXT;
ALTER TABLE goal_run_nodes ADD COLUMN attempt_id TEXT;
DELETE FROM goal_run_nodes;
DELETE FROM goal_run_reservations;
DELETE FROM factory_budget_reservations WHERE attempt_id='held';
CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,document TEXT NOT NULL);
''')
    derived = json.loads(conn.execute('SELECT document FROM factory_child_goal_runs').fetchone()[0])
    derived['origin']['factory_run_id'] = factory_run
    conn.execute('UPDATE factory_child_goal_runs SET document=?', (json.dumps(derived),))
    for index, revision, kind, runtime, output, reserved in (
            (0, 1, 'work', 2555, 21, 4000), (1, 1, 'validate', 2703, 0, 6000),
            (2, 2, 'work', 3794, 20, 4000)):
        attempt, node_id = 'graph-' + str(index), SECRET + '-' + kind
        reservation = dict(runtime_ms=reserved, output_bytes=1024, tokens=None, cost_microunits=None)
        row = dict(run_id=SECRET, node_id=node_id, plan_revision=revision, settled=True,
                   actual_runtime_ms=runtime, actual_output_bytes=output, reservation=reservation)
        conn.execute('INSERT INTO goal_run_reservations VALUES(?,?,?)', (attempt, SECRET, json.dumps(row)))
        state = 'candidate' if kind == 'work' else 'failed'
        node = dict(plan_revision=revision, attempt_id=attempt, state=state, stop_confirmed=True,
                    actual_runtime_ms=runtime, actual_output_bytes=output,
                    node=dict(id=node_id, kind=kind, reservation=copy.deepcopy(reservation)), private=SECRET)
        conn.execute('INSERT INTO goal_run_nodes(run_id,plan_revision,state,document,node_id,attempt_id) VALUES(?,?,?,?,?,?)',
                     (SECRET, revision, state, json.dumps(node), node_id, attempt))
    record = json.loads(conn.execute("SELECT document FROM factory_budget_reservations WHERE attempt_id='recovery'").fetchone()[0])
    record.update(run_id=factory_run, session_id=session, root_task_id=SECRET+'-parent', queued_ms=1373,
                  actual_runtime_ms=5981, actual_output_bytes=21)
    record['reservation']['output_bytes'] = 1024
    conn.execute("UPDATE factory_budget_reservations SET document=? WHERE attempt_id='recovery'", (json.dumps(record),))
    prepared = dict(source='factory_core', aggregate_id=factory_run, revision=7,
        kind='factory.child_goal_planner_recovery_prepared', payload=dict(task_id=legacy.child, goal_run_id=SECRET,
        failure_context=dict(goal_run_id=SECRET, plan_revision=1, all_graph_attempts_stopped=True,
                             unknown_effects=False, private=SECRET), new_permissions=False,
        reservation_refunded=False, task_accepted=False, private=SECRET))
    reserved = dict(source='factory_core', aggregate_id=factory_run, revision=8,
        kind='factory.attempt_reserved', payload=dict(session_id=session, attempt_id='recovery', task_id=legacy.child,
        root_task_id=record['root_task_id'], reservation=copy.deepcopy(record['reservation']),
        stop_confirmed=False, canonical_root=SECRET))
    conn.execute('INSERT INTO events(sequence,document) VALUES(10,?)', (json.dumps(prepared),))
    conn.execute('INSERT INTO events(sequence,document) VALUES(11,?)', (json.dumps(reserved),))
    for key in ('root-grant:' + legacy.root, 'project:' + legacy.project):
        conn.execute('UPDATE goal_ledgers SET document=? WHERE scope=?',
            (json.dumps(dict(runtime_ms=17570, observed_attempts=6, private=SECRET)), key))
    conn.commit(); conn.close(); f.receipt()
    return legacy


class FeeKindObservations(unittest.TestCase):
    def setUp(self):
        self.fee = seed(self); self.f = self.fee.f; self.original = original_budget_function()

    def change(self, table, where, transform):
        conn = sqlite3.connect(self.f.db)
        rows = conn.execute('SELECT rowid,document FROM ' + table + ' WHERE ' + where).fetchall()
        for rowid, raw in rows:
            value = json.loads(raw); transform(value)
            conn.execute('UPDATE ' + table + ' SET document=? WHERE rowid=?', (json.dumps(value), rowid))
        conn.commit(); conn.close(); self.f.receipt()

    def budget(self):
        before = self.f.db.read_bytes()
        conn = sqlite3.connect(self.f.db)
        try:
            conn.execute('PRAGMA query_only=ON')
            original = self.original(conn)
            value = P.budget_usage(conn)
        finally: conn.close()
        observed = value.pop('fee_kind_observations')
        self.assertEqual(value, original)  # Every original field/status/fee stays exact.
        value['fee_kind_observations'] = observed
        self.assertEqual(self.f.db.read_bytes(), before)
        self.assertNotIn(SECRET, json.dumps(value))
        return value

    def observations(self):
        return self.budget()['fee_kind_observations']

    def test_four_source_bound_records_group_without_arithmetic_inference(self):
        budget = self.budget(); value = budget['fee_kind_observations']
        self.assertEqual(budget['child']['settled_actual_runtime_ms'], 15033)
        self.assertEqual(value['status'], 'observed')
        self.assertEqual(value['record_count'], 4)
        self.assertTrue(value['not_an_authority_admission_or_stop_verdict'])
        self.assertEqual(value['snapshot_time'], 'post_owned_shutdown_not_admission_instant')
        rows = {(v['source_kind'], v['plan_revision']): v for v in value['groups']}
        self.assertEqual({k: v['settled_actual_runtime_ms'] for k, v in rows.items()},
            {('graph_work', 1): 2555, ('graph_validator', 1): 2703,
             ('graph_work', 2): 3794, ('factory_recovery_planner', 1): 5981})
        planner = rows['factory_recovery_planner', 1]
        self.assertEqual(planner['plan_relation'], 'failed_graph_context')
        self.assertEqual(planner['factory_queued_ms'], 1373)
        self.assertTrue(planner['direct_binding_established'])
        self.assertEqual(sum(v['settled_actual_runtime_ms'] for v in rows.values()), 15033)
        self.assertEqual(sum(v['settled_actual_output_bytes'] for v in rows.values()), 62)
        self.assertTrue(all(v['record_count'] == v['settled_count'] == 1 for v in rows.values()))
        self.assertTrue(all(v['factory_queued_ms'] is None for v in rows.values() if v['source_kind'].startswith('graph_')))

    def test_old_post_shutdown_main_exit_and_original_admission_are_unchanged(self):
        original_main = P.main
        output = io.StringIO()
        with contextlib.redirect_stdout(output): code = original_main(['--evidence-root', str(self.f.root)])
        new = json.loads(output.getvalue())
        with mock.patch.object(P, 'budget_usage', self.original), contextlib.redirect_stdout(io.StringIO()) as old_output:
            old_code = original_main(['--evidence-root', str(self.f.root)])
        old = json.loads(old_output.getvalue())
        self.assertEqual(code, old_code)
        self.assertTrue(new['not_a_product_or_test_pass'])
        new['cases'][0]['post_shutdown']['budget_usage'].pop('fee_kind_observations')
        self.assertEqual(new, old)
        self.assertEqual(new['cases'][0]['last_readonly_observation']['list_progress']['failure_code'], 'REWORK_REVISION2_TERMINAL')

    def test_no_recovery_relation_is_an_unlinked_factory_observation_never_a_planner(self):
        self.f.sql('DELETE FROM events WHERE sequence=10')
        value = self.observations(); self.assertEqual(value['status'], 'partial')
        row = next(v for v in value['groups'] if v['source_kind'].startswith('factory_'))
        self.assertEqual(row['source_kind'], 'factory_child_session')
        self.assertIsNone(row['plan_revision']); self.assertEqual(row['plan_relation'], 'not_established')
        self.assertFalse(row['direct_binding_established']); self.assertEqual(row['settled_actual_runtime_ms'], 5981)

    def test_missing_event_table_does_not_erase_old_amount_or_guess_planner(self):
        self.f.sql('DROP TABLE events')
        value = self.observations(); self.assertEqual(value['status'], 'partial')
        self.assertNotIn('factory_recovery_planner', [v['source_kind'] for v in value['groups']])

    def test_missing_queue_is_unknown_and_not_zero(self):
        self.change('factory_budget_reservations', "attempt_id='recovery'", lambda v: v.pop('queued_ms'))
        value = self.observations(); self.assertEqual(value['status'], 'partial')
        row = next(v for v in value['groups'] if v['source_kind'] == 'factory_recovery_planner')
        self.assertIsNone(row['factory_queued_ms']); self.assertEqual(row['queue_observation'], 'unavailable')

    def test_unknown_runtime_remains_null_and_never_rebates(self):
        self.change('factory_budget_reservations', "attempt_id='recovery'", lambda v: v.update(actual_runtime_ms=None))
        value = self.budget(); self.assertIsNone(value['child']['settled_actual_runtime_ms'])
        row = next(v for v in value['fee_kind_observations']['groups'] if v['source_kind'] == 'factory_recovery_planner')
        self.assertIsNone(row['settled_actual_runtime_ms'])
        self.assertEqual(value['status'], 'pending')

    def test_unsettled_record_uses_original_held_reservation_not_zero(self):
        self.change('factory_budget_reservations', "attempt_id='recovery'", lambda v: v.update(settled=False, actual_runtime_ms=None))
        value = self.budget(); self.assertEqual(value['child']['unsettled_reserved_runtime_ms'], 6000)
        row = next(v for v in value['fee_kind_observations']['groups'] if v['source_kind'] == 'factory_recovery_planner')
        self.assertEqual(row['unsettled_count'], 1); self.assertEqual(row['unsettled_reserved_runtime_ms'], 6000)
        self.assertEqual(row['settled_actual_runtime_ms'], 0)

    def test_duplicate_original_attempt_still_invalid_not_just_unknown_kind(self):
        self.f.sql('INSERT INTO goal_run_reservations VALUES(?,?,?)',
            ('recovery', SECRET, json.dumps(dict(run_id=SECRET, settled=True, actual_runtime_ms=1))))
        conn = sqlite3.connect(self.f.db)
        try:
            with self.assertRaises(P.Invalid): self.original(conn)
            with self.assertRaises(P.Invalid): P.budget_usage(conn)
        finally: conn.close()
        self.assertEqual(P.collect(str(self.f.root))['cases'][0]['status'], 'invalid')

    def test_duplicate_reservation_event_is_ambiguous_not_arbitrarily_chosen(self):
        self.f.sql('INSERT INTO events(document) SELECT document FROM events WHERE sequence=11')
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_duplicate_preparation_cannot_arbitrarily_bind_a_planner(self):
        self.f.sql('INSERT INTO events(sequence,document) SELECT 9,document FROM events WHERE sequence=10')
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_reserved_event_without_corresponding_record_is_unavailable(self):
        self.change('events', 'sequence=11', lambda v: v['payload'].update(attempt_id=SECRET + '-orphan'))
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_graph_missing_or_duplicate_exact_counterpart_is_unavailable(self):
        self.f.sql("DELETE FROM goal_run_nodes WHERE attempt_id='graph-0'")
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_one_preparation_cannot_label_two_separately_charged_attempts(self):
        conn = sqlite3.connect(self.f.db)
        value = json.loads(conn.execute("SELECT document FROM factory_budget_reservations WHERE attempt_id='recovery'").fetchone()[0])
        value.update(attempt_id='second', session_id=SECRET + '-second-session', actual_runtime_ms=100)
        conn.execute('INSERT INTO factory_budget_reservations VALUES(?,?,?)', ('second', self.fee.scope, json.dumps(value)))
        event = json.loads(conn.execute('SELECT document FROM events WHERE sequence=11').fetchone()[0])
        event['payload'].update(attempt_id='second', session_id=value['session_id'])
        conn.execute('INSERT INTO events(sequence,document) VALUES(12,?)', (json.dumps(event),))
        conn.commit(); conn.close(); self.f.receipt()
        self.assertEqual(self.budget()['child']['settled_actual_runtime_ms'], 15133)
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_bool_document_revision_and_event_reservation_are_not_integer_aliases(self):
        original = self.f.db.read_bytes()
        self.change('goal_run_nodes', "attempt_id='graph-0'", lambda v: v.update(plan_revision=True))
        self.assertEqual(self.observations()['status'], 'unavailable')
        self.f.db.write_bytes(original)
        self.change('events', 'sequence=11', lambda v: v['payload']['reservation'].update(output_bytes=True))
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_node_table_rows_are_bounded_even_when_original_fee_tables_are_small(self):
        conn = sqlite3.connect(self.f.db)
        row = conn.execute('SELECT * FROM goal_run_nodes LIMIT 1').fetchone()
        conn.executemany('INSERT INTO goal_run_nodes VALUES(?,?,?,?,?,?)', [row] * P.MAX_ROWS)
        conn.commit(); conn.close(); self.f.receipt()
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_paid_graph_node_without_corresponding_fee_record_is_unavailable(self):
        conn = sqlite3.connect(self.f.db)
        revision, node_id, attempt, raw = conn.execute("SELECT plan_revision,node_id,attempt_id,document FROM goal_run_nodes WHERE attempt_id='graph-0'").fetchone()
        node = json.loads(raw); node['node']['id'] += '-orphan'; node['attempt_id'] += '-orphan'
        conn.execute('INSERT INTO goal_run_nodes(run_id,plan_revision,state,document,node_id,attempt_id) VALUES(?,?,?,?,?,?)',
            (SECRET, revision, 'candidate', json.dumps(node), node['node']['id'], node['attempt_id']))
        conn.commit(); conn.close(); self.f.receipt()
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_graph_duplicate_exact_node_is_unavailable(self):
        self.f.sql("INSERT INTO goal_run_nodes SELECT * FROM goal_run_nodes WHERE attempt_id='graph-0'")
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_graph_unknown_kind_wrong_plan_attempt_fees_or_stop_never_guesses(self):
        transforms = [lambda v: v['node'].update(kind=SECRET), lambda v: v.update(plan_revision=3),
            lambda v: v.update(attempt_id=SECRET), lambda v: v.update(actual_runtime_ms=1),
            lambda v: v.update(actual_output_bytes=1), lambda v: v.update(stop_confirmed=False),
            lambda v: v['node']['reservation'].update(runtime_ms=True)]
        original = self.f.db.read_bytes()
        for index, transform in enumerate(transforms):
            with self.subTest(index=index):
                self.f.db.write_bytes(original)
                self.change('goal_run_nodes', "attempt_id='graph-0'", transform)
                self.assertEqual(self.observations()['status'], 'unavailable')

    def test_recovery_wrong_scope_session_run_flags_plan_or_relation_is_never_minted(self):
        changes = [('factory_budget_reservations', "attempt_id='recovery'", lambda v: v.update(run_id=SECRET)),
            ('events', 'sequence=11', lambda v: v['payload'].update(session_id=SECRET)),
            ('events', 'sequence=11', lambda v: v['payload'].update(stop_confirmed=True)),
            ('events', 'sequence=10', lambda v: v['payload'].update(new_permissions=True)),
            ('events', 'sequence=10', lambda v: v['payload'].update(reservation_refunded=True)),
            ('events', 'sequence=10', lambda v: v['payload']['failure_context'].update(unknown_effects=True)),
            ('events', 'sequence=10', lambda v: v['payload']['failure_context'].update(plan_revision=3))]
        original = self.f.db.read_bytes()
        for table, where, change in changes:
            with self.subTest(table=table, where=where):
                self.f.db.write_bytes(original); self.change(table, where, change)
                self.assertEqual(self.observations()['status'], 'unavailable')

    def test_out_of_order_preparation_has_no_recovery_link(self):
        self.f.sql('UPDATE events SET sequence=12 WHERE sequence=10')
        value = self.observations(); self.assertEqual(value['status'], 'partial')
        self.assertFalse(any(v['direct_binding_established'] for v in value['groups'] if v['source_kind'].startswith('factory_')))

    def test_private_fields_are_not_projected_and_known_aliases_have_no_authority(self):
        self.change('goal_run_nodes', "attempt_id='graph-0'", lambda v: v.update(token=SECRET, path=SECRET))
        self.change('events', 'sequence=10', lambda v: v.update(token=SECRET))
        value = self.observations(); self.assertEqual(value['status'], 'observed')
        raw = json.dumps(value)
        for text in [SECRET, 'task_id', 'attempt_id', 'session_id', 'root_task_id', 'canonical_root', 'payload']:
            self.assertNotIn(text, raw)

    def test_queue_wrong_type_negative_or_overflow_is_not_a_numeric_observation(self):
        original = self.f.db.read_bytes()
        for bad in [True, -1, '1', 1 << 64, 5982]:
            with self.subTest(bad=bad):
                self.f.db.write_bytes(original)
                self.change('factory_budget_reservations', "attempt_id='recovery'", lambda v: v.update(queued_ms=bad))
                self.assertEqual(self.observations()['status'], 'unavailable')

    def test_missing_schema_view_and_bounds_are_unknown_without_changing_original_aggregate(self):
        self.f.sql('DROP TABLE events')
        self.f.sql("CREATE VIEW events AS SELECT 10 sequence,'{}' document")
        self.assertEqual(self.observations()['status'], 'partial')
        self.f.sql('DROP VIEW events')
        self.f.sql('ALTER TABLE goal_run_nodes RENAME TO renamed_nodes')
        self.assertEqual(self.observations()['status'], 'unavailable')

    def test_source_row_and_event_limits_refuse_before_unbounded_classification(self):
        with mock.patch.object(P, 'MAX_FEE_GROUPS', 2):
            self.assertEqual(self.observations()['status'], 'unavailable')
        with mock.patch.object(P, 'MAX_EVENT_ROWS', 1):
            self.assertEqual(self.observations()['status'], 'partial')

    def test_original_failures_and_unknowns_keep_the_same_exit_status(self):
        self.f.sql("UPDATE goal_ledgers SET document=json_set(document,'$.runtime_ms',-1)")
        with contextlib.redirect_stdout(io.StringIO()):
            new_code = P.main(['--evidence-root', str(self.f.root)])
        with mock.patch.object(P, 'budget_usage', self.original), contextlib.redirect_stdout(io.StringIO()):
            original_code = P.main(['--evidence-root', str(self.f.root)])
        self.assertEqual(new_code, original_code)
        self.assertEqual(new_code, 1)

    def test_inverse_preserves_whole_original_aggregate_and_rejects_unknown_changes(self):
        text = (ROOT / 'scripts/record-factory-rework-ci-evidence.py').read_text()
        self.assertEqual(hashlib.sha256(original_budget_source(text).encode()).hexdigest(), ORIGINAL_BUDGET_SHA256)
        for before, after in [("attempt not in seen", 'True'), ("totals['settled_count'] += 1", "totals['settled_count'] += 0"),
                ("'existing_ledger_and_child_records'", "'unreviewed_fee_code'")]:
            self.assertIn(before, text)
            start = text.index('def budget_usage(')
            with self.subTest(before=before), self.assertRaises(ValueError):
                original_budget_source(text[:start] + text[start:].replace(before, after, 1))


if __name__ == '__main__':
    unittest.main()
