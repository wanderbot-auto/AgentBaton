#!/usr/bin/env python3
"""Schema15 built UI / fixed Owner bridges / actual Unix IPC / generated MCP5.

No OS Owner dialog, real model, account or generic Owner HTTP bridge. Running
without --run records NOT_RUN. --self-check starts no listener or subprocess.
"""
from __future__ import annotations

import argparse
import copy
from dataclasses import dataclass, field
from functools import partial
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import uuid

BASE_PATH = Path(__file__).with_name('factory-real-ipc.py')
_spec = importlib.util.spec_from_file_location('_child_planning_factory_support', BASE_PATH)
base = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = base
_spec.loader.exec_module(base)
FixedCall, Refusal, require = base.FixedCall, base.BoundaryRefusal, base.require
sha, safe_integer, unique_object = base.sha, base.safe_integer, base.unique_object
exact_json = base.support.exact_json_value
MAX_WIRE = 1_048_576
PLAN_BYTES = 262_144
ID = re.compile(r'[A-Za-z0-9._:-]{1,128}')
KEY = re.compile(r'[A-Za-z0-9:_-]{1,128}')
CHILD_COMMANDS = frozenset({'factory_child_plans_context', 'factory_child_plans_get',
    'factory_child_plans_list', 'factory_child_plans_submit'})
# Only this newly imported, private module instance uses schema15 prompt fields.
# No existing file/module object used by another harness is changed.
base.REQUEST_FIELDS = base.REQUEST_FIELDS | {'child_planning_context'}


@dataclass
class Bounds:
    project_id: str
    task_id: str
    scope_id: str = ''
    tasks: set[str] = field(default_factory=set)


def decode_envelope(value, bounds):
    require(type(value) is dict and set(value) <= {'command', 'args'} and 'command' in value)
    command, args = value['command'], value.get('args')
    if args is None: args = {}
    require(type(command) is str and type(args) is dict)
    if command not in CHILD_COMMANDS:
        # Existing exact fixed-read/start/cancel rules; approval still excluded.
        return base.decode_envelope(value, base.Bounds(bounds.project_id, bounds.task_id, bounds.scope_id))
    required = {'scopeId', 'taskId'}
    optional = {'planRevision'} if command.endswith('_get') else set()
    if command.endswith('_submit'):
        required |= {'planJson', 'expectedPlanRevision', 'expectedRevision', 'idempotencyKey'}
    require(set(args) >= required and set(args) <= required | optional)
    require(type(args['scopeId']) is str and args['scopeId'] == bounds.scope_id and bool(bounds.scope_id))
    require(type(args['taskId']) is str and args['taskId'] in bounds.tasks)
    params = {'scope_id': bounds.scope_id, 'task_id': args['taskId']}
    method = 'factory_child_plans.' + command.removeprefix('factory_child_plans_')
    if 'planRevision' in args:
        require(safe_integer(args['planRevision'], 1) and args['planRevision'] <= 256)
        params['plan_revision'] = args['planRevision']
    if command.endswith('_submit'):
        require(type(args['planJson']) is str and 0 < len(args['planJson'].encode('utf-8')) <= PLAN_BYTES)
        require(safe_integer(args['expectedPlanRevision']) and args['expectedPlanRevision'] <= 255)
        require(safe_integer(args['expectedRevision'], 1))
        require(type(args['idempotencyKey']) is str and KEY.fullmatch(args['idempotencyKey']))
        params.update(plan_json=args['planJson'], expected_plan_revision=args['expectedPlanRevision'])
        return FixedCall(method, params, args['idempotencyKey'], args['expectedRevision'])
    return FixedCall(method, params)


def decode_http(raw, *, origin, host, content_type, expected_origin, bounds):
    require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}', expected_origin) is not None)
    require(1 <= int(expected_origin.rsplit(':', 1)[1]) <= 65535)
    require(origin == expected_origin and host == expected_origin.removeprefix('http://'))
    require(content_type == 'application/json' and 0 < len(raw) <= MAX_WIRE)
    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
            parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
    except (ValueError, UnicodeError) as failure:
        raise Refusal(400, 'Complete bounded unique-key UTF8 JSON required') from failure
    return decode_envelope(value, bounds)


# Same GoalPlanProposal model as the actual Core-generated protocol fixture.
# No second plan language or local acceptance result is introduced.
PLAN_HELPER = r'''
def child_plan(ctx, revision):
 import copy
 binding=copy.deepcopy(ctx['execution_bindings'][0])
 outputs=[{'artifact_id':key,'contract_id':contract}for key,contract in ctx['outputs'].items()]
 required=sorted(set(rule for output in outputs for rule in ctx['contracts'][output['contract_id']]))
 inputs=[{'goal_task_id':ctx['goal']['task_id'],'artifact_id':key,'contract_id':value['contract_id'],'source':{'kind':'approved_external','sha256':value['sha256']}}for key,value in ctx['inputs'].items()]
 return {'schema_version':1,'plan_id':'plan-'+ctx['goal']['task_id'],'goal':copy.deepcopy(ctx['goal']),'scope_revision':ctx['scope_revision'],'revision':revision,'expected_revision':revision-1,'acceptance_rules':ctx['required_acceptance_rules'],'nodes':[{'id':'fixed-work','parent_id':None,'kind':'work','goal':'Plan only the exact approved outputs','dependencies':[],'workspace_id':binding['workspace_id'],'executor':{'kind':'connection','binding':binding,'program_sha256':ctx['program_sha256'][binding['connection_id']]},'recipients':[],'required_inputs':inputs,'outputs':outputs,'required_validators':required,'side_effect':'advisory_read_only','write_set':[],'budget':copy.deepcopy(ctx['budget']),'reservation':{'runtime_ms':1000,'output_bytes':1024,'tokens':None,'cost_microunits':None},'retry':{'max_attempts':1,'max_reworks':0,'retry_classes':[]},'idempotency_key':'fixed-plan-work','external_action_id':None}]}
def submit_plan(ctx, revision, key):
 p=child_plan(ctx,revision)
 args={'task_id':ctx['goal']['task_id'],'plan_json':json.dumps(p,indent=2)+'\n','expected_plan_revision':revision-1,'idempotency_key':key}
 value=tool('submit_child_plan',args)
 assert value['planning_only']is True and value['dispatched']is False and value['task_accepted']is False
 assert value['plan_json']==args['plan_json']and value['reference']['revision']==revision
 assert value['reference']['raw_sha256']==hashlib.sha256(args['plan_json'].encode()).hexdigest()
 assert tool('submit_child_plan',args)==value
 return value
'''


def generated_acp():
    text = base.GENERATED_ACP
    replacements = [
        ("mcp=None;mid=0", PLAN_HELPER + "\nmcp=None;mid=0"),
        ("['delegate_task','get_task_status','get_task_result','cancel_task']",
         "['delegate_task','get_task_status','get_task_result','cancel_task','submit_child_plan']"),
        ("'context_continuation','acceptance_state'}", "'context_continuation','acceptance_state','child_planning_context'}"),
        ("   if stage=='parent-initial':\n    child=tool", """   if stage=='parent-initial':
    ctx=c['child_planning_context'];assert ctx==status['child_planning_context']
    views=[submit_plan(ctx,1,'mcp-root-plan-one'),submit_plan(ctx,2,'mcp-root-plan-two')]
    fd=os.open('reports/root-plans.json',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w')as f:json.dump({'context':ctx,'views':views},f)
   if stage=='child':
    ctx=c['child_planning_context'];assert ctx['current_plan_revision']==1
    revised=submit_plan(ctx,2,'mcp-child-plan-two')
    fd=os.open('reports/child-plan.json',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w')as f:json.dump({'context':ctx,'view':revised},f)
   if stage=='parent-initial':
    child=tool"""),
        ("     assert len(c['handoff_results'])==1", """     assert c['handoff_result']['child_plan']['revision']==2
     import time
     until=time.monotonic()+15
     fd=os.open('reports/ui-parent-held',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
     with os.fdopen(fd,'w')as f:json.dump({'task_id':c['task_id'],'native_session_id':native_session_id,'child_plan':c['handoff_result']['child_plan']},f)
     while not os.path.exists('reports/ui-parent-release'):
      assert time.monotonic()<until,'UI did not release original bounded parent attempt'
      time.sleep(.005)
     assert len(c['handoff_results'])==1"""),
        ("    early=tool('get_task_result'", """    childctx=tool('get_task_status',{'task_id':child['task_id']})['child_planning_context']
    initial=submit_plan(childctx,1,'mcp-parent-plans-child-one')
    assert initial['caller_task_id']==c['task_id'] and initial['reference']['task_id']==child['task_id']
    early=tool('get_task_result'"""),
    ]
    for old, new in replacements:
        assert text.count(old) == 1, ('Inherited generated fixture shape changed', old)
        text = text.replace(old, new)
    compile(text, '<generated-account-free-planning-ACP>', 'exec')
    return text


