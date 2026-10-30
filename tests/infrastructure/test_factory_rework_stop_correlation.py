"""Generated real SQLite history only, not Core execution or receipt authority."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('existing_rework_controls',Path(__file__).with_name('test_factory_rework_ci_evidence.py'))
OLD=importlib.util.module_from_spec(spec);spec.loader.exec_module(OLD)
P=OLD.P
SECRET=OLD.SECRET
RUN=SECRET;SESSION=SECRET+'session';ATTEMPT=SECRET+'attempt';NODE=SECRET+'node'

class StopCorrelationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.f=OLD.Fixture(Path(self.temp.name))
        self.f.sql('CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,document TEXT NOT NULL)')
        self.observation=OLD.observation();self.observation['list_progress'].update(run_revision=11,run_state='outcome_unknown',failure_code='REWORK_OUTCOME_UNKNOWN')
        self.f.write('readonly-observation-last.json',self.observation)

    def add(self,sequence,kind,source,aggregate,payload,revision=11):
        self.f.sql('INSERT INTO events(sequence,document)VALUES(?,?)',(sequence,json.dumps(dict(kind=kind,source=source,aggregate_id=aggregate,revision=revision,payload=payload))))

    def populate(self,turn='completed',reserved=True,stopped=True):
        if reserved:self.add(1,'goal.node_reserved','goal_core',RUN,dict(node_id=NODE,attempt_id=ATTEMPT,session_id=SESSION,plan_revision=2,task_accepted=False),revision=4)
        if turn is not None:self.add(3,'session.turn_ended','provider_runtime',SESSION,dict(turn_state=turn,task_accepted=False))
        if stopped:self.add(5,'session.stopped','baton_supervisor',SESSION,dict(reason='cancelled',process_reaped=True,no_process_started=False,task_accepted=False,message=SECRET))
        self.add(10,'goal.node_settled','goal_core',RUN,dict(node_id=NODE,attempt_id=ATTEMPT,session_id=SESSION,task_accepted=False,actual_runtime_ms=4013,actual_output_bytes=21,budget_overrun=True,stop_confirmed=True,observed_runtime_lower_bound_ms=4013,state='candidate'))

    def update(self,sequence,mutate):
        with sqlite3.connect(self.f.db)as db:
            raw=db.execute('SELECT document FROM events WHERE sequence=?',(sequence,)).fetchone()[0]
        value=json.loads(raw);mutate(value)
        self.f.sql('UPDATE events SET document=? WHERE sequence=?',(json.dumps(value),sequence))

    def collect(self):
        raw=self.f.db.read_bytes();before=self.f.db.stat()
        value=P.collect(str(self.f.root));rendered=P.render_public(value)
        self.assertEqual(self.f.db.read_bytes(),raw)
        after=self.f.db.stat()
        self.assertEqual((before.st_dev,before.st_ino,before.st_size,before.st_mtime_ns,before.st_ctime_ns),(after.st_dev,after.st_ino,after.st_size,after.st_mtime_ns,after.st_ctime_ns))
        self.assertNotIn(SECRET,rendered);self.assertNotIn(str(self.f.root),rendered)
        self.assertLessEqual(len(rendered.encode()),P.MAX_PUBLIC)
        self.assertEqual(value['status'],'collected_post_shutdown')
        return value['cases'][0]['post_shutdown']['associated_events']

    def slot(self):
        events=self.collect();self.assertEqual(events['observed_run_revision'],11)
        self.assertEqual(len(events['settled_session_stops']),1)
        return events['settled_session_stops'][0]

    def test_exact_bound_completed_turn_and_cancelled_reason_are_distinct_facts(self):
        self.populate();out=self.slot()
        self.assertEqual(out,dict(node_settlement_ordinal=1,status='observed',stop_reason='cancelled',process_reaped=True,no_process_started=False,turn_status='observed',turn_completed=True,completed_turn_count=1,turn_completed_basis='derived_from_same_session_turn_state'))
        self.assertTrue(self.collect()['events'][0]['budget_overrun'])
        self.assertNotIn('cause',out);self.assertNotIn('task_accepted',out)

    def test_zero_completed_is_observed_only_after_unique_reservation_stop_window(self):
        self.populate(turn=None);out=self.slot()
        self.assertEqual(out['status'],'observed');self.assertEqual(out['turn_status'],'observed')
        self.assertIs(out['turn_completed'],False);self.assertEqual(out['completed_turn_count'],0)

    def test_other_closed_terminal_states_do_not_become_completed(self):
        self.populate(turn='interrupted');out=self.slot()
        self.assertFalse(out['turn_completed']);self.assertEqual(out['completed_turn_count'],0)

    def test_missing_reservation_or_stop_is_unavailable_not_zero_or_false(self):
        for kind in ['reserved','stopped']:
            with self.subTest(kind=kind):
                self.f.sql('DELETE FROM events');self.populate(**{kind:False});out=self.slot()
                self.assertEqual(out['status'],'unavailable')
                for key in ['stop_reason','process_reaped','no_process_started','turn_completed','completed_turn_count']:self.assertIsNone(out[key])

    def test_duplicate_reservation_or_stop_is_ambiguous_not_latest(self):
        for sequence in [1,5]:
            with self.subTest(sequence=sequence):
                self.f.sql('DELETE FROM events');self.populate()
                with sqlite3.connect(self.f.db)as db:raw=db.execute('SELECT document FROM events WHERE sequence=?',(sequence,)).fetchone()[0]
                self.f.sql('INSERT INTO events VALUES(?,?)',(2 if sequence==1 else 6,raw))
                out=self.slot();self.assertEqual(out['status'],'ambiguous');self.assertIsNone(out['process_reaped']);self.assertIsNone(out['completed_turn_count'])

    def test_cross_run_attempt_node_session_or_sender_never_supplies_stop(self):
        mutations=[(1,lambda v:v.update(aggregate_id='foreign-'+SECRET)),(1,lambda v:v['payload'].update(attempt_id='foreign-'+SECRET)),(1,lambda v:v['payload'].update(node_id='foreign-'+SECRET)),(10,lambda v:v['payload'].update(session_id='foreign-'+SECRET)),(5,lambda v:v.update(source='provider_runtime')),(1,lambda v:v.update(source='provider_runtime'))]
        for sequence,mutate in mutations:
            with self.subTest(sequence=sequence):
                self.f.sql('DELETE FROM events');self.populate();self.update(sequence,mutate)
                out=self.slot();self.assertEqual(out['status'],'unavailable');self.assertIsNone(out['turn_completed'])

    def test_null_or_absent_original_binding_is_unavailable(self):
        for key in ['session_id','attempt_id','node_id']:
            for value in [None,True,1.0,'']:
                with self.subTest(key=key,value=value):
                    self.f.sql('DELETE FROM events');self.populate();self.update(10,lambda v:v['payload'].update({key:value}))
                    self.assertEqual(self.slot()['status'],'unavailable')

    def test_strict_stop_booleans_never_coerce_numeric_or_null(self):
        for key in ['process_reaped','no_process_started']:
            for value in [0,1,1.0,None,'true']:
                with self.subTest(key=key,value=value):
                    self.f.sql('DELETE FROM events');self.populate();self.update(5,lambda v:v['payload'].update({key:value}))
                    out=self.slot();self.assertEqual(out['status'],'unavailable');self.assertIsNone(out[key])

    def test_unknown_stop_reason_is_closed_unrecognized_without_secret(self):
        self.populate();self.update(5,lambda v:v['payload'].update(reason=SECRET))
        out=self.slot();self.assertEqual(out['stop_reason'],'unrecognized');self.assertTrue(out['turn_completed'])

    def test_unknown_missing_or_nonterminal_turn_is_unavailable_not_completed(self):
        for state in [SECRET,None,True,1,'running','idle']:
            with self.subTest(state=state):
                self.f.sql('DELETE FROM events');self.populate();self.update(3,lambda v:v['payload'].update(turn_state=state));out=self.slot()
                self.assertEqual(out['turn_status'],'unavailable');self.assertIsNone(out['turn_completed']);self.assertIsNone(out['completed_turn_count'])

    def test_multiple_turns_and_illegal_window_never_choose_one(self):
        self.populate();self.add(4,'session.turn_ended','provider_runtime',SESSION,dict(turn_state='completed',task_accepted=False))
        self.assertEqual(self.slot()['turn_status'],'ambiguous');self.assertIsNone(self.slot()['completed_turn_count'])
        self.f.sql('DELETE FROM events WHERE sequence=4');self.f.sql('UPDATE events SET sequence=6 WHERE sequence=3')
        out=self.slot();self.assertEqual(out['turn_status'],'unavailable');self.assertIsNone(out['turn_completed'])

    def test_wrong_turn_sender_does_not_fabricate_observed_zero(self):
        self.populate();self.update(3,lambda v:v.update(source='goal_core'))
        out=self.slot();self.assertEqual(out['turn_status'],'unavailable');self.assertIsNone(out['completed_turn_count'])

    def test_foreign_turn_cannot_supply_completed_for_bound_session(self):
        self.populate(turn=None);self.add(3,'session.turn_ended','provider_runtime','foreign-'+SECRET,dict(turn_state='completed',task_accepted=False))
        out=self.slot();self.assertFalse(out['turn_completed']);self.assertEqual(out['completed_turn_count'],0)

    def test_bad_json_duplicate_keys_and_existing_source_schema_guards_remain_strict(self):
        self.populate()
        with sqlite3.connect(self.f.db)as db:original=db.execute('SELECT document FROM events WHERE sequence=5').fetchone()[0]
        duplicated=original.replace('"process_reaped": true','"process_reaped": true,"process_reaped": true')
        self.assertNotEqual(duplicated,original)
        self.f.sql('UPDATE events SET document=? WHERE sequence=5',(duplicated,))
        value=P.collect(str(self.f.root));self.assertNotIn(SECRET,P.render_public(value));self.assertEqual(value['cases'][0]['status'],'invalid')
        self.f.sql('UPDATE events SET document=? WHERE sequence=5',('{bad JSON',))
        self.assertEqual(P.collect(str(self.f.root))['cases'][0]['status'],'invalid')

    def test_reserved_before_stop_before_settlement_is_required(self):
        for sequence,replacement in [(5,12),(1,8)]:
            with self.subTest(sequence=sequence):
                self.f.sql('DELETE FROM events');self.populate()
                self.f.sql('UPDATE events SET sequence=? WHERE sequence=?',(replacement,sequence))
                out=self.slot();self.assertEqual(out['status'],'unavailable');self.assertIsNone(out['process_reaped'])

    def test_never_started_and_reaped_are_separate_exact_booleans(self):
        self.populate(turn=None)
        self.update(5,lambda v:v['payload'].update(reason='local_launch_failed_before_worker_start',process_reaped=False,no_process_started=True))
        self.update(10,lambda v:v['payload'].update(actual_output_bytes=0))
        out=self.slot();self.assertIs(out['process_reaped'],False);self.assertIs(out['no_process_started'],True)
        self.assertEqual(out['stop_reason'],'local_launch_failed_before_worker_start');self.assertFalse(out['turn_completed'])

    def test_direct_synthetic_readonly_transaction_does_not_change_original_db(self):
        self.populate();before=self.f.db.read_bytes();digest=hashlib.sha256(before).hexdigest()
        with sqlite3.connect(self.f.db.as_uri()+'?mode=ro&immutable=1',uri=True)as db:
            db.execute('PRAGMA query_only=ON');db.execute('PRAGMA trusted_schema=OFF');db.execute('BEGIN')
            out=P.project_events(db,self.observation);self.assertTrue(out['settled_session_stops'][0]['turn_completed'])
            self.assertEqual(db.execute('PRAGMA query_only').fetchone(),(1,))
            with self.assertRaises(sqlite3.OperationalError):db.execute('CREATE TABLE forbidden_write(x)')
        self.assertEqual(self.f.db.read_bytes(),before);self.assertEqual(hashlib.sha256(self.f.db.read_bytes()).hexdigest(),digest)

if __name__=='__main__':unittest.main()
