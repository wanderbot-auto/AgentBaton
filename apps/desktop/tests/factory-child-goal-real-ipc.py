#!/usr/bin/env python3
"""Schema15 real child-Goal browser/Unix IPC harness, not native/Provider proof.

The five Owner calls preserve the frozen camelCase/snake_case wire contract.
Default and --self-check start no listener, daemon, browser or subprocess.
Explicit --run requires Root runtime readiness and exclusive execution lease;
all successful business receipts then come from the supplied real Core15.
Private Owner simulation is test-only and never an actual OS confirmation.
"""
from __future__ import annotations

import argparse
from functools import partial
import http.server
import os
import signal
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import traceback
import uuid
import copy
from dataclasses import dataclass, field
import importlib.util
import json
from pathlib import Path
import re
import sys

PLANNING_PATH = Path(__file__).with_name('factory-child-planning-real-ipc.py')
_spec = importlib.util.spec_from_file_location('_child_goal_planning_support', PLANNING_PATH)
planning = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = planning
_spec.loader.exec_module(planning)
base, support = planning.base, planning.base.support
require, Refusal, FixedCall = planning.require, planning.Refusal, planning.FixedCall
# This private support instance only: execution scopes add exactly these fields.
base.REQUEST_FIELDS = base.REQUEST_FIELDS | {'factory_run_revision', 'child_goal_execution_binding'}

COMMANDS = frozenset({'factory_child_goals_start', 'factory_child_goals_get',
    'factory_child_goals_list', 'factory_child_goals_cancel', 'factory_child_goals_advance_plan'})
REF_FIELDS = frozenset({'id', 'scope_id', 'run_id', 'task_id', 'revision',
    'plan_digest', 'raw_sha256', 'receipt_sequence'})
MATRIX = {
    'normal': 'Explicit complete Native-fixture scope; real generated MCP7 saves exact raw plan; UI starts exact saved ref; real candidate/required validators/private Owner decisions/TaskAcceptance precede Factory task_accepted and all owned joins.',
    'permissions': 'Old false and planning-only4/5 tools retain omitted execution fields; no derived run. Deny forged origin, caller IDs, generic approvals, raised original Grant children/depth, foreign task/ref/scope and revoked/expired new actions.',
    'budget': 'Original shared Grant/project/root attempts/runtime/output/reworks/children counters never reset; explicit new execution policy only; project reworks<=1000 and attempts-1, root also planning max_revisions-1; unknown meters remain unknown.',
    'unknown': 'Drop one real start/advance reply after actual IPC. Persist key+exact payload/CAS; reconcile read-only original run/history; never fabricate completion/zero consumption or automatically dispatch again.',
    'cancel': 'Drop real cancel reply; retain request vs stop distinction. Scope/task cancel reaches child Graph jobs/sessions; require actual owned stop/join, lease and reservation evidence and no additional automatic cancel/dispatch.',
    'recovery': 'Same key returns original immutable receipt, changed payload conflicts. Expired/revoked history remains readable. Failed Graph adoption stays closed until exact server all-stopped proof fixture; only then same-run append-only adopted ref, immutable origin.initial_plan and original shared budgets.',
}


@dataclass
class Bounds:
    project_id: str
    root_task_id: str
    scope_id: str
    factory_run_id: str
    tasks: set[str] = field(default_factory=set)
    goal_runs: dict[str, str] = field(default_factory=dict)  # real Goal ID -> real task ID
    saved_refs: list[dict] = field(default_factory=list)  # populated ONLY by actual saved-plan reads


def positive(value):
    return planning.safe_integer(value, 1)


def identifier(value):
    return type(value) is str and planning.ID.fullmatch(value) is not None


def saved_reference(value, task_id, bounds):
    require(type(value) is dict and set(value) == REF_FIELDS)
    require(all(identifier(value[key]) for key in ['id', 'scope_id', 'run_id', 'task_id']))
    require(value['scope_id'] == bounds.scope_id and value['run_id'] == bounds.factory_run_id)
    require(value['task_id'] == task_id and task_id in bounds.tasks)
    require(positive(value['revision']) and value['revision'] <= 256 and positive(value['receipt_sequence']))
    require(all(type(value[key]) is str and re.fullmatch('[a-f0-9]{64}', value[key]) for key in ['plan_digest', 'raw_sha256']))
    require(any(support.exact_json_value(value, observed) for observed in bounds.saved_refs))
    return copy.deepcopy(value)


def submission(value, bounds):
    require(type(value) is dict and set(value) == {'task_id', 'plan', 'expected_factory_run_revision'})
    require(identifier(value['task_id']) and value['task_id'] in bounds.tasks)
    require(positive(value['expected_factory_run_revision']))
    return {'task_id': value['task_id'], 'plan': saved_reference(value['plan'], value['task_id'], bounds),
        'expected_factory_run_revision': value['expected_factory_run_revision']}


def decode_envelope(value, bounds):
    require(type(value) is dict and set(value) <= {'command', 'args'} and 'command' in value)
    command, args = value['command'], value.get('args')
    if args is None: args = {}
    require(type(command) is str and type(args) is dict)
    if command not in COMMANDS:
        return planning.decode_envelope(value, planning.Bounds(bounds.project_id, bounds.root_task_id,
            bounds.scope_id, bounds.tasks))
    required = {'scopeId'}
    if command in {'factory_child_goals_start', 'factory_child_goals_advance_plan'}:
        required |= {'request'}
    if command in {'factory_child_goals_get', 'factory_child_goals_list', 'factory_child_goals_cancel'}:
        required |= {'taskId'}
    if command in {'factory_child_goals_get', 'factory_child_goals_cancel', 'factory_child_goals_advance_plan'}:
        required |= {'goalRunId'}
    writes = command in {'factory_child_goals_start', 'factory_child_goals_cancel', 'factory_child_goals_advance_plan'}
    if writes: required |= {'expectedRevision', 'idempotencyKey'}
    if command in {'factory_child_goals_cancel', 'factory_child_goals_advance_plan'}:
        required |= {'expectedGoalRunRevision'}
    require(set(args) == required)
    require(type(args['scopeId']) is str and bool(bounds.scope_id) and args['scopeId'] == bounds.scope_id)
    params = {'scope_id': bounds.scope_id}
    if 'taskId' in args:
        require(identifier(args['taskId']) and args['taskId'] in bounds.tasks)
        params['task_id'] = args['taskId']
    if 'request' in args:
        params['request'] = submission(args['request'], bounds)
    if 'goalRunId' in args:
        run_id = args['goalRunId']
        target = args.get('taskId', params.get('request', {}).get('task_id'))
        require((run_id is None and command == 'factory_child_goals_get') or
            (identifier(run_id) and bounds.goal_runs.get(run_id) == target))
        params['goal_run_id'] = run_id
    if command == 'factory_child_goals_list': params['goal_run_id'] = None
    if 'expectedGoalRunRevision' in args:
        require(positive(args['expectedGoalRunRevision']))
        params['expected_goal_run_revision'] = args['expectedGoalRunRevision']
    method = 'factory_child_goals.' + command.removeprefix('factory_child_goals_')
    if writes:
        require(positive(args['expectedRevision']))
        require(type(args['idempotencyKey']) is str and planning.KEY.fullmatch(args['idempotencyKey']))
        return FixedCall(method, params, args['idempotencyKey'], args['expectedRevision'])
    return FixedCall(method, params)


def decode_http(raw, *, origin, host, content_type, expected_origin, bounds):
    require(type(raw) is bytes and 0 < len(raw) <= planning.MAX_WIRE)
    require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}', expected_origin) is not None)
    require(1 <= int(expected_origin.rsplit(':', 1)[1]) <= 65535)
    require(origin == expected_origin and host == expected_origin.removeprefix('http://'))
    require(content_type == 'application/json')
    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=planning.unique_object,
            parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
    except (ValueError, UnicodeError) as error:
        raise Refusal(400, 'Complete bounded unique-key UTF8 JSON required') from error
    return decode_envelope(value, bounds)


