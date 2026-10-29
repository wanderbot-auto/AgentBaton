#!/usr/bin/env python3
"""Read-only verifier controls on inert exact-column SQLite; no Core/browser proof."""
import copy
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import unittest

SOURCE = Path(__file__).with_name('factory-real-ipc.py')
spec = importlib.util.spec_from_file_location('_shared_factory_test', SOURCE)
base = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = base
spec.loader.exec_module(base)


def fixture(count=1, state='cancelled'):
    db = sqlite3.connect(':memory:')
    # Relevant production columns only; no schema-version claim or daemon.
    schemas = {
        'execution_grants': 'id TEXT, document TEXT',
        'factory_scopes': 'id TEXT, document TEXT', 'factory_runs': 'id TEXT, document TEXT',
        'factory_budget_reservations': 'attempt_id TEXT,scope_id TEXT,root_task_id TEXT,project_id TEXT,document TEXT',
        'attempts': 'id TEXT,task_id TEXT,root_task_id TEXT,binding TEXT',
        'factory_attempts': 'session_id TEXT,attempt_id TEXT,scope_id TEXT,run_id TEXT,task_id TEXT,root_task_id TEXT,reservation TEXT,document TEXT',
        'sessions': 'id TEXT,task_id TEXT,project_id TEXT,workspace_id TEXT,connection_id TEXT,grant_id TEXT,root_grant_id TEXT,state TEXT,document TEXT',
        'dispatch_intents': 'id TEXT,attempt_id TEXT,state TEXT,document TEXT',
        'factory_task_lineage': 'task_id TEXT,root_task_id TEXT,parent_task_id TEXT,scope_id TEXT',
        'events': 'sequence INTEGER PRIMARY KEY,document TEXT',
        'goal_ledgers': 'scope TEXT,document TEXT',
        'budgets': 'scope TEXT,total_limit INTEGER,parallel_limit INTEGER,consumed INTEGER,active INTEGER',
    }
    for table in base.support.LEGACY_GOAL_TABLES:
        schemas.setdefault(table, 'document TEXT')
    for name, columns in schemas.items(): db.execute('CREATE TABLE '+name+'('+columns+')')
    def put(name, row):
        db.execute('INSERT INTO '+name+'('+','.join(row)+') VALUES('+','.join('?' for _ in row)+')', tuple(row.values()))
    encoded = lambda value: json.dumps(value, separators=(',', ':'), ensure_ascii=False)
    limits = {'attempts':8,'parallel':1,'runtime_ms':30000,'output_bytes':12288,'children':1,'reworks':0,'tokens':None,'cost_microunits':None}
    grant = {'id':'g','root_grant_id':'g','parent_id':None,'revision':1,'project_id':'p','scope':{'max_children':0}}
    conn = {'id':'c','environment_id':'env','identity_ref':'unknown:c','host_id':'local','capability_snapshot_id':'cap','config_revision':'config'}
    scope = {'id':'scope','root_task_id':'root','root_grant_id':'g','project_id':'p',
        'selection':{'capabilities':{'child_goal_execution':False},'root_budget':copy.deepcopy(limits),'project_budget':copy.deepcopy(limits),'workspace_ids':['w'],'connection_ids':['c']},
        'bindings':{'approved_grant':copy.deepcopy(grant),'connections':[conn],'workspaces':[{'id':'w','base_revision':'head'}]}}
    run = {'id':'run','scope_id':'scope','root_task_id':'root','project_id':'p','state':state,'active_session_id':None}
    put('execution_grants',{'id':'g','document':encoded(grant)})
    put('factory_scopes',{'id':'scope','document':encoded(scope)})
    put('factory_runs',{'id':'run','document':encoded(run)})
    put('factory_task_lineage',{'task_id':'root','root_task_id':'root','parent_task_id':None,'scope_id':'scope'})
    for i in range(count):
        aid,sid,intent = 'a'+str(i),'s'+str(i),'i'+str(i)
        reservation={'runtime_ms':5000,'output_bytes':4096,'tokens':None,'cost_microunits':None}
        r={'attempt_id':aid,'session_id':sid,'task_id':'root','root_task_id':'root','project_id':'p','scope_id':'scope','run_id':'run','root_grant_id':'g','authority_kind':'factory_task','settled':True,'measurement_scope':'selected_artifacts_no_validator_streams','filesystem_total_bytes':None,'provider_message_capture_complete':False,'captured_provider_message_bytes':13,'reservation':reservation,'queued_ms':2,'actual_runtime_ms':100+i,'actual_output_bytes':20+i,'ancestors':['root']}
        binding={k:v for k,v in conn.items() if k!='id'}
        binding.update(connection_id='c',workspace_id='w',policy_revision='g:1',input_revision='head')
        put('factory_budget_reservations',{'attempt_id':aid,'scope_id':'scope','root_task_id':'root','project_id':'p','document':encoded(r)})
        put('attempts',{'id':aid,'task_id':'root','root_task_id':'root','binding':encoded(binding)})
        put('factory_attempts',{'session_id':sid,'attempt_id':aid,'scope_id':'scope','run_id':'run','task_id':'root','root_task_id':'root','reservation':encoded(reservation),'document':encoded({'attempt_id':aid,'session_id':sid,'stop_confirmed':1,'never_started':0})})
        put('sessions',{'id':sid,'task_id':'root','project_id':'p','workspace_id':'w','connection_id':'c','grant_id':'g','root_grant_id':'g','state':'stopped','document':encoded({'id':sid,'attempt_id':aid,'intent_id':intent,'task_id':'root','state':'stopped'})})
        put('dispatch_intents',{'id':intent,'attempt_id':aid,'state':'stopped','document':encoded({'id':intent,'attempt_id':aid,'task_id':'root','root_task_id':'root','state':'stopped','binding':binding})})
        for offset,kind,source,payload in [(1,'session.stopped','baton_supervisor',{'process_reaped':True,'no_process_started':False}),
                (2,'shared_budget.session_settled','budget_core',{'attempt_id':aid,'stopped':True,'never_started':False,'runtime_ms':100+i,'selected_artifact_bytes':20+i,'measurement_scope':r['measurement_scope'],'filesystem_total_bytes':None,'task_accepted':False})]:
            put('events',{'sequence':i*2+offset,'document':encoded({'kind':kind,'source':source,'aggregate_id':sid,'payload':payload})})
    for key in ['project:p','root-grant:g']:
        put('goal_ledgers',{'scope':key,'document':encoded({'limits':limits,'observed_attempts':count,'runtime_ms':sum(100+i for i in range(count)),'output_bytes':sum(20+i for i in range(count)),'tokens':None,'cost_microunits':None})})
    for key in ['project:p','root:root']:
        put('budgets',{'scope':key,'total_limit':8,'parallel_limit':1,'consumed':count,'active':0})
    db.commit()
    return db, scope, grant, run, count


