"""Pure data/fake connection controls only; no real SQLite engine or product calls."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_pure_attempt_budget', ROOT / 'scripts/record-factory-rework-ci-evidence.py')
P = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(P)
SECRET = 'PRIVATE_TOKEN_PATH_AND_ID_CANARY'


def encoded(value): return json.dumps(value)


def reservation(runtime=4000, output=4096):
    return dict(runtime_ms=runtime, output_bytes=output, tokens=None, cost_microunits=None)


def origin_fixture():
    origin = dict(goal=SECRET+'-goal', scope=SECRET+'-scope', child=SECRET+'-child',
        root=SECRET+'-grant', project=SECRET+'-project', factory=SECRET+'-factory', root_task=SECRET+'-root')
    run = dict(id=origin['goal'], task_id=origin['child'], root_grant_id=origin['root'], project_id=origin['project'])
    derived = dict(run_id=origin['goal'], initial_run=run,
        origin=dict(factory_scope_id=origin['scope'], factory_run_id=origin['factory'],
            root_task_id=origin['root_task'], initial_plan=dict(task_id=origin['child'])))
    return origin, (origin['goal'], origin['scope'], origin['child'], encoded(derived)), [(encoded(run),)]


def graph_fixture(kind='work', revision=1, actual=1234, settled=True):
    origin, _, _ = origin_fixture(); attempt, node = SECRET+'-attempt', SECRET+'-node'
    declared = reservation()
    value = dict(run_id=origin['goal'], node_id=node, plan_revision=revision, reservation=declared,
        ancestors=[], settled=settled, actual_runtime_ms=actual, actual_output_bytes=20,
        tokens=None, cost_microunits=None)
    session = SECRET+'-session' if kind == 'work' else None
    state='candidate'if kind=='work'else'failed'
    actual_node = dict(node=dict(id=node, kind=kind, reservation=declared), plan_revision=revision, state=state, started_at_ms=1,
        attempt_id=attempt, session_id=session, stop_confirmed=settled,
        **{k:value[k] for k in ('actual_runtime_ms','actual_output_bytes','tokens','cost_microunits')})
    event = dict(aggregate_id=origin['goal'], source='goal_core', kind='goal.node_reserved',
        payload=dict(attempt_id=attempt,node_id=node,plan_revision=revision,session_id=session,reservation=declared))
    return (attempt, origin['goal'], node, encoded(value)), [(origin['goal'],node,revision,attempt,session,state,encoded(actual_node))], [(9,encoded(event))]


def factory_fixture(actual=2400, settled=True):
    origin, _, _ = origin_fixture(); attempt, session = SECRET+'-planner', SECRET+'-session-planner'
    declared = reservation(6000)
    value = dict(authority_kind='factory_task',attempt_id=attempt,scope_id=origin['scope'],root_task_id=origin['root_task'],
        root_grant_id=origin['root'],project_id=origin['project'],run_id=origin['factory'],task_id=origin['child'],
        session_id=session,ancestors=[origin['root_task'],origin['child']],reservation=declared,
        settled=settled,actual_runtime_ms=actual,actual_output_bytes=None,queued_ms=0,
        measurement_scope='selected_artifacts_no_validator_streams',filesystem_total_bytes=None,
        captured_provider_message_bytes=None,provider_message_capture_complete=False)
    attempt_doc = dict(session_id=session,attempt_id=attempt,scope_id=origin['scope'],run_id=origin['factory'],
        task_id=origin['child'],root_task_id=origin['root_task'],stop_confirmed=int(settled))
    session_doc = dict(id=session,task_id=origin['child'],project_id=origin['project'],root_grant_id=origin['root'],attempt_id=attempt)
    return (attempt,origin['scope'],origin['root_task'],origin['project'],encoded(value)), [
        (session,attempt,origin['scope'],origin['factory'],origin['child'],origin['root_task'],encoded(declared),encoded(attempt_doc))],[
        (session,origin['child'],origin['project'],origin['root'],encoded(session_doc))]


class PureRecordBinding(unittest.TestCase):
    def setUp(self): self.origin, _, _ = origin_fixture()

    def test_graph_original_row_node_and_reserved_event_bind_before_finite_comparison(self):
        for kind in ('work','validate','owner_approval','summary'):
            row,nodes,events=graph_fixture(kind,revision=2)
            attempt,fact=P.graph_attempt_budget_fact(self.origin,row,nodes,events,0)
            self.assertEqual(attempt,row[0]); self.assertEqual(fact['record_kind'],'graph_'+kind)
            self.assertEqual(fact['plan_revision'],'2'); self.assertFalse(fact['runtime_exceeds_declared'])
            self.assertNotIn(SECRET,encoded(fact));self.assertNotIn('attempt_id',fact)
        row,nodes,events=graph_fixture(actual=4001)
        self.assertTrue(P.graph_attempt_budget_fact(self.origin,row,nodes,events,0)[1]['runtime_exceeds_declared'])

    def test_graph_swapped_binding_duplicate_mirror_types_metrics_and_event_refuse(self):
        row,nodes,events=graph_fixture()
        changes=[('row',1,'foreign'),('nodes',0,'foreign'),('nodes',1,'foreign'),('nodes',2,True),
            ('nodes',3,'foreign'),('nodes',4,'foreign')]
        for section,index,bad in changes:
            rr,nn,ee=copy.deepcopy((row,nodes,events))
            if section=='row': rr=list(rr);rr[index]=bad
            else: nn[0]=list(nn[0]);nn[0][index]=bad
            with self.subTest(section=section,index=index),self.assertRaises(P.Invalid):
                P.graph_attempt_budget_fact(self.origin,rr,nn,ee,0)
        for section,key,bad in [('node','actual_runtime_ms',None),('node','stop_confirmed',1),
            ('node','plan_revision',True),('event','attempt_id','foreign'),('event','plan_revision',True)]:
            rr,nn,ee=copy.deepcopy((row,nodes,events))
            if section=='node':
                doc=json.loads(nn[0][-1]);doc[key]=bad;nn[0]=(*nn[0][:-1],encoded(doc))
            else:
                doc=json.loads(ee[0][1]);doc['payload'][key]=bad;ee[0]=(ee[0][0],encoded(doc))
            with self.subTest(section=section,key=key),self.assertRaises(P.Invalid):
                P.graph_attempt_budget_fact(self.origin,rr,nn,ee,0)
        for nn,ee,mirror in [(nodes*2,events,0),(nodes,events*2,0),(nodes,events,1),(nodes,events,False)]:
            with self.assertRaises(P.Invalid):P.graph_attempt_budget_fact(self.origin,row,nn,ee,mirror)
        for target in ['node','event']:
            nn,ee=copy.deepcopy((nodes,events))
            if target=='node':
                doc=json.loads(nn[0][-1]);doc['node']['reservation']['runtime_ms']=True;nn[0]=(*nn[0][:-1],encoded(doc))
            else:
                doc=json.loads(ee[0][-1]);doc['payload']['reservation']['runtime_ms']=True;ee[0]=(ee[0][0],encoded(doc))
            with self.assertRaises(P.Invalid):P.graph_attempt_budget_fact(self.origin,row,nn,ee,0)

    def test_factory_session_and_original_attempt_reservation_bind_without_assigning_plan(self):
        row,attempts,sessions=factory_fixture()
        _,fact=P.factory_attempt_budget_fact(self.origin,row,attempts,sessions)
        self.assertEqual(fact['record_kind'],'factory_task');self.assertIsNone(fact['plan_revision'])
        self.assertEqual(fact['declared_runtime_ms'],6000);self.assertEqual(fact['actual_runtime_ms'],2400)
        self.assertFalse(fact['runtime_exceeds_declared']);self.assertIsNone(fact['output_exceeds_declared'])
        self.assertNotIn(SECRET,encoded(fact))
        value=json.loads(row[4]);value['ancestors']=[self.origin['root_task']]
        self.assertIsNone(P.factory_attempt_budget_fact(self.origin,(*row[:4],encoded(value)),[],[]))

    def test_factory_foreign_root_project_session_attempt_or_mirrored_document_refuse(self):
        row,attempts,sessions=factory_fixture()
        for section,index in [('row',1),('row',2),('row',3),('attempt',0),('attempt',1),('attempt',2),
            ('attempt',3),('attempt',4),('attempt',5),('session',0),('session',1),('session',2),('session',3)]:
            rr,aa,ss=copy.deepcopy((row,attempts,sessions))
            if section=='row':rr=list(rr);rr[index]='foreign'
            elif section=='attempt':aa[0]=list(aa[0]);aa[0][index]='foreign'
            else:ss[0]=list(ss[0]);ss[0][index]='foreign'
            with self.subTest(section=section,index=index),self.assertRaises(P.Invalid):
                P.factory_attempt_budget_fact(self.origin,rr,aa,ss)
        for aa,ss in [(attempts*2,sessions),(attempts,sessions*2),([],sessions),(attempts,[])]:
            with self.assertRaises(P.Invalid):P.factory_attempt_budget_fact(self.origin,row,aa,ss)
        aa=copy.deepcopy(attempts);declared=json.loads(aa[0][6]);declared['tokens']=False
        aa[0]=(*aa[0][:6],encoded(declared),aa[0][7])
        with self.assertRaises(P.Invalid):P.factory_attempt_budget_fact(self.origin,row,aa,sessions)

    def test_factory_stop_confirmed_production_integer_and_boolean_encodings_bind_strict_settlement(self):
        for settled, raw in ((True, 1), (False, 0), (True, True), (False, False)):
            row,attempts,sessions=factory_fixture(settled=settled)
            doc=json.loads(attempts[0][-1]);doc['stop_confirmed']=raw
            attempts[0]=(*attempts[0][:-1],encoded(doc))
            with self.subTest(settled=settled,raw_type=type(raw).__name__):
                fact=P.factory_attempt_budget_fact(self.origin,row,attempts,sessions)[1]
                self.assertIs(fact['settled'],settled)
        for settled,raw in ((True,0),(False,1)):
            row,attempts,sessions=factory_fixture(settled=settled)
            doc=json.loads(attempts[0][-1]);doc['stop_confirmed']=raw
            attempts[0]=(*attempts[0][:-1],encoded(doc))
            with self.assertRaises(P.Invalid):P.factory_attempt_budget_fact(self.origin,row,attempts,sessions)
        # The compatibility is local to this production field.
        for raw in (0,1):
            with self.assertRaises(P.Invalid):P.boolean(raw)
            row,attempts,sessions=factory_fixture()
            doc=json.loads(row[-1]);doc['settled']=raw
            with self.assertRaises(P.Invalid):
                P.factory_attempt_budget_fact(self.origin,(*row[:-1],encoded(doc)),attempts,sessions)

    def test_complete_production_reservation_shapes_reject_extra_and_missing_fields(self):
        row,nodes,events=graph_fixture();factory,attempts,sessions=factory_fixture()
        self.assertEqual(len(json.loads(row[3])),10)
        self.assertEqual(len(json.loads(factory[4])),19)
        # Both are diagnostic closed shapes; only FactoryReservation has the
        # production Rust deny_unknown_fields attribute.
        for shape in ('graph','factory'):
            for action in ('extra','missing'):
                if shape=='graph':
                    changed=list(row);value=json.loads(changed[3])
                    if action=='extra':value['unexpected']=SECRET
                    else:value.pop('tokens')
                    changed[3]=encoded(value)
                    with self.subTest(shape=shape,action=action),self.assertRaises(P.Invalid):
                        P.graph_attempt_budget_fact(self.origin,changed,nodes,events,0)
                else:
                    changed=list(factory);value=json.loads(changed[4])
                    if action=='extra':value['unexpected']=SECRET
                    else:value.pop('captured_provider_message_bytes')
                    changed[4]=encoded(value)
                    with self.subTest(shape=shape,action=action),self.assertRaises(P.Invalid):
                        P.factory_attempt_budget_fact(self.origin,changed,attempts,sessions)

    def test_missing_runtime_is_unknown_not_zero_and_wrong_types_are_refused(self):
        for settled in (True,False):
            row,nodes,events=graph_fixture(actual=None,settled=settled)
            fact=P.graph_attempt_budget_fact(self.origin,row,nodes,events,0)[1]
            self.assertIsNone(fact['actual_runtime_ms']);self.assertIsNone(fact['runtime_exceeds_declared'])
        for bad in (True,-1,2**64,'1',float('nan')):
            row,nodes,events=graph_fixture(actual=bad)
            with self.subTest(bad_type=type(bad).__name__),self.assertRaises((P.Invalid,ValueError)):
                P.graph_attempt_budget_fact(self.origin,row,nodes,events,0)
        for bad in (True,-1,2**64,None):
            with self.assertRaises(P.Invalid):P.attempt_budget_reservation(dict(reservation(),runtime_ms=bad))

    def test_origin_foreign_current_root_or_project_is_not_a_selected_child(self):
        origin,row,current=origin_fixture();self.assertEqual(P.attempt_budget_origin(row,current),origin)
        for key in ('id','task_id','root_grant_id','project_id'):
            doc=json.loads(current[0][0]);doc[key]='foreign'
            with self.subTest(key=key),self.assertRaises(P.Invalid):P.attempt_budget_origin(row,[(encoded(doc),)])
        with self.assertRaises(P.Invalid):P.attempt_budget_origin(row,current*2)


class Cursor(list):
    def fetchone(self):return self[0]if self else None
    def fetchall(self):return list(self)


class ScriptedConnection:
    def __init__(self):
        self.origin,self.origin_row,self.current=origin_fixture()
        self.graph,self.nodes,self.events=graph_fixture()
        self.factory,self.attempts,self.sessions=factory_fixture()
        self.commands=[];self.graph_rows=[self.graph];self.factory_rows=[self.factory]
        self.schema_bad=None;self.count_bad=None;self.mirror=0
    def execute(self,sql,params=()):
        self.commands.append((sql,params))
        if sql.startswith('SELECT type,sql FROM sqlite_master'):
            return Cursor([('view'if params[0]==self.schema_bad else 'table','CREATE TABLE fixed(document TEXT)')])
        if sql.startswith('SELECT count(*),coalesce(sum'):
            return Cursor([(513 if self.count_bad else 1,100,100)])
        if sql.startswith('SELECT goal_run_id,'):return Cursor([self.origin_row])
        if sql.startswith('SELECT document FROM goal_runs'):return Cursor(self.current)
        if sql.startswith('SELECT attempt_id,run_id,node_id,document'):return Cursor(self.graph_rows)
        if sql.startswith('SELECT run_id,node_id,plan_revision,attempt_id,session_id,state,document'):
            if 'WHERE run_id=? ORDER BY' in sql:return Cursor([r for r in self.nodes if r[0]==params[0]])
            if 'WHERE attempt_id=? OR' in sql:
                return Cursor([r for r in self.nodes if r[3]==params[0]or json.loads(r[-1]).get('attempt_id')==params[1]][:2])
            raise AssertionError('Same-attempt node query lost its reverse JSON/column coverage')
        if sql.startswith('SELECT sequence,document FROM events'):
            rows=[r for r in self.events if json.loads(r[1])['source']=='goal_core'and json.loads(r[1])['kind']=='goal.node_reserved']
            if "'$.aggregate_id')=?"in sql:return Cursor([r for r in rows if json.loads(r[1])['aggregate_id']==params[0]])
            return Cursor([r for r in rows if json.loads(r[1])['payload']['attempt_id']==params[0]][:2])
        if sql.startswith('SELECT count(*) FROM factory_budget_reservations'):
            if ' OR 'not in sql:raise AssertionError('Cross-kind JSON/column mirror coverage missing')
            return Cursor([(self.mirror+sum(r[0]==params[0]or json.loads(r[-1]).get('attempt_id')==params[1]for r in self.factory_rows),)])
        if sql.startswith('SELECT count(*) FROM goal_run_reservations'):
            return Cursor([(sum(r[0]==params[0]for r in self.graph_rows),)])
        if sql.startswith('SELECT attempt_id,scope_id,root_task_id,project_id,document'):return Cursor(self.factory_rows)
        if sql.startswith('SELECT session_id,attempt_id,scope_id,run_id,task_id,root_task_id,reservation,document'):
            if ' OR 'not in sql:raise AssertionError('Factory attempt reverse coverage missing')
            return Cursor([r for r in self.attempts if r[1]==params[0]or json.loads(r[-1]).get('attempt_id')==params[1]
                or r[0]==params[2]or json.loads(r[-1]).get('session_id')==params[3]][:2])
        if sql.startswith('SELECT id,task_id,project_id,root_grant_id,document'):
            if ' OR 'not in sql:raise AssertionError('Session JSON attempt uniqueness check missing')
            return Cursor([r for r in self.sessions if r[0]==params[0]or json.loads(r[-1]).get('attempt_id')==params[1]][:2])
        raise AssertionError('Unexpected or mutating cursor call')



class OptionalReaderControls(unittest.TestCase):
    def test_complete_bound_original_rows_publish_no_private_keys_or_admission_claim(self):
        conn=ScriptedConnection()
        with mock.patch.object(P.sqlite3,'connect',side_effect=AssertionError('No SQLite connection permitted')):
            value=P.attempt_budget_observations(conn)
        self.assertEqual(value['status'],'collected_bound_record_facts')
        self.assertEqual([r['ordinal']for r in value['records']],[1,2])
        self.assertEqual([r['record_kind']for r in value['records']],['graph_work','factory_task'])
        self.assertTrue(value['other_root_project_records_not_projected'])
        self.assertTrue(value['not_an_admission_stop_or_authority_verdict'])
        self.assertNotIn(SECRET,encoded(value))
        self.assertTrue(all(sql.startswith('SELECT ')for sql,_ in conn.commands))
        for sql,params in conn.commands:
            if 'WHERE' in sql and 'sqlite_master'not in sql:self.assertTrue(params)

    def test_partial_malformed_or_ambiguous_data_returns_no_partial_rows_and_no_raw_error(self):
        for variant in ('schema','count','node','events','mirror','factory','unknown','missing'):
            conn=ScriptedConnection()
            if variant=='schema':conn.schema_bad='sessions'
            elif variant=='count':conn.count_bad=True
            elif variant=='node':conn.nodes=[]
            elif variant=='events':conn.events*=2
            elif variant=='mirror':conn.mirror=1
            elif variant=='factory':conn.attempts=[]
            elif variant=='unknown':
                conn.factory=list(conn.factory);value=json.loads(conn.factory[4]);value['actual_runtime_ms']=SECRET
                conn.factory[4]=encoded(value);conn.factory_rows=[conn.factory]
            else:
                conn.graph_rows=[];conn.factory_rows=[]
                # Existing reserved nodes/events still require a reservation.
            with self.subTest(variant=variant):
                value=P.attempt_budget_observations(conn)
                self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])
                self.assertNotIn(SECRET,encoded(value))

    def test_duplicate_records_cross_kind_identity_and_row_bound_do_not_double_count(self):
        conn=ScriptedConnection();conn.graph_rows*=2
        self.assertEqual(P.attempt_budget_observations(conn)['records'],[])
        # Exercise the row cap separately with distinct identities; a duplicate
        # row must not make the size-bound assertion pass before that guard.
        template=ScriptedConnection()
        fact=P.graph_attempt_budget_fact(template.origin,template.graph,template.nodes,template.events,0)[1]
        for count in (P.MAX_ATTEMPT_BUDGET_ROWS,P.MAX_ATTEMPT_BUDGET_ROWS+1):
            conn=ScriptedConnection();conn.factory_rows=[]
            conn.graph_rows=[(SECRET+'-'+str(i),*conn.graph[1:])for i in range(count)]
            ids={row[0]for row in conn.graph_rows}
            with mock.patch.object(P,'graph_attempt_budget_fact',side_effect=lambda origin,row,*_: (row[0],copy.deepcopy(fact))),\
                mock.patch.object(P,'graph_budget_source_sets',return_value=(ids,ids)):
                value=P.attempt_budget_observations(conn)
            self.assertEqual(value['status'],'collected_bound_record_facts'if count==P.MAX_ATTEMPT_BUDGET_ROWS else'pending')
            self.assertEqual(len(value['records']),count if count==P.MAX_ATTEMPT_BUDGET_ROWS else 0)

    def test_reverse_graph_coverage_never_returns_factory_partial_for_missing_original_graph_rows(self):
        for missing in ('reservation','nodes','events'):
            conn=ScriptedConnection()
            self.assertEqual(P.attempt_budget_observations(conn)['status'],'collected_bound_record_facts')
            if missing=='reservation':conn.graph_rows=[]
            elif missing=='nodes':conn.nodes=[]
            else:conn.events=[]
            with self.subTest(missing=missing):
                value=P.attempt_budget_observations(conn)
                self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])

    def test_valid_never_reserved_graph_and_unstarted_nodes_do_not_invent_missing_attempts(self):
        for unstarted in (False,True):
            conn=ScriptedConnection();conn.graph_rows=[];conn.events=[]
            if unstarted:
                row=list(conn.nodes[0]);row[3]=row[4]=None;row[5]='not_started'
                doc=json.loads(row[-1]);doc.update(attempt_id=None,session_id=None,state='not_started',
                    started_at_ms=None,stop_confirmed=False,actual_runtime_ms=None,actual_output_bytes=None)
                row[-1]=encoded(doc);conn.nodes=[row]
            else:conn.nodes=[]
            value=P.attempt_budget_observations(conn)
            self.assertEqual(value['status'],'collected_bound_record_facts')
            self.assertEqual([r['record_kind']for r in value['records']],['factory_task'])
            conn.factory_rows=[]
            value=P.attempt_budget_observations(conn)
            self.assertEqual(value['status'],'collected_bound_record_facts');self.assertEqual(value['records'],[])
            self.assertEqual(value['code'],'no_selected_attempt_sources')

    def test_node_session_without_attempt_is_not_a_never_reserved_positive(self):
        conn=ScriptedConnection();conn.graph_rows=[];conn.events=[]
        row=list(conn.nodes[0]);row[3]=None
        doc=json.loads(row[-1]);doc['attempt_id']=None;row[-1]=encoded(doc);conn.nodes=[row]
        value=P.attempt_budget_observations(conn)
        self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])

    def test_same_attempt_foreign_nodes_events_sessions_and_attempt_documents_are_not_hidden_by_tuple_filters(self):
        for kind in ('node','node-json','event','session','session-json','factory-attempt'):
            conn=ScriptedConnection()
            self.assertEqual(P.attempt_budget_observations(conn)['status'],'collected_bound_record_facts')
            if kind in ('node','node-json'):
                shadow=list(conn.nodes[0]);shadow[0]='foreign-goal';shadow[1]='foreign-node';shadow[2]=2
                doc=json.loads(shadow[-1]);doc['node']['id']='foreign-node';doc['plan_revision']=2
                if kind=='node-json':shadow[3]='foreign-column-attempt'
                shadow[-1]=encoded(doc);conn.nodes.append(shadow)
            elif kind=='event':
                seq,raw=conn.events[0];doc=json.loads(raw);doc['aggregate_id']='foreign-goal'
                conn.events.append((seq+1,encoded(doc)))
            elif kind in ('session','session-json'):
                shadow=list(conn.sessions[0]);shadow[0]='foreign-session'
                doc=json.loads(shadow[-1]);doc['id']='foreign-session'
                if kind=='session-json':shadow[1]='foreign-task';doc['task_id']='foreign-task'
                shadow[-1]=encoded(doc);conn.sessions.append(shadow)
            else:
                shadow=list(conn.attempts[0]);shadow[0]='foreign-session';shadow[1]='foreign-column-attempt'
                shadow[-1]=conn.attempts[0][-1];conn.attempts.append(shadow)
            value=P.attempt_budget_observations(conn)
            with self.subTest(kind=kind):
                self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])
                self.assertNotIn(SECRET,encoded(value))

    def test_factory_stop_integer_encoding_whole_optional_positive_and_bad_values_refuse_all_rows(self):
        for settled,raw in ((True,1),(False,0),(True,True),(False,False)):
            conn=ScriptedConnection();conn.factory,conn.attempts,conn.sessions=factory_fixture(settled=settled)
            conn.factory_rows=[conn.factory]
            doc=json.loads(conn.attempts[0][-1]);doc['stop_confirmed']=raw
            conn.attempts[0]=(*conn.attempts[0][:-1],encoded(doc))
            value=P.attempt_budget_observations(conn)
            with self.subTest(settled=settled,raw_type=type(raw).__name__):
                self.assertEqual(value['status'],'collected_bound_record_facts')
                self.assertEqual(len(value['records']),2);self.assertIs(value['records'][1]['settled'],settled)
        for bad in (2,-1,'1','0',1.0,0.0,None,[],{},0):
            conn=ScriptedConnection()
            self.assertEqual(P.attempt_budget_observations(conn)['status'],'collected_bound_record_facts')
            doc=json.loads(conn.attempts[0][-1]);doc['stop_confirmed']=bad
            conn.attempts[0]=(*conn.attempts[0][:-1],encoded(doc))
            value=P.attempt_budget_observations(conn)
            with self.subTest(bad=bad):
                self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])
                self.assertNotIn(SECRET,encoded(value))

    def test_cross_kind_foreign_and_json_only_reservation_mirrors_are_pending(self):
        for direction in ('factory-json-mirror','foreign-graph-mirror'):
            conn=ScriptedConnection()
            self.assertEqual(P.attempt_budget_observations(conn)['status'],'collected_bound_record_facts')
            if direction=='factory-json-mirror':
                shadow=list(conn.factory);shadow[0]='foreign-column-attempt';shadow[1]='foreign-scope'
                doc=json.loads(shadow[-1]);doc['attempt_id']=conn.graph[0];doc['scope_id']='foreign-scope'
                shadow[-1]=encoded(doc);conn.factory_rows.append(shadow)
            else:
                shadow=list(conn.graph);shadow[0]=conn.factory[0];shadow[1]='foreign-goal'
                doc=json.loads(shadow[-1]);doc['run_id']='foreign-goal';shadow[-1]=encoded(doc)
                # This row is outside the selected child, but still conflicts
                # with the selected Factory attempt's original record kind.
                original_execute=conn.execute
                def execute(sql,params=()):
                    if sql.startswith('SELECT count(*) FROM goal_run_reservations'):
                        return Cursor([(int(shadow[0]==params[0]),)])
                    return original_execute(sql,params)
                conn.execute=execute
            value=P.attempt_budget_observations(conn)
            with self.subTest(direction=direction):
                self.assertEqual(value['status'],'pending');self.assertEqual(value['records'],[])
                self.assertNotIn(SECRET,encoded(value))

    def test_existing_projector_source_restores_exact_base_by_removing_only_new_helper_and_field(self):
        text=(ROOT/'scripts/record-factory-rework-ci-evidence.py').read_text()
        start=text.index('MAX_ATTEMPT_BUDGET_ROWS = 32\n');end=text.index('# Optional original-guard sample',start)
        field="                    'attempt_budget_observations': attempt_budget_observations(conn),\n"
        self.assertEqual(text.count(field),1)
        restored=(text[:start]+text[end:]).replace(field,'',1)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),
            'ca0be16d4b9d7f1383280123a6eb702d11492d4226db83c810f854c588f00f53')


class ProjectDBGuardComposition(unittest.TestCase):
    class FakeDB:
        def __init__(self,bad_schema=False):self.closed=False;self.bad_schema=bad_schema
        def close(self):self.closed=True
        def set_progress_handler(self,*_):pass
        def execute(self,sql,params=()):
            if sql=='PRAGMA user_version':return Cursor([(17,)])
            if sql=='PRAGMA query_only':return Cursor([(1,)])
            if sql.startswith('PRAGMA '):return Cursor([])
            if sql.startswith('SELECT type,sql FROM sqlite_master'):
                return Cursor([('view'if self.bad_schema else'table','CREATE TABLE fixed(document TEXT)')])
            if sql.startswith('SELECT count(*),'):return Cursor([(1,100,100)])
            if sql.startswith('SELECT count(*) FROM "'):return Cursor([(0,)])
            if sql=='SELECT count(*) FROM factory_child_goal_runs':return Cursor([(1,)])
            if sql.startswith('SELECT state,json_extract'):return Cursor([('blocked','blocked',2,'false',0,'false',0)])
            if sql.startswith('SELECT plan_revision,state,json_extract'):return Cursor([])
            if sql.startswith('SELECT plan_revision,json_extract'):return Cursor([])
            raise AssertionError('Unexpected outer fake database call')

    def test_optional_success_never_masks_original_early_schema_or_late_event_guard_and_connection_closes(self):
        for phase in ('positive','early','late'):
            db=self.FakeDB(bad_schema=phase=='early')
            optional={'status':'collected_bound_record_facts','records':[]}
            with mock.patch.object(P.sqlite3,'connect',return_value=db),\
                mock.patch.object(P,'validation_observations',return_value={}),\
                mock.patch.object(P,'budget_usage',return_value={}),\
                mock.patch.object(P,'attempt_budget_observations',return_value=optional)as observed,\
                mock.patch.object(P,'project_events',side_effect=P.Invalid()if phase=='late'else None,return_value={}):
                if phase=='positive':
                    value=P.project_db(b'synthetic bytes; fake connection only')
                    self.assertEqual(value['attempt_budget_observations'],optional)
                else:
                    with self.assertRaises(P.Invalid):P.project_db(b'synthetic bytes; fake connection only')
                self.assertTrue(db.closed)
                self.assertEqual(observed.call_count,0 if phase=='early'else 1)


if __name__=='__main__':unittest.main()
