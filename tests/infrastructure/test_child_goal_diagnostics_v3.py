"""Pure v3 metadata/source controls; no Core, browser, process or SQL execution."""
import ast
import copy
import json
from pathlib import Path
import re
import types
import unittest

ROOT=Path(__file__).resolve().parents[2]
P=types.ModuleType('v3_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')
exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)
SECRET='PRIVATE_CANARY_TOKEN_KEY_PID_ARGV_PATH'


def harness_helpers():
    path=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
    wanted={'graph_identity_initial','graph_empty_identity_diagnostic','remember_graph_cancel','exact_graph_cancel_rows'}
    nodes=[node for node in ast.parse(path.read_text()).body if isinstance(node,ast.FunctionDef)and node.name in wanted]
    env={'copy':copy,'re':re,'json':json,'finite_projection':P,'Refusal':ValueError,
        'support':types.SimpleNamespace(exact_json_value=P.strict_json_equal),
        'planning':types.SimpleNamespace(unique_object=P.unique_object,KEY=re.compile(r'[a-zA-Z0-9:_-]{1,128}'))}
    exec(compile(ast.Module(body=nodes,type_ignores=[]),str(path),'exec'),env)
    return env


def reply(code='WORKSPACE_BUSY',retryable=True,effect='not_started'):
    return {'error':{'code':code,'retryable':retryable,'side_effect_state':effect,'message':SECRET,'details_ref':SECRET},'request_id':SECRET}


def cancel_inputs():
    scope={'id':'scope','root_task_id':'root'}
    fixed={'run_id':'goal','request':{'task_binding':{'task_id':'child'}}}
    params={'scope_id':'scope','task_id':'child','goal_run_id':'goal','expected_goal_run_revision':3}
    call={'params':params,'revision':1,'key':'cancel-key'}
    record={'run_id':'goal','scope_id':'derived','origin':{'factory_scope_id':'scope','factory_run_id':'factory','root_task_id':'root'},
        'initial_run':{'id':'goal','scope_id':'derived','task_id':'child'}}
    current={'id':'goal','scope_id':'derived','task_id':'child','revision':5,'state':'cancelled','cancel_requested':True,'task_accepted':False}
    receipt={'factory_scope_id':'scope','factory_run_id':'factory','factory_root_task_id':'root','task_id':'child',
        'goal':{'task_accepted':False,'run':{**current,'revision':4,'state':'cancel_requested'}}}
    return scope,'factory',fixed,call,[('goal','derived','scope','factory','child','root',json.dumps(record))],\
        [('goal','derived','child',5,'cancelled',json.dumps(current))],[(json.dumps([1,params]),json.dumps(receipt))]