class SharedMeters(unittest.TestCase):
    def setUp(self): self.args=fixture(); self.db=self.args[0]
    def tearDown(self): self.db.close()
    def check(self): return base.verify_legacy_factory_shared_budget(*self.args)
    def alter(self, table, fn, column='document', where='rowid=1'):
        raw=self.db.execute('SELECT '+column+' FROM '+table+' WHERE '+where).fetchone()[0]
        value=json.loads(raw);fn(value)
        self.db.execute('UPDATE '+table+' SET '+column+'=? WHERE '+where,(json.dumps(value),))
    def test_three_terminal_states_read_only(self):
        for state,count in [('completed',3),('cancelled',1),('outcome_unknown',1)]:
            db,*rest=fixture(count,state)
            try:
                before=db.serialize();db.execute('PRAGMA query_only=ON')
                result=base.verify_legacy_factory_shared_budget(db,*rest)
                self.assertEqual(result['observed_attempts'],count);self.assertEqual(before,db.serialize())
            finally: db.close()
    def test_every_structural_goal_table_and_grant_history_stays_zero(self):
        for table in sorted(base.support.LEGACY_GOAL_TABLES-{'goal_ledgers'}):
            with self.subTest(table=table):
                self.db.execute('INSERT INTO '+table+' VALUES(?)',('{}',))
                with self.assertRaises(AssertionError):self.check()
                self.db.execute('DELETE FROM '+table)
    def test_extra_foreign_meter_refused(self):
        self.db.execute("INSERT INTO goal_ledgers SELECT 'project:foreign',document FROM goal_ledgers LIMIT 1")
        with self.assertRaises(AssertionError):self.check()
    def test_missing_meter_refused(self):
        self.db.execute('DELETE FROM goal_ledgers WHERE rowid=1')
        with self.assertRaises(AssertionError):self.check()
    def test_refunded_or_reset_counters_refused(self):
        for field in ['observed_attempts','runtime_ms','output_bytes']:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.alter('goal_ledgers',lambda m:m.update({field:0}))
            with self.assertRaises(AssertionError):self.check()
    def test_unknown_and_untyped_meter_refused(self):
        for value in [None,True,100.0,'100',-1,2**64]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.alter('goal_ledgers',lambda m:m.update(runtime_ms=value))
            with self.assertRaises(AssertionError):self.check()
    def test_limits_widened_or_narrowed_refused(self):
        for cap in [29999,30001]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.alter('goal_ledgers',lambda m:m['limits'].update(runtime_ms=cap))
            with self.assertRaises(AssertionError):self.check()
    def test_duplicate_json_keys_refused(self):
        self.db.execute("UPDATE goal_ledgers SET document=substr(document,1,length(document)-1)||',\"runtime_ms\":100}' WHERE rowid=1")
        with self.assertRaises((AssertionError,base.BoundaryRefusal,ValueError)):self.check()
    def test_goal_authorized_or_grant_changed_refused(self):
        self.alter('execution_grants',lambda g:g['scope'].update(max_children=1))
        with self.assertRaises(AssertionError):self.check()
    def test_foreign_reservation_or_legacy_authority_refused(self):
        for field,value in [('scope_id',None),('root_grant_id','foreign'),('authority_kind','legacy_shared_session'),('run_id','foreign'),('task_id','foreign'),('settled',False)]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.alter('factory_budget_reservations',lambda r:r.update({field:value}))
            with self.assertRaises((AssertionError,KeyError)):self.check()
    def test_measurement_unknown_not_whole_output_zero(self):
        for field,value in [('actual_runtime_ms',None),('actual_output_bytes',None),('filesystem_total_bytes',0),('provider_message_capture_complete',True)]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.alter('factory_budget_reservations',lambda r:r.update({field:value}))
            with self.assertRaises(AssertionError):self.check()
    def test_active_attempt_or_consumption_refund_refused(self):
        for field,value in [('active',1),('consumed',0),('total_limit',9),('parallel_limit',2)]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.db.execute('UPDATE budgets SET '+field+'=? WHERE rowid=1',(value,))
            with self.assertRaises(AssertionError):self.check()
    def test_session_attempt_intent_and_config_binding_refused(self):
        for table,column,value in [('sessions','root_grant_id','foreign'),('attempts','root_task_id','foreign'),('dispatch_intents','state','sent'),('factory_attempts','scope_id','foreign')]:
            self.db.close();self.args=fixture();self.db=self.args[0]
            self.db.execute('UPDATE '+table+' SET '+column+'=? WHERE rowid=1',(value,))
            with self.assertRaises(AssertionError):self.check()
    def test_connection_snapshot_drift_refused(self):
        self.alter('attempts',lambda b:b.update(config_revision='changed'),column='binding')
        with self.assertRaises(AssertionError):self.check()
    def test_missing_duplicated_or_foreign_settlement_refused(self):
        for variant in ['missing','duplicate','foreign','wrong_usage','false_stop','order']:
            self.db.close();self.args=fixture();self.db=self.args[0]
            if variant=='missing':self.db.execute('DELETE FROM events WHERE sequence=2')
            elif variant=='duplicate':self.db.execute('INSERT INTO events SELECT 3,document FROM events WHERE sequence=2')
            elif variant=='order':self.db.execute('UPDATE events SET sequence=3 WHERE sequence=1')
            else:
                self.alter('events',lambda e:e.update(source='foreign') if variant=='foreign' else e['payload'].update(runtime_ms=0) if variant=='wrong_usage' else e['payload'].update(stopped=False),where='sequence=2')
            with self.assertRaises(AssertionError):self.check()
    def test_unknown_stop_cannot_become_confirmed(self):
        self.alter('factory_attempts',lambda f:f.update(stop_confirmed=0))
        with self.assertRaises(AssertionError):self.check()
    def test_lineage_mismatch_refused(self):
        self.db.execute("UPDATE factory_task_lineage SET parent_task_id='foreign'")
        with self.assertRaises(AssertionError):self.check()
    def test_preflight_charge_not_hidden(self):
        self.db.execute('INSERT INTO events VALUES(3,?)',(json.dumps({'kind':'factory.preflight_finished'}),))
        with self.assertRaises(AssertionError):self.check()


if __name__=='__main__':unittest.main()
