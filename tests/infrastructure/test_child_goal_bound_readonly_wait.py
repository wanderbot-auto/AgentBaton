"""Closed readonly observation controls; no Core, browser or process execution."""
import ast
import copy
from collections import namedtuple
from pathlib import Path
import re
import types
import unittest

ROOT=Path(__file__).resolve().parents[2]
DRIVER=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
SUPPORT=ROOT/'apps/desktop/tests/goal-graph-real-ipc.py'


def load_functions(path,names,namespace):
    nodes=[n for n in ast.parse(path.read_text()).body if isinstance(n,(ast.FunctionDef,ast.ClassDef))and n.name in names]
    assert {n.name for n in nodes}==set(names)
    exec(compile(ast.Module(body=nodes,type_ignores=[]),str(path),'exec'),namespace)


class Clock:
    def __init__(self):self.now=0.0;self.sleeps=[]
    def monotonic(self):return self.now
    def sleep(self,seconds):self.sleeps.append(seconds);self.now+=seconds


def environment():
    clock=Clock();ns={'copy':copy,'time':clock,'planning':types.SimpleNamespace(ID=re.compile(r'[A-Za-z0-9._:-]{1,128}')),
        'FixedCall':namedtuple('FixedCall','method params key revision')}
    load_functions(SUPPORT,['BoundaryRefusal','require','exact_json_value'],ns)
    ns['support']=types.SimpleNamespace(exact_json_value=ns['exact_json_value'])
    load_functions(DRIVER,['identifier','bound_child_goal_query','exact_pending_child_goal_read','child_observation_deadline','wait_bound_child_goal'],ns)
    bounds=types.SimpleNamespace(scope_id='scope-fixed',tasks={'task-fixed'},goal_runs={'goal-fixed':'task-fixed'})
    query={'scope_id':'scope-fixed','task_id':'task-fixed','goal_run_id':'goal-fixed'}
    return clock,ns,bounds,query


def busy():
    return {'request_id':'generated-request','revision':19,'warnings':[],
        'error':{'code':'WORKSPACE_BUSY','message':'Fixed private diagnostic only','retryable':True,
                 'side_effect_state':'not_started','details_ref':None}}


def ready(accepted=True):return {'result':{'details':{'task_accepted':accepted}}}


class ScriptedHost:
    def __init__(self,clock,rows):self.clock=clock;self.rows=list(rows);self.calls=[]
    def wire(self,call):
        self.calls.append(copy.deepcopy(call))
        row=self.rows.pop(0)if len(self.rows)>1 else self.rows[0]
        if isinstance(row,BaseException):raise row
        if callable(row):row=row()
        return copy.deepcopy(row),b'original-test-reply'