class V3DiagnosticsTests(unittest.TestCase):
    def test_v1_v2_inputs_preserved_and_v3_unknown_fields_and_guessed_epoch_refused(self):
        current=P.unobserved_diagnostics();self.assertEqual(current['version'],3)
        for version in (1,2):
            old=copy.deepcopy(current);old['version']=version;old.pop('owner_refresh');old.pop('graph_provider_identity')
            if version==1:old.pop('graph_provider_argv')
            self.assertEqual(P.finite_diagnostics(old),old)
        for change in (lambda v:v.update(token=SECRET),lambda v:v['owner_refresh'].update(source_epoch=1),
                       lambda v:v['owner_refresh'].update(full_fresh=True),lambda v:v['owner_refresh'].update(refresh_completed=True),
                       lambda v:v['owner_refresh'].update(loading=False),lambda v:v['owner_refresh'].update(pending_kind='none')):
            bad=copy.deepcopy(current);change(bad)
            with self.assertRaises(P.Refused)as error:P.finite_diagnostics(bad)
            self.assertNotIn(SECRET,str(error.exception))

    def test_owner_reply_requires_exact_binding_and_monotonic_window_without_click_causality(self):
        recorder=P.FiniteRuntimeDiagnostics();q={'scope_id':SECRET,'task_id':'task','goal_run_id':'goal'}
        recorder.owner_refresh_begin(q,100)
        recorder.owner_refresh_reply('factory_child_goals.get',q,reply(),99,101)
        recorder.owner_refresh_reply('factory_child_goals.get',{**q,'goal_run_id':'foreign'},reply(),101,102)
        recorder.owner_refresh_reply('factory_child_goals.cancel',q,reply(),101,102)
        recorder.owner_refresh_reply('factory_child_goals.list',{k:q[k]for k in ('scope_id','task_id')},reply(),101,102)
        recorder.owner_refresh_reply('factory_child_goals.get',q,reply(),102,103)
        recorder.owner_refresh_click_finished(104);recorder.owner_refresh_end(110)
        recorder.owner_refresh_reply('factory_child_goals.get',q,reply(),111,112)
        value=P.finite_diagnostics(recorder.snapshot())['owner_refresh']
        self.assertEqual(len(value['reads']),1);self.assertEqual(value['scope_task_only_read_count'],1)
        self.assertEqual(value['reads'][0]['error']['code'],'WORKSPACE_BUSY')
        self.assertEqual(value['reads'][0]['binding'],'scope_task_run')
        self.assertEqual(value['click_causality'],'not_established');self.assertEqual(value['correlation'],'temporal_only')
        self.assertIsNone(value['source_epoch']);self.assertIsNone(value['refresh_completed'])
        self.assertNotIn(SECRET,json.dumps(value))

    def test_owner_unknown_error_is_explicitly_unmapped_and_collection_is_capped(self):
        recorder=P.FiniteRuntimeDiagnostics();q={'scope_id':'scope','task_id':'task','goal_run_id':'goal'}
        recorder.owner_refresh_begin(q,100)
        for _ in range(100):recorder.owner_refresh_reply('factory_child_goals.get',q,reply(code=SECRET),101,102)
        value=P.finite_diagnostics(recorder.snapshot())['owner_refresh']
        self.assertEqual(len(value['reads']),64);self.assertTrue(value['truncated'])
        self.assertEqual(value['reads'][0]['reply_class'],'unmapped_error');self.assertIsNone(value['reads'][0]['error'])
        self.assertNotIn(SECRET,json.dumps(value))

    def test_owner_timing_types_backwards_time_and_unknown_nested_fields_are_refused(self):
        recorder=P.FiniteRuntimeDiagnostics();q={'scope_id':'s','task_id':'t','goal_run_id':'g'}
        recorder.owner_refresh_begin(q,100);recorder.owner_refresh_reply('factory_child_goals.get',q,reply(),101,102);recorder.owner_refresh_end(103)
        original=recorder.snapshot()
        for mutate in (lambda v:v['owner_refresh'].update(started_ns=True),lambda v:v['owner_refresh'].update(finished_ns=99),
                       lambda v:v['owner_refresh']['reads'][0].update(finished_ns=100),
                       lambda v:v['owner_refresh']['reads'][0].update(raw=SECRET),
                       lambda v:v['owner_refresh']['reads'][0]['error'].update(retryable=1)):
            bad=copy.deepcopy(original);mutate(bad)
            with self.assertRaises(P.Refused):P.finite_diagnostics(bad)

    def test_owner_window_cannot_finish_before_returned_click_and_partial_null_stays_unknown(self):
        recorder=P.FiniteRuntimeDiagnostics();q={'scope_id':'scope','task_id':'task','goal_run_id':'goal'}
        recorder.owner_refresh_begin(q,100)
        self.assertIsNone(P.finite_diagnostics(recorder.snapshot())['owner_refresh']['finished_ns'])
        recorder.owner_refresh_click_finished(120)
        partial=P.finite_diagnostics(recorder.snapshot())['owner_refresh']
        self.assertIsNone(partial['finished_ns']);self.assertIsNone(partial['refresh_completed'])
        recorder.owner_refresh_end(110)
        with self.assertRaises(P.Refused):P.finite_diagnostics(recorder.snapshot())
        recorder.owner_refresh_end(130)
        final=P.finite_diagnostics(recorder.snapshot())['owner_refresh']
        self.assertEqual(final['click_causality'],'not_established');self.assertIsNone(final['source_epoch'])

    def test_initial_identity_requires_real_double_sample_and_all_known_fields(self):
        h=harness_helpers();expected={'pid':123,'pgid':123,'start_ticks':'456'}
        sample={'observation':'observed','pid':123,'pgid':123,'ppid':99,'uid':1000,'start_ticks':'456'}
        self.assertEqual(h['graph_identity_initial'](expected,sample,sample,1000),sample)
        for field,value in (('pid',124),('pgid',124),('ppid',100),('uid',1001),('start_ticks','457')):
            after={**sample,field:value}
            self.assertIsNone(h['graph_identity_initial'](expected,sample,after,1000))
        self.assertIsNone(h['graph_identity_initial'](expected,sample,sample,1001))
        self.assertIsNone(h['graph_identity_initial'](expected,{**sample,'uid':True},{**sample,'uid':True},True))

    def test_empty_identity_is_observation_only_and_original_argv_refusal_stays(self):
        h=harness_helpers();known={'observation':'observed','pid':123,'pgid':123,'ppid':99,'uid':1000,'start_ticks':'456'}
        value=h['graph_empty_identity_diagnostic'](known,known,known,True)
        self.assertTrue(value['known_same_identity']);self.assertTrue(value['exact_cancel_durable'])
        self.assertEqual(value['after_read'],'same')
        with self.assertRaises(P.GraphProviderArgvFailure):P.check_graph_provider_argv(b'',[SECRET,'--fixture-settings',SECRET],True)
        absent=h['graph_empty_identity_diagnostic'](known,known,{'observation':'absent'},None)
        self.assertEqual(absent['after_read'],'absent');self.assertIsNone(absent['known_same_identity'])
        changed=h['graph_empty_identity_diagnostic'](known,known,{**known,'ppid':100},False)
        self.assertEqual(changed['after_read'],'changed');self.assertFalse(changed['ppid_matches'])
        unknown=h['graph_empty_identity_diagnostic'](None,known,known,None)
        self.assertEqual(unknown['initial_binding'],'unobserved');self.assertIsNone(unknown['uid_matches'])

    def test_graph_public_fields_refuse_raw_identity_unknown_keys_and_nonboolean_flags(self):
        h=harness_helpers();known={'observation':'observed','pid':123,'pgid':123,'ppid':99,'uid':1000,'start_ticks':'456'}
        original=h['graph_empty_identity_diagnostic'](known,known,known,True)
        for mutate in (lambda v:v.update(pid=123),lambda v:v.update(raw=SECRET),lambda v:v.update(uid_matches=1),
                       lambda v:v.update(after_read=SECRET),lambda v:v.update(exact_cancel_durable='true'),
                       lambda v:v.update(initial_binding='unobserved')):
            bad=copy.deepcopy(original);mutate(bad)
            with self.assertRaises(P.Refused):P.graph_identity_diagnostic(bad)

    def test_same_identity_projection_requires_explicit_true_even_when_all_field_matches_are_true(self):
        h=harness_helpers();known={'observation':'observed','pid':123,'pgid':123,'ppid':99,'uid':1000,'start_ticks':'456'}
        valid=h['graph_empty_identity_diagnostic'](known,known,known,True)
        self.assertEqual(P.graph_identity_diagnostic(valid),valid)
        for flag in (None,False):
            bad={**valid,'known_same_identity':flag}
            with self.subTest(flag=flag):
                with self.assertRaises(P.Refused):P.graph_identity_diagnostic(bad)
                value=P.unobserved_diagnostics();value['graph_provider_identity']=bad
                with self.assertRaises(P.Refused):P.finite_diagnostics(value)

    def test_exact_cancel_predicate_checks_request_source_mirrors_and_durable_receipt(self):
        predicate=harness_helpers()['exact_graph_cancel_rows'];original=cancel_inputs()
        self.assertTrue(predicate(*original))
        for index in (4,5,6):
            missing=copy.deepcopy(original);missing=list(missing);missing[index]=[]
            self.assertFalse(predicate(*missing))
        for mutate in (lambda v:v[3]['params'].update(goal_run_id='foreign'),lambda v:v[3].update(revision=True),
                       lambda v:v[3]['params'].update(expected_goal_run_revision=4),lambda v:v[3].update(key=None)):
            bad=copy.deepcopy(original);mutate(bad);self.assertFalse(predicate(*bad))
        for index,slot,mutate in ((4,6,lambda v:v['origin'].update(factory_scope_id='foreign')),
                                 (5,5,lambda v:v.update(cancel_requested=False)),
                                 (5,5,lambda v:v.update(revision=True)),
                                 (5,5,lambda v:v.update(state='accepted')),
                                 (6,1,lambda v:v['goal']['run'].update(task_id='foreign')),
                                 (6,1,lambda v:v['goal'].update(task_accepted=True))):
            bad=list(copy.deepcopy(original));rows=list(bad[index]);row=list(rows[0]);value=json.loads(row[slot]);mutate(value);row[slot]=json.dumps(value);rows[0]=tuple(row);bad[index]=rows
            self.assertFalse(predicate(*bad))

    def test_cancel_observation_never_replays_or_turns_absence_into_false_stop(self):
        h=harness_helpers();context={};call=types.SimpleNamespace(method='factory_child_goals.cancel',params={'scope_id':SECRET},revision=1,key=SECRET)
        h['remember_graph_cancel'](context,call);h['remember_graph_cancel'](context,call)
        self.assertNotIn('_graph_cancel_request_conflict',context)
        other=copy.deepcopy(call);other.revision=2;h['remember_graph_cancel'](context,other)
        self.assertTrue(context['_graph_cancel_request_conflict'])
        source=(ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_text();tree=ast.parse(source)
        observe=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='observe_exact_graph_cancel')
        text=ast.get_source_segment(source,observe)
        self.assertIn('if call is None:return None',text)
        for forbidden in ('host.wire','host.result','time.sleep','os.kill','os.waitpid'):
            self.assertNotIn(forbidden,text)
        from tests.infrastructure.test_graph_observation_handshake import strip_composed_outer_layer
        historical=strip_composed_outer_layer(source,'theirs')
        old_tree=ast.parse(historical)
        old_collector=next(n for n in old_tree.body if isinstance(n,ast.FunctionDef)and n.name=='collect_graph_records')
        self.assertIn("finite_projection.check_graph_provider_argv(argv,row['provider_argv'],old is not None)",ast.get_source_segment(historical,old_collector))
        # The actual collector delegates to Root's unchanged strong binding;
        # the finite bridge can annotate only its existing closed refusal.
        functions={n.name:n for n in tree.body if isinstance(n,ast.FunctionDef)}
        collector=functions['collect_graph_records']
        self.assertIn('bind_graph_observation(fixed, row[',ast.unparse(collector))
        finite=functions['graph_finite_observation'];binding=functions['bind_graph_observation']
        env={'Path':Path,'base':types.SimpleNamespace(write_private_json=lambda *args:None),
             'require':lambda value,*args:None if value else (_ for _ in ()).throw(AssertionError('original binding refused')),
             'graph_valid_identity':lambda value:True,'publish_graph_binding_ack':lambda *args:None}
        exec(compile(ast.Module(body=[finite,binding],type_ignores=[]),'<actual-closed-finite-binding-bridge>','exec'),env)
        fixed={'path':'synthetic-record','owned_provider':{'pid':2001}}
        for kind,reason in [('empty','empty'),('oversize','oversized'),('missing_nul','missing_nul'),('mismatch','argv_mismatch'),('invalid_utf8',None)]:
            # Empty identity metadata is tested above; this branch tests only
            # the original refusal→closed argv diagnostic→same assertion order.
            observation={'argv':{'kind':kind},'before':{},'after':{}}
            result={'kind':'not_verified','binding':None,'stop_proved':False}
            env['observe_graph_identity']=lambda *args:(copy.deepcopy(result),copy.deepcopy(observation))
            env['graph_empty_identity_diagnostic']=h['graph_empty_identity_diagnostic']
            recorder=P.FiniteRuntimeDiagnostics()
            with self.subTest(kind=kind),self.assertRaises(AssertionError):
                env['bind_graph_observation'](fixed,[],None,{},Path('/synthetic'),recorder)
            value=recorder.snapshot()['graph_provider_argv']
            self.assertEqual(value,None if reason is None else {'reason':reason,'previously_bound':False})
        for kind in ('initial_live','repeat_live','repeat_terminal'):
            recorder=P.FiniteRuntimeDiagnostics()
            env['graph_finite_observation'](recorder,None,{'kind':kind},{'argv':{'kind':'valid'}},None)
            self.assertIsNone(recorder.snapshot()['graph_provider_argv'])


if __name__=='__main__':unittest.main()