def self_check():
    """Synthetic boundary controls only; never invent Core/Owner receipts."""
    checks = []
    ref = {'id': 'saved-one', 'scope_id': 'scope', 'run_id': 'factory-run', 'task_id': 'child',
        'revision': 1, 'plan_digest': 'a' * 64, 'raw_sha256': 'b' * 64, 'receipt_sequence': 7}
    bounds = Bounds('project', 'root', 'scope', 'factory-run', {'root', 'child'}, {'graph': 'child'}, [ref])
    request = {'task_id': 'child', 'plan': copy.deepcopy(ref), 'expected_factory_run_revision': 4}
    calls = {
        'start': {'scopeId': 'scope', 'request': request, 'expectedRevision': 1, 'idempotencyKey': 'start-original'},
        'get': {'scopeId': 'scope', 'taskId': 'child', 'goalRunId': 'graph'},
        'list': {'scopeId': 'scope', 'taskId': 'child'},
        'cancel': {'scopeId': 'scope', 'taskId': 'child', 'goalRunId': 'graph', 'expectedGoalRunRevision': 2, 'expectedRevision': 1, 'idempotencyKey': 'cancel-original'},
        'advance_plan': {'scopeId': 'scope', 'goalRunId': 'graph', 'request': request, 'expectedGoalRunRevision': 2, 'expectedRevision': 1, 'idempotencyKey': 'advance-original'},
    }
    def check(name, value, accepted):
        try: actual = decode_envelope(value, bounds)
        except Refusal: assert not accepted, name
        else:
            assert accepted, name
            assert actual.method in base.READS | base.MUTATIONS | {'factory_child_plans.context',
                'factory_child_plans.get', 'factory_child_plans.list', 'factory_child_plans.submit'} | {
                'factory_child_goals.' + suffix for suffix in calls}
        checks.append(name)
    for suffix, args in calls.items():
        command = 'factory_child_goals_' + suffix
        value = {'command': command, 'args': copy.deepcopy(args)}
        check('exact:' + suffix, value, True)
        actual = decode_envelope(value, bounds)
        assert actual.method == 'factory_child_goals.' + suffix
        assert actual.params['scope_id'] == 'scope'
        assert 'expectedRevision' not in actual.params and 'idempotencyKey' not in actual.params
        for key, bad in [('scopeId', 'foreign'), ('approved', True), ('binding', {}), ('token', 'forged'), ('method', 'approvals.finalize')]:
            check(suffix + ':reject:' + key, {'command': command, 'args': {**args, key: bad}}, False)
        for key in args:
            missing = copy.deepcopy(args); del missing[key]
            check(suffix + ':missing:' + key, {'command': command, 'args': missing}, False)
        for key in ['expectedRevision', 'expectedGoalRunRevision']:
            if key in args:
                for bad in [0, -1, True, 1.0, '1', None, 9_007_199_254_740_992]:
                    check(suffix + ':' + key + ':' + repr(bad), {'command': command, 'args': {**args, key: bad}}, False)
    get = {'command':'factory_child_goals_get','args':{'scopeId':'scope','taskId':'child','goalRunId':None}}
    check('latest-get-explicit-null', get, True)
    for suffix in ['get','cancel','advance_plan']:
        args = calls[suffix]
        check(suffix + ':foreign-goal', {'command':'factory_child_goals_' + suffix,'args':{**args,'goalRunId':'foreign'}}, False)
    start = {'command':'factory_child_goals_start','args':copy.deepcopy(calls['start'])}
    for key, bad in [('task_id','root'),('plan',None),('expected_factory_run_revision',True),('approved',True)]:
        value=copy.deepcopy(start);value['args']['request'][key]=bad
        check('request-reject:' + key,value,False)
    for key, bad in [('id','other'),('scope_id','other'),('run_id','other'),('task_id','root'),('revision',True),
        ('revision',257),('receipt_sequence',0),('plan_digest','A'*64),('raw_sha256','c'*64),('plan_id','forged')]:
        value=copy.deepcopy(start);value['args']['request']['plan'][key]=bad
        check('saved-ref-reject:' + key + ':' + repr(bad),value,False)
    # Exact observed raw ref and original key/CAS survive mapping with no normalization.
    actual=decode_envelope(start,bounds)
    assert support.exact_json_value(actual.params['request'],request)
    assert actual.key == 'start-original' and actual.revision == 1
    checks.append('exact-original-request-CAS-key-preserved')
    for command in ['approvals_finalize','factory_scope_approve','goal_node_approve','goal_node_reject',
        'factory_child_goals_propose','execute_child_goal','advance_child_goal','generic_ipc','sessions_create']:
        check('forbidden:' + command,{'command':command,'args':{}},False)
    origin='http://127.0.0.1:42001';raw=json.dumps(start).encode()
    for name, changes in [('foreign-origin',{'origin':'http://evil.invalid'}),('foreign-host',{'host':'localhost:42001'}),
        ('wrong-content',{'content_type':'text/plain'}),('oversize',{'raw':b'x'*(planning.MAX_WIRE+1)}),
        ('duplicate',{'raw':b'{"command":"system_snapshot","command":"approvals_finalize"}'}),
        ('nan',{'raw':b'{"command":"system_snapshot","args":{"x":NaN}}'}),('utf8',{'raw':b'\xff'})]:
        kwargs=dict(raw=raw,origin=origin,host='127.0.0.1:42001',content_type='application/json',expected_origin=origin,bounds=bounds)
        kwargs.update(changes)
        try: decode_http(**kwargs)
        except Refusal: pass
        else: raise AssertionError(name)
        checks.append('http-reject:' + name)
    class ManifestBinary:
        reads = 0
        def read_bytes(self):
            self.reads += 1
            return b'pure manifest-control bytes, not executable'
    binary = ManifestBinary()
    manifest = {'schema':15,'binaries':{'batond':base.sha(binary.read_bytes())},'source_head':'a'*40}
    before = binary.reads
    assert support.verify_candidate(binary,json.dumps(manifest).encode()) == manifest
    assert binary.reads == before + 1
    checks.append('integer15-manifest-positive-pure-bytes-only')
    for bad in [12,13,14,16,True,15.0,'15',None]:
        before=binary.reads
        try: support.verify_candidate(binary,json.dumps({**manifest,'schema':bad}).encode())
        except AssertionError: pass
        else: raise AssertionError(bad)
        assert binary.reads == before
        checks.append('manifest-reject-before-binary-read:' + repr(bad))
    # Fake query return controls only, not a SQLite schema or migration surrogate.
    class FakeDB:
        def __init__(self,schema=15,values=None): self.schema,self.values=schema,values or {}
        def execute(self,query):
            if query=='PRAGMA user_version': value=self.schema
            else:
                table=query.split('"')[1]
                if table in self.values:
                    value=self.values[table]
                    if isinstance(value,BaseException): raise value
                else: value=0
            class Row:
                def fetchone(self): return (value,)
            return Row()
    counts=support.current_factory_counts(FakeDB())
    assert set(counts)==set(support.FACTORY_RECORD_TABLES) and len(counts)==12 and all(v==0 for v in counts.values())
    checks.append('all12-exact-current-count-fields')
    # A nonzero child Goal remains visible, never silently rewritten to zero.
    assert support.current_factory_counts(FakeDB(values={'factory_child_goal_runs':1}))['child_goal_runs']==1
    checks.append('nonzero-child-goal-preserved')
    for table in support.FACTORY_RECORD_TABLES.values():
        for bad in [None,True,0.0,-1,RuntimeError('missing table or query failure')]:
            try: support.current_factory_counts(FakeDB(values={table:bad}))
            except (AssertionError,RuntimeError): pass
            else: raise AssertionError((table,bad))
            checks.append('count-reject:' + table + ':' + repr(bad))
    for schema in [12,13,14,16,15.0,True,'15',None]:
        try: support.current_factory_counts(FakeDB(schema=schema))
        except AssertionError: pass
        else: raise AssertionError(schema)
        checks.append('db-schema-reject:' + repr(schema))
    # Return/seal controls exercise no worker and never stand for a stop receipt.
    seed={'status':'passed','http_unexpected_errors':[],'cleanup_complete':False}
    passed_report=copy.deepcopy(seed)
    assert planning.seal_runtime_report(passed_report,True,True,True)==0
    checks.append('pure-exit-all-three-confirmed')
    for index in range(3):
        for unknown in [False,None]:
            flags=[True,True,True];flags[index]=unknown;candidate=copy.deepcopy(seed)
            assert planning.seal_runtime_report(candidate,*flags)==1 and candidate['cleanup_complete'] is False
            checks.append('pure-exit-refuses-cleanup-'+str(index)+':'+repr(unknown))
    late=copy.deepcopy(seed);late['http_unexpected_errors']=['synthetic late handler failure']
    assert planning.seal_runtime_report(late,True,True,True)==1 and late['failure']['stage']=='http_final_error_check'
    checks.append('pure-exit-late-error-retained')
    primary=copy.deepcopy(seed);base.note_failure(primary,AssertionError('synthetic original failure'),'first')
    base.note_failure(primary,AssertionError('synthetic later failure'),'cleanup')
    assert planning.seal_runtime_report(primary,True,True,True)==1 and primary['failure']['stage']=='first'
    assert primary['secondary_failures'][0]['stage']=='cleanup'
    checks.append('pure-exit-original-cause-preserved')
    gates={key:True for key in ['daemon_complete','factory_identities_complete','http_complete','graph_complete',
        'raw_database_retained','session_coverage_confirmed','business_passed']}
    assert fixture_cleanup_permitted(**gates)
    checks.append('private-directory-cleanup-only-after-all-evidence-positive')
    for key in gates:
        for bad in [False,None,1]:
            assert not fixture_cleanup_permitted(**{**gates,key:bad})
            checks.append('retain-private-directory:'+key+':'+repr(bad))
    snapshot={'schema':15,'factory_counts':{key:2 if key=='scopes'else 0 for key in support.FACTORY_RECORD_TABLES},
        'tables':{table:{'count':PREAPPROVAL_COUNTS.get(table,0),'sha256':'a'*64}for table in PREAPPROVAL_ZERO_TABLES|PREAPPROVAL_COUNTS.keys()|{'events'}},
        'sequence':1,'active_workspace_leases':0,'active_dispatch_intents':0}
    verify_approval_did_not_execute(snapshot);checks.append('pure-approval-zero-execution-domain-positive')
    for table in sorted(PREAPPROVAL_ZERO_TABLES):
        bad=copy.deepcopy(snapshot);bad['tables'][table]['count']=1
        try:verify_approval_did_not_execute(bad)
        except Refusal:pass
        else:raise AssertionError(table)
        checks.append('pure-approval-refuses-auto-execution:'+table)
    for key in ['active_workspace_leases','active_dispatch_intents']:
        bad=copy.deepcopy(snapshot);bad[key]=1
        try:verify_approval_did_not_execute(bad)
        except Refusal:pass
        else:raise AssertionError(key)
        checks.append('pure-approval-refuses:'+key)
    for bad_value in [None,True,0.0]:
        bad=copy.deepcopy(snapshot);bad['tables']['sessions']['count']=bad_value
        try:verify_approval_did_not_execute(bad)
        except Refusal:pass
        else:raise AssertionError(bad_value)
        checks.append('pure-approval-refuses-unknown-count:'+repr(bad_value))
    missing=copy.deepcopy(snapshot);del missing['tables']['attempts']
    try:verify_approval_did_not_execute(missing)
    except Refusal:pass
    else:raise AssertionError('missing execution table')
    checks.append('pure-approval-requires-complete-domain')
    run={'active_session_id':None,'active_task_id':None,'cancel_requested':False,'context_continuation':'new_attempt_explicit_handoff_only',
        'created_at':'synthetic','updated_at':'synthetic','id':'factory-run','last_error':None,'project_id':'project','revision':1,'root_task_id':'root','scope_id':'scope','state':'queued'}
    entry={'method':'factory_runs.start','params':{'scope_id':'scope'},'revision':1,'key':'test-original-key','request_body_sha256':'a'*64,
        'response_complete':True,'original_core_reply_sha256':'b'*64,'response':{'request_id':'synthetic','result':run,'revision':1,'warnings':[]}}
    assert base.confirmed_start_response([entry],{'id':'scope','revision':1},'project','root')==entry
    checks.append('pure-explicit-Factory-start-receipt-positive')
    cases=[]
    denied=copy.deepcopy(entry);denied['response']={'request_id':'synthetic','error':{'code':'CONFIG_CONFLICT'},'revision':1,'warnings':[]};cases.append(('auto-start-conflict',[denied]))
    foreign=copy.deepcopy(entry);foreign['response']['result']['scope_id']='foreign';cases.append(('foreign-scope',[foreign]))
    wrong_state=copy.deepcopy(entry);wrong_state['response']['result']['state']='running';cases.append(('not-original-queued-receipt',[wrong_state]))
    cases.append(('duplicate-start',[entry,entry]))
    for name,entries in cases:
        try:base.confirmed_start_response(entries,{'id':'scope','revision':1},'project','root')
        except AssertionError:pass
        else:raise AssertionError(name)
        checks.append('pure-explicit-Factory-start-reject:'+name)
    incomplete=copy.deepcopy(entry);incomplete['response_complete']=False
    assert base.confirmed_start_response([incomplete],{'id':'scope','revision':1},'project','root')is None
    checks.append('pure-incomplete-Factory-start-never-confirmed')
    program=generated_acp();compile(program,'<generated-child-goal-ACP-pure>','exec')
    assert "'submit_child_plan','execute_child_goal','advance_child_goal'" in program
    assert "time.monotonic()+15" in program and "mcp_servers':[]" in program
    checks.extend(['generated-MCP7-Graph-no-MCP-source-compiled','original-bounded15s-parent-gate-retained'])
    return {'status':'passed','count':len(checks),'checks':checks,
        'dimension':'pure Python fixed-boundary and fake-query-return controls; NO Core/SQLite migration/browser/OS approval/process execution'}


# Kept separate from all old4/5-tool generated programs. Every replacement is
# asserted exactly once; no runtime source rewriting of another harness occurs.
EXEC_PLAN_HELPER = planning.PLAN_HELPER.replace(
    "'side_effect':'advisory_read_only','write_set':[]",
    "'side_effect':'workspace_write','write_set':[{'kind':'workspace','path':p}for p in ctx['edit_context']['writable_prefixes']]")