class BoundChildGoalReadonlyWaitTests(unittest.TestCase):
    def invoke(self,ns,host,bounds,query,pump=lambda:None,capture=lambda:None,predicate=None):
        return ns['wait_bound_child_goal'](host,bounds,query,
            predicate or(lambda v:v['details']['task_accepted']is True),pump=pump,capture=capture)

    def test_strict_busy_then_real_shaped_result_uses_only_same_bound_read(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[busy(),ready()]);events=[]
        value=self.invoke(ns,host,bounds,query,pump=lambda:events.append('pump'),capture=lambda:events.append('capture'))
        self.assertTrue(value['details']['task_accepted']);self.assertEqual(events,['pump','capture']*2)
        self.assertEqual(clock.sleeps,[.02]);self.assertEqual(len(host.calls),2)
        for call in host.calls:
            self.assertEqual(call.method,'factory_child_goals.get');self.assertEqual(call.params,query)
            self.assertIsNone(call.key);self.assertIsNone(call.revision)

    def test_success_waits_for_original_predicate_instead_of_treating_busy_as_success(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[busy(),ready(False),ready(True)])
        self.assertTrue(self.invoke(ns,host,bounds,query)['details']['task_accepted']);self.assertEqual(len(host.calls),3)

    def test_only_well_typed_closed_busy_reply_is_pending(self):
        mutations=[lambda v:v['error'].update(code='PROVIDER_DISCONNECTED'),lambda v:v['error'].update(code='RESOURCE_LIMIT'),
            lambda v:v['error'].update(code='PERMISSION_DENIED'),lambda v:v['error'].update(retryable=False),
            lambda v:v['error'].update(retryable=1),lambda v:v['error'].update(retryable='true'),
            lambda v:v['error'].update(side_effect_state='unknown'),lambda v:v['error'].update(side_effect_state='confirmed'),
            lambda v:v['error'].update(extra='private'),lambda v:v['error'].pop('message'),
            lambda v:v['error'].update(message=None),lambda v:v['error'].update(details_ref={}),
            lambda v:v.update(result={'details':{'task_accepted':True}}),lambda v:v.update(revision=True),
            lambda v:v.update(revision=-1),lambda v:v.update(revision=2**64),lambda v:v.update(revision=1.0),
            lambda v:v.update(warnings=['unexpected']),lambda v:v.update(warnings=None),
            lambda v:v.update(request_id=''),lambda v:v.update(request_id=None),lambda v:v.update(extra='private')]
        for mutate in mutations:
            clock,ns,bounds,query=environment();reply=busy();mutate(reply);host=ScriptedHost(clock,[reply,ready()])
            with self.subTest(reply=reply),self.assertRaises(AssertionError):self.invoke(ns,host,bounds,query)
            self.assertEqual(len(host.calls),1);self.assertEqual(clock.sleeps,[])

    def test_error_null_empty_or_contradictory_response_does_not_fall_through(self):
        for reply in [{'error':None,'result':ready()['result']},{'error':{}},[],None]:
            clock,ns,bounds,query=environment();host=ScriptedHost(clock,[reply,ready()])
            with self.subTest(reply=reply),self.assertRaises((AssertionError,ns['BoundaryRefusal'])):
                self.invoke(ns,host,bounds,query)
            self.assertEqual(len(host.calls),1)

    def test_foreign_missing_extra_or_non_string_binding_never_dispatches(self):
        mutations=[lambda q:q.update(scope_id='foreign'),lambda q:q.update(task_id='foreign'),
            lambda q:q.update(goal_run_id='foreign'),lambda q:q.pop('goal_run_id'),lambda q:q.update(task_id=True),
            lambda q:q.update(expected_revision=1),lambda q:q.update(idempotency_key='write-key')]
        for mutate in mutations:
            clock,ns,bounds,query=environment();mutate(query);host=ScriptedHost(clock,[ready()])
            with self.subTest(query=query),self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query)
            self.assertEqual(host.calls,[])

    def test_unstarted_child_binding_never_dispatches(self):
        clock,ns,bounds,query=environment();bounds.goal_runs.clear();host=ScriptedHost(clock,[ready()])
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query)
        self.assertEqual(host.calls,[])

    def test_expired_before_first_observation_makes_no_dispatch(self):
        clock,ns,bounds,query=environment();ticks=iter([0,15]);clock.monotonic=lambda:next(ticks)
        host=ScriptedHost(clock,[ready()])
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query)
        self.assertEqual(host.calls,[])

    def test_pump_or_capture_cost_consumes_original_deadline_without_dispatch(self):
        for edge in ['pump','capture']:
            clock,ns,bounds,query=environment();host=ScriptedHost(clock,[ready()])
            callback=lambda:setattr(clock,'now',15)
            with self.subTest(edge=edge),self.assertRaises(ns['BoundaryRefusal']):
                self.invoke(ns,host,bounds,query,**{edge:callback})
            self.assertEqual(host.calls,[])

    def test_accepted_response_arriving_after_wire_cost_cannot_pass_late(self):
        clock,ns,bounds,query=environment()
        def delayed():clock.now=15;return ready()
        host=ScriptedHost(clock,[delayed])
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query)
        self.assertEqual(len(host.calls),1)

    def test_predicate_cost_cannot_pass_after_original_deadline(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[ready()])
        def late(_):clock.now=15;return True
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query,predicate=late)
        self.assertEqual(len(host.calls),1)

    def test_repeated_busy_exhausts_same_original_window_without_renewal(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[busy()])
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query)
        self.assertLessEqual(len(host.calls),751);self.assertGreater(len(host.calls),1)
        self.assertGreaterEqual(clock.now,15);self.assertLess(clock.now,15.03)
        self.assertEqual(set(clock.sleeps),{.02})

    def test_transport_failure_is_not_retried(self):
        clock,ns,bounds,query=environment();error=TimeoutError('fixed transport');host=ScriptedHost(clock,[error,ready()])
        with self.assertRaises(TimeoutError)as caught:self.invoke(ns,host,bounds,query)
        self.assertIs(caught.exception,error);self.assertEqual(len(host.calls),1)

    def test_binding_changed_by_existing_capture_is_refused_before_next_read(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[ready()])
        def change():query['goal_run_id']='foreign'
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query,capture=change)
        self.assertEqual(host.calls,[])

    def test_started_binding_replaced_during_pending_observation_is_not_reused(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[busy(),ready()]);count=0
        def capture():
            nonlocal count
            count+=1
            if count==2:bounds.goal_runs['goal-fixed']='foreign'
        with self.assertRaises(ns['BoundaryRefusal']):self.invoke(ns,host,bounds,query,capture=capture)
        self.assertEqual(len(host.calls),1)

    def test_scope_and_side_effect_operations_are_not_helper_arguments(self):
        clock,ns,bounds,query=environment();host=ScriptedHost(clock,[ready()])
        with self.assertRaises(TypeError):ns['wait_bound_child_goal'](host,bounds,query,lambda _:True,
            pump=lambda:None,capture=lambda:None,method='factory_child_goals.start')
        self.assertEqual(host.calls,[])


if __name__=='__main__':unittest.main()
