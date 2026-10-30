"""Generated SQL/filesystem controls only; no Cargo, Actor or private runtime."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('rework_evidence', ROOT / 'scripts/record-factory-rework-ci-evidence.py')
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)
SECRET = 'PRIVATE_CANARY_TOKEN_PATH_ID_AND_MESSAGE'


def observation():
    return dict(phase='wait_rework_awaiting_owner', method='factory_child_goals.list',
        stage='response_received', call_number=111, wall_ms=1793308590211,
        scenario_elapsed_ms=62688, call_elapsed_ms=332,
        observation_remaining_ms=0, observation_overdue_ms=210,
        approved_scope_remaining_ms=955, configured_client_read_timeout_ms=5000,
        configured_actor_request_budget_ms=60000, reply_object_supplied=True,
        reply_is_error=False, reply_revision=133, run_revision=None,
        plan_revision=None, node_count=None, run_completed=None, run_outcome_unknown=None,
        method_calls=dict(zip(P.METHODS, (4, 107, 0))),
        method_total_elapsed_ms=dict(zip(P.METHODS, (280, 50327, 0))),
        received_before_original_deadline=False,
        list_progress=dict(summary_count=1, plan_revision=2, run_plan_revision=2,
            run_revision=9, run_state='blocked', task_accepted=False,
            run_task_accepted=False, cancel_requested=False, plan_digest_matches=True,
            failure_code='REWORK_REVISION2_TERMINAL'))


def database(path):
    # Exact relevant real column names; generated minimal rows, not a product DB.
    conn = sqlite3.connect(path)
    conn.executescript('''PRAGMA user_version=16;
CREATE TABLE factory_child_goal_runs(goal_run_id TEXT PRIMARY KEY,document TEXT);
CREATE TABLE goal_runs(id TEXT PRIMARY KEY,state TEXT,document TEXT);
CREATE TABLE goal_run_nodes(run_id TEXT,plan_revision INTEGER,state TEXT,document TEXT);
CREATE TABLE goal_graph_validations(run_id TEXT,plan_revision INTEGER,document TEXT);
''')
    conn.execute('INSERT INTO factory_child_goal_runs VALUES(?,?)', (SECRET, '{}'))
    conn.execute('INSERT INTO goal_runs VALUES(?,?,?)', (SECRET, 'cancelled', json.dumps({
        'plan_revision': 2, 'state': 'cancelled', 'task_accepted': False, 'cancel_requested': True,
        'id': SECRET, 'last_error': {'message': SECRET}})))
    for revision, state, kind in [(1, 'failed', 'validate'), (2, 'awaiting_owner', 'owner_approval')]:
        conn.execute('INSERT INTO goal_run_nodes VALUES(?,?,?,?)',
            (SECRET, revision, state, json.dumps({'plan_revision': revision, 'state': state, 'node': {'id': SECRET, 'kind': kind}})))
    for revision, state, passed in [(1, 'failed', False), (2, 'passed', True)]:
        conn.execute('INSERT INTO goal_graph_validations VALUES(?,?,?)',
            (SECRET, revision, json.dumps({'plan_revision': revision, 'state': state, 'passed': passed,
                'stop_confirmed': True, 'io_completion': 'worker_joined', 'observation': SECRET})))
    conn.commit()
    conn.close()
    path.chmod(0o600)


class Fixture:
    def __init__(self, root):
        # tempfile may return a platform alias (macOS /var -> /private/var).
        # Resolve only our generated fixture root, never collector input.
        self.root = root.resolve(strict=True) / 'evidence'
        self.root.mkdir(mode=0o755)
        self.case = self.root / 'rework-12345678-1234-4234-8234-123456789abc'
        self.case.mkdir(mode=0o700)
        self.db = self.case / 'runtime.raw.sqlite3'
        database(self.db)
        self.write('readonly-observation-last.json', observation())
        self.receipt()
        self.write('post-shutdown-snapshot.json', dict(contract='rework-post-join-main-file.v1',
            main_file_copy_consistent=True, status='observed_consistent',
            snapshot_time='post_owned_shutdown_not_deadline_instant',
            not_a_general_sqlite_snapshot_or_process_stop_proof=True))

    def write(self, name, value):
        path = self.case / name
        path.write_text(json.dumps(value))
        path.chmod(0o644)

    def receipt(self):
        raw = self.db.read_bytes()
        self.write('owned-cleanup.json', dict(joined=True, daemon_result='ok',
            raw_db_bytes=len(raw), raw_db_sha256=hashlib.sha256(raw).hexdigest(),
            test_panicking=True))

    def sql(self, sql, params=()):
        conn = sqlite3.connect(self.db)
        conn.execute(sql, params)
        conn.commit()
        conn.close()
        self.receipt()


class Projection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))

    def collect(self):
        out = P.collect(str(self.f.root))
        self.assertNotIn(SECRET, json.dumps(out))
        self.assertNotIn(str(self.f.root), json.dumps(out))
        return out

    def test_actual_array_projection_and_post_shutdown_are_different_observations(self):
        before = {p.name: p.read_bytes() for p in self.f.case.iterdir()}
        result = self.collect()
        self.assertEqual(result['status'], 'collected_post_shutdown')
        self.assertTrue(result['not_a_product_or_test_pass'])
        case = result['cases'][0]
        self.assertEqual(case['last_readonly_observation']['list_progress']['run_state'], 'blocked')
        self.assertEqual(case['post_shutdown']['runs'][0]['state'], 'cancelled')
        self.assertTrue(case['test_panicking'])
        self.assertEqual(case['snapshot_time'], 'post_owned_shutdown_not_deadline_instant')
        self.assertEqual(case['post_shutdown']['node_counts']['2:owner_approval:awaiting_owner'], 1)
        self.assertEqual(len(case['post_shutdown']['validation_counts']), 2)
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.f.case.iterdir()})

    def test_fixture_canonicalizes_owned_temp_alias_but_collector_still_refuses_it(self):
        base = Path(self.temp.name).resolve(strict=True)
        physical = base / 'physical-temp-root'
        physical.mkdir(mode=0o700)
        alias = base / 'generated-var-alias'
        alias.symlink_to(physical, target_is_directory=True)
        fixture = Fixture(alias)
        self.assertEqual(fixture.root, physical / 'evidence')
        self.assertEqual(P.collect(str(fixture.root))['status'], 'collected_post_shutdown')
        refused = P.collect(str(alias / 'evidence'))
        self.assertEqual(refused['status'], 'invalid')
        self.assertEqual(refused['code'], 'INVALID_EVIDENCE_ROOT')
        self.assertEqual(refused['cases'], [])

    def test_missing_root_case_and_scalar_are_pending_not_pass(self):
        self.assertEqual(P.collect(str(self.f.root / 'missing'))['status'], 'pending')
        (self.f.case / 'readonly-observation-last.json').unlink()
        out = self.collect()
        self.assertEqual(out['status'], 'invalid_or_pending')
        self.assertEqual(out['cases'][0]['code'], 'MISSING_READONLY_OBSERVATION')
        (self.f.case / 'owned-cleanup.json').unlink()
        self.assertEqual(self.collect()['cases'][0]['status'], 'pending')

    def test_no_join_or_daemon_error_never_reads_db(self):
        for joined, daemon in [(False, 'missing_owned_thread'), (True, 'panic'), (True, 'error')]:
            with self.subTest(joined=joined, daemon=daemon):
                self.f.write('owned-cleanup.json', {'joined': joined, 'daemon_result': daemon, 'error': SECRET})
                with mock.patch.object(P, 'project_db', side_effect=AssertionError('must not read')):
                    out = self.collect()
                self.assertEqual(out['cases'][0]['code'], 'OWNED_SHUTDOWN_UNCONFIRMED')
                self.assertNotEqual(out['status'], 'collected_post_shutdown')

    def test_forged_sha_or_bool_bytecount_refused(self):
        for change in ({'raw_db_sha256': '0' * 64}, {'raw_db_bytes': True}, {'joined': 1}):
            with self.subTest(change=change):
                self.f.receipt()
                p = self.f.case / 'owned-cleanup.json'
                value = json.loads(p.read_bytes())
                value.update(change)
                self.f.write(p.name, value)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_bad_json_duplicate_fields_and_nonfinite_refused(self):
        p = self.f.case / 'owned-cleanup.json'
        for raw in (b'{broken', b'{"joined":true,"joined":true}', b'{"joined":NaN}'):
            with self.subTest(raw=raw):
                p.write_bytes(raw)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_file_symlink_hardlink_fifo_and_mode_rejected_without_blocking(self):
        p = self.f.case / 'owned-cleanup.json'
        original = p.read_bytes()
        for kind in ('symlink', 'hardlink', 'fifo', 'mode'):
            with self.subTest(kind=kind):
                p.unlink()
                outside = self.f.root / 'other'
                outside.write_bytes(original)
                outside.chmod(0o644)
                if kind == 'symlink': p.symlink_to(outside)
                elif kind == 'hardlink': os.link(outside, p)
                elif kind == 'fifo': os.mkfifo(p, 0o600)
                else:
                    p.write_bytes(original)
                    p.chmod(0o666)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_private_directory_symlink_alias_and_modes_refused(self):
        self.f.case.chmod(0o755)
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        self.f.case.chmod(0o700)
        alias = self.f.root / 'rework-22345678-1234-4234-8234-123456789abc'
        alias.symlink_to(self.f.case, target_is_directory=True)
        self.assertEqual(self.collect()['status'], 'invalid_or_pending')
        self.f.root.chmod(0o777)
        self.assertEqual(self.collect()['status'], 'invalid')

    def test_size_and_entry_count_limits(self):
        p = self.f.case / 'readonly-observation-last.json'
        p.write_bytes(b' ' * 4097)
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        p.write_text(json.dumps(observation()))
        with self.f.db.open('r+b') as stream: stream.truncate(P.MAX_DB + 1)
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        for i in range(P.MAX_ENTRIES): (self.f.root / f'irrelevant-{i}').touch()
        self.assertEqual(self.collect()['status'], 'invalid')

    def test_uid_and_identity_change_rejected(self):
        real = P.os.fstat
        class WrongOwner:
            def __init__(self, value): self.__dict__['value'] = value
            def __getattr__(self, key): return os.geteuid() + 1 if key == 'st_uid' else getattr(self.value, key)
        with mock.patch.object(P.os, 'fstat', side_effect=lambda fd: WrongOwner(real(fd))):
            self.assertEqual(self.collect()['status'], 'invalid')
        (self.f.root / 'other').write_bytes(b'bounded metadata identity control')
        fd = P.directory(path=str(self.f.root))
        self.addCleanup(os.close, fd)
        real_stat = P.os.stat
        with mock.patch.object(P.os, 'stat', side_effect=lambda *a, **kw: real_stat(self.f.db)):
            with self.assertRaises(P.Invalid): P.read_file(fd, 'other', 4096)

    def test_unknown_states_types_and_schema_refused_without_leaking(self):
        for sql, params in [
            ('PRAGMA user_version=15', ()),
            ('PRAGMA user_version=17', ()),
            ('UPDATE goal_runs SET state=?', (SECRET,)),
            ('UPDATE goal_runs SET state=?', ('running',)),
            ("UPDATE goal_run_nodes SET document=json_set(document,'$.plan_revision',1.5)", ()),
            ("UPDATE goal_graph_validations SET document=json_set(document,'$.plan_revision',99)", ()),
            ("UPDATE goal_runs SET document=json_set(document,'$.task_accepted',0)", ()),
            ("UPDATE goal_run_nodes SET document=json_set(document,'$.node.kind',?)", (SECRET,)),
            ("UPDATE goal_graph_validations SET document=json_set(document,'$.io_completion',?)", (SECRET,)),
        ]:
            with self.subTest(sql=sql):
                self.f.db.unlink()
                database(self.f.db)
                self.f.sql(sql, params)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_sql_row_limits_and_no_view_evaluation(self):
        self.f.sql('UPDATE goal_runs SET document=?', ('x' * (1024 * 1024 + 1),))
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        self.f.db.unlink()
        database(self.f.db)
        conn = sqlite3.connect(self.f.db)
        conn.execute('DROP TABLE goal_run_nodes')
        conn.execute("CREATE VIEW goal_run_nodes AS SELECT 'private' AS document")
        conn.commit(); conn.close()
        self.f.receipt()
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_missing_incomplete_or_sidecar_snapshot_is_not_complete(self):
        p = self.f.case / 'post-shutdown-snapshot.json'
        original = p.read_bytes()
        p.unlink()
        self.assertEqual(self.collect()['cases'][0]['status'], 'pending')
        p.write_bytes(original)
        value = json.loads(original)
        value.update(status='incomplete', main_file_copy_consistent=False)
        self.f.write(p.name, value)
        with mock.patch.object(P, 'project_db', side_effect=AssertionError('must not open')):
            self.assertEqual(self.collect()['cases'][0]['code'], 'MAIN_FILE_CAPTURE_INCOMPLETE')
        self.f.write(p.name, json.loads(original))
        sidecar = self.f.case / 'runtime.raw.sqlite3-wal'
        sidecar.write_bytes(b'not published')
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_case_count_and_symlink_ancestor_rejected(self):
        for i in range(P.MAX_CASES):
            (self.f.root / f'rework-{i:08x}-1234-4234-8234-123456789abc').mkdir(mode=0o700)
        self.assertEqual(self.collect()['status'], 'invalid')
        alias = Path(self.temp.name) / 'alias'
        alias.symlink_to(self.f.root, target_is_directory=True)
        self.assertEqual(P.collect(str(alias))['status'], 'invalid')
        self.assertEqual(P.collect(str(alias / 'nested'))['status'], 'invalid')

    def test_db_row_count_and_file_replacement_after_read_refused(self):
        conn = sqlite3.connect(self.f.db)
        conn.executemany('INSERT INTO goal_run_nodes VALUES(?,?,?,?)',
            [(SECRET, 2, 'failed', '{"node":{"kind":"validate"}}')] * P.MAX_ROWS)
        conn.commit(); conn.close(); self.f.receipt()
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        fd = P.directory(path=str(self.f.case))
        self.addCleanup(os.close, fd)
        real_read = os.read
        changed = False
        def replacing(source, count):
            nonlocal changed
            raw = real_read(source, count)
            if raw and not changed:
                changed = True
                old = self.f.case / 'readonly-observation-last.json'
                old.unlink()
                self.f.write(old.name, observation())
            return raw
        with mock.patch.object(P.os, 'read', side_effect=replacing):
            with self.assertRaises(P.Invalid): P.read_file(fd, 'readonly-observation-last.json', 4096)

    def test_actual_pretty_stdout_budget_including_newline_is_enforced(self):
        from itertools import product
        model = self.collect()['cases'][0]
        model['post_shutdown']['node_counts'] = {f'{rev}:{kind}:{state}': 1
            for rev, kind, state in list(product(('1', '2', 'later'),
                ('work', 'validate', 'owner_approval', 'summary'), sorted(P.NODE_STATES)))[:136]}
        model['post_shutdown']['validation_counts'] = {f'{rev}:{state}:passed_{passed}:stopped_{stop}:{io}': 1
            for rev, state, passed, stop, io in list(product(('1', '2', 'later'),
                ('passed', 'failed', 'cancelled', 'outcome_unknown'), ('true', 'false'),
                ('true', 'false'), ('worker_joined', 'not_started')))[:64]}
        for i in range(11):
            (self.f.root / f'rework-{i:08x}-1234-4234-8234-123456789abc').mkdir(mode=0o700)
        unbounded = {'contract': P.CONTRACT, 'status': 'collected_post_shutdown',
            'code': 'FINITE_PROJECTION_COLLECTED', 'not_a_product_or_test_pass': True,
            'cases': [model] * 12}
        self.assertLess(len(json.dumps(unbounded).encode()), P.MAX_PUBLIC)
        self.assertGreater(len(P.render_public(unbounded).encode()), P.MAX_PUBLIC)
        capture = io.StringIO()
        with mock.patch.object(P, 'case', return_value=model), contextlib.redirect_stdout(capture):
            exit_code = P.main(['--evidence-root', str(self.f.root)])
        result = json.loads(capture.getvalue())
        self.assertEqual(result['code'], 'PUBLIC_PROJECTION_LIMIT')
        self.assertEqual(exit_code, 1)
        self.assertLessEqual(len(capture.getvalue().encode()), P.MAX_PUBLIC)
        self.assertTrue(capture.getvalue().endswith('\n'))

    def test_invalid_cli_never_echoes_argv_or_exception(self):
        for argv in ([], ['--' + SECRET], ['--evidence-root'],
                     ['--evidence-root', str(self.f.root), '--' + SECRET],
                     ['--evidence', SECRET]):
            with self.subTest(argv=argv):
                stdout, stderr = io.StringIO(), io.StringIO()
                with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                    with self.assertRaises(SystemExit) as stopped: P.main(argv)
                self.assertEqual(stopped.exception.code, 2)
                self.assertEqual(stdout.getvalue(), '')
                self.assertEqual(stderr.getvalue(), 'INVALID_COLLECTOR_ARGUMENTS\n')
                self.assertNotIn(SECRET, stderr.getvalue())

    def test_public_cli_exit_is_not_a_test_pass_and_failure_has_no_raw_message(self):
        for missing in (False, True):
            path = self.f.root / 'missing' if missing else self.f.root
            capture = io.StringIO()
            with contextlib.redirect_stdout(capture): exit_code = P.main(['--evidence-root', str(path)])
            out = json.loads(capture.getvalue())
            self.assertEqual(exit_code, 1 if missing else 0)
            self.assertTrue(out['not_a_product_or_test_pass'])
            self.assertNotIn(SECRET, capture.getvalue())


class FiniteScalars(unittest.TestCase):
    def test_max_scalar_under_original_4k_cap(self):
        value = observation()
        for key in ('call_number', 'wall_ms', 'scenario_elapsed_ms', 'call_elapsed_ms',
            'observation_remaining_ms', 'observation_overdue_ms', 'reply_revision'):
            value[key] = (1 << 64) - 1
        for key in ('method_calls', 'method_total_elapsed_ms'):
            value[key] = dict.fromkeys(P.METHODS, (1 << 64) - 1)
        self.assertLess(len(json.dumps(P.scalar(value), indent=2).encode()), 4096)

    def test_typed_bool_enum_extra_field_and_raw_result_not_accepted(self):
        for key, bad in [('run_completed', 0), ('reply_object_supplied', 1),
                         ('phase', SECRET), ('call_number', True), ('raw_result', SECRET)]:
            value = observation(); value[key] = bad
            with self.subTest(key=key), self.assertRaises(P.Invalid): P.scalar(value)
        value = observation(); value['list_progress']['run_state'] = SECRET
        with self.assertRaises(P.Invalid): P.scalar(value)

    def test_missing_null_run_is_not_false_and_array_empty_is_valid(self):
        value = observation(); value['list_progress'] = {'summary_count': 0, 'failure_code': None}
        out = P.scalar(value)
        self.assertIsNone(out['run_completed'])
        self.assertIsNone(out['run_outcome_unknown'])
        self.assertEqual(out['list_progress']['summary_count'], 0)

    def test_rev1_failure_permitted_but_rev2_terminal_and_unknown_are_fixed(self):
        value = observation()['list_progress']
        for revision, state, expected in [
            (1, 'failed', None), (1, 'blocked', None),
            (2, 'blocked', 'REWORK_REVISION2_TERMINAL'),
            (2, 'cancelled', 'REWORK_REVISION2_TERMINAL'),
            (2, 'failed', 'REWORK_REVISION2_TERMINAL'),
            (2, 'cancel_requested', 'REWORK_REVISION2_TERMINAL'),
            (1, 'outcome_unknown', 'REWORK_OUTCOME_UNKNOWN'),
            (2, 'outcome_unknown', 'REWORK_OUTCOME_UNKNOWN'),
            (2, 'completed', 'REWORK_EARLY_ACCEPTANCE'),
            (2, 'accepted', 'REWORK_EARLY_ACCEPTANCE'),
            (2, 'awaiting_acceptance', None), (2, 'running', None), (2, 'queued', None),
        ]:
            with self.subTest(revision=revision, state=state):
                entry = dict(value, plan_revision=revision, run_plan_revision=revision,
                             run_state=state, failure_code=expected)
                self.assertEqual(P.progress(entry)['failure_code'], expected)
                if expected:
                    entry['failure_code'] = None
                    with self.assertRaises(P.Invalid): P.progress(entry)

    def test_rust_projection_and_original_safety_order_are_present(self):
        source = (ROOT / 'crates/core/tests/fixtures/factory_child_goal_rework.rs').read_text()
        self.assertIn('result.as_array()', source)
        self.assertIn('plan_revision == Some(2)', source)
        self.assertIn('REWORK_REVISION2_TERMINAL', source)
        self.assertIn('fn rework_list_progress_distinguishes_revision1_recovery_and_revision2_terminal', source)
        self.assertLess(source.index('let received_before_deadline'), source.index('self.method_elapsed_ms[method_index] ='))
        self.assertLess(source.index('self.record_observation(&observed)'), source.index('received_before_deadline,\n'))


class EventProjection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))
        self.f.sql('CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL)')

    def add(self, kind='goal.run_blocked', revision=9, subject=SECRET,
            source='goal_core', payload=None, sequence=None):
        if payload is None:
            payload = dict(reason_code='RESOURCE_LIMIT', automatic_retry=False,
                           task_accepted=False, message=SECRET)
        event = dict(aggregate_id=subject, source=source, revision=revision,
                     kind=kind, payload=payload, event_id=SECRET)
        self.f.sql('INSERT INTO events(sequence,document) VALUES(?,?)',
                   (sequence, json.dumps(event)))

    def collected(self):
        result = P.collect(str(self.f.root))
        raw = P.render_public(result)
        self.assertNotIn(SECRET, raw)
        self.assertNotIn(str(self.f.root), raw)
        self.assertLessEqual(len(raw.encode()), P.MAX_PUBLIC)
        return result

    def events(self):
        result = self.collected()
        self.assertEqual(result['status'], 'collected_post_shutdown')
        return result['cases'][0]['post_shutdown']['associated_events']

    def node(self, overrun=False):
        return dict(task_accepted=False, actual_runtime_ms=4123,
            actual_output_bytes=19, budget_overrun=overrun, stop_confirmed=True,
            observed_runtime_lower_bound_ms=4123, state='candidate', node_id=SECRET)

    def capture(self):
        return dict(task_accepted=False, actual_runtime_ms=4125,
            actual_output_bytes=19, state='failed', io_completion='worker_joined',
            capture_worker_joined=True, producer_stop_confirmed=True, job_id=SECRET)

    def confirmed_failure(self):
        return dict(plan_revision=2, task_accepted=False, reservation_refunded=False,
            proof=dict(run_id=SECRET, plan_revision=2, failure_refs=['validation:' + SECRET],
                       plan_digest=SECRET), proof_digest=SECRET)

    def test_confirmed_failure_exact_observed_revision_counts_not_private_proof(self):
        value = observation()
        value['list_progress'].update(run_revision=16, run_state='failed')
        self.f.write('readonly-observation-last.json', value)
        self.add(kind='goal.factory_failure_confirmed', revision=16, payload=self.confirmed_failure())
        result = self.events()
        self.assertEqual(result['observed_run_revision'], 16)
        self.assertEqual(result['events'], [dict(kind='goal.factory_failure_confirmed',
            task_accepted=False, plan_revision=2, failure_ref_count=1,
            reservation_refunded=False, ordinal=1)])

    def test_failure_confirmation_foreign_plan_subject_empty_and_overflow_refused(self):
        variants = []
        for change in ('foreign_subject', 'foreign_plan', 'empty_refs', 'many_refs', 'not_ref_string'):
            value = self.confirmed_failure()
            if change == 'foreign_subject': value['proof']['run_id'] = 'OTHER_' + SECRET
            elif change == 'foreign_plan': value['proof']['plan_revision'] = 1
            elif change == 'empty_refs': value['proof']['failure_refs'] = []
            elif change == 'many_refs': value['proof']['failure_refs'] = [SECRET] * (P.MAX_ROWS + 1)
            else: value['proof']['failure_refs'] = [True]
            variants.append(value)
        for value in variants:
            with self.subTest(proof=value['proof']):
                self.f.sql('DELETE FROM events')
                self.add(kind='goal.factory_failure_confirmed', payload=value)
                self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_same_goal_revision_only_not_bootstrap_or_rev1_or_wrong_source(self):
        self.add(subject='UNRELATED_BOOTSTRAP_' + SECRET)
        self.add(revision=8)
        self.add(source=SECRET)
        self.add(revision=10)
        self.add(kind='UNSELECTED_' + SECRET)
        self.add()
        result = self.events()
        self.assertEqual(result['status'], 'collected_historical_events')
        self.assertEqual(result['observed_run_revision'], 9)
        self.assertEqual(result['observed_plan_revision'], 2)
        self.assertEqual(len(result['events']), 1)
        self.assertEqual(result['events'][0], dict(kind='goal.run_blocked',
            reason_code='RESOURCE_LIMIT', automatic_retry=False, task_accepted=False, ordinal=1))
        self.assertEqual(result['snapshot_time'], 'post_owned_shutdown_not_deadline_instant')

    def test_zero_events_and_missing_table_are_explicit_pending(self):
        result = self.events()
        self.assertEqual(result['status'], 'pending')
        self.assertEqual(result['code'], 'unknown_not_available')
        self.assertEqual(result['events'], [])
        self.f.sql('DROP TABLE events')
        self.assertEqual(self.events()['code'], 'unknown_not_available')

    def test_unknown_reason_is_unrecognized_pending_not_raw_message(self):
        self.add(payload=dict(reason_code=SECRET, automatic_retry=False, task_accepted=False))
        result = self.events()
        self.assertEqual(result['status'], 'pending')
        self.assertEqual(result['code'], 'unrecognized_reason')
        self.assertEqual(result['events'][0]['reason_code'], 'unrecognized')

    def test_all_six_event_kinds_emit_only_known_scalars_in_sequence_order(self):
        self.add(kind='goal.owner_node_settled', payload=dict(decision='rejected', task_accepted=False), sequence=50)
        self.add(sequence=10)
        self.add(kind='goal.node_settled', payload=self.node(True), sequence=20)
        self.add(kind='goal.capture_settled', payload=self.capture(), sequence=30)
        self.add(kind='goal.validation_settled', payload=dict(task_accepted=False,
            actual_runtime_ms=1501, actual_output_bytes=0, passed=False, budget_overrun=True,
            stop_confirmed=True, observed_runtime_lower_bound_ms=1501,
            io_completion='worker_joined', error={'message': SECRET}), sequence=40)
        self.add(kind='goal.factory_failure_confirmed', payload=self.confirmed_failure(), sequence=60)
        result = self.events()
        self.assertEqual([e['kind'] for e in result['events']], list(P.EVENT_KINDS))
        self.assertEqual([e['ordinal'] for e in result['events']], list(range(1, 7)))
        self.assertTrue(result['events'][1]['budget_overrun'])
        self.assertEqual(result['events'][3]['actual_runtime_ms'], 1501)

    def test_capture_prior_node_is_separate_and_never_called_same_plan(self):
        self.add(kind='goal.node_settled', revision=7, payload=self.node())
        self.add(kind='goal.node_settled', revision=8, payload=self.node(True))
        self.add(kind='goal.node_settled', revision=8, subject='OTHER_' + SECRET, payload=self.node())
        self.add(kind='goal.capture_settled', payload=self.capture())
        result = self.events()
        self.assertEqual(result['observed_run_revision'], 9)
        self.assertEqual(len(result['events']), 1)
        previous = result['previous_node_settlement']
        self.assertEqual(previous['run_revision'], 8)
        self.assertFalse(previous['same_observed_revision'])
        self.assertTrue(previous['same_plan_not_established'])
        self.assertTrue(previous['event']['budget_overrun'])

    def test_later_sequence_is_not_presented_as_previous_settlement(self):
        self.add(kind='goal.node_settled', revision=8, payload=self.node(True), sequence=99)
        self.add(kind='goal.capture_settled', payload=self.capture(), sequence=50)
        self.assertIsNone(self.events()['previous_node_settlement'])

    def test_absent_or_factory_get_observation_never_uses_cleanup_revision(self):
        self.add()
        p = self.f.case / 'readonly-observation-last.json'
        for changed in (dict(method='factory_runs.get', list_progress=None),
                        dict(stage='request_begin', reply_object_supplied=False, list_progress=None),
                        dict(list_progress=dict(summary_count=0, failure_code=None))):
            value = observation(); value.update(changed)
            self.f.write(p.name, value)
            result = self.events()
            self.assertEqual(result['code'], 'unknown_not_available')
            self.assertNotIn('observed_run_revision', result)
        p.unlink()
        result = self.collected()
        self.assertEqual(result['status'], 'invalid_or_pending')
        self.assertEqual(result['cases'][0]['post_shutdown']['associated_events']['code'], 'unknown_not_available')

    def test_duplicate_event_and_duplicate_prior_node_are_rejected(self):
        self.add(); self.add()
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')
        self.f.sql('DELETE FROM events')
        self.add(kind='goal.node_settled', revision=8, payload=self.node())
        self.add(kind='goal.node_settled', revision=8, payload=self.node())
        self.add(kind='goal.capture_settled', payload=self.capture())
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_associated_event_limit_never_truncates(self):
        for _ in range(P.MAX_ASSOCIATED_EVENTS + 1): self.add()
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_boolean_number_state_and_sequence_corruption_refused(self):
        for kind, payload in [
            ('goal.run_blocked', dict(reason_code=0, automatic_retry=False, task_accepted=False)),
            ('goal.run_blocked', dict(reason_code='INPUT_CHANGED', automatic_retry=0, task_accepted=False)),
            ('goal.node_settled', dict(self.node(), budget_overrun=1)),
            ('goal.node_settled', dict(self.node(), actual_runtime_ms=True)),
            ('goal.capture_settled', dict(self.capture(), state=SECRET)),
            ('goal.validation_settled', dict(task_accepted=False, actual_runtime_ms=1,
                actual_output_bytes=0, budget_overrun=False, stop_confirmed=True,
                observed_runtime_lower_bound_ms=1, io_completion='worker_joined', passed=1)),
        ]:
            with self.subTest(kind=kind, payload=payload):
                self.f.sql('DELETE FROM events')
                self.add(kind=kind, payload=payload)
                self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')
        self.f.sql('DELETE FROM events'); self.add(sequence=-1)
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_events_table_schema_and_document_bounds_are_checked(self):
        self.f.sql('INSERT INTO events(document) VALUES(?)', (' ' * (1024 * 1024 + 1),))
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')
        self.f.sql('DROP TABLE events')
        self.f.sql('CREATE TABLE events(sequence TEXT,document TEXT)')
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_selected_duplicate_json_and_invalid_json_are_refused(self):
        event = dict(aggregate_id=SECRET, source='goal_core', revision=9,
            kind='goal.run_blocked', payload=dict(reason_code='AUTH_REQUIRED',
                                                automatic_retry=False, task_accepted=False))
        raw = json.dumps(event)[:-1] + ',"payload":{}}'
        self.f.sql('INSERT INTO events(document) VALUES(?)', (raw,))
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')
        self.f.sql('DELETE FROM events')
        self.f.sql('INSERT INTO events(document) VALUES(?)', ('{' + SECRET,))
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_global_event_row_budget_is_checked_before_event_materialization(self):
        conn = sqlite3.connect(self.f.db)
        conn.executemany('INSERT INTO events(document) VALUES(?)',
            [('{}',)] * (P.MAX_EVENT_ROWS + 1))
        conn.commit(); conn.close(); self.f.receipt()
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_previous_node_invalid_sequence_is_not_normalized(self):
        self.add(kind='goal.node_settled', revision=8, payload=self.node(), sequence=-1)
        self.add(kind='goal.capture_settled', payload=self.capture())
        self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_public_stdout_cap_and_cli_redaction_remain_global(self):
        # Actual linked event case remains below the exact same renderer cap.
        self.add()
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout): code = P.main(['--evidence-root', str(self.f.root)])
        self.assertEqual(code, 0)
        self.assertLessEqual(len(stdout.getvalue().encode()), P.MAX_PUBLIC)
        self.assertNotIn(SECRET, stdout.getvalue())
        self.assertEqual(P.SCHEMA, 16)


class ValidationObservations(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))

    def write_receipt(self, revision, observation, runtime=1234):
        value = dict(run_id=SECRET, plan_revision=revision, state='failed', passed=False,
            stop_confirmed=True, io_completion='worker_joined',
            actual_runtime_ms=runtime, observation=observation)
        self.f.sql('UPDATE goal_graph_validations SET document=? WHERE plan_revision=?',
            (json.dumps(value), revision))

    def collect(self):
        out = P.collect(str(self.f.root))
        public = P.render_public(out)
        self.assertNotIn(SECRET, public)
        self.assertLessEqual(len(public.encode()), P.MAX_PUBLIC)
        return out

    def observations(self):
        out = self.collect()
        self.assertEqual(out['status'], 'collected_post_shutdown')
        return out['cases'][0]['post_shutdown']['validation_observations']

    def test_actual_command_exit_one_and_zero_are_distinct_not_inferred_from_join(self):
        for revision, code in ((1, 1), (2, 0)):
            self.write_receipt(revision, dict(process_started=True, exit_code=code,
                goal_io=dict(state='observed'), stdout=SECRET, argv=[SECRET]))
        out = self.observations()
        self.assertEqual(set(out), {'1', '2'})
        for revision, code in (('1', 1), ('2', 0)):
            self.assertEqual(out[revision], dict(actual_runtime_ms=1234,
                goal_io_state='observed', error_code=None, process_started=True, exit_code=code))

    def test_joined_preflight_rejection_has_unknown_process_not_false(self):
        self.write_receipt(2, dict(goal_io=dict(state='not_started_or_preflight_rejected',
            error_code='DEADLINE_EXCEEDED'), message=SECRET), runtime=4501)
        self.assertEqual(self.observations()['2'], dict(actual_runtime_ms=4501,
            goal_io_state='not_started_or_preflight_rejected', error_code='DEADLINE_EXCEEDED',
            process_started=None, exit_code=None))

    def test_post_command_io_failure_retains_real_exit_zero(self):
        self.write_receipt(2, dict(process_started=True, exit_code=0,
            goal_io=dict(state='observation_rejected', error_code='INPUT_CHANGED')))
        value = self.observations()['2']
        self.assertEqual(value['goal_io_state'], 'observation_rejected')
        self.assertEqual(value['error_code'], 'INPUT_CHANGED')
        self.assertIs(value['process_started'], True)
        self.assertEqual(value['exit_code'], 0)

    def test_missing_and_explicit_false_start_are_different(self):
        self.write_receipt(1, None, runtime=None)
        self.write_receipt(2, dict(process_started=False, exit_code=None))
        out = self.observations()
        self.assertIsNone(out['1']['actual_runtime_ms'])
        self.assertIsNone(out['1']['process_started'])
        self.assertIsNone(out['1']['goal_io_state'])
        self.assertIs(out['2']['process_started'], False)

    def test_unknown_state_and_code_are_fixed_unrecognized_without_canary(self):
        self.write_receipt(2, dict(goal_io=dict(state=SECRET, error_code=SECRET)))
        out = self.observations()['2']
        self.assertEqual(out['goal_io_state'], 'unrecognized')
        self.assertEqual(out['error_code'], 'unrecognized')

    def test_scalar_types_are_strict_and_invalid_never_copies_message(self):
        variants = [dict(process_started=1), dict(process_started='false'),
            dict(exit_code=True), dict(exit_code='1'), dict(goal_io=False),
            dict(goal_io=dict(state=1)), dict(goal_io=dict(error_code=True))]
        for value in variants:
            with self.subTest(value=value):
                self.write_receipt(2, value)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        for runtime in (True, -1, '1', 1 << 64):
            with self.subTest(runtime=runtime):
                self.write_receipt(2, {}, runtime=runtime)
                self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_exit_code_is_exact_signed_i32_not_unbounded(self):
        for code in (-(1 << 31), (1 << 31) - 1):
            self.write_receipt(2, dict(exit_code=code))
            self.assertEqual(self.observations()['2']['exit_code'], code)
        for code in (-(1 << 31) - 1, 1 << 31):
            self.write_receipt(2, dict(exit_code=code))
            self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_other_goal_receipt_cannot_fill_selected_revision(self):
        self.f.sql('DELETE FROM goal_graph_validations WHERE plan_revision=2')
        self.f.sql('INSERT INTO goal_graph_validations VALUES(?,?,?)', ('another-generated-goal', 2,
            json.dumps(dict(plan_revision=2, observation=dict(process_started=True, exit_code=99)))))
        self.assertIsNone(self.observations()['2'])

    def test_duplicate_same_revision_receipts_are_not_arbitrarily_selected(self):
        self.f.sql('INSERT INTO goal_graph_validations SELECT * FROM goal_graph_validations WHERE plan_revision=2')
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_absent_validation_rows_leave_two_explicit_nulls(self):
        self.f.sql('DELETE FROM goal_graph_validations')
        self.assertEqual(self.observations(), {'1': None, '2': None})

    def test_later_revision_is_not_published_as_a_third_observation(self):
        self.f.sql('UPDATE goal_graph_validations SET plan_revision=3,document=json_set(document,\'$.plan_revision\',3) WHERE plan_revision=2')
        out = self.observations()
        self.assertEqual(set(out), {'1', '2'})
        self.assertIsNone(out['2'])

    def test_duplicate_json_key_in_selected_receipt_is_rejected(self):
        self.f.sql('UPDATE goal_graph_validations SET document=? WHERE plan_revision=2',
            ('{"plan_revision":2,"state":"failed","passed":false,"stop_confirmed":true,'
             '"io_completion":"worker_joined","actual_runtime_ms":1,"actual_runtime_ms":2}',))
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')


class BudgetProjection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))
        self.scope, self.child = SECRET + '-scope', SECRET + '-child'
        self.root, self.project = SECRET + '-grant', SECRET + '-project'
        conn = sqlite3.connect(self.f.db)
        conn.executescript('''ALTER TABLE factory_child_goal_runs ADD COLUMN factory_scope_id TEXT;
ALTER TABLE factory_child_goal_runs ADD COLUMN task_id TEXT;
CREATE TABLE goal_ledgers(scope TEXT PRIMARY KEY,document TEXT);
CREATE TABLE goal_run_reservations(attempt_id TEXT PRIMARY KEY,run_id TEXT,document TEXT);
CREATE TABLE factory_budget_reservations(attempt_id TEXT PRIMARY KEY,scope_id TEXT,document TEXT);
''')
        run = dict(id=SECRET, task_id=self.child, root_grant_id=self.root,
            project_id=self.project, plan_revision=2, state='cancelled',
            task_accepted=False, cancel_requested=True)
        derived = dict(run_id=SECRET, initial_run=run,
            origin=dict(factory_scope_id=self.scope, initial_plan=dict(task_id=self.child)))
        conn.execute('UPDATE goal_runs SET document=?', (json.dumps(run),))
        conn.execute('UPDATE factory_child_goal_runs SET factory_scope_id=?,task_id=?,document=?',
            (self.scope, self.child, json.dumps(derived)))
        for key in ('root-grant:' + self.root, 'project:' + self.project):
            conn.execute('INSERT INTO goal_ledgers VALUES(?,?)',
                (key, json.dumps(dict(runtime_ms=15000, observed_attempts=6, private=SECRET))))
        for i, runtime in enumerate((1000, 2000, 3000)):
            conn.execute('INSERT INTO goal_run_reservations VALUES(?,?,?)',
                ('graph-' + str(i), SECRET, json.dumps(dict(run_id=SECRET,
                    settled=True, actual_runtime_ms=runtime))))
        for attempt, task, ancestors, settled, runtime in (
                ('recovery', self.child, [SECRET + '-parent', self.child], True, 4000),
                ('parent', SECRET + '-parent', [SECRET + '-parent'], True, 5000),
                ('held', self.child, [SECRET + '-parent', self.child], False, None)):
            conn.execute('INSERT INTO factory_budget_reservations VALUES(?,?,?)',
                (attempt, self.scope, json.dumps(dict(attempt_id=attempt,
                    authority_kind='factory_task', scope_id=self.scope,
                    root_grant_id=self.root, project_id=self.project, task_id=task,
                    ancestors=ancestors, settled=settled, actual_runtime_ms=runtime,
                    reservation=dict(runtime_ms=6000)))))
        conn.commit(); conn.close(); self.f.receipt()

    def mutated_raw(self, sql, params=()):
        path = Path(self.temp.name) / 'generated-budget-control.sqlite3'
        path.write_bytes(self.f.db.read_bytes())
        conn = sqlite3.connect(path)
        try:
            conn.execute(sql, params)
            conn.commit()
        finally:
            conn.close()
        return path.read_bytes()

    def collect(self):
        out = P.collect(str(self.f.root))
        self.assertNotIn(SECRET, P.render_public(out))
        return out

    def budget(self):
        out = self.collect()
        self.assertEqual(out['status'], 'collected_post_shutdown')
        return out['cases'][0]['post_shutdown']['budget_usage']

    def test_existing_ledgers_and_child_recovery_are_separate_without_graph_mirror(self):
        value = self.budget()
        self.assertEqual(value['status'], 'collected_post_shutdown_budget_facts')
        self.assertEqual(value['root'], dict(actual_runtime_ms=15000, observed_attempts=6, runtime_unknown=False))
        self.assertEqual(value['project'], value['root'])
        self.assertEqual(value['child'], dict(settled_count=4, unsettled_count=1,
            settled_actual_runtime_ms=10000, unsettled_reserved_runtime_ms=6000,
            settled_runtime_unknown=False))
        self.assertTrue(value['not_an_authority_or_admission_verdict'])
        self.assertEqual(value['snapshot_time'], 'post_owned_shutdown_not_admission_instant')

    def test_missing_table_ledger_or_binding_is_pending_never_zero(self):
        for sql in ('DROP TABLE goal_ledgers', 'DELETE FROM goal_ledgers',
                "UPDATE factory_child_goal_runs SET document='{}'"):
            with self.subTest(sql=sql):
                value = P.project_db(self.mutated_raw(sql))['budget_usage']
                self.assertEqual(value['status'], 'pending')
                self.assertIsNone(value['child'])

    def test_unknown_runtime_is_null_pending_not_refunded_zero(self):
        self.f.sql("UPDATE factory_budget_reservations SET document=json_set(document,'$.actual_runtime_ms',NULL) WHERE attempt_id='recovery'")
        self.f.sql("UPDATE goal_ledgers SET document=json_set(document,'$.runtime_ms',NULL)")
        value = self.budget()
        self.assertEqual(value['status'], 'pending')
        self.assertTrue(value['root']['runtime_unknown'])
        self.assertIsNone(value['root']['actual_runtime_ms'])
        self.assertTrue(value['child']['settled_runtime_unknown'])
        self.assertIsNone(value['child']['settled_actual_runtime_ms'])
        self.assertEqual(value['child']['unsettled_reserved_runtime_ms'], 6000)

    def test_legacy_row_other_scope_and_other_goal_do_not_charge_this_child(self):
        self.f.sql('INSERT INTO factory_budget_reservations VALUES(?,?,?)',
            ('unrelated', None, json.dumps(dict(authority_kind='legacy_shared_session', private=SECRET))))
        self.f.sql('INSERT INTO goal_run_reservations VALUES(?,?,?)',
            ('foreign-graph', 'another-goal', json.dumps(dict(private=SECRET))))
        self.assertEqual(self.budget()['child']['settled_actual_runtime_ms'], 10000)

    def test_wrong_root_project_scope_task_and_duplicate_ancestor_are_refused(self):
        for field, value in (('root_grant_id', 'other'), ('project_id', 'other'),
                ('scope_id', 'other'), ('authority_kind', SECRET), ('settled', 1),
                ('ancestors', [self.child, self.child]), ('task_id', 'foreign-task')):
            with self.subTest(field=field):
                conn = sqlite3.connect(self.f.db)
                raw = json.loads(conn.execute("SELECT document FROM factory_budget_reservations WHERE attempt_id='recovery'").fetchone()[0])
                conn.close()
                raw[field] = value
                data = self.mutated_raw("UPDATE factory_budget_reservations SET document=? WHERE attempt_id='recovery'", (json.dumps(raw),))
                with self.assertRaises(P.Invalid): P.project_db(data)

    def test_duplicate_attempt_across_original_tables_is_not_double_counted(self):
        self.f.sql('INSERT INTO goal_run_reservations VALUES(?,?,?)',
            ('recovery', SECRET, json.dumps(dict(run_id=SECRET, settled=True, actual_runtime_ms=4000))))
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')

    def test_boolean_negative_and_overflow_metrics_are_rejected(self):
        for runtime in (True, -1, '1', 1 << 64):
            with self.subTest(runtime=runtime):
                data = self.mutated_raw('UPDATE goal_ledgers SET document=?',
                    (json.dumps(dict(runtime_ms=runtime, observed_attempts=6)),))
                with self.assertRaises(P.Invalid): P.project_db(data)

    def test_row_budget_and_view_are_refused_before_materializing(self):
        self.f.sql('INSERT INTO goal_ledgers SELECT ?1,document FROM goal_ledgers LIMIT 1', ('extra',))
        with mock.patch.object(P, 'MAX_ROWS', 2):
            self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')
        self.f.sql('DROP TABLE goal_ledgers')
        self.f.sql("CREATE VIEW goal_ledgers AS SELECT 1 scope,'{}' document")
        self.assertEqual(self.collect()['cases'][0]['status'], 'invalid')


class ReceiptBindingCorrection(unittest.TestCase):
    def setUp(self):
        self.validation = ValidationObservations()
        self.validation.setUp()
        self.addCleanup(self.validation.doCleanups)
        self.budget_fixture = BudgetProjection()
        self.budget_fixture.setUp()
        self.addCleanup(self.budget_fixture.doCleanups)

    def test_foreign_selected_document_is_rejected_not_published_as_this_goal(self):
        fixture = self.validation
        fixture.write_receipt(2, dict(process_started=True, exit_code=42,
            goal_io=dict(state='observed')), runtime=777)
        fixture.f.sql("UPDATE goal_graph_validations SET document=json_set(document,'$.run_id',?) WHERE plan_revision=2",
            (SECRET + '-foreign-run',))
        self.assertEqual(fixture.collect()['cases'][0]['status'], 'invalid')

    def test_present_null_or_non_string_run_is_not_missing_history(self):
        fixture = self.validation
        fixture.write_receipt(2, dict(process_started=True, exit_code=0))
        for value in (None, True, 1, [], {}):
            with self.subTest(value=value):
                conn = sqlite3.connect(fixture.f.db)
                raw = json.loads(conn.execute('SELECT document FROM goal_graph_validations WHERE plan_revision=2').fetchone()[0])
                conn.close()
                raw['run_id'] = value
                fixture.f.sql('UPDATE goal_graph_validations SET document=? WHERE plan_revision=2', (json.dumps(raw),))
                self.assertEqual(fixture.collect()['cases'][0]['status'], 'invalid')

    def test_missing_historical_run_id_is_unavailable_not_fabricated_zero(self):
        fixture = self.validation
        fixture.write_receipt(2, dict(process_started=True, exit_code=42), runtime=777)
        fixture.f.sql("UPDATE goal_graph_validations SET document=json_remove(document,'$.run_id') WHERE plan_revision=2")
        self.assertIsNone(fixture.observations()['2'])

    def test_duplicate_missing_history_is_still_ambiguous_not_last_row_wins(self):
        fixture = self.validation
        fixture.f.sql('INSERT INTO goal_graph_validations SELECT * FROM goal_graph_validations WHERE plan_revision=2')
        self.assertEqual(fixture.collect()['cases'][0]['status'], 'invalid')

    def test_duplicate_root_or_project_ledger_is_rejected_even_if_values_equal(self):
        fixture = self.budget_fixture
        for name, key in (('root', 'root-grant:' + fixture.root), ('project', 'project:' + fixture.project)):
            for runtime, attempts in ((91111, 99), (15000, 6)):
                with self.subTest(name=name, runtime=runtime):
                    conn = sqlite3.connect(fixture.f.db)
                    # Keep this generated damaged schema separate from the
                    # valid Fixture; the collector must not assume a SQL PK.
                    original = conn.execute('SELECT scope,document FROM goal_ledgers').fetchall()
                    conn.execute('ALTER TABLE goal_ledgers RENAME TO original_ledgers')
                    conn.execute('CREATE TABLE goal_ledgers(scope TEXT,document TEXT)')
                    conn.execute('INSERT INTO goal_ledgers VALUES(?,?)',
                        (key, json.dumps(dict(runtime_ms=runtime, observed_attempts=attempts))))
                    conn.executemany('INSERT INTO goal_ledgers VALUES(?,?)', original)
                    conn.commit(); conn.close(); fixture.f.receipt()
                    self.assertEqual(fixture.collect()['cases'][0]['status'], 'invalid')
                    conn = sqlite3.connect(fixture.f.db)
                    conn.execute('DROP TABLE goal_ledgers')
                    conn.execute('ALTER TABLE original_ledgers RENAME TO goal_ledgers')
                    conn.commit(); conn.close(); fixture.f.receipt()

    def test_exact_valid_binding_preserves_real_zero_exit_and_unique_ledger(self):
        fixture = self.validation
        fixture.write_receipt(2, dict(process_started=True, exit_code=0,
            goal_io=dict(state='observed')), runtime=777)
        self.assertEqual(fixture.observations()['2']['exit_code'], 0)
        self.assertEqual(fixture.observations()['2']['actual_runtime_ms'], 777)
        self.assertEqual(self.budget_fixture.budget()['root']['actual_runtime_ms'], 15000)


class CaptureTimings(unittest.TestCase):
    setUp = EventProjection.setUp
    add = EventProjection.add
    collected = EventProjection.collected
    events = EventProjection.events
    capture = EventProjection.capture

    def projected(self, payload):
        return P.event_fields(dict(kind='goal.capture_settled', payload=payload), SECRET, 2)

    def test_exact_six_timings_project_without_changing_runtime_or_input(self):
        values = (0, (1 << 64) - 1, 80, 20, 0, 31)
        payload = dict(self.capture(), **dict(zip(P.CAPTURE_TIMING_FIELDS, values)))
        self.add(kind='goal.capture_settled', payload=payload)
        before = {p.name: p.read_bytes() for p in self.f.case.iterdir()}
        result = self.events()
        event = result['events'][0]
        self.assertEqual({key: event[key] for key in P.CAPTURE_TIMING_FIELDS}, dict(zip(P.CAPTURE_TIMING_FIELDS, values)))
        self.assertEqual(event['capture_timing_status'], 'observed')
        self.assertEqual(event['actual_runtime_ms'], 4125)
        self.assertFalse(event['task_accepted'])
        self.assertEqual(result['snapshot_time'], 'post_owned_shutdown_not_deadline_instant')
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.f.case.iterdir()})

    def test_historical_absent_null_and_partial_fields_preserve_unknown(self):
        for timings in ({}, dict.fromkeys(P.CAPTURE_TIMING_FIELDS), {'producer_elapsed_ms': 0}):
            event = self.projected(dict(self.capture(), **timings))
            for key in P.CAPTURE_TIMING_FIELDS:
                self.assertEqual(event[key], timings.get(key))
            self.assertEqual(event['capture_timing_status'], 'observed' if timings.get('producer_elapsed_ms') == 0 else 'not_available')
            self.assertEqual(event['actual_runtime_ms'], 4125)

    def test_partial_malformed_values_use_existing_invalid_path_without_secret(self):
        for key in P.CAPTURE_TIMING_FIELDS:
            for bad in (True, False, -1, 1 << 64, 1.0, SECRET, {'private': SECRET}, [SECRET]):
                with self.subTest(key=key, bad_type=type(bad).__name__):
                    self.f.sql('DELETE FROM events')
                    self.add(kind='goal.capture_settled', payload=dict(self.capture(), **{key: bad}))
                    result = self.collected()
                    self.assertEqual(result['cases'][0]['status'], 'invalid')
                    self.assertNotIn(SECRET, P.render_public(result))

    def test_unknown_nested_keys_are_not_forwarded_as_timing_evidence(self):
        payload = dict(self.capture(), producer_elapsed_ms=7,
            private_timing={'message': SECRET, 'token': SECRET}, timing_source=SECRET)
        self.add(kind='goal.capture_settled', payload=payload)
        event = self.events()['events'][0]
        self.assertEqual(event['producer_elapsed_ms'], 7)
        self.assertNotIn('private_timing', event)
        self.assertNotIn('timing_source', event)
        self.assertNotIn(SECRET, json.dumps(event))

    def test_failed_or_not_started_with_unknown_timings_never_implies_join(self):
        for state, io, joined, stopped in (('failed', 'not_started', False, False),
                ('outcome_unknown', 'not_started', False, False), ('failed', 'worker_joined', True, True)):
            payload = dict(self.capture(), state=state, io_completion=io,
                capture_worker_joined=joined, producer_stop_confirmed=stopped,
                actual_runtime_ms=None, actual_output_bytes=None, **dict.fromkeys(P.CAPTURE_TIMING_FIELDS))
            event = self.projected(payload)
            self.assertEqual(event['capture_state'], state)
            self.assertEqual(event['io_completion'], io)
            self.assertIs(event['capture_worker_joined'], joined)
            self.assertIs(event['producer_stop_confirmed'], stopped)
            self.assertIsNone(event['actual_runtime_ms'])
            self.assertEqual(event['capture_timing_status'], 'not_available')
            self.assertTrue(all(event[key] is None for key in P.CAPTURE_TIMING_FIELDS))

    def test_timing_keys_on_other_event_kinds_do_not_expand_whitelist(self):
        payload = dict(reason_code='RESOURCE_LIMIT', automatic_retry=False, task_accepted=False,
            **{key: {'private': SECRET} for key in P.CAPTURE_TIMING_FIELDS})
        event = P.event_fields(dict(kind='goal.run_blocked', payload=payload), SECRET, 2)
        self.assertEqual(event, dict(kind='goal.run_blocked', task_accepted=False,
            reason_code='RESOURCE_LIMIT', automatic_retry=False))


if __name__ == '__main__':
    unittest.main()