def generated_acp():
    text = base.GENERATED_ACP
    graph_new = r'''   if not servers:
    os.makedirs('reports',exist_ok=True)
    fd=os.open('reports/fixture-stderr-'+str(os.getpid())+'.txt',os.O_WRONLY|os.O_CREAT|os.O_APPEND|os.O_NOFOLLOW,0o600)
    sys.stderr=os.fdopen(fd,'a',buffering=1)
    native_session_id='generated-child-goal-'+str(os.getpid())+'-'+settings['nonce']
    response(request,{'sessionId':native_session_id});continue
'''
    graph_prompt = r'''   if mcp is None:
    assert c['kind']=='agentbaton_goal_task_request' and c['schema_version']==1
    assert set(c)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location','validation_contracts','task_binding'}
    assert c['node']['id']=='fixed-work' and c['node']['kind']=='work'
    assert c['workspace']['canonical_root']==os.path.realpath('.')
    assert c['task_binding']['attempt_id']==c['lineage']['attempt_id']
    assert type(c['task_binding']['task_id'])is str and c['task_binding']['task_id']
    assert c['return_location']['run_id']==c['run_id']
    assert c['outputs'][0]['artifact']['artifact_id']=='report-child'
    assert c['outputs'][0]['artifact']['relative_path']=='reports/report-child.txt'
    assert len(c['outputs'])==1
    proof={'request_text':raw,'request_sha256':hashlib.sha256(raw.encode()).hexdigest(),
     'owned_provider':identity(os.getpid()),'fixture_nonce':settings['nonce'],
     'native_session_id':native_session_id,'program_sha256':file_hash(__file__),
     'settings_sha256':file_hash(settings_path),'provider_argv':sys.argv,'mcp_servers':[]}
    record='reports/graph-request-'+str(os.getpid())+'.json'
    fd=os.open(record+'.tmp',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w')as f:json.dump(proof,f)
    os.replace(record+'.tmp',record)
    output=c['outputs'][0]['artifact']['relative_path']
    fd=os.open(output,os.O_WRONLY|os.O_CREAT|os.O_TRUNC|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb')as f:f.write(b'fixed child Goal report\n')
    if settings['case']=='cancel':continue
    response(request,{'stopReason':'end_turn'});continue
'''
    parent = r'''   if stage=='parent-initial':
    import copy,time
    child=tool('delegate_task',{'goal':'Produce and accept only the exact generated child report',
     'connection_id':settings['connection_id'],'workspace_id':settings['workspace_id'],
     'input_artifact_ids':['input'],'output_artifact_ids':['report-child'],
     'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':1,'reworks':0,'tokens':None,'cost_microunits':None},
     'wait_for_child':True,'idempotency_key':'child-goal-delegate-once'})
    assert child['state']=='blocked_on_live_parent' and child['parent_task_id']==c['task_id']
    status=tool('get_task_status',{'task_id':child['task_id']});ctx=status['child_planning_context']
    assert ctx['goal_execution_authorized']is False and ctx['new_workspace_authorized']is False
    assert status['child_goal_execution_binding']['authority_kind']=='factory_child_goal_execution_v1'
    p=child_plan(ctx,1);work=p['nodes'][0];work['budget']['children']=0
    owner=copy.deepcopy(work);owner.update({'id':'owner-report','kind':'owner_approval','goal':'Review the exact child Goal candidate',
     'dependencies':[work['id']],'executor':{'kind':'owner','rule_id':ctx['required_acceptance_rules'][0]},'outputs':[],
     'required_inputs':[{'goal_task_id':ctx['goal']['task_id'],'artifact_id':'report-child','contract_id':ctx['outputs']['report-child'],
      'source':{'kind':'node','node_id':work['id'],'candidate_for_validation':True}}],
     'required_validators':ctx['required_acceptance_rules'],'side_effect':'advisory_read_only','write_set':[],'idempotency_key':'owner-child-report-once'})
    p['nodes'].append(owner);original=json.dumps(p,ensure_ascii=False,indent=2)+'\n'
    saved=tool('submit_child_plan',{'task_id':child['task_id'],'plan_json':original,'expected_plan_revision':0,'idempotency_key':'child-goal-plan-one'})
    assert saved['plan_json']==original and saved['reference']['raw_sha256']==hashlib.sha256(original.encode()).hexdigest()
    assert saved['planning_only']is True and saved['dispatched']is saved['task_accepted']is False
    ready={'child':child,'context':ctx,'saved':saved,'fresh_status':tool('get_task_status',{'task_id':child['task_id']})}
    fd=os.open('reports/child-plan-ready.json.tmp',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w')as f:json.dump(ready,f)
    os.replace('reports/child-plan-ready.json.tmp','reports/child-plan-ready.json')
    until=time.monotonic()+15
    while not os.path.exists('reports/release-parent'):
     assert time.monotonic()<until,'Exact UI start did not commit within the bounded parent gate'
     time.sleep(.005)
    close_mcp();response(request,{'stopReason':'end_turn'});continue
'''
    # This is a separate test program and never receives a native Provider login.
    replacements = [
        ('mcp=None;mid=0', EXEC_PLAN_HELPER + '\nmcp=None;mid=0'),
        ("servers=request['params']['mcpServers'];assert len(servers)==1", "servers=request['params']['mcpServers']\n"+graph_new+"   assert len(servers)==1"),
        ("['delegate_task','get_task_status','get_task_result','cancel_task']", "['delegate_task','get_task_status','get_task_result','cancel_task','submit_child_plan','execute_child_goal','advance_child_goal']"),
        ("   assert set(c)==", graph_prompt+"   assert set(c)=="),
        ("'context_continuation','acceptance_state'}", "'context_continuation','acceptance_state','child_planning_context','factory_run_revision','child_goal_execution_binding'}"),
        ("\n   if settings['case']=='cancel':", "\n   if settings['case']=='parent-cancel-unused':"),
        ("   if stage=='parent-initial':", parent+"   if stage=='parent-initial':"),
        ("result['acceptance_state']=='not_established'", "result['acceptance_state']=='task_accepted'"),
        ("    artifact_id='report-child' if stage=='child' else 'report-parent'", "    assert stage=='parent-resumed'\n    assert any(v.startswith('goal-task-acceptance:')for v in result['validation_refs'])\n    artifact_id='report-parent'"),
    ]
    for old,new in replacements:
        assert text.count(old)==1, ('Inherited fixed generated source changed',old)
        text=text.replace(old,new)
    compile(text,'<generated-child-goal-seven-tool-ACP>','exec')
    return text


