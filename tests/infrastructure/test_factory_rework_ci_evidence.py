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


if __name__ == '__main__':
    unittest.main()