def review_scope(proposal, selection, resources):
    """Lossless full review; retain new binding, reuse all old field checks."""
    require(proposal['kind'] == 'factory' and exact_json(proposal['snapshot']['selection'], selection))
    snapshot = proposal['snapshot']; binding = snapshot['bindings']['child_planning']
    require(set(binding) == {'schema_version', 'authority_kind', 'policy', 'owner_rule_id', 'input_rule_id',
        'validator_definitions', 'definition_digest', 'goal_execution_authorized', 'new_workspace_authorized'})
    require(binding['schema_version'] == 1 and binding['authority_kind'] == 'factory_child_planning_only')
    require(binding['policy'] == selection['child_planning_policy'] and binding['validator_definitions'] == {})
    require(binding['goal_execution_authorized'] is False and binding['new_workspace_authorized'] is False)
    require(binding['owner_rule_id'] == 'factory-child-owner-review' and binding['input_rule_id'] == 'factory-child-input-sha256')
    require(re.fullmatch('[a-f0-9]{64}', binding['definition_digest']))
    old_shape = copy.deepcopy(proposal); del old_shape['snapshot']['bindings']['child_planning']
    base.review_factory(old_shape, selection, resources['project'], resources['task'], resources['grant'],
        resources['connection'], resources['workspace'], resources['companion'])
    raw = json.dumps(proposal, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()
    require(len(raw) <= MAX_WIRE)
    require(len(json.dumps({'api_version':1,'request_id':'\0'*128,'result':proposal,'warnings':[]}, ensure_ascii=False).encode()) <= MAX_WIRE)
    return {'authority': 'simulated Native private Owner fixture, NOT OS dialog', 'proposal': proposal,
        'complete_review_bytes': len(raw), 'complete_review_sha256': sha(raw)}


def fingerprint_db(runtime):
    """Readonly, bounded exact rows; no Store::open/migration/recovery."""
    with sqlite3.connect('file:' + str(runtime / 'baton.sqlite3') + '?mode=ro', uri=True) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        assert db.execute('PRAGMA user_version').fetchone()[0] == 15
        names = {v[0] for v in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        fixed = {'execution_grants', 'managed_workspaces', 'tasks', 'attempts', 'sessions', 'factory_attempts', 'factory_runs',
            'factory_child_plan_heads', 'factory_child_plan_revisions', 'budgets', 'workspace_leases',
            'factory_budget_reservations', 'goal_ledgers', 'idempotency', 'events'}
        fixed |= base.support.LEGACY_GOAL_TABLES
        fixed |= set(base.support.FACTORY_RECORD_TABLES.values())
        assert fixed <= names, ('Missing required current tables', sorted(fixed - names))
        counts = base.support.current_factory_counts(db)
        assert counts['child_goal_runs'] == 0, 'Planning-only scope cannot dispatch a derived Goal'
        rows = {}; total = 0
        for name in sorted(fixed):
            count = db.execute('SELECT count(*) FROM "' + name + '"').fetchone()[0]
            assert count <= 1024
            values = db.execute('SELECT * FROM "' + name + '" ORDER BY rowid').fetchall()
            raw = json.dumps(values, separators=(',', ':'), allow_nan=False).encode()
            total += len(raw); assert total <= 8 * 1024 * 1024
            rows[name] = {'count': count, 'sha256': sha(raw)}
        db.rollback()
    return rows


def verify_plan(view, scope, task_id):
    ref = view['reference']; raw = view['plan_json']
    assert ref['scope_id'] == scope['id'] and ref['task_id'] == task_id
    assert ref['raw_sha256'] == sha(raw.encode()) and json.loads(raw) == view['proposal']
    assert view['proposal']['acceptance_rules'] == [scope['bindings']['child_planning']['owner_rule_id']]
    assert view['scope_digest'] == scope['contract_digest']
    assert view['planning_policy_digest'] == scope['bindings']['child_planning']['definition_digest']
    assert view['authority_kind'] == 'factory_child_planning_only'
    assert view['root_task_id'] == scope['root_task_id'] and view['root_grant_id'] == scope['root_grant_id']
    assert view['root_grant_revision'] == scope['root_grant_revision']
    assert view['input_revision'] == view['proposal']['goal']['input_revision']
    assert view['target_spec_revision'] == view['proposal']['goal']['spec_revision']
    assert view['proposal']['goal']['task_id'] == task_id
    assert safe_integer(ref['receipt_sequence'], 1) and safe_integer(ref['revision'], 1)
    assert view['planning_only'] is True and view['dispatched'] is False and view['task_accepted'] is False
    assert ref['revision'] == view['proposal']['revision'] == view['proposal']['expected_revision'] + 1


def verify_exact_recovery(original, recovered, scope, task_id):
    """Compare the immutable business receipt, not fresh transport UUIDs.

    Envelope revision is the current journal cursor, not the plan receipt
    sequence. Require it to cover the receipt and never regress on recovery.
    """
    for trace in (original, recovered):
        assert trace['method'] == 'factory_child_plans.submit'
        assert type(trace['key']) is str and KEY.fullmatch(trace['key'])
        assert safe_integer(trace['revision'], 1)
        params = trace['params']
        assert type(params) is dict and set(params) == {'scope_id','task_id','plan_json','expected_plan_revision'}
        assert params['scope_id'] == scope['id'] and params['task_id'] == task_id
        assert type(params['plan_json']) is str and safe_integer(params['expected_plan_revision'])
        envelope = trace['response']
        assert type(envelope) is dict and set(envelope) == {'request_id','result','revision','warnings'}
        assert type(envelope['request_id']) is str
        assert str(uuid.UUID(envelope['request_id'])) == envelope['request_id']
        assert safe_integer(envelope['revision'], 1) and exact_json(envelope['warnings'], [])
        verify_plan(envelope['result'], scope, task_id)
        view = envelope['result']
        assert view['plan_json'] == params['plan_json']
        assert view['reference']['revision'] == params['expected_plan_revision'] + 1
        assert envelope['revision'] >= view['reference']['receipt_sequence']
    assert exact_json(original['params'], recovered['params'])
    assert exact_json(original['key'], recovered['key']) and exact_json(original['revision'], recovered['revision'])
    assert exact_json(original['response']['result'], recovered['response']['result'])
    assert recovered['response']['revision'] >= original['response']['revision']
    return {'original_request_id':original['response']['request_id'],
        'recovered_request_id':recovered['response']['request_id'],
        'original_journal_cursor':original['response']['revision'],
        'recovered_journal_cursor':recovered['response']['revision'],
        'immutable_reference':original['response']['result']['reference'],
        'complete_result_typed_equal':True,'exact_request_key_params_CAS':True}


def record_http_abort(report, method, stage, error, clean=str):
    event = {'stage':stage,'method':method,'type':type(error).__name__,'message':clean(error)}
    reads = base.READS | {'factory_child_plans.context','factory_child_plans.get','factory_child_plans.list'}
    if isinstance(error, (BrokenPipeError, ConnectionResetError)) and stage == 'response_body' and method in reads:
        report['readonly_aborts'].append(event)
    else:
        report['http_unexpected_errors'].append(event)


def seal_runtime_report(report, daemon_complete, identities_complete, http_complete, clean=str):
    """Final decision only after handlers and all cleanup have completed."""
    if report['http_unexpected_errors']:
        base.note_failure(report, AssertionError('Unexpected HTTP handler errors after join'), 'http_final_error_check', clean)
    complete = all(value is True for value in (daemon_complete, identities_complete, http_complete))
    report['cleanup_complete'] = complete and report['status'] == 'passed' and not report.get('failure')
    if not report['cleanup_complete']:
        report['status'] = 'failed_or_interrupted'
    return runtime_exit(report)


def runtime_exit(report):
    # Defense in depth: a late error must never produce exit0 even if a caller
    # accidentally skips the final seal or presents an old cleanup flag.
    return 0 if (report['status'] == 'passed' and report.get('cleanup_complete') is True
        and not report.get('failure') and not report['http_unexpected_errors']) else 1


def next_raw(view, goal):
    p = copy.deepcopy(view['proposal']); p['revision'] += 1; p['expected_revision'] += 1
    p['nodes'][0]['goal'] = goal
    return json.dumps(p, ensure_ascii=False, indent=2) + '\n'


def self_check():
    b = Bounds('project', 'root', 'scope', {'root', 'child'}); checks = []
    def check(label, value, accepted):
        try:
            result = decode_envelope(value, b)
        except Refusal:
            assert not accepted, label
        else:
            assert accepted, label
            assert result.method in base.READS | base.MUTATIONS | {'factory_child_plans.context',
                'factory_child_plans.list', 'factory_child_plans.get', 'factory_child_plans.submit'}
        checks.append(label)
    for command in sorted(CHILD_COMMANDS):
        args = {'scopeId': 'scope', 'taskId': 'child'}
        if command.endswith('submit'):
            args.update(planJson='{}', expectedPlanRevision=0, expectedRevision=1, idempotencyKey='exact-key')
        check(command, {'command': command, 'args': args}, True)
        for key, value in [('scopeId', 'foreign'), ('taskId', 'foreign'), ('callerTaskId', 'root')]:
            check(command + ':' + key, {'command': command, 'args': {**args, key: value}}, False)
    good = {'command': 'factory_child_plans_submit', 'args': {'scopeId': 'scope', 'taskId': 'root',
        'planJson': '{}', 'expectedPlanRevision': 0, 'expectedRevision': 1, 'idempotencyKey': 'fixed'}}
    for key, value in [('expectedPlanRevision', True), ('expectedRevision', True), ('expectedPlanRevision', 256),
            ('expectedPlanRevision', -1), ('idempotencyKey', 'x' * 129), ('planJson', 'x' * (PLAN_BYTES + 1)),
            ('planJson', ''), ('token', 'forged')]:
        mutated = copy.deepcopy(good); mutated['args'][key] = value; check(key + ':' + str(type(value)), mutated, False)
    for command in ['factory_scope_approve', 'approvals_finalize', 'approvals.finalize', 'generic_ipc',
            'submit_child_plan', 'delegate_task', 'factory_scopes_propose', 'factory_scopes_revoke', 'sessions_create']:
        check('forbidden:' + command, {'command': command, 'args': {}}, False)
    for value in [False, 0, [], '']:
        check('nonobject-args:' + repr(value), {'command':'system_snapshot','args':value}, False)
    origin = 'http://127.0.0.1:42001'; raw = json.dumps(good).encode()
    for label, overrides in [('origin', {'origin': 'http://evil.invalid'}), ('host', {'host': 'localhost:42001'}),
            ('content', {'content_type': 'text/plain'}), ('oversize', {'raw': b'x' * (MAX_WIRE + 1)}),
            ('duplicate', {'raw': b'{"command":"system_snapshot","command":"approvals_finalize"}'}),
            ('nonfinite', {'raw': b'{"command":"system_snapshot","args":{"x":NaN}}'})]:
        kwargs = dict(raw=raw, origin=origin, host='127.0.0.1:42001', content_type='application/json', expected_origin=origin, bounds=b)
        kwargs.update(overrides)
        try: decode_http(**kwargs)
        except Refusal: pass
        else: raise AssertionError(label)
        checks.append('http:' + label)
    assert not exact_json({'child_planning':True},{'child_planning':1})
    assert not exact_json({'revision':1},{'revision':1.0})
    checks.extend(['private-Owner-bool-int-distinct','private-Owner-int-float-distinct'])
    exact=copy.deepcopy(good);exact['args']['expectedPlanRevision']=3
    exact_raw=json.dumps(exact)
    assert cp08_submission_request_hash(exact_raw,b,'{}','root',1)==sha(exact_raw.encode())
    malformed=[exact_raw.replace('"command":','"command":"factory_child_plans_submit","command":',1),
        exact_raw.replace('"expectedRevision": 1','"expectedRevision": 1,"expectedRevision": 1'),
        exact_raw.replace('"expectedRevision": 1','"expectedRevision": true'),
        exact_raw.replace('"expectedPlanRevision": 3','"expectedPlanRevision": 3.0'),
        exact_raw.replace('"idempotencyKey": "fixed"','"idempotencyKey": NaN'),
        json.dumps({**exact,'extra':True}),
        json.dumps({**exact,'args':{**exact['args'],'extra':True}}),
        ' '*(MAX_WIRE+1),exact_raw+' trailing']
    for raw in malformed:
        assert cp08_submission_request_hash(raw,b,'{}','root',1) is None
    checks.append('CP08-exact-request-rejects-duplicate-extra-bool-float-nonfinite-and-oversize')
    program = generated_acp(); assert 'submit_child_plan' in program
    checks.append('generated-ACP-compiled-not-executed')
    return {'status': 'passed', 'checks': checks, 'count': len(checks),
        'dimension': 'pure fixed-boundary/shape controls and generated Python syntax; NO Core/browser/OS approval/process execution'}


def fixture_self_check(path):
    raw, pin = base.bounded_file(path, 1_048_576, path.parent)
    value = json.loads(raw, object_pairs_hook=unique_object)
    scope = value['scope']; rows = value['rows']; checks = []
    assert len(rows) == 4
    assert scope['selection']['capabilities']['child_planning'] is True
    assert scope['selection']['capabilities']['child_goal_execution'] is False
    assert scope['selection']['capabilities']['create_workspace'] is False
    assert scope['bindings']['approved_grant']['scope']['max_children'] == 0
    for i, view in enumerate(rows):
        verify_plan(view, scope, view['reference']['task_id']); checks.append('actual-row-' + str(i))
    assert sorted(v['reference']['revision'] for v in rows) == [1, 1, 2, 2]
    for label, mutate in [
        ('raw-bytes', lambda v: v.__setitem__('plan_json', v['plan_json'] + ' ')),
        ('scope', lambda v: v['reference'].__setitem__('scope_id', 'foreign')),
        ('policy', lambda v: v.__setitem__('planning_policy_digest', '0' * 64)),
        ('task', lambda v: v['reference'].__setitem__('task_id', 'foreign')),
        ('dispatched', lambda v: v.__setitem__('dispatched', True)),
        ('accepted', lambda v: v.__setitem__('task_accepted', True)),
        ('revision', lambda v: v['proposal'].__setitem__('revision', 99)),
    ]:
        bad = copy.deepcopy(rows[0]); mutate(bad)
        try: verify_plan(bad, scope, rows[0]['reference']['task_id'])
        except AssertionError: pass
        else: raise AssertionError('Corrupt evidence accepted: ' + label)
        checks.append('corruption:' + label)
    return {'status':'passed','checks':checks,'count':len(checks),'actual_fixture_path':str(path),
        'actual_fixture_pin':pin,'dimension':'pure checks of Core-author actual recorded MCP/IPC DTOs plus synthetic corruption; NO new Core/UI/browser execution'}


def review_self_check(path):
    """Finite receipt/late-cleanup controls; no Core or browser execution."""
    raw, pin = base.bounded_file(path, MAX_WIRE, path.parent)
    fixture = json.loads(raw, object_pairs_hook=unique_object); scope = fixture['scope']
    # Derive a full synthetic Owner receipt from the recorded same-model view.
    # This modified object is NOT represented as an actually stored Owner row.
    view = copy.deepcopy(fixture['rows'][0]); task_id = view['reference']['task_id']
    view.update(caller_task_id=task_id,caller_session_id=None,caller_attempt_id=None,caller_fencing_token=None)
    original = {'method':'factory_child_plans.submit','key':'exact-recovery-control','revision':scope['revision'],
        'params':{'scope_id':scope['id'],'task_id':task_id,'plan_json':view['plan_json'],
            'expected_plan_revision':view['reference']['revision']-1},
        'response':{'request_id':'11111111-1111-4111-8111-111111111111','result':view,
            'revision':view['reference']['receipt_sequence']+10,'warnings':[]}}
    recovered=copy.deepcopy(original);recovered['response']['request_id']='22222222-2222-4222-8222-222222222222'
    checks=[]
    for label,cursor_add in [('same-result-new-request-id',0),('same-result-forward-journal-cursor',1)]:
        changed=copy.deepcopy(recovered);changed['response']['revision']+=cursor_add
        result=verify_exact_recovery(original,changed,scope,task_id)
        assert result['complete_result_typed_equal'] is True
        assert result['original_request_id']!=result['recovered_request_id']
        checks.append(label)
    def put(path,value):
        def mutate(row):
            target=row
            for key in path[:-1]:target=target[key]
            target[path[-1]]=value
        return mutate
    changes=[
        ('key',put(['key'],'new-key')),
        ('request-CAS',put(['revision'],scope['revision']+1)),
        ('request-CAS-bool',put(['revision'],True)),
        ('method',put(['method'],'factory_runs.start')),
        ('request-raw',put(['params','plan_json'],view['plan_json']+' ')),
        ('expected-plan-CAS',put(['params','expected_plan_revision'],99)),
        ('expected-plan-CAS-bool',put(['params','expected_plan_revision'],False)),
        ('business-raw',put(['response','result','plan_json'],view['plan_json']+' ')),
        ('business-ref-id',put(['response','result','reference','id'],'foreign')),
        ('business-ref-revision',put(['response','result','reference','revision'],99)),
        ('business-ref-sequence',put(['response','result','reference','receipt_sequence'],1)),
        ('business-plan-digest',put(['response','result','reference','plan_digest'],'0'*64)),
        ('business-raw-hash',put(['response','result','reference','raw_sha256'],'0'*64)),
        ('business-authority',put(['response','result','authority_kind'],'foreign')),
        ('business-scope-digest',put(['response','result','scope_digest'],'0'*64)),
        ('business-policy-digest',put(['response','result','planning_policy_digest'],'0'*64)),
        ('business-proposal-bool-alias',put(['response','result','proposal','schema_version'],True)),
        ('business-proposal-float-alias',put(['response','result','proposal','schema_version'],1.0)),
        ('business-caller',put(['response','result','caller_task_id'],'foreign')),
        ('business-planning-only',put(['response','result','planning_only'],1)),
        ('business-dispatched',put(['response','result','dispatched'],True)),
        ('business-accepted',put(['response','result','task_accepted'],True)),
        ('envelope-bool-cursor',put(['response','revision'],True)),
        ('envelope-regressed-cursor',put(['response','revision'],original['response']['revision']-1)),
        ('envelope-before-receipt',put(['response','revision'],view['reference']['receipt_sequence']-1)),
        ('envelope-warnings',put(['response','warnings'],['unexpected'])),
        ('envelope-warning-type',put(['response','warnings'],{})),
        ('envelope-bad-request-id',put(['response','request_id'],'not-a-UUID')),
        ('envelope-extra-field',put(['response','error'],None)),
    ]
    for label,mutate in changes:
        bad=copy.deepcopy(recovered);mutate(bad)
        try:verify_exact_recovery(original,bad,scope,task_id)
        except (AssertionError,ValueError):pass
        else:raise AssertionError('Recovery corruption accepted: '+label)
        checks.append('reject:'+label)
    def fresh():return {'status':'passed','http_unexpected_errors':[],'readonly_aborts':[]}
    report=fresh()
    record_http_abort(report,'factory_child_plans.submit','response_body',BrokenPipeError('synthetic late mutation abort'))
    assert seal_runtime_report(report,True,True,True)==1
    assert report['status']=='failed_or_interrupted' and report['cleanup_complete'] is False
    assert report['http_unexpected_errors'] and report['failure']['stage']=='http_final_error_check'
    assert runtime_exit(report)==1
    checks.append('postjoin-mutation-error-report-and-exit-fail')
    report=fresh();report['cleanup_complete']=True
    report['http_unexpected_errors'].append('synthetic error after an earlier good flag')
    assert runtime_exit(report)==1
    checks.append('stale-good-flag-cannot-hide-late-error')
    report=fresh();record_http_abort(report,'factory_child_plans.get','response_body',BrokenPipeError('synthetic readonly body abort'))
    assert len(report['readonly_aborts'])==1 and not report['http_unexpected_errors']
    assert seal_runtime_report(report,True,True,True)==0
    checks.append('defined-readonly-body-abort-separate')
    report=fresh();record_http_abort(report,'factory_child_plans.get','core_call',BrokenPipeError('synthetic Core socket failure'))
    assert seal_runtime_report(report,True,True,True)==1
    checks.append('readonly-core-socket-failure-not-UI-body-abort')
    report=fresh();report['http']=[{'actual_committed_reply_deliberately_dropped':True}]
    assert seal_runtime_report(report,True,True,True)==0
    checks.append('intentional-CP08-drop-is-not-unexpected-error')
    report=fresh();assert seal_runtime_report(report,True,True,False)==1
    checks.append('unjoined-HTTP-never-passes')
    return {'status':'passed','count':len(checks),'checks':checks,'actual_fixture_pin':pin,
        'dimension':'pure finite recovery/cleanup controls using a synthetic Owner receipt derived from recorded Core model; NO Core/browser/socket/thread execution'}


def require_fixed_field(descriptor, caption, tag):
    assert tag in {'select','textarea'}
    assert type(descriptor) is dict and set(descriptor)=={'caption','direct_controls'}
    assert descriptor['caption']==caption and descriptor['direct_controls']==[tag]


def fixed_form_control(container, caption, tag):
    # A wrapping label's text includes its dynamic <option> text in Playwright's
    # get_by_label text selector. Match candidates, then verify the complete
    # static caption and exactly one direct control on a detached clone.
    labels=container.locator('label').filter(has_text=re.compile(re.escape(caption)))
    labels.wait_for(state='attached')
    assert labels.count()==1, ('Expected exactly one fixed field',caption,labels.count())
    descriptor=labels.evaluate("""label=>{const copy=label.cloneNode(true);copy.querySelectorAll('input,select,textarea').forEach(e=>e.remove());return {caption:copy.textContent.trim(),direct_controls:[...label.children].filter(e=>['INPUT','SELECT','TEXTAREA'].includes(e.tagName)).map(e=>e.tagName.toLowerCase())};}""")
    require_fixed_field(descriptor,caption,tag)
    control=labels.locator(':scope > '+tag)
    assert control.count()==1
    return control


def install_submission_probe(page, task_id):
    """Observe current DOM immediately; never wait, invoke or retry a click."""
    return page.evaluate_handle("""taskId=>{
      const regions=[...document.querySelectorAll('[data-testid="factory-runtime"] section[aria-label="Child task plans"]')].filter(region=>region.parentElement?.querySelector(':scope > summary')?.textContent==='Child task plans · '+taskId);
      if(regions.length!==1)throw new Error('DiagnosticRegionMissing');
      const region=regions[0],buttons=[...region.children].filter(e=>e.tagName==='BUTTON'&&e.textContent==='Store next plan revision');
      if(buttons.length!==1)throw new Error('DiagnosticButtonMissing');
      const button=buttons[0],events=[],errors=[];let omitted=0,last='';
      const takeSnapshot=kind=>{
        const direct=[...region.children].filter(e=>e.tagName==='P');
        const state={disabled:button.disabled,connected:button.isConnected,
          head:direct.find(e=>e.textContent.startsWith('Current plan head:'))?.textContent.slice(0,200)??null,
          alerts:direct.filter(e=>e.getAttribute('role')==='alert').slice(0,4).map(e=>e.textContent.slice(0,400)),
          pending:[...region.querySelectorAll('button')].some(e=>e.textContent==='Recover the exact plan submission'),
          draft_chars:region.querySelector(':scope > label > textarea')?.value.length??null};
        const identity=JSON.stringify(state);if(kind==='mutation'&&identity===last)return;last=identity;
        if(events.length<32)events.push({kind,utc_ms:Date.now(),...state});else ++omitted;
      };
      const snapshot=kind=>{try{takeSnapshot(kind);}catch(error){if(errors.length<4)errors.push({stage:kind,type:String(error?.name??'Error').slice(0,64)});}};
      const onEvent=event=>{if(event.target instanceof Element&&event.target.closest('button')===button){snapshot(event.type);queueMicrotask(()=>snapshot(event.type+'-after'));}};
      const names=['pointerdown','pointerup','click'];names.forEach(name=>region.addEventListener(name,onEvent,true));
      const observer=new MutationObserver(()=>snapshot('mutation'));
      observer.observe(region,{subtree:true,attributes:true,childList:true,characterData:true});
      const probe={events,errors,get omitted(){return omitted;},finish(){try{snapshot('finished');}finally{observer.disconnect();names.forEach(name=>region.removeEventListener(name,onEvent,true));}}};
      snapshot('armed');return probe;
    }""", task_id)


def finish_submission_probe(handle):
    # The handle holds only the diagnostic closure. Even a detached/replaced
    # button is inspected directly, never resolved again through a Locator.
    return handle.evaluate("""probe=>{
      probe.finish();return {events:probe.events,omitted:probe.omitted,errors:probe.errors};
    }""")


def cp08_submission_request_hash(raw, bounds, raw4, task_id, scope_revision):
    """Closed passive match; never interpret duplicate keys as an exact call."""
    if type(raw) is not str or not 0<len(raw)<=MAX_WIRE: return None
    try:
        encoded=raw.encode('utf-8')
        if len(encoded)>MAX_WIRE: return None
        envelope=json.loads(raw,object_pairs_hook=unique_object,
            parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
        if type(envelope) is not dict or set(envelope)!={'command','args'}: return None
        call=decode_envelope(envelope,bounds)
    except (ValueError,UnicodeError,Refusal,RecursionError): return None
    if (call.method!='factory_child_plans.submit' or call.revision!=scope_revision
            or call.params!={'scope_id':bounds.scope_id,'task_id':task_id,
                'plan_json':raw4,'expected_plan_revision':3}): return None
    return sha(encoded)


def capture_browser_failure(browser, output, report, clean=str):
    if browser is None:return
    try:page=browser.contexts[0].pages[0]
    except BaseException as error:
        report['failure_capture_error']=clean(error);return
    try:
        path=output/'failure-current-ui.png';page.screenshot(path=str(path))
        report['screenshots'].append({'path':str(path),'sha256':sha(path.read_bytes())})
    except BaseException as error:report['failure_capture_error']=clean(error)
    try:
        raw=clean(page.content()).encode('utf-8')
        assert len(raw)<=4*MAX_WIRE,'Controlled failure DOM exceeds4MiB capture cap'
        path=output/'failure-current-ui.html'
        with path.open('xb')as writer:writer.write(raw)
        report['failure_dom']={'path':str(path),'sha256':sha(raw),'bytes':len(raw),
            'dimension':'controlled generated-fixture UI DOM; not Native OS evidence','redaction':'generated Owner token replaced if present'}
    except BaseException as error:report['failure_dom_capture_error']=clean(error)


def close_browser_engine(browser, playwright, report, clean=str):
    if browser is not None:base.cleanup_call(report,'browser_close',browser.close,clean)
    if playwright is not None:base.cleanup_call(report,'playwright_stop',playwright.stop,clean)


def locator_self_check(previous_report, output):
    """Finite tests of field uniqueness and failure capture ordering, no browser."""
    raw,pin=base.bounded_file(previous_report,4*MAX_WIRE,previous_report.parent)
    previous=json.loads(raw,object_pairs_hook=unique_object)
    assert previous['harness_sha256']=='20bcfb1a4dfc77b15ad70ab1586ce38f94b38ae857e1b66f25c4df21e28da0e5'
    assert previous['checks']==[] and previous['cleanup_complete'] is False
    assert previous['failure']['type']=='TimeoutError' and 'Existing root Grant' in previous['failure']['message']
    assert 'Event loop is closed' in previous['failure_capture_error']
    checks=['actual-run1-timeout-and-closed-context-evidence-retained']
    # The same wrapping-label structure used by FactoryRuntime.tsx. The old
    # exact-label string includes dynamic option text, while static clone text
    # excludes controls. This is a finite model, NOT a new DOM/browser run.
    fields=[('Existing root Grant','grants.list','id'),('Existing connections','connections.list','name'),
        ('Existing workspaces','workspaces.list','root')]
    class Control:
        def count(self):return 1
    class Labels:
        def __init__(self,descriptor,count=1):self.descriptor=descriptor;self.n=count
        def filter(self,has_text):assert has_text.search(self.descriptor['caption']);return self
        def count(self):return self.n
        def wait_for(self,state):assert state=='attached'
        def evaluate(self,script):
            assert 'cloneNode(true)' in script and 'e.remove()' in script
            return self.descriptor
        def locator(self,selector):assert selector==':scope > select';return Control()
    class Container:
        def __init__(self,labels):self.labels=labels
        def locator(self,selector):assert selector=='label';return self.labels
    for caption,method,key in fields:
        row=next(r['response']['result'][0] for r in previous['http'] if r['method']==method)
        complete_label=caption+' — '+str(row[key]);assert complete_label!=caption
        descriptor={'caption':caption,'direct_controls':['select']}
        assert isinstance(fixed_form_control(Container(Labels(descriptor)),caption,'select'),Control)
        checks.append('dynamic-option-exact-caption:'+caption)
    for label,descriptor,count in [
        ('zero-fields',{'caption':'Existing root Grant','direct_controls':['select']},0),
        ('ambiguous-fields',{'caption':'Existing root Grant','direct_controls':['select']},2),
        ('wrong-static-caption',{'caption':'Existing root Grant expanded','direct_controls':['select']},1),
        ('wrong-control',{'caption':'Existing root Grant','direct_controls':['textarea']},1),
        ('two-direct-controls',{'caption':'Existing root Grant','direct_controls':['select','select']},1),
        ('nested-only-control',{'caption':'Existing root Grant','direct_controls':[]},1),
    ]:
        try:fixed_form_control(Container(Labels(descriptor,count)),'Existing root Grant','select')
        except AssertionError:pass
        else:raise AssertionError('Field ambiguity accepted: '+label)
        checks.append('reject:'+label)
    require_fixed_field({'caption':'Step goal fixed-work','direct_controls':['textarea']},'Step goal fixed-work','textarea')
    checks.append('exact-editor-textarea-caption')
    from types import SimpleNamespace
    state={'active':True,'events':[]}
    class Page:
        def screenshot(self,path):
            if not state['active']:raise RuntimeError('Event loop is closed! synthetic old ordering')
            state['events'].append('capture');Path(path).write_bytes(b'synthetic unit bytes, NOT an actual browser PNG')
        def content(self):
            if not state['active']:raise RuntimeError('Event loop is closed! synthetic old ordering')
            state['events'].append('capture_dom');return '<p>synthetic unit DOM, NOT a browser capture</p>'
    class Browser:
        contexts=[SimpleNamespace(pages=[Page()])]
        def close(self):assert state['active'];state['events'].append('close')
    class Engine:
        def stop(self):state['events'].append('stop');state['active']=False
    old={'screenshots':[],'status':'failed_or_interrupted'};Engine().stop()
    capture_browser_failure(Browser(),output,old)
    assert 'Event loop is closed' in old['failure_capture_error'] and old['screenshots']==[]
    checks.append('old-stop-before-capture-model-refuses-screenshot')
    state.update(active=True,events=[]);current={'screenshots':[],'status':'failed_or_interrupted'}
    capture_browser_failure(Browser(),output,current)
    close_browser_engine(Browser(),Engine(),current)
    assert state['events']==['capture','capture_dom','close','stop'] and len(current['screenshots'])==1
    assert 'failure_capture_error' not in current and 'failure' not in current
    checks.append('capture-and-close-before-engine-stop')
    state.update(active=True,events=[])
    class BrokenBrowser(Browser):
        def close(self):raise RuntimeError('synthetic close error')
    report={'screenshots':[],'status':'passed'}
    close_browser_engine(BrokenBrowser(),Engine(),report)
    assert report['failure']['stage']=='browser_close' and state['events']==['stop']
    checks.append('close-error-retained-engine-still-stopped')
    import ast
    tree=ast.parse(Path(__file__).read_text());fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
    assert not any(isinstance(n,ast.With) and 'sync_playwright' in ast.unparse(n.items[0].context_expr) for n in ast.walk(fn))
    body=next(n for n in fn.body if isinstance(n,ast.Try))
    assert 'capture_browser_failure(browser, output, report, clean)' in ast.unparse(body.handlers[0])
    assert 'close_browser_engine(browser, playwright, report, clean)' in ast.unparse(ast.Module(body=body.finalbody,type_ignores=[]))
    checks.append('actual-runtime-AST-active-engine-until-finally')
    return {'status':'passed','count':len(checks),'checks':checks,'previous_actual_report_pin':pin,
        'synthetic_capture':current['screenshots'],'dimension':'actual prior failure evidence plus finite fake locator/engine controls and source AST; NO new browser/Core/IPC run'}


def summary_self_check(previous_report):
    """The observed nested-summary ambiguity, without a browser retry."""
    import ast
    import xml.etree.ElementTree as ET
    raw,pin=base.bounded_file(previous_report,4*MAX_WIRE,previous_report.parent)
    previous=json.loads(raw,object_pairs_hook=unique_object)
    assert previous['harness_sha256']=='613ebf871d5d98510cddd5dfad5e595a69f0186ac4ad47d21ac7aab28487adeb'
    assert [c['id']for c in previous['checks']]==['CP01','CP02','CP03']
    assert 'strict mode violation' in previous['failure']['message']
    assert 'resolved to 2 elements' in previous['failure']['message']
    assert 'Selected recorded revision: r1' in previous['failure']['message']
    assert 'Plan reference and receipt' in previous['failure']['message']
    assert previous['cleanup_complete'] is False and len(previous['screenshots'])==2
    checks=['actual-run2-strict-two-summary-failure-preserved']
    tree=ET.fromstring('<details><summary>Selected recorded revision: r1</summary><p>date</p><details><summary>Plan reference and receipt</summary><pre>ref</pre></details><pre>original plan</pre></details>')
    assert len(tree.findall('.//summary'))==2
    checks.append('old-descendant-summary-model-is-ambiguous')
    def exact_direct(node):
        rows=node.findall('./summary')
        assert len(rows)==1 and rows[0].text=='Selected recorded revision: r1'
        return rows[0]
    assert exact_direct(tree).text=='Selected recorded revision: r1'
    checks.append('direct-summary-model-selects-only-recorded-revision')
    foreign=copy.deepcopy(tree);foreign.find('./summary').text='Selected recorded revision: r2'
    duplicate=copy.deepcopy(tree);ET.SubElement(duplicate,'summary').text='Selected recorded revision: r1'
    for name,bad in [('foreign-caption',foreign),('duplicate-direct-summary',duplicate)]:
        try:exact_direct(bad)
        except AssertionError:pass
        else:raise AssertionError('Ambiguous or foreign direct summary accepted')
        checks.append('reject:'+name)
    fn=next(n for n in ast.parse(Path(__file__).read_text()).body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
    calls=[n for n in ast.walk(fn)if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='locator'and 'Selected recorded revision: r1' in ast.unparse(n.func.value)]
    assert len(calls)==1 and ast.literal_eval(calls[0].args[0])==':scope > summary'
    checks.append('actual-runtime-AST-uses-direct-summary-at-exact-revision')
    pending=ET.fromstring('<section><p role="alert">Submission outcome unknown. Read history.</p><p>Submission outcome unknown. Read history.</p><p role="alert">History changed while editing.</p><button>Recover the exact plan submission</button></section>')
    assert len([e for e in pending.findall('./p')if e.text.startswith('Submission outcome unknown.')])==2
    assert len(pending.findall('./p[@role="alert"]'))==2
    assert len([e for e in pending.findall('./p[@role="alert"]')if e.text.startswith('Submission outcome unknown.')])==1
    checks.append('pending-text-two-render-sites-one-alert-model')
    source=ast.unparse(fn)
    pending_calls=[n for n in ast.walk(fn)if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='to_contain_text'and n.args and isinstance(n.args[0],ast.Constant)and n.args[0].value=='Submission outcome unknown.']
    assert len(pending_calls)==1 and ':scope > p[role="alert"]' in ast.unparse(pending_calls[0])
    checks.append('actual-runtime-pending-assertion-is-unique-alert')
    assert "fixed_form_control(root_region, 'Complete GoalPlan JSON draft', 'textarea')" in source
    checks.append('actual-runtime-draft-uses-exact-static-field-check')
    return {'status':'passed','count':len(checks),'checks':checks,'previous_actual_report_pin':pin,
        'dimension':'actual prior browser failure plus finite XML descendant/direct-child model and source AST; NO new browser/Core/IPC execution'}


def alert_self_check(previous_report, output):
    """Observed parent-error versus nested-editor warning, no browser retry."""
    import ast
    import xml.etree.ElementTree as ET
    raw,pin=base.bounded_file(previous_report,4*MAX_WIRE,previous_report.parent)
    previous=json.loads(raw,object_pairs_hook=unique_object)
    assert previous['harness_sha256']=='94a72464698468d1196a786df9650a72c11f991f441aa9e9206de7a48a75bae5'
    assert [c['id']for c in previous['checks']]==['CP01','CP02','CP03','CP04','CP05']
    assert 'strict mode violation' in previous['failure']['message'] and 'resolved to 2 elements' in previous['failure']['message']
    assert 'Plan submission refused: limit' in previous['failure']['message']
    assert 'The JSON draft or latest stored version changed.' in previous['failure']['message']
    assert len(previous['screenshots'])==4 and previous['cleanup_complete'] is False
    checks=['actual-run3-refusal-plus-editor-warning-failure-preserved']
    tree=ET.fromstring('<section><p role="alert">Plan submission refused: limit</p><details><summary>editor</summary><p role="alert">The JSON draft or latest stored version changed.</p></details><p role="alert">History changed while editing.</p></section>')
    assert len(tree.findall('.//p[@role="alert"]'))==3
    checks.append('old-descendant-all-alerts-is-ambiguous-model')
    def exact_refusal(node):
        candidates=[e for e in node.findall('./p[@role="alert"]')if re.search(r'^Plan submission refused(?::|\s|$)',e.text or '')]
        assert len(candidates)==1
        return candidates[0]
    assert exact_refusal(tree).text=='Plan submission refused: limit'
    checks.append('direct-refusal-only-despite-editor-and-stale-warnings')
    absent=copy.deepcopy(tree);absent.remove(absent.find('./p'))
    nested=copy.deepcopy(absent);nested.find('./details/p').text='Plan submission refused: nested-only'
    duplicate=copy.deepcopy(tree);ET.SubElement(duplicate,'p',role='alert').text='Plan submission refused: second'
    foreign=copy.deepcopy(tree);foreign.find('./p').text='Plan submission refusedness'
    for label,bad in [('no-root-refusal',absent),('nested-only-refusal',nested),('duplicate-root-refusals',duplicate),('foreign-prefix',foreign)]:
        try:exact_refusal(bad)
        except AssertionError:pass
        else:raise AssertionError('Missing or ambiguous refusal accepted: '+label)
        checks.append('reject:'+label)
    fn=next(n for n in ast.parse(Path(__file__).read_text()).body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
    exact_calls=[n for n in ast.walk(fn)if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='to_contain_text'and n.args and isinstance(n.args[0],ast.Constant)and n.args[0].value=='Plan submission refused']
    assert len(exact_calls)==1
    source=ast.unparse(exact_calls[0])
    assert ':scope > p[role="alert"]' in source and '^Plan submission refused' in source
    assert 'get_by_role' not in source and '.nth(' not in source
    checks.append('actual-runtime-uses-unique-direct-refusal-alert')
    unknown=ET.fromstring('<section><p role="alert">Submission outcome unknown. Read history.</p><p>Submission outcome unknown. Read history.</p><details><p role="alert">The JSON draft changed.</p></details><p role="alert">History changed while editing.</p></section>')
    def exact_unknown(node):
        rows=[e for e in node.findall('./p[@role="alert"]')if re.search(r'^Submission outcome unknown\.(?:\s|$)',e.text or '')]
        assert len(rows)==1;return rows[0]
    assert exact_unknown(unknown).text.startswith('Submission outcome unknown.')
    checks.append('direct-unknown-only-with-pending-stale-and-editor-warning')
    nested_unknown=ET.fromstring('<section><details><p role="alert">Submission outcome unknown. Nested only.</p></details></section>')
    duplicate_unknown=copy.deepcopy(unknown);ET.SubElement(duplicate_unknown,'p',role='alert').text='Submission outcome unknown. duplicate'
    for label,bad in [('nested-only-unknown',nested_unknown),('duplicate-root-unknown',duplicate_unknown)]:
        try:exact_unknown(bad)
        except AssertionError:pass
        else:raise AssertionError('Missing or ambiguous unknown accepted:'+label)
        checks.append('reject:'+label)
    cleared=ET.fromstring('<section><details><p role="alert">The JSON draft changed.</p></details></section>')
    assert len(cleared.findall('./p[@role="alert"]'))==0 and len(cleared.findall('.//p[@role="alert"]'))==1
    checks.append('cleared-root-errors-do-not-erase-legitimate-nested-warning')
    source=ast.unparse(fn)
    assert "expect(root_region.get_by_text('Submission outcome unknown.', exact=False)).to_have_count(0)" in source
    assert "expect(root_region.locator(':scope > p[role=\"alert\"]')).to_have_count(0)" in source
    assert not any(isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='get_by_role'and n.args and isinstance(n.args[0],ast.Constant)and n.args[0].value=='alert' for n in ast.walk(fn))
    checks.append('original-global-pending-zero-retained-plus-direct-error-zero')
    from types import SimpleNamespace
    class FakePage:
        def __init__(self,dom,bad_screenshot=False):self.dom=dom;self.bad_screenshot=bad_screenshot
        def screenshot(self,path):
            if self.bad_screenshot:raise RuntimeError('synthetic screenshot failure')
            Path(path).write_bytes(b'synthetic unit bytes, NOT a browser PNG')
        def content(self):return self.dom
    def capture_case(name,page,clean=str):
        directory=output/name;directory.mkdir(mode=0o700)
        browser=SimpleNamespace(contexts=[SimpleNamespace(pages=[page])]);result={'screenshots':[]}
        capture_browser_failure(browser,directory,result,clean);return result
    bounded=capture_case('unit-redacted-dom',FakePage('<p>generated-token-unit</p>'),lambda value:str(value).replace('generated-token-unit','[redacted]'))
    saved=Path(bounded['failure_dom']['path']).read_text();assert 'generated-token-unit' not in saved and '[redacted]' in saved
    checks.append('bounded-controlled-DOM-redacts-generated-token')
    over=capture_case('unit-overlimit-dom',FakePage('x'*(4*MAX_WIRE+1)))
    assert len(over['screenshots'])==1 and 'failure_dom_capture_error' in over and 'failure_dom' not in over
    assert not (output/'unit-overlimit-dom/failure-current-ui.html').exists()
    checks.append('overlimit-DOM-refused-without-file-screenshot-retained')
    independent=capture_case('unit-independent-dom',FakePage('<p>synthetic preserved DOM</p>',True))
    assert 'failure_capture_error' in independent and independent['failure_dom']['bytes']>0 and independent['screenshots']==[]
    checks.append('screenshot-failure-does-not-suppress-DOM-artifact')
    return {'status':'passed','count':len(checks),'checks':checks,'previous_actual_report_pin':pin,
        'dimension':'actual prior failure plus finite XML nested/direct-alert model and source AST; NO new browser/Core/IPC execution'}


def sql_self_check(previous_report):
    """Actual retained14 rows on the exact Core events DDL; no daemon rerun."""
    import ast
    raw,pin=base.bounded_file(previous_report,4*MAX_WIRE,previous_report.parent)
    previous=json.loads(raw,object_pairs_hook=unique_object)
    assert previous['harness_sha256']=='c1fc95300144eda6d227f34925dbfa78d0f0504fc992b4c4be2cbb1ec3cc7157'
    assert [c['id']for c in previous['checks']]==['CP01','CP02','CP03','CP04','CP05','CP06','CP07','CP08']
    assert previous['failure']['type']=='OperationalError' and previous['failure']['message']=='no such column: kind'
    assert previous['cleanup_complete'] is False and len(previous['screenshots'])==5
    checks=['actual-run4-no-kind-column-failure-preserved']
    diagnostics=previous['observations']['final_diagnostics'];path=Path(diagnostics['path'])
    observed,_=base.bounded_file(path,16*MAX_WIRE,path.parent)
    assert sha(observed)==diagnostics['sha256']
    data=json.loads(observed,object_pairs_hook=unique_object);assert data['schema']==14 and data['complete'] is True
    rows=data['tables']['events']['rows'];assert all(set(r)=={'sequence','document'}for r in rows)
    # Exact production initial storage DDL, unchanged by schema14 migrations.
    # Provenance/immutable source blob is recorded separately in SQL_DDL_SOURCE.
    ddl='CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL)'
    fn=next(n for n in ast.parse(Path(__file__).read_text()).body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
    queries=[n.args[0].value for n in ast.walk(fn)if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='execute'and n.args and isinstance(n.args[0],ast.Constant)and isinstance(n.args[0].value,str)and "session.stopped" in n.args[0].value]
    assert queries==["SELECT document FROM events WHERE json_extract(document,'$.kind')='session.stopped'"]
    query=queries[0]
    db=sqlite3.connect(':memory:');db.execute(ddl)
    assert [r[1]for r in db.execute('PRAGMA table_info(events)')]==['sequence','document']
    for row in rows:db.execute('INSERT INTO events(sequence,document) VALUES(?,?)',(row['sequence'],row['document']))
    try:db.execute("SELECT document FROM events WHERE kind='session.stopped'").fetchall()
    except sqlite3.OperationalError as error:assert str(error)=='no such column: kind'
    else:raise AssertionError('Old no-column query unexpectedly accepted')
    checks.append('old-query-reproduces-exact-no-column-error-on-real-DDL')
    def selected():return [json.loads(r[0])for r in db.execute(query)]
    def real_stops(events):return [e for e in events if e['payload'].get('process_reaped') is True and e['payload'].get('no_process_started') is False]
    events=selected();assert len(real_stops(events))==3
    assert {e['aggregate_id']for e in real_stops(events)}=={r['session_id']for r in data['tables']['factory_attempts']['rows']}
    checks.append('new-query-selects-three-retained-real-owned-stops')
    def insert(event):
        cursor=db.execute('INSERT INTO events(document) VALUES(?)',(json.dumps(event),));return cursor.lastrowid
    decoy={'kind':'unrelated.event','source':'session.stopped','payload':{'kind':'session.stopped','process_reaped':True,'no_process_started':False}}
    seq=insert(decoy);assert len(selected())==len(events);db.execute('DELETE FROM events WHERE sequence=?',(seq,))
    checks.append('source-and-payload-kind-cannot-replace-document-kind')
    seq=insert({'kind':'session.stopped.extra','payload':{'process_reaped':True,'no_process_started':False}})
    assert len(selected())==len(events);db.execute('DELETE FROM events WHERE sequence=?',(seq,))
    checks.append('top-level-kind-must-match-exactly')
    for label,payload in [('unreaped',{'process_reaped':False,'no_process_started':False}),('bool-alias',{'process_reaped':1,'no_process_started':False}),('never-started',{'process_reaped':True,'no_process_started':True})]:
        seq=insert({'kind':'session.stopped','source':'synthetic','payload':payload})
        assert len(selected())==len(events)+1 and len(real_stops(selected()))==3
        db.execute('DELETE FROM events WHERE sequence=?',(seq,));checks.append('does-not-count:'+label)
    seq=insert({'kind':'session.stopped','source':'foreign','aggregate_id':'foreign','payload':{'process_reaped':True,'no_process_started':False}})
    try:assert len(real_stops(selected()))==3
    except AssertionError:pass
    else:raise AssertionError('Extra foreign qualifying event was silently accepted')
    db.execute('DELETE FROM events WHERE sequence=?',(seq,));checks.append('extra-foreign-stop-retains-original-exact-count-refusal')
    seq=insert({'kind':'session.stopped','source':'foreign','payload':[]})
    try:real_stops(selected())
    except AttributeError:pass
    else:raise AssertionError('Malformed stop payload did not fail decoding predicate')
    db.execute('DELETE FROM events WHERE sequence=?',(seq,));checks.append('malformed-stop-payload-fails-not-success')
    seq=db.execute('INSERT INTO events(document) VALUES(?)',('{',)).lastrowid
    try:selected()
    except sqlite3.OperationalError as error:assert 'malformed JSON' in str(error)
    else:raise AssertionError('Malformed JSON was silently accepted')
    db.execute('DELETE FROM events WHERE sequence=?',(seq,));checks.append('malformed-JSON-query-fails-closed')
    assert len(real_stops(selected()))==3;db.close()
    return {'status':'passed','count':len(checks),'checks':checks,'previous_actual_report_pin':pin,
        'retained_diagnostic_sha256':sha(observed),'events_DDL':ddl,'fixed_runtime_query':query,
        'dimension':'pure SQLite replay of retained real Core14 events on exact source-derived events table DDL; NOT new daemon/browser/complete Store migration'}


def runtime_driver(args, output, manifest):
    from playwright.sync_api import expect, sync_playwright
    raw_daemon = args.tools / 'batond'; raw_cli = args.tools / 'baton'
    frontend = args.frontend.resolve(strict=True)
    source = Path(__file__).read_bytes()
    asset_paths=list(frontend.rglob('*'))
    assert len(asset_paths)<=4096 and not any(p.is_symlink() for p in asset_paths)
    assert sum(p.stat().st_size for p in asset_paths if p.is_file())<=64*1024*1024
    assets = {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(asset_paths) if p.is_file()}
    assert (frontend / 'index.html').is_file() and len(assets) <= 4096
    assert assets == manifest['frontend_assets'], 'Root-provided exact built schema15 frontend required'
    for name, path in [('batond', raw_daemon), ('baton', raw_cli)]:
        assert sha(path.read_bytes()) == manifest['binaries'][name]
    assert type(manifest['schema']) is int and manifest['schema'] == 15
    assert re.fullmatch('[a-f0-9]{40}',manifest['source_head']) and re.fullmatch('[a-f0-9]{40}',manifest['compiled_core_source_head'])
    report = {'status': 'failed_or_interrupted', 'schema_required': 15, 'harness_sha256': sha(source), 'candidate_manifest_sha256': sha(args.manifest.read_bytes()),
        'support_sha256': sha(BASE_PATH.read_bytes()), 'candidate': manifest, 'frontend_assets': assets,
        'harness_git_head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=Path(__file__).resolve().parents[3],text=True).strip(),
        'checks': [], 'screenshots': [], 'http': [], 'private_owner_reviews': [], 'cleanup': [],
        'http_unexpected_errors': [], 'readonly_aborts': [], 'observations': {}, 'cleanup_complete': False,
        'not_claimed': ['OS Native Owner dialog', 'macOS', 'real Provider/model', 'child Goal execution',
            'new workspace authority', 'TaskAccepted', 'complete fault matrix', 'full product completion']}
    temporary = tempfile.TemporaryDirectory(prefix='baton-child-plan-browser-')
    root = Path(temporary.name).resolve(); root.chmod(0o700)
    daemon = server = http_thread = browser = playwright = log = reaper = None
    runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
    workspace = scope = host = None; verified = []; active = {}; lock = threading.Lock()
    token = ''; pump = lambda: None
    def clean(value):
        return str(value).replace(token, '[redacted generated Owner token]') if token else str(value)
    def passed(check_id, evidence):
        report['checks'].append({'id': check_id, 'evidence': evidence, 'status': 'passed'})
    def fail(stage, error):
        base.note_failure(report, error, stage, clean)
    def wait(read, predicate, seconds=15):
        until = time.monotonic() + seconds
        while True:
            pump(); value = read()
            if predicate(value): return value
            assert time.monotonic() < until, clean(('Observation deadline', value))
            time.sleep(.02)
    resources = {}; bounds = None; record_context = {}; program = generated_acp()
    try:
        report['controller_reaper'] = {}; reaper = base.FixtureControllerReaper(report['controller_reaper']); reaper.enable()
        report['private_installation'] = {}
        binary, companion = base.install_private_tools(raw_daemon, raw_cli, root / 'installed', report['private_installation'])
        provider = root / 'generated-factory-acp'; settings = root / 'generated-settings.json'
        provider.write_text('#!' + sys.executable + '\n' + program); provider.chmod(0o700)
        report['generated_program_sha256'] = sha(provider.read_bytes())
        log = (output / 'daemon.log').open('xb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        wait(lambda: ((runtime / 'session.token').exists(), (runtime / 'batond.sock').exists(), daemon.poll()),
            lambda v: v[0] and v[1] and v[2] is None)
        host = base.PrivateHost(runtime); token = host.token
        def resource(method, params, kind, key):
            proposal = host.result(method, params, key)
            assert proposal['kind'] == kind
            assert len(json.dumps(proposal).encode()) <= base.MAX_LEGACY_REVIEW_BYTES
            report['private_owner_reviews'].append({'kind': kind, 'proposal': proposal,
                'authority': 'private generated fixture, NOT OS Native approval'})
            return host.result('approvals.finalize', {'proposal_id': proposal['id']},
                'proposal:' + proposal['id'], proposal['revision'])
        connection = resource('connections.propose', {'name': 'Generated schema15 MCP5', 'provider': 'custom_acp',
            'executable': str(provider), 'arguments': ['--fixture-settings', str(settings)]}, 'connection', 'planning:connection')
        cid = connection['id']
        rows = wait(lambda: host.result('connections.list', {}), lambda rows: any(c['id'] == cid and c['probe_state'] == 'ready' for c in rows))
        connection = next(c for c in rows if c['id'] == cid)
        source_root = root / 'source'; source_root.mkdir(mode=0o700)
        (source_root / 'input.txt').write_bytes(b'fixed input\n'); (source_root / '.gitignore').write_text('reports/\n')
        git_env = {**os.environ, 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_TERMINAL_PROMPT': '0'}
        def git(*argv):
            return subprocess.check_output(['git', '-C', str(source_root), '-c', 'core.hooksPath=/dev/null',
                '-c', 'user.name=Generated fixture', '-c', 'user.email=fixture@example.invalid', *argv], env=git_env, text=True).strip()
        git('init', '-q', '--initial-branch=main'); git('add', 'input.txt', '.gitignore'); git('commit', '-qm', 'generated fixed input')
        source_before = {'head': git('rev-parse', 'HEAD'), 'sha256': sha((source_root / 'input.txt').read_bytes()),
            'mtime_ns': (source_root / 'input.txt').stat().st_mtime_ns}
        project = host.result('projects.import', {'path': str(source_root), 'name': 'Generated child planning'}, 'planning:project')
        workspace = host.result('workspaces.create', {'project_id': project['id'], 'expected_head': source_before['head']}, 'planning:workspace', project['revision'])
        wroot = Path(workspace['root']); assert wroot.resolve().is_relative_to(runtime / 'workspaces')
        grant = resource('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'],
            'connection_id': cid, 'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 30_000}, 'grant', 'planning:grant')
        task = host.result('tasks.create', {'project_id': project['id'], 'goal': 'Plan bounded parent and child revisions'}, 'planning:task')
        settings_value = {'case': 'normal', 'task_id': task['id'], 'connection_id': cid, 'workspace_id': workspace['id'],
            'companion': str(companion), 'nonce': uuid.uuid4().hex}
        base.write_private_json(settings, settings_value)
        record_context.update(nonce=settings_value['nonce'], program_sha256=sha(provider.read_bytes()),
            settings_sha256=sha(settings.read_bytes()), connection_id=cid, provider=str(provider), settings=str(settings),
            companion=str(companion), companion_sha256=sha(companion.read_bytes()))
        budget = {'attempts': 8, 'parallel': 1, 'runtime_ms': 30_000, 'output_bytes': 12288,
            'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
        outputs = [{'artifact_id': name, 'relative_path': 'reports/' + name + '.txt', 'max_bytes': 4096, 'expected_sha256': None}
            for name in ['report-child', 'report-parent']]
        expected_selection = {'root_task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'],
            'expected_grant_revision': grant['revision'], 'connection_ids': [cid], 'workspace_ids': [workspace['id']],
            'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'max_bytes': 1024, 'expected_sha256': sha(b'fixed input\n')}],
            'outputs': outputs, 'recipients': [], 'max_depth': 2, 'max_children': 1, 'max_total_nodes': 3,
            'root_budget': budget, 'project_budget': dict(budget), 'ttl_ms': 30_000,
            'capabilities': {'delegate_task': True, 'resume_parent_as_new_attempt': True, 'child_planning': True,
                'child_goal_execution': False, 'create_workspace': False},
            'child_planning_policy': {'max_nodes': 8, 'max_revisions': 16, 'max_plan_bytes': PLAN_BYTES,
                'outputs': [{'artifact': a, 'validators': []} for a in outputs],
                'writable_prefixes': [], 'owner_acceptance_criteria': 'Review both fixed reports; storing plans does not execute or accept tasks'}}
        resources.update(project=project, task=task, grant=grant, workspace=workspace, connection=connection, companion=companion)
        bounds = Bounds(project['id'], task['id'], tasks={task['id']})
        report['observations']['before_factory_approval'] = fingerprint_db(runtime)
        faults = {'drop_next_submit': False}
        class Server(http.server.ThreadingHTTPServer):
            daemon_threads = False; block_on_close = True
            def handle_error(self, request, address):
                report['http_unexpected_errors'].append(clean(traceback.format_exc()))
        class Handler(http.server.SimpleHTTPRequestHandler):
            def setup(self): super().setup(); self.connection.settimeout(5)
            def log_message(self, *_): pass
            def list_directory(self, _): self.send_error(403); return None
            def do_POST(self):
                identity = id(self); stage = 'headers'; call = None
                with lock: active[identity] = 'headers'
                try:
                    if self.path != '/__test_child_plan_ipc': self.send_error(404); return
                    try:
                        require(self.headers.get('Transfer-Encoding') is None)
                        require(all(len(self.headers.get_all(k, [])) == 1 for k in ['Content-Length', 'Content-Type', 'Origin', 'Host']))
                        length = self.headers['Content-Length']
                        require(re.fullmatch('[1-9][0-9]{0,6}', length) is not None and int(length) <= MAX_WIRE)
                        raw = self.rfile.read(int(length)); require(len(raw) == int(length))
                        call = decode_http(raw, origin=self.headers['Origin'], host=self.headers['Host'],
                            content_type=self.headers['Content-Type'], expected_origin=url, bounds=bounds)
                    except Refusal as error: self.send_error(error.status); return
                    trace = {'method': call.method, 'params': call.params, 'key': call.key, 'revision': call.revision,
                        'browser_request_sha256': sha(raw), 'response_complete': False, 'host_observed_utc_ms':int(time.time()*1000)}
                    with lock: report['http'].append(trace); active[identity] = call.method
                    stage = 'core_call'
                    value, reply = host.wire(call)
                    assert len(reply) <= MAX_WIRE
                    path = output / ('core-reply-' + uuid.uuid4().hex + '.json')
                    with path.open('xb') as writer: writer.write(reply)
                    trace.update(response=value, original_reply_path=str(path), original_reply_sha256=sha(reply))
                    if call.method == 'factory_child_plans.submit' and faults['drop_next_submit']:
                        faults['drop_next_submit'] = False
                        assert not value.get('error'), value
                        trace['actual_committed_reply_deliberately_dropped'] = True
                        self.close_connection = True; self.connection.shutdown(socket.SHUT_RDWR); return
                    stage = 'response_headers'
                    self.send_response(200); self.send_header('Content-Type', 'application/json')
                    self.send_header('Content-Length', str(len(reply))); self.end_headers()
                    stage = 'response_body'; self.wfile.write(reply)
                    trace['response_complete'] = True
                except (BrokenPipeError, ConnectionResetError) as error:
                    record_http_abort(report, call.method if call else None, stage, error, clean)
                except BaseException as error:
                    report['http_unexpected_errors'].append(clean(error)); self.send_error(502)
                finally:
                    with lock: active.pop(identity, None)
        server = Server(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
        url = 'http://127.0.0.1:' + str(server.server_port)
        http_thread = threading.Thread(target=server.serve_forever); http_thread.start()
        playwright = sync_playwright().start()
        browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--no-sandbox'])
        context = browser.new_context(viewport={'width': 1488, 'height': 1400}, locale='en-US')
        # Simulates ONLY the genuine Native Owner surface in this private
        # test browser. It is not an HTTP endpoint or a product IPC route.
        def private_owner_approve(_source, command, payload):
            nonlocal scope
            assert command == 'factory_scope_approve'
            assert set(payload) == {'selection', 'idempotencyKey'} and exact_json(payload['selection'], expected_selection)
            assert type(payload['idempotencyKey']) is str and KEY.fullmatch(payload['idempotencyKey'])
            assert scope is None, 'This fixture approves exactly one complete scope'
            proposal = host.result('factory_scopes.propose', payload['selection'], payload['idempotencyKey'])
            review = review_scope(proposal, expected_selection, resources)
            report['private_owner_reviews'].append(review)
            envelope, original = host.wire(FixedCall('approvals.finalize', {'proposal_id': proposal['id']},
                'proposal:' + proposal['id'], proposal['revision']))
            assert not envelope.get('error') and len(original) <= MAX_WIRE, envelope
            scope = envelope['result']; bounds.scope_id = scope['id']
            assert exact_json(scope['selection'], expected_selection) and exact_json(scope['bindings'], proposal['snapshot']['bindings'])
            assert scope['contract_digest'] == proposal['snapshot']['contract_digest']
            report['observations']['factory_approval_reply'] = {'envelope': envelope, 'original_sha256': sha(original)}
            return envelope
        context.expose_binding('__private_test_factory_owner', private_owner_approve)
        context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{if(command==='factory_scope_approve')return window.__private_test_factory_owner(command,args);const r=await fetch('/__test_child_plan_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed child-plan test bridge refused request');return r.json();}};""")
        context.set_default_timeout(15000)
        context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
        page = context.new_page(); errors = []; page.on('pageerror', lambda error: errors.append(str(error)))
        pump = lambda: page.wait_for_timeout(1)
        def shot(label, focus):
            focus.evaluate("e=>e.scrollIntoView({block:'start'})")
            path = output / (label + '.png'); page.screenshot(path=str(path))
            report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'dimension': 'Linux Chromium; NOT Native/macOS'})
        def http_call(command, args2):
            return page.evaluate("async v=>{const r=await fetch('/__test_child_plan_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(v)});return {status:r.status,value:r.ok?await r.json():null}}", {'command': command, 'args': args2})
        page.goto(url); page.get_by_test_id('task-card').filter(has_text=task['goal']).click(); page.locator('#tab-details').click()
        panel = page.get_by_test_id('factory-runtime')
        assert not panel.locator(':scope > details').evaluate('e=>e.open')
        panel.locator(':scope > details > summary').click()
        form = panel.locator('form')
        fixed_form_control(form,'Existing root Grant','select').select_option(grant['id'])
        fixed_form_control(form,'Existing connections','select').select_option([cid])
        fixed_form_control(form,'Existing workspaces','select').select_option([workspace['id']])
        form.locator('details').filter(has=page.get_by_text('Selected inputs / Declared outputs', exact=True)).locator('summary').click()
        form.get_by_label('Selected inputs', exact=True).fill('input | input.txt | 1024 | ' + sha(b'fixed input\n'))
        form.get_by_label('Declared outputs', exact=True).fill('\n'.join(a['artifact_id'] + ' | ' + a['relative_path'] + ' | 4096' for a in outputs))
        for label, value in [('Maximum delegation depth', 2), ('Maximum child tasks', 1), ('Maximum total tasks', 3), ('Approval lifetime (ms)', 30000)]:
            form.get_by_label(label, exact=True).fill(str(value))
        for legend in ['Root shared limits', 'Project shared limits']:
            group = form.get_by_role('group', name=legend, exact=True)
            for label, key in [('Attempts', 'attempts'), ('Parallel attempts', 'parallel'), ('Runtime ms', 'runtime_ms'),
                    ('Declared output bytes', 'output_bytes'), ('Children', 'children'), ('Reworks', 'reworks')]:
                group.get_by_label(label, exact=True).fill(str(budget[key]))
        planning = form.locator('details').filter(has=page.get_by_text('Child-planning policy for Native review', exact=True))
        planning.locator('summary').click(); planning.get_by_label('Allow bounded child planning', exact=True).check()
        for label, value in [('Maximum plan nodes', 8), ('Maximum revisions per task', 16), ('Maximum original plan bytes', PLAN_BYTES)]:
            planning.get_by_label(label, exact=True).fill(str(value))
        planning.get_by_label('Owner acceptance criteria', exact=True).fill(expected_selection['child_planning_policy']['owner_acceptance_criteria'])
        shot('01-explicit-planning-policy', form)
        form.get_by_role('button', name='Review Factory scope in native dialog', exact=True).click()
        expect(panel.get_by_role('button', name='Start approved Factory run', exact=True)).to_be_enabled()
        assert scope is not None and host.result('sessions.list', {}) == []
        after_approval = fingerprint_db(runtime)
        assert all(after_approval[name]['count'] == 0 for name in base.support.LEGACY_GOAL_TABLES)
        assert after_approval['factory_child_goal_runs']['count'] == 0
        assert after_approval['factory_attempts']['count'] == after_approval['factory_runs']['count'] == 0
        assert after_approval['execution_grants']['count'] == 1
        passed('CP01', 'Actual UI complete policy -> private simulated Native review -> actual Core approval, no spawn/no Grant delegation')
        forbidden = [http_call(cmd, {})['status'] for cmd in ['factory_scope_approve', 'approvals_finalize',
            'factory_scopes_propose', 'submit_child_plan', 'generic_ipc', 'sessions_create']]
        assert forbidden == [403] * len(forbidden)
        passed('CP02', 'HTTP excludes Native approval/finalize/generic/scoped tool/session routes')
        panel.get_by_role('button', name='Start approved Factory run', exact=True).click()
        # Observe original HTTP start receipt completion before assuming a run exists.
        wait(lambda: [r for r in report['http'] if r['method'] == 'factory_runs.start'],
            lambda r: len(r) == 1 and r[0].get('response_complete') and not r[0].get('response', {}).get('error'))
        def observe():
            details = host.result('factory_runs.get', {'scope_id': scope['id']})
            record_context['run_id'] = details['run']['id']
            base.collect_started_records(runtime, workspace, scope, record_context, verified, output, reaper)
            return details
        details = wait(observe, lambda d: len(verified)==3 and (wroot/'reports/ui-parent-held').exists())
        assert details['run']['state'] not in {'completed','cancelled','outcome_unknown','blocked'}
        assert details['run']['active_session_id'] is not None and not details['run']['cancel_requested']
        assert len(details['tasks']) == 2
        assert scope['expires_at_ms'] > int(time.time()*1000) and grant['expires_at_ms'] > int(time.time()*1000)
        bounds.tasks = {t['task_id'] for t in details['tasks']}
        child_id = next(t['task_id'] for t in details['tasks'] if t['parent_task_id'] == task['id'])
        assert len(verified) == 3
        held_raw,held_pin=base.bounded_file(wroot/'reports/ui-parent-held',MAX_WIRE,wroot/'reports')
        held=json.loads(held_raw,object_pairs_hook=unique_object)
        resumed=next(r for r in verified if r['stage']=='parent-resumed')
        assert held['task_id']==task['id'] and held['native_session_id']==resumed['native_session_id']
        child_head=host.result('factory_child_plans.get',{'scope_id':scope['id'],'task_id':child_id})
        assert held['child_plan']==child_head['reference'] and held['child_plan']['revision']==2
        report['observations']['actual_generated_live_barrier']={'pin':held_pin,'value':held,'session_id':resumed['session_id'],'attempt_id':resumed['attempt_id']}
        report['observations']['live_held_run_before_UI_writes'] = details
        provider_views = {}
        for name in ['root-plans.json', 'child-plan.json']:
            raw, pin = base.bounded_file(wroot / 'reports' / name, MAX_WIRE, wroot / 'reports')
            provider_views[name] = {'data': json.loads(raw), 'pin': pin}
        report['observations']['actual_generated_mcp_plans'] = provider_views
        for task_id in [task['id'], child_id]:
            refs = host.result('factory_child_plans.list', {'scope_id': scope['id'], 'task_id': task_id})
            assert [r['revision'] for r in refs] == [1, 2] and all('plan_json' not in r and 'proposal' not in r for r in refs)
            for ref in refs:
                view = host.result('factory_child_plans.get', {'scope_id': scope['id'], 'task_id': task_id, 'plan_revision': ref['revision']})
                verify_plan(view, scope, task_id); assert view['reference'] == ref
        passed('CP03', 'Generated ACP -> actual private MCP5 -> root 1/2 and child 1/2 immutable refs; live third parent held after real child-ref2 handoff, prior attempts truly stopped')
        panel.get_by_role('button', name='Refresh Factory records', exact=True).click()
        root_summary = panel.locator('summary').filter(has_text='Child task plans · ' + task['id'])
        root_summary.click()
        root_region = root_summary.locator('..').get_by_role('region', name='Child task plans', exact=True)
        expect(root_region.get_by_text('Current plan head: r2', exact=False)).to_be_visible()
        child_summary = panel.locator('summary').filter(has_text='Child task plans · ' + child_id); child_summary.click()
        child_region = child_summary.locator('..').get_by_role('region', name='Child task plans', exact=True)
        expect(child_region.get_by_text('Current plan head: r2', exact=False)).to_be_visible()
        child_region.locator('details').filter(has=page.get_by_text('Immutable plan revisions', exact=True)).locator('summary').click()
        child_region.get_by_role('button', name=re.compile(r'^r1 · ')).click()
        child_region.locator('details').filter(has=page.get_by_text('Selected recorded revision: r1', exact=True)).locator(':scope > summary').click()
        recorded = child_region.get_by_label('Original stored plan text', exact=True).text_content()
        actual_child_one = host.result('factory_child_plans.get', {'scope_id': scope['id'], 'task_id': child_id, 'plan_revision': 1})
        assert recorded == actual_child_one['plan_json']
        shot('02-real-child-ref-history-original-plan', child_region)
        passed('CP04', 'UI list refs and select r1 retrieves exact original GoalPlan bytes and raw SHA, not fabricated history')
        editor = root_region.get_by_test_id('factory-child-plan-editor'); editor.locator(':scope > summary').click()
        editor.get_by_role('button', name='Prepare steps from latest revision', exact=True).click()
        fixed_form_control(editor,'Step goal fixed-work','textarea').fill('Actual UI revised bounded root plan')
        editor.get_by_role('button', name='Apply steps to JSON draft', exact=True).click()
        draft = fixed_form_control(root_region,'Complete GoalPlan JSON draft','textarea')
        original_draft = draft.input_value(); parsed = json.loads(original_draft)
        assert parsed['revision'] == 3 and parsed['expected_revision'] == 2
        assert parsed['nodes'][0]['goal'] == 'Actual UI revised bounded root plan'
        live = host.result('factory_runs.get',{'scope_id':scope['id']})
        assert live['run']['active_session_id']==details['run']['active_session_id'] and not live['run']['cancel_requested']
        assert scope['expires_at_ms']>int(time.time()*1000) and grant['expires_at_ms']>int(time.time()*1000)
        before_plan = fingerprint_db(runtime)
        root_region.get_by_role('button', name='Store next plan revision', exact=True).click()
        expect(root_region.get_by_text('Current plan head: r3', exact=False)).to_be_visible()
        stored = host.result('factory_child_plans.get', {'scope_id': scope['id'], 'task_id': task['id']})
        verify_plan(stored, scope, task['id']); assert stored['plan_json'] == original_draft
        after_plan = fingerprint_db(runtime)
        assert after_plan['factory_child_plan_revisions']['count'] == before_plan['factory_child_plan_revisions']['count'] + 1
        for name in ['execution_grants', 'managed_workspaces', 'attempts', 'sessions', 'factory_attempts', 'budgets', 'factory_runs']:
            assert after_plan[name] == before_plan[name], ('Planning unexpectedly changed execution', name)
        passed('CP05', 'Real structured GoalPlan UI edit -> fixed Owner submit -> atomic revision3; no execution/new Grant/workspace/budget mutation')
        shot('03-real-ui-stored-root-plan-r3', root_region)
        # Local UI guards and actual backend checks are distinct dimensions.
        candidate = json.loads(next_raw(stored, 'Denied local candidate'))
        variants = []
        p = copy.deepcopy(candidate); p['nodes'][0]['budget']['attempts'] = budget['attempts'] + 1; variants.append(('budget', p, 'AUTH_REQUIRED'))
        p = copy.deepcopy(candidate); p['goal']['input_revision'] = 'coherent-foreign'; p['nodes'][0]['executor']['binding']['input_revision'] = 'coherent-foreign'; variants.append(('input', p, 'INPUT_CHANGED'))
        p = copy.deepcopy(candidate); p['nodes'][0]['outputs'] = []; variants.append(('outputs', p, 'AUTH_REQUIRED'))
        p = copy.deepcopy(candidate); p['nodes'][0]['required_validators'] = []; variants.append(('validators', p, 'INVALID_REQUEST'))
        refusals = []
        for label, invalid, code in variants:
            before = fingerprint_db(runtime); n = sum(r['method'] == 'factory_child_plans.submit' for r in report['http'])
            text = json.dumps(invalid, indent=2); draft.fill(text)
            root_region.get_by_role('button', name='Store next plan revision', exact=True).click()
            expect(root_region.locator(':scope > p[role="alert"]').filter(has_text=re.compile(r'^Plan submission refused(?::|\s|$)'))).to_contain_text('Plan submission refused')
            assert sum(r['method'] == 'factory_child_plans.submit' for r in report['http']) == n
            reply = http_call('factory_child_plans_submit', {'scopeId': scope['id'], 'taskId': task['id'], 'planJson': text,
                'expectedPlanRevision': 3, 'expectedRevision': scope['revision'], 'idempotencyKey': 'negative-' + label})
            assert reply['status'] == 200 and reply['value']['error']['code'] == code, reply
            assert reply['value']['error']['side_effect_state'] == 'not_started'
            assert fingerprint_db(runtime) == before
            refusals.append({'label': label, 'actual_core_response': reply['value'], 'ui_sent_no_mutation': True})
        report['observations']['actual_plan_refusals'] = refusals
        passed('CP06', 'UI rejects coherent budget/input/output/mandatory-owner-validator drift; actual Core independently rejects exact payloads with zero DB mutation')
        # Stale head uses a new key and is denied; this is not transport replay.
        before_stale = fingerprint_db(runtime)
        stale = http_call('factory_child_plans_submit', {'scopeId': scope['id'], 'taskId': task['id'], 'planJson': original_draft,
            'expectedPlanRevision': 2, 'expectedRevision': scope['revision'], 'idempotencyKey': 'actual-stale-head'})
        assert stale['value']['error']['code'] == 'STALE_REVISION'
        assert stale['value']['error']['side_effect_state'] == 'not_started' and fingerprint_db(runtime) == before_stale
        passed('CP07', 'Actual stale plan CAS refuses atomically; observed head/floors remain r3')
        root_region.get_by_role('button', name='Discard local draft', exact=True).click()
        raw4 = next_raw(stored, 'Actual lost-response UI revision four')
        draft.fill(raw4); faults['drop_next_submit'] = True
        store_button = root_region.get_by_role('button', name='Store next plan revision', exact=True)
        diagnostic = {'raw_sha256': sha(raw4.encode()), 'armed_utc_ms': int(time.time()*1000),
            'submit_count_before': sum(r['method']=='factory_child_plans.submit' for r in report['http']),
            'single_click_only': True}
        report['observations']['CP08_submission_transition'] = diagnostic
        diagnostic_errors = []; probe = None; listener_installed = False
        def note_diagnostic_error(stage, error):
            if len(diagnostic_errors)<4:
                diagnostic_errors.append({'stage':stage,'type':type(error).__name__[:64]})
        try: probe=install_submission_probe(page,task['id'])
        except BaseException as error: note_diagnostic_error('install',error)
        browser_requests = []; browser_request_count = 0
        def observe_original_request(request):
            nonlocal browser_request_count
            try:
                if request.method!='POST' or request.url!=url+'/__test_child_plan_ipc': return
                digest=cp08_submission_request_hash(request.post_data,bounds,raw4,task['id'],scope['revision'])
                if digest is None: return
                browser_request_count += 1
                if len(browser_requests)<4:
                    browser_requests.append({'browser_request_sha256':digest,
                        'host_observed_utc_ms':int(time.time()*1000)})
            except BaseException as error: note_diagnostic_error('request_callback',error)
        try:
            page.on('request',observe_original_request);listener_installed=True
        except BaseException as error: note_diagnostic_error('listener_install',error)
        try:
            # Observe without adding any wait, invoking handlers, or retrying.
            # The existing single click and5s unknown assertion stay intact.
            store_button.click()
            expect(root_region.locator(':scope > p[role="alert"]').filter(has_text=re.compile(r'^Submission outcome unknown\.(?:\s|$)'))).to_contain_text('Submission outcome unknown.')
        finally:
            if listener_installed:
                try: page.remove_listener('request',observe_original_request)
                except BaseException as error: note_diagnostic_error('listener_remove',error)
            diagnostic['matching_browser_requests'] = browser_requests
            diagnostic['matching_browser_request_count'] = browser_request_count
            diagnostic['fault_still_armed'] = faults['drop_next_submit']
            diagnostic['submit_count_after'] = sum(r['method']=='factory_child_plans.submit' for r in report['http'])
            diagnostic['deliberate_drop_count'] = sum(bool(r.get('actual_committed_reply_deliberately_dropped')) for r in report['http'])
            diagnostic['capture_missing'] = probe is None
            if probe is not None:
                try: diagnostic['click_and_DOM'] = finish_submission_probe(probe)
                except BaseException as error: note_diagnostic_error('finish',error)
                try: probe.dispose()
                except BaseException as error: note_diagnostic_error('dispose',error)
            diagnostic['errors'] = diagnostic_errors
        # An original click/unknown failure propagates before these diagnostic
        # checks. Observation errors never replace it during finally cleanup.
        assert not diagnostic_errors, ('CP08 diagnostic errors',diagnostic_errors)
        assert not diagnostic.get('click_and_DOM',{}).get('errors'), ('CP08 DOM diagnostic errors',diagnostic)
        assert browser_request_count==1, ('Expected exactly one raw4 browser POST',browser_request_count,browser_requests)
        lost = [r for r in report['http'] if r.get('actual_committed_reply_deliberately_dropped')]
        assert len(lost) == 1 and lost[0]['params']['plan_json'] == raw4
        assert lost[0]['host_observed_utc_ms']<min(scope['expires_at_ms'],grant['expires_at_ms'])
        committed = lost[0]['response']['result']; verify_plan(committed, scope, task['id'])
        assert committed['reference']['revision'] == 4
        root_region.get_by_role('button', name='Refresh child-plan history', exact=True).click()
        expect(root_region.get_by_text('Current plan head: r4', exact=False)).to_be_visible()
        assert len([r for r in report['http'] if r['method'] == 'factory_child_plans.submit' and r['key'] == lost[0]['key']]) == 1
        expect(root_region.get_by_role('button', name='Store next plan revision', exact=True)).to_be_disabled()
        before_recovery = fingerprint_db(runtime)
        root_region.get_by_role('button', name='Recover the exact plan submission', exact=True).click()
        expect(root_region.get_by_text('Submission outcome unknown.', exact=False)).to_have_count(0)
        expect(root_region.locator(':scope > p[role="alert"]')).to_have_count(0)
        expect(root_region.get_by_text('Current plan head: r4', exact=False)).to_be_visible()
        recovered = [r for r in report['http'] if r['method'] == 'factory_child_plans.submit' and r['key'] == lost[0]['key']]
        assert len(recovered) == 2
        report['observations']['exact_key_recovery_receipt'] = verify_exact_recovery(recovered[0], recovered[1], scope, task['id'])
        assert fingerprint_db(runtime) == before_recovery
        conflict = http_call('factory_child_plans_submit', {'scopeId': scope['id'], 'taskId': task['id'], 'planJson': raw4 + ' ',
            'expectedPlanRevision': 3, 'expectedRevision': scope['revision'], 'idempotencyKey': lost[0]['key']})
        assert conflict['value']['error']['code'] == 'CONFIG_CONFLICT' and fingerprint_db(runtime) == before_recovery
        passed('CP08', 'Actual committed reply dropped; UI only reads/reconciles then explicit exact raw/key/CAS replay returns same immutable receipt without new row, changed payload conflicts')
        shot('04-exact-key-recovered-r4', root_region)
        # All new submits above occurred before releasing the actual live
        # parent. Historical terminal reads never become write authority.
        live_before_release=host.result('factory_runs.get',{'scope_id':scope['id']})
        assert live_before_release['run']['active_session_id']==details['run']['active_session_id']
        assert not live_before_release['run']['cancel_requested']
        check_held,check_pin=base.bounded_file(wroot/'reports/ui-parent-held',MAX_WIRE,wroot/'reports')
        assert check_held==held_raw and check_pin==held_pin
        base.write_private_json(wroot/'reports/ui-parent-release',{'fixture':'bounded UI edits finished'})
        details=wait(observe,lambda d:d['run']['state']=='completed')
        assert all(t['confirmed_stop'] for t in details['tasks']) and details['run']['active_session_id'] is None
        report['observations']['terminal_run']=details
        # Immutable refs and SQL journal agree independently of UI state.
        with sqlite3.connect('file:' + str(runtime / 'baton.sqlite3') + '?mode=ro', uri=True) as db:
            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
            rows = db.execute('SELECT document,plan_json,raw_sha256,plan_digest,record_digest,receipt_sequence FROM factory_child_plan_revisions ORDER BY receipt_sequence').fetchall()
            assert len(rows) == 6
            journals = []
            for document, raw, raw_hash, digest, record_digest, seq in rows:
                view = json.loads(document); verify_plan(view, scope, view['reference']['task_id'])
                assert view['plan_json'] == raw and sha(raw.encode()) == raw_hash == view['reference']['raw_sha256']
                assert digest == view['reference']['plan_digest'] and seq == view['reference']['receipt_sequence']
                event = json.loads(db.execute('SELECT document FROM events WHERE sequence=?', (seq,)).fetchone()[0])
                assert event['kind'] == 'factory.child_plan_stored' and event['source'] == 'factory_child_planning'
                assert event['aggregate_id'] == view['reference']['id'] and event['revision'] == view['reference']['revision']
                payload = event['payload']; assert payload['record_digest'] == record_digest
                assert payload['scope_id'] == scope['id'] and payload['run_id'] == details['run']['id'] and payload['task_id'] == view['reference']['task_id']
                assert payload['raw_sha256'] == raw_hash and payload['plan_digest'] == digest
                assert payload['planning_only'] is True and payload['dispatched'] is payload['task_accepted'] is payload['new_grant'] is False
                journals.append({'view': view, 'event': event, 'record_digest': record_digest})
            report['observations']['actual_plan_journal_chain'] = journals
            assert db.execute('SELECT count(*) FROM execution_grants').fetchone()[0] == 1
            actual_grant = json.loads(db.execute('SELECT document FROM execution_grants WHERE id=?', (grant['id'],)).fetchone()[0])
            assert actual_grant['scope']['max_children'] == grant['scope']['max_children'] == 0
            assert actual_grant == grant
            assert db.execute('SELECT count(*) FROM factory_attempts').fetchone()[0] == 3
            assert db.execute("SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'").fetchone()[0] == 0
            assert db.execute("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'").fetchone()[0] == 0
            assert db.execute("SELECT count(*) FROM workspace_leases WHERE holder!=''").fetchone()[0] == 0
            budget_rows = [dict(zip(['scope', 'consumed', 'active'], r)) for r in db.execute('SELECT scope,consumed,active FROM budgets')]
            base.verify_attempt_budgets(budget_rows, task['id'], project['id'], 3)
            report['observations']['actual_budgets'] = budget_rows
            report['observations']['shared_goal_meters'] = base.verify_legacy_factory_shared_budget(
                db, scope, grant, details['run'], 3)
            owned_stops = [json.loads(r[0]) for r in db.execute("SELECT document FROM events WHERE json_extract(document,'$.kind')='session.stopped'")]
            assert len([e for e in owned_stops if e['payload'].get('process_reaped') is True and e['payload'].get('no_process_started') is False]) == 3
            db.rollback()
        assert git('status', '--porcelain') == '' and source_before == {'head': git('rev-parse', 'HEAD'),
            'sha256': sha((source_root / 'input.txt').read_bytes()), 'mtime_ns': (source_root / 'input.txt').stat().st_mtime_ns}
        final_state = fingerprint_db(runtime)
        for name in sorted(base.support.LEGACY_GOAL_TABLES):
            if name == 'goal_ledgers':
                continue  # Exact shared meters proved above; no structural Goal exemption.
            assert final_state[name] == after_approval[name], ('Unexpected Goal dispatch', name)
        assert not errors and not report['http_unexpected_errors'], (errors, report['http_unexpected_errors'])
        assert sha(Path(__file__).read_bytes()) == sha(source)
        passed('CP09', 'Six raw revisions bind independent real journal receipts; original Grant children0, exactly3 attempts, active0, no new Goal/TaskAccepted/workspace, source unchanged')
        report['status'] = 'passed'
    except BaseException as error:
        fail('runtime', error)
        report['traceback'] = clean(traceback.format_exc())
        capture_browser_failure(browser,output,report,clean)
    finally:
        pump = lambda: None
        if host is not None:
            def final_diagnostics():
                info, _ = base.diagnostic_snapshot(runtime, workspace, output, token)
                report['observations']['final_diagnostics'] = info
                assert info['complete'] is True and info['schema'] == 15
            base.cleanup_call(report, 'readonly_final_diagnostics', final_diagnostics, clean)
        close_browser_engine(browser,playwright,report,clean)
        http_complete = True
        if server is not None:
            base.cleanup_call(report, 'http_shutdown', server.shutdown, clean)
            http_thread.join(timeout=5)
            http_complete = not http_thread.is_alive()
            base.cleanup_call(report, 'http_handlers_close', server.server_close, clean)
            http_complete = http_complete and not active
            report['cleanup'].append({'action': 'HTTP serve and handler join', 'confirmed': http_complete, 'active': list(active.values())})
            if not http_complete: fail('http_join', AssertionError('HTTP workers remain owned'))
        if host is not None and scope is not None and workspace is not None:
            try:
                current = host.result('factory_runs.get', {'scope_id': scope['id']}); record_context['run_id'] = current['run']['id']
                base.collect_started_records(runtime, workspace, scope, record_context, verified, output, reaper)
            except BaseException as error: fail('last_record_binding', error)
        daemon_complete = False
        if daemon is not None:
            def stop_daemon():
                if daemon.poll() is None: daemon.terminate()
                try: code = daemon.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(daemon.pid, signal.SIGKILL); daemon.wait(); raise AssertionError('Forced owned daemon cleanup; NOT successful stop')
                report['cleanup'].append({'action': 'private daemon actual wait', 'pid': daemon.pid, 'exitcode': code})
                assert code == 0
                return True
            daemon_complete = base.cleanup_call(report, 'daemon_wait', stop_daemon, clean) is True
        if reaper is not None:
            base.cleanup_call(report, 'bound_adopted_MCP_waits', lambda: [reaper.reap(r) for r in verified], clean)
        identities_complete = False
        if verified:
            identities_complete = base.cleanup_call(report, 'actual_owned_identity_gone', lambda: base.inspect_owned_records(
                report, verified, 3, provider, settings, companion, runtime, clean,
                require_full=report['status'] == 'passed' and not report.get('failure')), clean) is True
        elif daemon is not None: fail('identity_coverage', AssertionError('No actual bound process records; cannot claim cleanup completeness'))
        if log is not None:
            log.close(); path = output / 'daemon.log'
            if token: path.write_bytes(path.read_bytes().replace(token.encode(), b'[redacted generated Owner token]'))
        if reaper is not None: base.cleanup_call(report, 'subreaper_restore', reaper.restore, clean)
        for name, raw in [('batond', raw_daemon), ('baton', raw_cli)]:
            try: assert sha(raw.read_bytes()) == manifest['binaries'][name]
            except BaseException as error: fail('immutable_tool_changed', error)
        try:
            assert {str(p.relative_to(frontend)):sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()} == assets
            assert sha(args.manifest.read_bytes()) == report['candidate_manifest_sha256']
        except BaseException as error: fail('frozen_inputs_changed',error)
        base.cleanup_call(report, 'private_temporary_fixture_cleanup', temporary.cleanup, clean)
        seal_runtime_report(report, daemon_complete, identities_complete, http_complete, clean)
        base.write_private_json(output / 'report.json', json.loads(clean(json.dumps(report))))
    print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'report': str(output / 'report.json')}))
    return runtime_exit(report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--fixture-self-check', type=Path)
    parser.add_argument('--review-self-check', type=Path)
    parser.add_argument('--locator-self-check', type=Path)
    parser.add_argument('--summary-self-check', type=Path)
    parser.add_argument('--alert-self-check', type=Path)
    parser.add_argument('--sql-self-check', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--tools', type=Path)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--manifest', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args(); assert sum([args.self_check,args.run,bool(args.fixture_self_check),bool(args.review_self_check),bool(args.locator_self_check),bool(args.summary_self_check),bool(args.alert_self_check),bool(args.sql_self_check)]) <= 1
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False); output.chmod(0o700)
    if args.self_check:
        value = self_check(); value.update(harness_sha256=sha(Path(__file__).read_bytes()), support_sha256=sha(BASE_PATH.read_bytes()))
        base.write_private_json(output / 'self-check.json', value); print(json.dumps(value)); return 0
    if args.fixture_self_check:
        value=fixture_self_check(args.fixture_self_check.resolve(strict=True));value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'fixture-check.json',value);print(json.dumps(value));return 0
    if args.review_self_check:
        value=review_self_check(args.review_self_check.resolve(strict=True));value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'review-check.json',value);print(json.dumps(value));return 0
    if args.locator_self_check:
        value=locator_self_check(args.locator_self_check.resolve(strict=True),output);value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'locator-check.json',value);print(json.dumps(value));return 0
    if args.summary_self_check:
        value=summary_self_check(args.summary_self_check.resolve(strict=True));value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'summary-check.json',value);print(json.dumps(value));return 0
    if args.alert_self_check:
        value=alert_self_check(args.alert_self_check.resolve(strict=True),output);value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'alert-check.json',value);print(json.dumps(value));return 0
    if args.sql_self_check:
        value=sql_self_check(args.sql_self_check.resolve(strict=True));value['harness_sha256']=sha(Path(__file__).read_bytes())
        base.write_private_json(output/'sql-check.json',value);print(json.dumps(value));return 0
    if not args.run:
        base.write_private_json(output / 'NOT_RUN.json', {'actual_runtime': 'not_run',
            'pending': ['Root reviewed harness', 'fixed schema15 tools/source manifest', 'Root freshly composed built frontend/assets', 'exclusive actual browser CPU lease']})
        return 2
    assert args.tools and args.frontend and args.manifest
    manifest = json.loads(args.manifest.read_bytes(), object_pairs_hook=unique_object)
    try:
        return runtime_driver(args, output, manifest)
    except BaseException as error:
        if not (output/'report.json').exists():
            base.write_private_json(output/'preflight-refusal.json',{'status':'preflight_refused','actual_runtime':'not_run','error':str(error),'traceback':traceback.format_exc()})
        raise


if __name__ == '__main__':
    raise SystemExit(main())