def review_execution_scope(proposal, selection, resources):
    """Complete private fixture review; no renderer/fake Owner authorization."""
    require(proposal['kind']=='factory' and support.exact_json_value(proposal['snapshot']['selection'],selection))
    bindings=proposal['snapshot']['bindings'];execution=bindings['child_goal_execution'];planning_binding=bindings['child_planning']
    require(set(execution)=={'schema_version','authority_kind','policy','planning_policy_digest','root_grant_id',
        'root_grant_revision','original_max_children','original_max_depth','graph_contract_version','definition_digest','new_workspace_authorized'})
    require(type(execution['schema_version'])is int and execution['schema_version']==1 and type(execution['graph_contract_version'])is int and execution['graph_contract_version']==1)
    require(all(planning.safe_integer(execution[k],1)for k in ['root_grant_revision','original_max_children','original_max_depth']))
    require(execution['authority_kind']=='factory_child_goal_execution_v1' and execution['new_workspace_authorized'] is False)
    require(support.exact_json_value(execution['policy'],selection['child_goal_execution_policy']))
    grant=resources['grant'];require(execution['root_grant_id']==grant['id'] and execution['root_grant_revision']==grant['revision'])
    require(execution['original_max_children']==grant['scope']['max_children']==3 and execution['original_max_depth']==grant['scope']['max_depth']==1)
    require(execution['planning_policy_digest']==planning_binding['definition_digest'])
    require(planning_binding['goal_execution_authorized'] is False and planning_binding['new_workspace_authorized'] is False)
    require(support.exact_json_value(planning_binding['policy'],selection['child_planning_policy']) and planning_binding['validator_definitions']=={})
    require(all(re.fullmatch('[a-f0-9]{64}',execution[k]) for k in ['definition_digest','planning_policy_digest']))
    old=copy.deepcopy(proposal);del old['snapshot']['bindings']['child_goal_execution'];del old['snapshot']['bindings']['child_planning']
    base.review_factory(old,selection,resources['project'],resources['task'],grant,resources['connection'],resources['workspace'],resources['companion'])
    raw=json.dumps(proposal,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode()
    require(len(raw)<=planning.MAX_WIRE)
    return {'authority':'private generated host fixture; NOT real OS Owner dialog','proposal':proposal,
        'complete_utf8_bytes':len(raw),'complete_sha256':base.sha(raw)}


def database_snapshot(runtime):
    """Actual readonly SQL observations, never synthesized Store receipts."""
    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as db:
        db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
        counts=support.current_factory_counts(db)
        tables=set(support.FACTORY_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|{'events','attempts','sessions','dispatch_intents','workspace_leases','budgets','execution_grants'}
        result={};total=0
        for table in sorted(tables):
            rows=db.execute('SELECT * FROM "'+table+'" ORDER BY rowid').fetchall();assert len(rows)<=2048
            raw=json.dumps(rows,separators=(',',':'),allow_nan=False).encode();total+=len(raw);assert total<=16*1024*1024
            result[table]={'count':len(rows),'sha256':base.sha(raw)}
        sequence=db.execute('SELECT COALESCE(max(sequence),0)FROM events').fetchone()[0]
        active_leases=db.execute("SELECT count(*)FROM workspace_leases WHERE holder!=''").fetchone()[0]
        active_intents=db.execute("SELECT count(*)FROM dispatch_intents WHERE state!='stopped'").fetchone()[0];db.rollback()
    return {'schema':15,'factory_counts':counts,'tables':result,'sequence':sequence,'active_workspace_leases':active_leases,'active_dispatch_intents':active_intents}


def collect_graph_records(runtime,workspace,scope,context,verified,output):
    """Bind saved generated identities to actual Core text/session/node/attempt."""
    directory=Path(workspace['root'])/'reports'
    if not directory.exists():return
    paths=sorted(directory.glob('graph-request-*.json'));assert len(paths)<=2
    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as db:
        db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
        sessions={json.loads(r)['native_session_id']:json.loads(r) for r, in db.execute('SELECT document FROM sessions')if json.loads(r).get('native_session_id')}
        for path in paths:
            raw,pin=base.bounded_file(path,base.MAX_HTTP_BYTES,directory);row=json.loads(raw,object_pairs_hook=planning.unique_object)
            assert pin['mode']==0o600 and row['fixture_nonce']==context['nonce'] and row['mcp_servers']==[]
            assert row['program_sha256']==context['program_sha256'] and row['settings_sha256']==context['settings_sha256']
            assert row['provider_argv']==[context['provider'],'--fixture-settings',context['settings']]
            request=json.loads(row['request_text'],object_pairs_hook=planning.unique_object)
            assert set(request)==support.GRAPH_TASK_FIELDS and row['request_sha256']==base.sha(row['request_text'].encode())
            session=sessions[row['native_session_id']];assert session['task_id']==request['task_binding']['task_id']
            assert session['attempt_id']==request['lineage']['attempt_id'] and session['workspace_id']==workspace['id']
            assert request['lineage']['root_task_id']==scope['root_task_id']
            assert session['grant_id']==scope['root_grant_id'] and session['connection_id']==context['connection_id']
            texts=[json.loads(r)['text']for r, in db.execute('SELECT document FROM session_turns WHERE session_id=?',[session['id']])]
            assert texts.count(row['request_text'])==1
            node=json.loads(db.execute('SELECT document FROM goal_run_nodes WHERE run_id=? AND node_id=? AND plan_revision=?',
                [request['run_id'],request['node']['id'],request['plan_revision']]).fetchone()[0])
            assert node['session_id']==session['id'] and node['attempt_id']==session['attempt_id']
            source=db.execute('SELECT factory_scope_id,task_id FROM factory_child_goal_runs WHERE goal_run_id=?',[request['run_id']]).fetchone()
            assert source==(scope['id'],session['task_id'])
            identity=row['owned_provider'];assert type(identity['pid'])is int and identity['pid']>1 and identity['pgid']==identity['pid']
            assert type(identity['start_ticks'])is str and re.fullmatch('[0-9]+',identity['start_ticks'])
            assert path.name=='graph-request-'+str(identity['pid'])+'.json'
            assert row['native_session_id']=='generated-child-goal-'+str(identity['pid'])+'-'+context['nonce']
            fixed={'path':str(path),'sha256':base.sha(raw),'file_pin':pin,'owned_provider':identity,
                'session_id':session['id'],'attempt_id':session['attempt_id'],'run_id':request['run_id'],'request':request}
            old=next((v for v in verified if v['path']==fixed['path']),None);assert old is None or old==fixed
            try:current=support.proc_identity(identity['pid'])
            except FileNotFoundError:current=None
            if current is not None and current['start_ticks']==identity['start_ticks']:
                argv=Path('/proc',str(identity['pid']),'cmdline').read_bytes();assert 0<len(argv)<=8192 and argv.endswith(b'\0')
                assert [v.decode()for v in argv[:-1].split(b'\0')][-3:]==row['provider_argv']
                assert os.getpgid(identity['pid'])==identity['pgid']
            if old is None:
                base.write_private_json(output/('verified-graph-'+str(identity['pid'])+'.json'),fixed);verified.append(fixed)
        db.rollback()


PREAPPROVAL_COUNTS = {'factory_scopes':2,'goal_scopes':1,'goal_grant_revisions':2,
    'goal_ledgers':2,'execution_grants':1,'workspace_leases':1,'budgets':2}
PREAPPROVAL_ZERO_TABLES = (frozenset(support.FACTORY_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|
    {'attempts','sessions','dispatch_intents'}) - PREAPPROVAL_COUNTS.keys()


def verify_approval_did_not_execute(snapshot):
    # These are actual SQL observations at runtime. Unit dictionaries below
    # exercise refusal only; no count becomes a synthetic Core receipt.
    require(type(snapshot)is dict and set(snapshot)=={'schema','factory_counts','tables','sequence','active_workspace_leases','active_dispatch_intents'})
    require(type(snapshot['schema'])is int and snapshot['schema']==15)
    require(type(snapshot['factory_counts'])is dict and set(snapshot['factory_counts'])==set(support.FACTORY_RECORD_TABLES))
    require(type(snapshot['tables'])is dict and set(snapshot['tables'])==PREAPPROVAL_ZERO_TABLES|PREAPPROVAL_COUNTS.keys()|{'events'})
    require(planning.safe_integer(snapshot['sequence']))
    for key in ['active_workspace_leases','active_dispatch_intents']:
        require(type(snapshot[key])is int and snapshot[key]==0)
    for key,table in support.FACTORY_RECORD_TABLES.items():
        value=snapshot['factory_counts'][key]
        require(type(value)is int and value==(2 if key=='scopes'else 0))
        require(snapshot['tables'][table]['count']==value)
    for table,entry in snapshot['tables'].items():
        require(type(entry)is dict and set(entry)=={'count','sha256'})
        require(type(entry['count'])is int and entry['count']>=0)
        require(type(entry['sha256'])is str and re.fullmatch('[a-f0-9]{64}',entry['sha256']))
        if table!='events':require(entry['count']==PREAPPROVAL_COUNTS.get(table,0))


def fixture_cleanup_permitted(*,daemon_complete,factory_identities_complete,http_complete,
        graph_complete,raw_database_retained,session_coverage_confirmed,business_passed):
    # Only affirmative evidence permits removal. None/unknown/bool aliases do
    # not stand for a join. Failure evidence is retained even after actual stop.
    return all(value is True for value in (daemon_complete,factory_identities_complete,http_complete,
        graph_complete,raw_database_retained,session_coverage_confirmed,business_passed))


def runtime_driver(args, output):
    """Real built UI / authenticated Core15 / generated processes, never a mock.

    Each --case owns fresh resources. Private Owner simulation is outside HTTP
    and checks the complete exact server proposal; no real OS consent is claimed.
    """
    from playwright.sync_api import expect, sync_playwright
    raw_daemon=(args.tools/'batond').resolve(strict=True);raw_cli=(args.tools/'baton').resolve(strict=True)
    frontend=args.frontend.resolve(strict=True);manifest_raw=args.manifest.resolve(strict=True).read_bytes()
    manifest=json.loads(manifest_raw,object_pairs_hook=planning.unique_object)
    require(type(manifest['schema'])is int and manifest['schema']==15)
    require(all(re.fullmatch('[a-f0-9]{40}',manifest[k])for k in ['source_head','compiled_core_source_head']))
    require(manifest['binaries']['batond']==base.sha(raw_daemon.read_bytes()) and manifest['binaries']['baton']==base.sha(raw_cli.read_bytes()))
    files=list(frontend.rglob('*'));require(len(files)<=4096 and not any(p.is_symlink()for p in files))
    require(sum(p.stat().st_size for p in files if p.is_file())<=64*1024*1024 and (frontend/'index.html').is_file())
    assets={str(p.relative_to(frontend)):base.sha(p.read_bytes())for p in files if p.is_file()}
    require(assets==manifest['frontend_assets'])
    capacity=os.statvfs(output);free=capacity.f_bavail*capacity.f_frsize
    require(free>=200*1024*1024+raw_daemon.stat().st_size+raw_cli.stat().st_size+32*1024*1024,'Keep200MiB guard plus exact private tools and bounded evidence allowance')
    source=Path(__file__).read_bytes();old_support={str(p):base.sha(p.read_bytes())for p in [PLANNING_PATH,planning.BASE_PATH,base.SUPPORT_PATH]}
    report={'status':'failed_or_interrupted','dimension':'actual built React/Linux Chromium/fixed test-only adapter/authenticated Unix Core15/generated account-free ACP/MCP7',
        'case':args.case,'capacity_preflight':{'free_bytes':free,'guard_bytes':200*1024*1024,'private_tool_copy_bytes':raw_daemon.stat().st_size+raw_cli.stat().st_size,'evidence_allowance_bytes':32*1024*1024},'candidate':manifest,'manifest_sha256':base.sha(manifest_raw),'harness_sha256':base.sha(source),
        'support_sha256':old_support,'frontend_assets':assets,'checks':[],'http':[],'faults':[],'private_owner_reviews':[],
        'screenshots':[],'observations':{},'cleanup':[],'http_unexpected_errors':[],'readonly_aborts':[],'cleanup_complete':False,
        'not_claimed':['native OS Owner dialog','macOS','native WebView','real Provider/model','OS sandbox','unknown meters as zero','all recovery/adoption paths','advance_plan end-to-end','real Validator execution (validators[] in these cases)','whole product completion']}
    # Explicit ownership: no TemporaryDirectory/finalizer may erase a failed
    # runtime or files still referenced by unknown/live processes.
    root=Path(tempfile.mkdtemp(prefix='baton-child-goal-browser-')).resolve();root.chmod(0o700)
    runtime=root/'runtime';runtime.mkdir(mode=0o700)
    daemon=server=http_thread=browser=playwright=log=reaper=host=workspace=scope=None
    token='';pump=lambda:None;active={};lock=threading.Lock();verified=[];graph_verified=[];record_context={};resources={};bounds=None
    def clean(v):return str(v).replace(token,'[redacted generated Owner token]')if token else str(v)
    def passed(name,details):report['checks'].append({'id':name,'status':'passed','evidence':details})
    def fail(stage,error):base.note_failure(report,error,stage,clean)
    def wait(read,predicate,seconds=15):
        end=time.monotonic()+seconds
        while True:
            pump();value=read()
            if predicate(value):return value
            assert time.monotonic()<end,clean(('Bounded observation deadline',value))
            time.sleep(.02)
    def capture():
        if host is None or workspace is None or scope is None or not record_context.get('run_id'):return
        base.collect_started_records(runtime,workspace,scope,record_context,verified,output,reaper)
        collect_graph_records(runtime,workspace,scope,record_context,graph_verified,output)
    try:
        report['controller_reaper']={};reaper=base.FixtureControllerReaper(report['controller_reaper']);reaper.enable()
        report['private_installation']={}
        binary,companion=base.install_private_tools(raw_daemon,raw_cli,root/'installed',report['private_installation'])
        provider=root/'generated-child-goal-acp';settings=root/'generated-settings.json'
        provider.write_text('#!'+sys.executable+'\n'+generated_acp());provider.chmod(0o700)
        report['generated_program_sha256']=base.sha(provider.read_bytes())
        log=(output/'daemon.log').open('xb')
        daemon=subprocess.Popen([str(binary)],env={**os.environ,'BATON_RUNTIME_DIR':str(runtime)},stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        wait(lambda:((runtime/'session.token').is_file(),(runtime/'batond.sock').exists(),daemon.poll()),lambda v:v[0]and v[1]and v[2]is None)
        report['actual_core_wires']=[];wire_bytes=0
        class ObservedHost(base.PrivateHost):
            def wire(self,call):
                nonlocal wire_bytes
                require(len(report['actual_core_wires'])<2048,'Bounded actual Core observation count exceeded')
                value,raw=super().wire(call)
                # Always preserve original bytes before any proposal/result check.
                capacity=os.statvfs(output);require(capacity.f_bavail*capacity.f_frsize>=200*1024*1024+len(raw),'Retain200MiB evidence guard')
                path=output/('wire-reply-'+str(len(report['actual_core_wires']))+'.json')
                with path.open('xb')as writer:writer.write(raw)
                path.chmod(0o600);wire_bytes+=len(raw)
                report['actual_core_wires'].append({'method':call.method,'params':call.params,'key':call.key,'revision':call.revision,
                    'original_reply_path':str(path),'sha256':base.sha(raw),'bytes':len(raw)})
                require(wire_bytes<=16*1024*1024,'Bounded original Core reply evidence exceeded16MiB')
                return value,raw
        host=ObservedHost(runtime);token=host.token
        def approve(method,params,kind,key):
            proposal=host.result(method,params,key);require(proposal['kind']==kind)
            raw=json.dumps(proposal,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode()
            require(len(raw)<=base.MAX_LEGACY_REVIEW_BYTES)
            if kind=='goal':
                require(support.exact_json_value(proposal['snapshot']['intent'],params))
                require(proposal['snapshot']['resolved']['grant_after']['scope']['max_children']==3)
                require(proposal['snapshot']['resolved']['grant_after']['scope']['max_depth']==1)
            report['private_owner_reviews'].append({'authority':'private generated host fixture; NOT OS Owner dialog','proposal':proposal,'complete_sha256':base.sha(raw)})
            return host.result('approvals.finalize',{'proposal_id':proposal['id']},'proposal:'+proposal['id'],proposal['revision'])
        connection=approve('connections.propose',{'name':'Generated bounded child Goal ACP7','provider':'custom_acp','executable':str(provider),'arguments':['--fixture-settings',str(settings)]},'connection','cg:connection')
        cid=connection['id'];rows=wait(lambda:host.result('connections.list',{}),lambda rows:any(r['id']==cid and r['probe_state']=='ready'for r in rows));connection=next(r for r in rows if r['id']==cid)
        source_root=root/'source';source_root.mkdir(mode=0o700);(source_root/'input.txt').write_bytes(b'fixed input\n');(source_root/'.gitignore').write_text('reports/\n')
        git_env={**os.environ,'GIT_CONFIG_GLOBAL':'/dev/null','GIT_CONFIG_NOSYSTEM':'1','GIT_TERMINAL_PROMPT':'0','GIT_NO_LAZY_FETCH':'1'}
        def git(*argv):return subprocess.check_output(['git','-C',str(source_root),'-c','core.hooksPath=/dev/null','-c','user.name=Generated fixture','-c','user.email=fixture@example.invalid',*argv],env=git_env,text=True).strip()
        git('init','-q','--initial-branch=main');git('add','input.txt','.gitignore');git('commit','-qm','generated input')
        source_before={'head':git('rev-parse','HEAD'),'sha256':base.sha((source_root/'input.txt').read_bytes()),'mtime_ns':(source_root/'input.txt').stat().st_mtime_ns}
        project=host.result('projects.import',{'path':str(source_root),'name':'Generated child Goal browser'},'cg:project')
        workspace=host.result('workspaces.create',{'project_id':project['id'],'expected_head':source_before['head']},'cg:workspace',project['revision'])
        wroot=Path(workspace['root']);assert wroot.resolve().is_relative_to(runtime/'workspaces')
        original_grant=approve('grants.propose',{'project_id':project['id'],'workspace_id':workspace['id'],'connection_id':cid,'max_attempts':8,'max_concurrency':1,'max_duration_ms':30000},'grant','cg:grant')
        assert original_grant['scope']['max_children']==original_grant['scope']['max_depth']==0
        budget={'attempts':8,'parallel':1,'runtime_ms':30000,'output_bytes':12288,'children':3,'reworks':0,'tokens':None,'cost_microunits':None}
        inputs=[{'artifact_id':'input','relative_path':'input.txt','max_bytes':1024,'expected_sha256':base.sha(b'fixed input\n')}]
        bootstrap=host.result('tasks.create',{'project_id':project['id'],'goal':'Explicitly review original bounded child ceiling; never run this bootstrap task'},'cg:bootstrap-task')
        native_selection={'task_id':bootstrap['id'],'expected_task_revision':bootstrap['revision'],'grant_id':original_grant['id'],'expected_grant_revision':original_grant['revision'],
            'max_nodes':4,'new_max_children':3,'new_max_depth':1,'root_budget':copy.deepcopy(budget),'project_budget':copy.deepcopy(budget),'inputs':inputs,
            'outputs':[{'artifact':{'artifact_id':'bootstrap-output','relative_path':'reports/bootstrap.txt','max_bytes':1024,'expected_sha256':None},'validators':[]}],
            'writable_prefixes':['reports'],'owner_acceptance_criteria':'Bounded Grant ceiling review only; no bootstrap candidate is approved'}
        native_scope=approve('goal_scopes.propose',native_selection,'goal','cg:explicit-original-ceiling')
        grant=next(g for g in host.result('grants.list',{'project_id':project['id']})if g['id']==original_grant['id'])
        assert grant['id']==original_grant['id'] and grant['revision']>original_grant['revision'] and grant['scope']['max_children']==3 and grant['scope']['max_depth']==1
        report['observations']['explicit_original_grant_ceiling']={'initial_children0':original_grant,'separate_native_goal_scope':native_scope,'effective_original_grant':grant}
        task=host.result('tasks.create',{'project_id':project['id'],'goal':'Execute and review the exact saved child Goal report, then resume the original parent'},'cg:root-task')
        settings_value={'case':args.case,'task_id':task['id'],'connection_id':cid,'workspace_id':workspace['id'],'companion':str(companion),'nonce':uuid.uuid4().hex}
        base.write_private_json(settings,settings_value)
        record_context.update(nonce=settings_value['nonce'],program_sha256=base.sha(provider.read_bytes()),settings_sha256=base.sha(settings.read_bytes()),connection_id=cid,provider=str(provider),settings=str(settings),companion=str(companion),companion_sha256=base.sha(companion.read_bytes()))
        outputs=[{'artifact_id':key,'relative_path':'reports/'+key+'.txt','max_bytes':4096,'expected_sha256':None}for key in ['report-child','report-parent']]
        selection={'root_task_id':task['id'],'expected_task_revision':task['revision'],'grant_id':grant['id'],'expected_grant_revision':grant['revision'],'connection_ids':[cid],'workspace_ids':[workspace['id']],
            'inputs':inputs,'outputs':outputs,'recipients':[],'max_depth':2,'max_children':3,'max_total_nodes':4,'root_budget':copy.deepcopy(budget),'project_budget':copy.deepcopy(budget),'ttl_ms':30000,
            'capabilities':{'delegate_task':True,'resume_parent_as_new_attempt':True,'child_planning':True,'child_goal_execution':True,'create_workspace':False},
            'child_planning_policy':{'max_nodes':4,'max_revisions':8,'max_plan_bytes':planning.PLAN_BYTES,'outputs':[{'artifact':a,'validators':[]}for a in outputs],
                'writable_prefixes':['reports/report-child.txt'],'owner_acceptance_criteria':'Review the exact generated child report SHA, workspace root, input version and actual owned stop'},
            'child_goal_execution_policy':{'max_nodes':2,'max_runs_per_task':1,'max_total_runs':2,'root_budget':copy.deepcopy(budget),'project_budget':copy.deepcopy(budget),'require_owner_acceptance':True}}
        resources.update(project=project,task=task,grant=grant,workspace=workspace,connection=connection,companion=companion)
        # Real private-IPC denials, not synthetic success or HTTP Owner authority.
        def denied(method,params,key,revision,code):
            before=database_snapshot(runtime);reply,raw=host.wire(FixedCall(method,params,key,revision));assert reply.get('error',{}).get('code')==code,reply
            assert reply['error']['side_effect_state']=='not_started';after=database_snapshot(runtime);assert before==after,(method,'denial changed DB')
            report['observations'].setdefault('actual_denials',[]).append({'method':method,'params':params,'key':key,'revision':revision,'response':reply,'raw_sha256':base.sha(raw),'before':before,'after':after})
        false_task=host.result('tasks.create',{'project_id':project['id'],'goal':'Retain old false scope with no child Goal authority'},'cg:false-task')
        false_selection=copy.deepcopy(selection);false_selection.update(root_task_id=false_task['id'],expected_task_revision=false_task['revision'])
        false_selection['capabilities']['child_planning']=False;false_selection['capabilities']['child_goal_execution']=False
        del false_selection['child_planning_policy'];del false_selection['child_goal_execution_policy']
        false_proposal=host.result('factory_scopes.propose',false_selection,'cg:false-scope')
        base.review_factory(false_proposal,false_selection,project,false_task,grant,connection,workspace,companion)
        false_scope=host.result('approvals.finalize',{'proposal_id':false_proposal['id']},'proposal:'+false_proposal['id'],false_proposal['revision'])
        assert 'child_goal_execution'not in false_scope['bindings'] and 'child_planning'not in false_scope['bindings']
        fake_ref={'id':'negative-nonexistent','scope_id':false_scope['id'],'run_id':'negative-nonexistent','task_id':false_task['id'],'revision':1,'plan_digest':'a'*64,'raw_sha256':'b'*64,'receipt_sequence':1}
        denied('factory_child_goals.start',{'scope_id':false_scope['id'],'request':{'task_id':false_task['id'],'plan':fake_ref,'expected_factory_run_revision':1}},'cg:deny-old-false',false_scope['revision'],'AUTH_REQUIRED')
        too_many=copy.deepcopy(selection);too_many['child_goal_execution_policy']['max_nodes']=5
        denied('factory_scopes.propose',too_many,'cg:deny-nodes-beyond-grant',None,'AUTH_REQUIRED')
        too_much=copy.deepcopy(selection);too_much['root_budget']['attempts']=9;too_much['project_budget']['attempts']=9
        too_much['child_goal_execution_policy']['root_budget']['attempts']=9;too_much['child_goal_execution_policy']['project_budget']['attempts']=9
        denied('factory_scopes.propose',too_much,'cg:deny-attempt-ceiling',None,'AUTH_REQUIRED')
        passed('CG01','Separate actual Native fixture establishes original Grant ceiling; old false and excess policy requests denied without DB/journal/attempt mutation')
        bounds=Bounds(project['id'],task['id'],'','',tasks={task['id']})
        fault={'method':'factory_child_goals.start'if args.case=='lost-start'else 'factory_child_goals.cancel'if args.case=='cancel'else None,'used':False}
        def observe(call,response):
            if response.get('error'):return
            value=response['result']
            if call.method=='factory_runs.get':
                require(value['run']['scope_id']==bounds.scope_id)
                bounds.factory_run_id=value['run']['id'];record_context['run_id']=value['run']['id']
                for t in value['tasks']:
                    require(t['root_task_id']==bounds.root_task_id and (t['parent_task_id']is None or t['parent_task_id']==bounds.root_task_id));bounds.tasks.add(t['task_id'])
            if call.method=='factory_child_plans.get':
                require(value['reference']['scope_id']==bounds.scope_id and value['reference']['run_id']==bounds.factory_run_id)
                require(value['reference']['task_id']in bounds.tasks and value['reference']['raw_sha256']==base.sha(value['plan_json'].encode()))
                if value['reference']not in bounds.saved_refs:bounds.saved_refs.append(value['reference'])
            summaries=value if call.method=='factory_child_goals.list'else [value['summary']]if call.method=='factory_child_goals.get'else [value]if call.method in {'factory_child_goals.start','factory_child_goals.cancel','factory_child_goals.advance_plan'}else []
            for row in summaries:
                require(row['factory_scope_id']==bounds.scope_id and row['factory_run_id']==bounds.factory_run_id and row['task_id']in bounds.tasks)
                bounds.goal_runs[row['goal']['run']['id']]=row['task_id']
        class Server(http.server.ThreadingHTTPServer):
            daemon_threads=False;block_on_close=True
            def handle_error(self,request,address):report['http_unexpected_errors'].append(clean(traceback.format_exc()))
        class Handler(http.server.SimpleHTTPRequestHandler):
            def setup(self):super().setup();self.connection.settimeout(5)
            def log_message(self,*_):pass
            def list_directory(self,_):self.send_error(403);return None
            def do_POST(self):
                identity=id(self);call=None;stage='headers'
                with lock:active[identity]=stage
                try:
                    if self.path!='/__test_child_goal_ipc':self.send_error(404);return
                    try:
                        require(self.headers.get('Transfer-Encoding')is None)
                        require(all(len(self.headers.get_all(k,[]))==1 for k in ['Content-Length','Content-Type','Origin','Host']))
                        length=self.headers['Content-Length'];require(re.fullmatch('[1-9][0-9]{0,6}',length)is not None and int(length)<=planning.MAX_WIRE)
                        raw=self.rfile.read(int(length));require(len(raw)==int(length))
                        call=decode_http(raw,origin=self.headers['Origin'],host=self.headers['Host'],content_type=self.headers['Content-Type'],expected_origin=url,bounds=bounds)
                    except Refusal as error:self.send_error(error.status);return
                    trace={'method':call.method,'params':call.params,'key':call.key,'revision':call.revision,'request_sha256':base.sha(raw),'response_complete':False}
                    with lock:active[identity]=call.method;report['http'].append(trace)
                    stage='core_call';value,reply=host.wire(call);require(len(reply)<=planning.MAX_WIRE)
                    path=output/('core-reply-'+uuid.uuid4().hex+'.json');path.write_bytes(reply);path.chmod(0o600)
                    trace.update(response=value,original_reply_path=str(path),original_reply_sha256=base.sha(reply));observe(call,value)
                    if call.method==fault['method']and not fault['used']:
                        require(not value.get('error'));fault['used']=True;trace['actual_committed_reply_deliberately_dropped']=True
                        report['faults'].append(trace.copy());self.close_connection=True;self.connection.shutdown(socket.SHUT_RDWR);return
                    stage='response_headers';self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(reply)));self.end_headers()
                    stage='response_body';self.wfile.write(reply);trace['response_complete']=True
                except (BrokenPipeError,ConnectionResetError)as error:
                    readonly=call is not None and call.method in base.READS|{'factory_child_plans.context','factory_child_plans.get','factory_child_plans.list','factory_child_goals.get','factory_child_goals.list'}and stage in {'response_headers','response_body'}
                    if readonly:report['readonly_aborts'].append({'method':call.method,'stage':stage,'error':type(error).__name__})
                    else:fail('unexpected_http_abort',error)
                except BaseException as error:
                    report['http_unexpected_errors'].append(clean(error));fail('http_handler',error);self.send_error(502)
                finally:
                    with lock:active.pop(identity,None)
        server=Server(('127.0.0.1',0),partial(Handler,directory=str(frontend)));url='http://127.0.0.1:'+str(server.server_port)
        http_thread=threading.Thread(target=server.serve_forever);http_thread.start()
        playwright=sync_playwright().start();browser=playwright.chromium.launch(executable_path=args.chromium,headless=True,args=['--no-sandbox'])
        context=browser.new_context(viewport={'width':1488,'height':1400},locale='en-US');context.set_default_timeout(15000)
        def private_owner(_source,command,payload):
            nonlocal scope
            require(type(payload)is dict)
            if command=='factory_scope_approve':
                require(scope is None and set(payload)=={'selection','idempotencyKey'}and support.exact_json_value(payload['selection'],selection))
                require(type(payload['idempotencyKey'])is str and planning.KEY.fullmatch(payload['idempotencyKey']))
                proposal=host.result('factory_scopes.propose',payload['selection'],payload['idempotencyKey'])
                report['private_owner_reviews'].append(review_execution_scope(proposal,selection,resources))
                reply,raw=host.wire(FixedCall('approvals.finalize',{'proposal_id':proposal['id']},'proposal:'+proposal['id'],proposal['revision']))
                require(not reply.get('error')and len(raw)<=planning.MAX_WIRE);scope=reply['result'];bounds.scope_id=scope['id']
                require(support.exact_json_value(scope['bindings'],proposal['snapshot']['bindings'])and scope['contract_digest']==proposal['snapshot']['contract_digest'])
            else:
                require(command=='goal_node_approve'and set(payload)=={'selection','expectedRevision','idempotencyKey'})
                s=payload['selection'];require(set(s)=={'run_id','node_id','expected_plan_revision','expected_ledger_revision'})
                require(s['run_id']in bounds.goal_runs and s['node_id']=='owner-report')
                current=host.result('factory_child_goals.get',{'scope_id':scope['id'],'task_id':bounds.goal_runs[s['run_id']],'goal_run_id':s['run_id']})
                detail=current['details'];require(payload['expectedRevision']==detail['run']['revision'] and s['expected_plan_revision']==detail['run']['plan_revision']and s['expected_ledger_revision']==detail['ledger_revision'])
                require(detail['task_accepted']is False and len(detail['candidates'])==1 and not detail['validations'])
                producer=next(n for n in detail['nodes']if n['node']['id']=='fixed-work');require(producer['stop_confirmed']is True)
                proposal=host.result('goal_node_approvals.propose',s,payload['idempotencyKey'],payload['expectedRevision'])
                reviewed=support.check_full_review(proposal,expected_kind='goal_node_acceptance')
                require(support.exact_json_value(proposal['snapshot']['run_binding']['factory_origin'],current['origin']))
                require(support.exact_json_value(proposal['snapshot']['candidate_receipts'],detail['candidates']))
                require(current['origin']['original_native_approval_id']==scope['approval_id']and current['origin']['root_grant_revision']==grant['revision'])
                require(support.exact_json_value(current['origin']['initial_plan'],current['summary']['plan']))
                report['private_owner_reviews'].append({'authority':'private exact-candidate test fixture; NOT OS Owner dialog','proposal':proposal,'review':reviewed,'before':current})
                reply,raw=host.wire(FixedCall('approvals.finalize',{'proposal_id':proposal['id']},'proposal:'+proposal['id'],proposal['revision']))
                require(not reply.get('error')and reply['result']['task_accepted']is True)
            base.write_private_json(output/('private-owner-'+uuid.uuid4().hex+'.json'),{'response':reply,'original_sha256':base.sha(raw)})
            return reply
        context.expose_binding('__private_child_goal_owner',private_owner)
        context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{if(command==='factory_scope_approve'||command==='goal_node_approve')return window.__private_child_goal_owner(command,args);const r=await fetch('/__test_child_goal_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed child Goal test bridge refused request');return r.json();}};""")
        context.route('**/*',lambda route:route.continue_()if route.request.url.startswith(url+'/')else route.abort())
        page=context.new_page();errors=[];page.on('pageerror',lambda e:errors.append(str(e)));pump=lambda:page.wait_for_timeout(1)
        def shot(name,focus):
            focus.evaluate("e=>e.scrollIntoView({block:'start'})");path=output/(name+'.png');page.screenshot(path=str(path));report['screenshots'].append({'path':str(path),'sha256':base.sha(path.read_bytes()),'dimension':'Linux Chromium, NOT Native/macOS'})
        def expand(details):
            if not details.evaluate('e=>e.open'):details.locator(':scope > summary').click()
        page.goto(url);page.get_by_test_id('task-card').filter(has_text=task['goal']).click();page.locator('#tab-details').click()
        panel=page.get_by_test_id('factory-runtime');expand(panel.locator(':scope > details'));form=panel.locator('form')
        planning.fixed_form_control(form,'Existing root Grant','select').select_option(grant['id'])
        planning.fixed_form_control(form,'Existing connections','select').select_option([cid]);planning.fixed_form_control(form,'Existing workspaces','select').select_option([workspace['id']])
        expand(form.locator('details').filter(has=page.get_by_text('Selected inputs / Declared outputs',exact=True)))
        form.get_by_label('Selected inputs',exact=True).fill('input | input.txt | 1024 | '+base.sha(b'fixed input\n'))
        form.get_by_label('Declared outputs',exact=True).fill('\n'.join(a['artifact_id']+' | '+a['relative_path']+' | 4096'for a in outputs))
        for label,value in [('Maximum delegation depth',2),('Maximum child tasks',3),('Maximum total tasks',4),('Approval lifetime (ms)',30000)]:form.get_by_label(label,exact=True).fill(str(value))
        def fill_budget(group):
            for label,key in [('Attempts','attempts'),('Parallel attempts','parallel'),('Runtime ms','runtime_ms'),('Declared output bytes','output_bytes'),('Children','children'),('Reworks','reworks')]:group.get_by_label(label,exact=True).fill(str(budget[key]))
        fill_budget(form.get_by_role('group',name='Root shared limits',exact=True));fill_budget(form.get_by_role('group',name='Project shared limits',exact=True))
        policy=form.locator('details').filter(has=page.get_by_text('Child-planning policy for Native review',exact=True));expand(policy)
        policy.get_by_label('Allow bounded child planning',exact=True).check()
        for label,value in [('Maximum plan nodes',4),('Maximum revisions per task',8),('Maximum original plan bytes',planning.PLAN_BYTES)]:policy.get_by_label(label,exact=True).fill(str(value))
        policy.get_by_label('Owner acceptance criteria',exact=True).fill(selection['child_planning_policy']['owner_acceptance_criteria'])
        policy.get_by_label('Exact declared output paths (one per line)',exact=True).fill('reports/report-child.txt')
        execution=form.locator('details').filter(has=page.get_by_text('Child Goal execution policy',exact=True));expand(execution)
        execution.get_by_label('Request separate bounded child Goal execution in this Native Factory review',exact=True).check()
        execution.get_by_label('Maximum nodes per child Goal',exact=True).fill('2');execution.get_by_label('Maximum child Goal runs in this scope',exact=True).fill('2')
        fill_budget(execution.get_by_role('group',name='Child Goal root budget',exact=True));fill_budget(execution.get_by_role('group',name='Child Goal project budget',exact=True))
        shot('01-complete-explicit-execution-policy',form)
        form.get_by_role('button',name='Review Factory scope in native dialog',exact=True).click()
        wait(lambda:scope,lambda value:value is not None)
        approval_snapshot=database_snapshot(runtime);verify_approval_did_not_execute(approval_snapshot)
        require(not (wroot/'reports/fixture-stages.txt').exists()and not (wroot/'reports/child-plan-ready.json').exists())
        report['observations']['approval_before_any_explicit_start']=approval_snapshot
        expect(panel.get_by_role('button',name='Start approved Factory run',exact=True)).to_be_enabled()
        passed('CG02','Complete private-Owner-approved UI scope; all Factory/Graph/session/attempt/dispatch/reservation execution domains0, inactive leases, only explicit unused Native metadata remains')
        panel.get_by_role('button',name='Start approved Factory run',exact=True).click()
        def actual_factory_start():
            entries=[{**row,'request_body_sha256':row['request_sha256'],
                'original_core_reply_sha256':row.get('original_reply_sha256')}for row in report['http']if row['method']=='factory_runs.start']
            return base.confirmed_start_response(entries,scope,project['id'],task['id'])
        root_start=wait(actual_factory_start,lambda value:value is not None)
        factory_run=root_start['response']['result'];bounds.factory_run_id=record_context['run_id']=factory_run['id']
        report['observations']['unique_explicit_UI_factory_start']=root_start
        ready_path=wroot/'reports/child-plan-ready.json'
        wait(lambda:(host.result('factory_runs.get',{'scope_id':scope['id']}),ready_path.exists()),lambda v:v[1])
        family=host.result('factory_runs.get',{'scope_id':scope['id']});record_context['run_id']=bounds.factory_run_id=family['run']['id'];bounds.tasks.update(t['task_id']for t in family['tasks'])
        raw,pin=base.bounded_file(ready_path,planning.MAX_WIRE,wroot/'reports');ready=json.loads(raw,object_pairs_hook=planning.unique_object)
        child=ready['child'];saved=ready['saved'];require(child['task_id']in bounds.tasks and child['parent_task_id']==task['id'])
        actual_saved=host.result('factory_child_plans.get',{'scope_id':scope['id'],'task_id':child['task_id'],'plan_revision':1})
        require(support.exact_json_value(actual_saved,saved)and saved['reference']['raw_sha256']==base.sha(saved['plan_json'].encode()))
        require(ready['context']['goal_execution_authorized']is False and ready['fresh_status']['child_goal_execution_binding']==scope['bindings']['child_goal_execution'])
        bounds.saved_refs.append(saved['reference']);report['observations']['actual_scoped_saved_plan']={'file_pin':pin,'ready':ready,'actual_read':actual_saved}
        capture()
        # Authoritative real CAS rejection before any child Goal is reserved.
        request={'task_id':child['task_id'],'plan':saved['reference'],'expected_factory_run_revision':family['run']['revision']}
        denied('factory_child_goals.start',{'scope_id':scope['id'],'request':request},'cg:stale-scope',scope['revision']+1,'STALE_REVISION')
        wrong=copy.deepcopy(request);wrong['expected_factory_run_revision']+=1
        denied('factory_child_goals.start',{'scope_id':scope['id'],'request':wrong},'cg:stale-factory-run',scope['revision'],'STALE_REVISION')
        panel.get_by_role('button',name='Refresh Factory records',exact=True).click()
        # Use actual task container; never choose a same-caption root panel by first().
        task_item=panel.get_by_role('region',name='Factory run',exact=True).locator(':scope > ul > li').filter(has_text=child['task_id'])
        require(task_item.count()==1,'Exact child task container required')
        plan_details=task_item.locator(':scope > details');expand(plan_details)
        plans=task_item.get_by_role('region',name='Child task plans',exact=True)
        plans.get_by_role('button',name='Refresh child-plan history',exact=True).click()
        revisions=plans.locator('details').filter(has=page.get_by_text('Immutable plan revisions',exact=True));expand(revisions)
        revisions.get_by_role('button',name='r1 · '+saved['reference']['plan_digest'],exact=True).click()
        goal_details=plans.get_by_test_id('factory-child-goals');expand(goal_details)
        goal_panel=goal_details.get_by_test_id('factory-child-goal-panel')
        goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()
        expect(goal_panel.get_by_role('button',name='Execute this exact saved child plan',exact=True)).to_be_enabled()
        shot('02-exact-saved-raw-reference-before-start',goal_panel)
        goal_panel.get_by_role('button',name='Execute this exact saved child plan',exact=True).click()
        started=wait(lambda:[r for r in report['http']if r['method']=='factory_child_goals.start'and not r.get('response',{}).get('error')and r.get('response')],lambda v:len(v)==1)[0]
        initial=started['response']['result'];goal_id=initial['goal']['run']['id'];bounds.goal_runs[goal_id]=child['task_id']
        assert support.exact_json_value(initial['plan'],saved['reference'])and initial['goal']['task_accepted']is False
        assert not (wroot/'reports/release-parent').exists();base.write_private_json(wroot/'reports/release-parent',{'committed_goal_run_id':goal_id,'actual_start_reply_sha256':started['original_reply_sha256']})
        if args.case=='lost-start':
            assert fault['used'];expect(goal_panel.get_by_role('status').filter(has_text='Outcome unknown')).to_be_visible()
            assert len([r for r in report['http']if r['method']=='factory_child_goals.start'])==1
            goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()
            recover=goal_panel.get_by_role('button',name='Recover the exact child Goal request',exact=True);expect(recover).to_be_enabled();recover.click()
            calls=wait(lambda:[r for r in report['http']if r['method']=='factory_child_goals.start'and r.get('response_complete')],lambda v:len(v)==1)
            recovered=calls[0];assert (recovered['params'],recovered['key'],recovered['revision'])==(started['params'],started['key'],started['revision'])
            assert support.exact_json_value(recovered['response']['result'],initial)
            wait(lambda:host.result('factory_child_goals.get',{'scope_id':scope['id'],'task_id':child['task_id'],'goal_run_id':goal_id}),lambda v:any(n['node']['id']=='owner-report'and n['state']=='awaiting_owner'for n in v['details']['nodes']))
            changed=copy.deepcopy(started['params']);changed['request']['expected_factory_run_revision']+=1
            denied('factory_child_goals.start',changed,started['key'],started['revision'],'CONFIG_CONFLICT')
            passed('CG03','Actual committed start reply lost; no auto replay; explicit exact-key/payload/CAS recovery returns original receipt; changed payload conflicts')
        query={'scope_id':scope['id'],'task_id':child['task_id'],'goal_run_id':goal_id}
        if args.case=='cancel':
            wait(lambda:host.result('factory_child_goals.get',query),lambda v:any(n['session_id']for n in v['details']['nodes']))
            wait(lambda:(capture(),len(graph_verified))[1],lambda count:count==1)
            goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()
            cancel=goal_panel.get_by_role('button',name='Request child Goal cancellation',exact=True);expect(cancel).to_be_enabled();cancel.click()
            wait(lambda:report['faults'],lambda v:len(v)==1)
            assert fault['used']and len([r for r in report['http']if r['method']=='factory_child_goals.cancel'])==1
            goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()
            recover=goal_panel.get_by_role('button',name='Recover the exact child Goal request',exact=True);expect(recover).to_be_enabled();recover.click()
            cancels=wait(lambda:[r for r in report['http']if r['method']=='factory_child_goals.cancel'and r.get('response')],lambda v:len(v)==2)
            assert (cancels[0]['params'],cancels[0]['key'],cancels[0]['revision'])==(cancels[1]['params'],cancels[1]['key'],cancels[1]['revision'])
            final_view=wait(lambda:host.result('factory_child_goals.get',query),lambda v:v['details']['run']['state']=='cancelled')
            assert final_view['details']['task_accepted']is False and final_view['details']['task_acceptance']is None
            family=host.result('factory_runs.get',{'scope_id':scope['id']})
            if family['run']['state']not in {'cancelled','completed'}:host.result('factory_runs.cancel',{'scope_id':scope['id']},'cg:final-parent-cancel',family['run']['revision'])
            final_factory=wait(lambda:host.result('factory_runs.get',{'scope_id':scope['id']}),lambda v:v['run']['state']in {'cancelled','blocked'})
            passed('CG04','Actual live Graph cancel reply lost, explicit same-key recovery and actual cancelled view; no TaskAccepted or automatic opposite action')
        else:
            before_owner=wait(lambda:host.result('factory_child_goals.get',query),lambda v:any(n['node']['id']=='owner-report'and n['state']=='awaiting_owner'for n in v['details']['nodes']))
            capture();require(before_owner['details']['task_accepted']is False and before_owner['details']['task_acceptance']is None)
            goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()
            owner=goal_panel.get_by_role('button',name='Review exact Factory-origin candidate in native dialog',exact=True);expect(owner).to_be_enabled()
            shot('03-real-owned-candidate-before-owner',goal_panel);owner.click()
            final_view=wait(lambda:host.result('factory_child_goals.get',query),lambda v:v['details']['task_accepted']is True)
            receipt=final_view['details']['task_acceptance'];require(receipt is not None and receipt['run_id']==goal_id)
            final_factory=wait(lambda:host.result('factory_runs.get',{'scope_id':scope['id']}),lambda v:v['run']['state']=='completed')
            require((wroot/'reports/fixture-stages.txt').read_text()=='parent-initial\nparent-resumed\n')
            require((wroot/'reports/report-child.txt').read_bytes()==b'fixed child Goal report\n')
            require((wroot/'reports/report-parent.txt').read_bytes()==b'parent-resumed report-parent report\n')
            latest_task=next(t for t in host.result('system.snapshot',{})['tasks']if t['id']==child['task_id'])
            require(latest_task['state']=='accepted'and latest_task['acceptance_ref']==receipt['id'])
            resumed=wait(lambda:(capture(),verified)[1],lambda v:len(v)==2)
            resumed_raw,_=base.bounded_file(Path(next(v['path']for v in resumed if v['stage']=='parent-resumed')),base.MAX_HTTP_BYTES)
            resumed_request=json.loads(json.loads(resumed_raw)['request_text']);handoff=resumed_request['handoff_result']
            require(handoff['acceptance_state']=='task_accepted'and handoff['task']['confirmed_stop']is True and 'goal-task-acceptance:'+receipt['id']in handoff['validation_refs'])
            require(handoff['artifacts'][0]['sha256']==base.sha(b'fixed child Goal report\n'))
            report['observations']['durable_acceptance_and_parent_resume']={'before_owner':before_owner,'accepted':final_view,'task':latest_task,'handoff':handoff}
            passed('CG05','Exact real owned candidate + private full Factory-origin Owner approval creates durable TaskAccepted, then original parent resumes as a new attempt with accepted SHA-bound handoff')
        capture();require(len(graph_verified)==1 and len(verified)==(1 if args.case=='cancel'else 2))
        goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click();shot('04-persisted-final-graph-and-factory',goal_panel)
        snapshot=database_snapshot(runtime);report['observations']['final_snapshot']=snapshot
        require(snapshot['factory_counts']['child_goal_runs']==1 and snapshot['factory_counts']['child_plan_heads']==snapshot['factory_counts']['child_plan_revisions']==1)
        with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as db:
            db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
            require(support.exact_json_value(json.loads(db.execute('SELECT document FROM execution_grants WHERE id=?',[grant['id']]).fetchone()[0]),grant))
            # The same Grant explicitly changed0->3 only at the separate Native
            # Goal approval. Factory's original is this confirmed after3 record.
            # Both persisted revisions must retain their exact original objects.
            history=db.execute('SELECT revision,goal_approval_id,document FROM goal_grant_revisions WHERE grant_id=? ORDER BY revision',[grant['id']]).fetchall()
            require(len(history)==2 and [v[0]for v in history]==[original_grant['revision'],grant['revision']])
            require(history[0][1]is None and history[1][1]==native_scope['id'])
            require(support.exact_json_value(json.loads(history[0][2]),original_grant))
            require(support.exact_json_value(json.loads(history[1][2]),grant))
            report['observations']['original_Native_grant_revision_history']=[{'revision':revision,'goal_approval_id':approval,'document':json.loads(document)}for revision,approval,document in history]
            for sql in ["SELECT count(*)FROM dispatch_intents WHERE state!='stopped'","SELECT count(*)FROM workspace_leases WHERE holder!=''","SELECT count(*)FROM factory_scoped_handles WHERE revoked=0"]:require(db.execute(sql).fetchone()[0]==0,sql)
            require(db.execute('SELECT count(*)FROM goal_graph_capabilities').fetchone()[0]==0)
            require(db.execute('SELECT count(*)FROM goal_run_reservations').fetchone()[0]==1)
            budgets=[dict(zip(['scope','consumed','active'],r))for r in db.execute('SELECT scope,consumed,active FROM budgets')]
            # The independent original Native scope creates its own unused root
            # counter. Preserve that real row; do not pretend only2 rows exist.
            unused=[r for r in budgets if r['scope']=='root:'+bootstrap['id']]
            require(len(unused)==1 and unused[0]['consumed']==unused[0]['active']==0)
            active_family=[r for r in budgets if r['scope']!='root:'+bootstrap['id']]
            base.verify_attempt_budgets(active_family,task['id'],project['id'],2 if args.case=='cancel'else 3)
            require(len(budgets)==3)
            meters={key:json.loads(raw)for key,raw in db.execute('SELECT scope,document FROM goal_ledgers')}
            require(set(meters)=={'project:'+project['id'],'root-grant:'+grant['id']})
            for meter in meters.values():
                require(support.exact_json_value(meter['limits'],budget))
                require(type(meter['observed_attempts'])is int and meter['observed_attempts']==(2 if args.case=='cancel'else 3))
                require(meter['tokens']is None and meter['cost_microunits']is None)
                for key in ['runtime_ms','output_bytes']:
                    require(meter[key]is None or planning.safe_integer(meter[key]))
            report['observations']['actual_shared_attempt_budgets']=budgets
            report['observations']['actual_shared_cumulative_meters']={'records':meters,'unknown_preserved_as_null':True,'whole_filesystem_measurement_claimed':False}
            events=[json.loads(raw)for raw, in db.execute('SELECT document FROM events ORDER BY sequence')]
            stops=[e for e in events if e['kind']=='session.stopped'and e['payload'].get('process_reaped')is True and e['payload'].get('no_process_started')is False]
            require(len(stops)==2 if args.case=='cancel'else len(stops)==3)
            require(db.execute("SELECT count(*)FROM tasks WHERE json_extract(document,'$.state')='accepted'").fetchone()[0]==(0 if args.case=='cancel'else 1))
            report['observations'].update(actual_owned_stop_events=stops,journal=events,final_factory=final_factory,final_child_goal=final_view);db.rollback()
        require(git('status','--porcelain')==''and source_before=={'head':git('rev-parse','HEAD'),'sha256':base.sha((source_root/'input.txt').read_bytes()),'mtime_ns':(source_root/'input.txt').stat().st_mtime_ns})
        require(not errors and not report['http_unexpected_errors'],(errors,report['http_unexpected_errors']))
        passed('CG06','Factory-bound Native-confirmed grant3 unchanged after Factory approval, both explicit Native revision histories retained; one derived run/one immutable saved ref, shared3(or2 cancellation) attempts consumed with active0, no refund/new identity/hidden Native Graph approval, actual owned-stop journal and unchanged source')
        report['status']='passed'
    except BaseException as error:
        fail('runtime',error);planning.capture_browser_failure(browser,output,report,clean)
    finally:
        pump=lambda:None
        if host is not None:
            def diagnostics():
                info,_=base.diagnostic_snapshot(runtime,workspace,output,token);report['observations']['final_diagnostics']=info
                require(info['complete']is True and info['schema']==15)
                if scope is not None:report['observations']['last_actual_SQL_snapshot']=database_snapshot(runtime)
            base.cleanup_call(report,'readonly_failure_diagnostics',diagnostics,clean)
        if scope is not None and host is not None:
            def cancel_remaining():
                current=host.result('factory_runs.get',{'scope_id':scope['id']});record_context['run_id']=current['run']['id']
                if current['run']['state']not in {'completed','cancelled','blocked'}:
                    host.result('factory_runs.cancel',{'scope_id':scope['id']},'cg:cleanup-cancel',current['run']['revision'])
                capture()
            base.cleanup_call(report,'owned_scope_cleanup_request',cancel_remaining,clean)
        planning.close_browser_engine(browser,playwright,report,clean)
        http_complete=True
        if server is not None:
            base.cleanup_call(report,'HTTP_shutdown',server.shutdown,clean);http_thread.join(timeout=5)
            http_complete=not http_thread.is_alive();base.cleanup_call(report,'HTTP_handlers_close',server.server_close,clean);http_complete=http_complete and not active
            report['cleanup'].append({'action':'HTTP_serve_and_handler_join','confirmed':http_complete,'active':dict(active)})
            if not http_complete:fail('HTTP_join',AssertionError('Owned HTTP threads remain'))
        daemon_complete=False
        if daemon is not None:
            def stop_daemon():
                if daemon.poll()is None:daemon.terminate()
                try:code=daemon.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(daemon.pid,signal.SIGKILL);daemon.wait();raise AssertionError('Forced owned daemon kill; NOT successful Core shutdown')
                report['cleanup'].append({'action':'actual_private_daemon_wait','pid':daemon.pid,'exitcode':code});require(code==0);return True
            daemon_complete=base.cleanup_call(report,'daemon_join',stop_daemon,clean)is True
        if reaper is not None:base.cleanup_call(report,'bound_adopted_MCP_wait',lambda:[reaper.reap(v)for v in verified],clean)
        identities_complete=False
        if verified:
            identities_complete=base.cleanup_call(report,'Factory_bound_identities_gone',lambda:base.inspect_owned_records(report,verified,1 if args.case=='cancel'else 2,provider,settings,companion,runtime,clean,require_full=report['status']=='passed'and not report.get('failure')),clean)is True
        elif daemon is not None:fail('Factory_identity_coverage',AssertionError('No bound actual Factory process records'))
        graph_gone=True if graph_verified else None
        for record in graph_verified:
            try:
                raw,pin=base.bounded_file(Path(record['path']),base.MAX_HTTP_BYTES);require(pin==record['file_pin']and base.sha(raw)==record['sha256'])
                identity=record['owned_provider']
                try:current=support.proc_identity(identity['pid'])
                except FileNotFoundError:current=None
                gone=current is None or current['start_ticks']!=identity['start_ticks']
                report['cleanup'].append({'action':'bound_Graph_provider_identity_gone','expected':identity,'observed':current,'confirmed':gone,'signal_sent':False})
                if not gone:
                    fail('Graph_Core_join_missing',AssertionError('Owned Graph Provider remains after daemon join; forced cleanup is NOT acceptance'))
                    deadline=time.monotonic()+3;sent=False
                    while current is not None and current['start_ticks']==identity['start_ticks']:
                        require(os.getpgid(identity['pid'])==identity['pgid']==identity['pid'])
                        if current['state']=='Z'and current['parent_pid']==os.getpid():
                            waited,status=os.waitpid(identity['pid'],os.WNOHANG);require(waited==identity['pid'])
                            report['cleanup'].append({'action':'unexpected_adopted_Graph_zombie_wait','pid':waited,'raw_wait_status':status,'Core_join_claimed':False})
                        elif not sent:
                            argv=Path('/proc',str(identity['pid']),'cmdline').read_bytes();require(0<len(argv)<=8192 and argv.endswith(b'\0'))
                            require([v.decode()for v in argv[:-1].split(b'\0')][-3:]==[str(provider),'--fixture-settings',str(settings)])
                            require(support.proc_identity(identity['pid'])['start_ticks']==identity['start_ticks'])
                            os.kill(identity['pid'],signal.SIGTERM);sent=True
                            report['cleanup'].append({'action':'forced_exact_Graph_SIGTERM','expected':identity,'not_successful_Core_join':True})
                        if time.monotonic()>=deadline:
                            require(sent,'Graph cleanup could not prove signal authority')
                            require(support.proc_identity(identity['pid'])['start_ticks']==identity['start_ticks'])
                            os.kill(identity['pid'],signal.SIGKILL)
                            raise AssertionError('Forced exact Graph SIGKILL; no clean-stop claim')
                        time.sleep(.01)
                        try:current=support.proc_identity(identity['pid'])
                        except FileNotFoundError:current=None
                    gone=current is None or current['start_ticks']!=identity['start_ticks']
                require(gone,'Owned Graph Provider remains after bound cleanup')
            except BaseException as error:graph_gone=False;fail('Graph_identity_gone',error)
        if report['status']=='passed'and len(graph_verified)!=1:graph_gone=False;fail('Graph_identity_coverage',AssertionError('Expected exactly one actual bound Graph worker'))
        database_retained=False
        if daemon_complete:
            def preserve_database():
                capacity=os.statvfs(output);require(capacity.f_bavail*capacity.f_frsize>=216*1024*1024,'Retain200MiB plus bounded16MiB raw DB guard')
                target=output/'runtime.raw.sqlite3';fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600);os.close(fd)
                with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as source_db,sqlite3.connect(target)as target_db:source_db.backup(target_db)
                require(target.stat().st_size<=16*1024*1024);report['observations']['retained_generated_private_database']={'path':str(target),'sha256':base.sha(target.read_bytes()),'bytes':target.stat().st_size,'mode':oct(target.stat().st_mode&0o777),'contains_only_generated_test_resources':True,'not_publish_without_generated_token_review':True}
                return True
            database_retained=base.cleanup_call(report,'private_raw_DB_after_owned_join',preserve_database,clean)is True
        if log is not None:
            log.close();p=output/'daemon.log'
            if token:p.write_bytes(p.read_bytes().replace(token.encode(),b'[redacted generated Owner token]'))
        if reaper is not None:base.cleanup_call(report,'controller_subreaper_restore',reaper.restore,clean)
        def unchanged_inputs():
            require(Path(__file__).read_bytes()==source and args.manifest.read_bytes()==manifest_raw)
            require(all(base.sha(Path(p).read_bytes())==digest for p,digest in old_support.items()))
            require(base.sha(raw_daemon.read_bytes())==manifest['binaries']['batond']and base.sha(raw_cli.read_bytes())==manifest['binaries']['baton'])
            require({str(p.relative_to(frontend)):base.sha(p.read_bytes())for p in frontend.rglob('*')if p.is_file()}==assets)
        base.cleanup_call(report,'immutable_inputs_after',unchanged_inputs,clean)
        # Reconcile every persisted session to a previously Core-bound process
        # record; an empty glob or daemon exit alone proves no descendant gone.
        session_coverage=False
        if daemon_complete:
            def confirm_session_coverage():
                with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as db:
                    db.execute('PRAGMA query_only=ON');db.execute('BEGIN')
                    sessions=[json.loads(raw)for raw, in db.execute('SELECT document FROM sessions')]
                    connections=[json.loads(raw)for raw, in db.execute('SELECT document FROM connections')]
                    bound={r['session_id']for r in verified}|{r['session_id']for r in graph_verified}
                    require({r['id']for r in sessions}==bound and all(r['state']=='stopped'for r in sessions))
                    require(all((r.get('version_evidence')or{}).get('process_reaped')is True for r in connections))
                    for query in ["SELECT count(*)FROM dispatch_intents WHERE state!='stopped'","SELECT count(*)FROM workspace_leases WHERE holder!=''","SELECT count(*)FROM factory_scoped_handles WHERE revoked=0"]:
                        require(db.execute(query).fetchone()[0]==0)
                    db.rollback()
                report['cleanup'].append({'action':'all_persisted_sessions_match_bound_stopped_processes','confirmed':True,'session_ids':sorted(bound)})
                return True
            session_coverage=base.cleanup_call(report,'private_stop_coverage_before_directory_cleanup',confirm_session_coverage,clean)is True
        states={'daemon_complete':daemon_complete,'factory_identities_complete':identities_complete,
            'http_complete':http_complete,'graph_complete':graph_gone,'raw_database_retained':database_retained,
            'session_coverage_confirmed':session_coverage,'business_passed':report['status']=='passed'and not report.get('failure')}
        if fixture_cleanup_permitted(**states):
            base.cleanup_call(report,'private_fixture_directory_cleanup_after_all_confirmed',lambda:shutil.rmtree(root),clean)
        else:
            retention={'root':str(root),'runtime':str(runtime),'source':str(root/'source'),
                'root_mode':oct(root.stat().st_mode&0o777),'cleanup_gate':states,
                'bound_factory_records':verified,'bound_graph_records':graph_verified,
                'automatic_finalizer':False,'reason':'Retain original private files/DB/PID evidence on failure or any unconfirmed stop; no clean-stop inference'}
            report['observations']['retained_private_fixture']=retention
            base.write_private_json(output/'retained-private-fixture.json',retention)
        planning.seal_runtime_report(report,daemon_complete,identities_complete and graph_gone,http_complete,clean)
        base.write_private_json(output/'report.json',json.loads(clean(json.dumps(report))))
    print(json.dumps({'status':report['status'],'report':str(output/'report.json'),'checks':len(report['checks']),'screenshots':len(report['screenshots'])}))
    return planning.runtime_exit(report)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument('--self-check',action='store_true')
    modes.add_argument('--run',action='store_true')
    parser.add_argument('--case',choices=['normal','lost-start','cancel'],default='normal')
    parser.add_argument('--tools',type=Path)
    parser.add_argument('--manifest',type=Path)
    parser.add_argument('--frontend',type=Path)
    parser.add_argument('--chromium',default='/usr/bin/chromium')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);out.chmod(0o700)
    if args.run:
        require(args.tools is not None and args.manifest is not None and args.frontend is not None)
        try:return runtime_driver(args,out)
        except BaseException as error:
            if not (out/'report.json').exists():base.write_private_json(out/'preflight-refusal.json',{'status':'preflight_refused','actual_runtime':'not_run','error':str(error),'traceback':traceback.format_exc()})
            raise
    value=self_check() if args.self_check else {'status':'pending_dependencies','actual_runtime':'not_run',
        'runtime_driver':'source_implemented_not_run','implemented_cases':['normal','lost-start','cancel'],'matrix':MATRIX,'pending':[
            'Core15 exact lifecycle success/failure/stop-chain fixtures and independent review',
            'Fresh immutable Core15 tools/source manifest and composed frontend assets',
            'Independent review of real browser driver; Root exclusive browser/daemon lease',
            'Additional permission/budget/unknown/adoption/recovery matrix beyond these three cases remains pending']}
    value.update(schema_required=15,harness_sha256=base.sha(Path(__file__).read_bytes()),
        support_sha256=base.sha(PLANNING_PATH.read_bytes()))
    base.write_private_json(out/('self-check.json' if args.self_check else 'NOT_RUN.json'),value)
    print(json.dumps(value));return 0 if args.self_check else 2


if __name__=='__main__': raise SystemExit(main())
