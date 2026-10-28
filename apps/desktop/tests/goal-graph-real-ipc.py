#!/usr/bin/env python3
"""Actual Graph UI/Unix IPC fixture, never synthetic execution success.

The finite HTTP boundary permits scoped reads, existing plan storage and Graph
cancel/advance. Actual Graph/Owner proposals and Accept/Reject belong only to
an authenticated private host fixture OUTSIDE HTTP. --owner-only connects the
four-node Work/Owner/Summary/Owner contract; it does not verify deterministic
Validate. Both self-check modes are static checks, never execution evidence.
"""
from __future__ import annotations

import argparse
import copy
from dataclasses import dataclass
from functools import partial
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import signal
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import threading
import time
import traceback
from typing import Any, Mapping
import uuid

MAX_HTTP_BYTES = 270_336
MAX_IPC_BYTES = 262_144
MAX_RESPONSE_BYTES = 4_194_304
MAX_GRAPH_REVIEW_BYTES = 1_048_576
MAX_LEGACY_REVIEW_BYTES = 16_384
SAFE_INTEGER = 9_007_199_254_740_991
GRAPH_KINDS = frozenset({'goal_graph_execution', 'goal_node_acceptance'})
MUTATION_METHODS = frozenset({'goal_plans.propose', 'goal_plans.revise', 'goal_graph_runs.cancel', 'goal_graph_runs.advance_plan'})
READ_ONLY_HTTP_METHODS = frozenset({'system.snapshot', 'projects.list', 'connections.list', 'sessions.list',
    'workspaces.list', 'grants.list', 'goal_scopes.list', 'goal_scopes.get', 'goal_plans.get', 'goal_plans.list',
    'goal_plans.ready_preview', 'goal_graph_runs.list', 'goal_graph_runs.get', 'goal_runs.list', 'events.subscribe'})
PENDING_DEPENDENCIES = (
    'Exact approved deterministic Validate definition and producer subject-rebinding fixture',
    'Drift, owned-stop unknown, cancellation, ledger/CAS and explicit rejection actual negative fixtures',
    'Core canonical connection|validator|owner execution kinds and fresh acceptance_state projection fixes',
    'Root-frozen schema12 binary/manifest and composed Graph UI with stable selectors',
)


class BoundaryRefusal(ValueError):
    def __init__(self, status: int, message: str):
        super().__init__(message)
        self.status = status


def require(condition: bool, message: str = 'Malformed fixed fixture request'):
    if not condition:
        raise BoundaryRefusal(400, message)


def safe_integer(value: Any, minimum: int = 0) -> bool:
    return type(value) is int and minimum <= value <= SAFE_INTEGER


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, 'Duplicate fixture JSON field')
        value[key] = item
    return value


@dataclass(frozen=True)
class FixtureBounds:
    # Populated only by the private host from actual authenticated Core results;
    # renderer-supplied names, roots, identities or candidate labels do not bind.
    project_ids: frozenset[str]
    task_ids: frozenset[str]
    scope_ids: frozenset[str]
    run_tasks: Mapping[str, str]


@dataclass(frozen=True)
class FixedCall:
    method: str
    params: dict[str, Any]
    key: str | None = None
    revision: int | None = None


def decode_envelope(envelope: Any, bounds: FixtureBounds) -> FixedCall:
    require(isinstance(envelope, dict) and set(envelope) <= {'command', 'args'} and 'command' in envelope)
    command, values = envelope['command'], envelope.get('args')
    require(isinstance(command, str))
    if values is None:
        values = {}
    require(isinstance(values, dict))
    fixed_read = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list',
                  'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}
    scoped_read = {
        'workspaces_list': ('workspaces.list', 'projectId', 'project_id', bounds.project_ids),
        'grants_list': ('grants.list', 'projectId', 'project_id', bounds.project_ids),
        'goal_scopes_list': ('goal_scopes.list', 'taskId', 'task_id', bounds.task_ids),
        'goal_scopes_get': ('goal_scopes.get', 'scopeId', 'scope_id', bounds.scope_ids),
        'goal_plans_get': ('goal_plans.get', 'scopeId', 'scope_id', bounds.scope_ids),
        'goal_plans_list': ('goal_plans.list', 'scopeId', 'scope_id', bounds.scope_ids),
        'goal_graph_runs_list': ('goal_graph_runs.list', 'taskId', 'task_id', bounds.task_ids),
        # UI separately checks legacy pending/unknown history; never translate
        # it to Graph evidence or allow legacy dispatch through this adapter.
        'goal_runs_list': ('goal_runs.list', 'taskId', 'task_id', bounds.task_ids),
    }
    key = revision = None
    if command in fixed_read:
        require(not values)
        method, params = fixed_read[command], {}
    elif command in scoped_read:
        method, camel, snake, allowed = scoped_read[command]
        require(set(values) == {camel} and isinstance(values[camel], str) and values[camel] in allowed)
        params = {snake: values[camel]}
    elif command == 'events_subscribe':
        require(set(values) <= {'cursor', 'limit'})
        cursor, limit = values.get('cursor', 0), values.get('limit', 100)
        require(safe_integer(cursor) and safe_integer(limit, 1) and limit <= 100)
        method, params = 'events.subscribe', {'cursor': cursor, 'limit': limit}
    elif command in {'goal_plans_propose', 'goal_plans_revise', 'goal_plans_ready_preview'}:
        store = command != 'goal_plans_ready_preview'
        required = {'scopeId', 'expectedRevision'} | ({'planJson', 'idempotencyKey'} if store else set())
        require(set(values) == required and isinstance(values['scopeId'], str) and values['scopeId'] in bounds.scope_ids)
        revision = values['expectedRevision']
        require(safe_integer(revision, 0 if command == 'goal_plans_propose' else 1))
        method = {'goal_plans_propose': 'goal_plans.propose', 'goal_plans_revise': 'goal_plans.revise', 'goal_plans_ready_preview': 'goal_plans.ready_preview'}[command]
        params = {'scope_id': values['scopeId']}
        if store:
            raw = values['planJson']
            require(isinstance(raw, str))
            try:
                require(0 < len(raw.encode()) <= MAX_IPC_BYTES)
            except UnicodeError as failure:
                raise BoundaryRefusal(400, 'Malformed UTF-8 plan text') from failure
            # Original opaque text, including duplicate inner keys, is sent
            # intact for actual Core schema/CAS validation. No parser repair.
            params['plan_json'], key = raw, values['idempotencyKey']
    elif command in {'goal_graph_runs_get', 'goal_graph_runs_cancel', 'goal_graph_run_advance_plan'}:
        write = command != 'goal_graph_runs_get'
        advance = command == 'goal_graph_run_advance_plan'
        required = {'runId'} | ({'expectedRevision', 'idempotencyKey'} if write else set()) | ({'expectedPlanRevision', 'expectedLedgerRevision'} if advance else set())
        require(set(values) == required and isinstance(values['runId'], str) and values['runId'] in bounds.run_tasks)
        require(bounds.run_tasks[values['runId']] in bounds.task_ids)
        method = {'goal_graph_runs_get': 'goal_graph_runs.get', 'goal_graph_runs_cancel': 'goal_graph_runs.cancel', 'goal_graph_run_advance_plan': 'goal_graph_runs.advance_plan'}[command]
        params = {'run_id': values['runId']}
        if write:
            key, revision = values['idempotencyKey'], values['expectedRevision']
            require(safe_integer(revision, 1))
        if advance:
            require(safe_integer(values['expectedPlanRevision'], 1) and safe_integer(values['expectedLedgerRevision'], 1))
            params.update(expected_plan_revision=values['expectedPlanRevision'], expected_ledger_revision=values['expectedLedgerRevision'])
    else:
        # Includes graph_run_approve, goal_node_approve/reject, every propose,
        # approvals.finalize/decline and all generic/session/tool execution.
        raise BoundaryRefusal(403, 'Command is outside the fixed Graph test boundary')
    if method in MUTATION_METHODS:
        require(isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key) is not None)
    else:
        require(key is None)
    return FixedCall(method, params, key, revision)


def decode_http(raw: bytes, *, origin: str, host: str, content_type: str,
                expected_origin: str, bounds: FixtureBounds) -> FixedCall:
    if origin != expected_origin or host != expected_origin.removeprefix('http://'):
        raise BoundaryRefusal(403, 'Exact loopback Origin and Host required')
    loopback = re.fullmatch(r'http://127\.0\.0\.1:([1-9][0-9]{0,4})', expected_origin)
    require(loopback is not None and 1 <= int(loopback[1]) <= 65_535)
    require(content_type == 'application/json' and 0 < len(raw) <= MAX_HTTP_BYTES)
    try:
        envelope = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object)
    except (UnicodeError, json.JSONDecodeError) as failure:
        raise BoundaryRefusal(400, 'Malformed JSON request') from failure
    return decode_envelope(envelope, bounds)


class PrivateUnixFixture:
    """Host-only fixture transport; token is never returned in reports/HTTP."""
    def __init__(self, runtime: Path):
        self.runtime = runtime.resolve(strict=True)
        self._token = (self.runtime / 'session.token').read_text()

    def call(self, call: FixedCall) -> dict[str, Any]:
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': self._token,
                 'method': call.method, 'params': call.params}
        if call.key is not None:
            frame['idempotency_key'] = call.key
        if call.revision is not None:
            frame['expected_revision'] = call.revision
        raw = json.dumps(frame).encode() + b'\n'
        require(len(raw) <= MAX_IPC_BYTES, 'Complete IPC frame exceeds existing Core bound')
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(5)
            stream.connect(str(self.runtime / 'batond.sock'))
            stream.sendall(raw)
            with stream.makefile('rb') as response:
                received = response.readline(MAX_RESPONSE_BYTES + 1)
        require(len(received) <= MAX_RESPONSE_BYTES and received.endswith(b'\n'), 'Bounded whole Graph response required; never truncate')
        value = json.loads(received, object_pairs_hook=unique_object)
        require(isinstance(value, dict))
        return value

    # These methods have no HTTP route. They will be invoked only by the
    # generated local owner fixture after full actual contract/reference checks.
    def prepare_graph(self, selection: dict[str, Any], key: str):
        require(set(selection) == {'scope_id', 'expected_plan_revision', 'expected_ledger_revision'})
        return self.call(FixedCall('goal_graph_runs.propose', selection, key))

    def prepare_owner(self, selection: dict[str, Any], run_revision: int, key: str):
        require(set(selection) == {'run_id', 'node_id', 'expected_plan_revision', 'expected_ledger_revision'})
        return self.call(FixedCall('goal_node_approvals.propose', selection, key, run_revision))

    def decide_owner(self, proposal: dict[str, Any], *, explicit_reject: bool, key: str):
        require(type(explicit_reject) is bool, 'Private Owner decision must be an explicit boolean action')
        check_full_review(proposal, expected_kind='goal_node_acceptance')
        method = 'approvals.decline' if explicit_reject else 'approvals.finalize'
        return self.call(FixedCall(method, {'proposal_id': proposal['id']}, key, proposal['revision']))

    def finalize_graph(self, proposal: dict[str, Any], key: str):
        check_full_review(proposal, expected_kind='goal_graph_execution')
        return self.call(FixedCall('approvals.finalize', {'proposal_id': proposal['id']}, key, proposal['revision']))


def check_full_review(proposal: Any, *, expected_kind: str) -> dict[str, Any]:
    require(expected_kind in GRAPH_KINDS and isinstance(proposal, dict))
    require(proposal.get('kind') == expected_kind and isinstance(proposal.get('id'), str)
            and safe_integer(proposal.get('revision'), 1) and isinstance(proposal.get('snapshot'), dict))
    full = json.dumps(proposal, ensure_ascii=False, separators=(',', ':')).encode()
    snapshot = json.dumps(proposal['snapshot'], ensure_ascii=False, separators=(',', ':')).encode()
    require(len(full) <= MAX_GRAPH_REVIEW_BYTES and len(snapshot) <= MAX_GRAPH_REVIEW_BYTES,
            'Exact full Graph proposal and snapshot must fit new-kind cap; no digest-only/truncated review')
    require(proposal['snapshot'].get('graph_contract_version') == 1)
    if expected_kind == 'goal_graph_execution':
        required = {'graph_contract_version', 'intent', 'scope', 'root_grant', 'existing_connection', 'plan', 'definition_reuse', 'subject_rebinding', 'budgets', 'enforcement', 'new_workspace_authority'}
        require(required <= set(proposal['snapshot']) and proposal['snapshot']['new_workspace_authority'] is False)
    else:
        required = {'graph_contract_version', 'intent', 'run_binding', 'node', 'criterion', 'candidate_refs', 'validation_refs', 'input_bindings', 'capability_digest', 'expected_run_revision', 'expected_ledger_revision'}
        require(required <= set(proposal['snapshot']))
    return {'kind': expected_kind, 'proposal_utf8_bytes': len(full), 'snapshot_utf8_bytes': len(snapshot),
            'proposal_sha256': hashlib.sha256(full).hexdigest(), 'snapshot_sha256': hashlib.sha256(snapshot).hexdigest(),
            'os_owner_dialog': 'not_run; private host fixture outside HTTP'}


def boundary_self_check() -> dict[str, Any]:
    bounds = FixtureBounds(frozenset({'project'}), frozenset({'task'}), frozenset({'scope'}), {'run': 'task'})
    rows = []
    def accepted(name, command, args, expected_method):
        call = decode_envelope({'command': command, 'args': args}, bounds)
        assert call.method == expected_method
        rows.append({'name': name, 'result': 'passed'})
    def refused(name, envelope, status):
        try:
            decode_envelope(envelope, bounds)
        except BoundaryRefusal as failure:
            assert failure.status == status
            rows.append({'name': name, 'result': 'passed: refused'})
        else:
            raise AssertionError('Unexpected boundary admission: ' + name)
    for command, args, method in [
        ('goal_graph_runs_list', {'taskId': 'task'}, 'goal_graph_runs.list'),
        ('goal_graph_runs_get', {'runId': 'run'}, 'goal_graph_runs.get'),
        ('goal_graph_runs_cancel', {'runId': 'run', 'expectedRevision': 2, 'idempotencyKey': 'cancel:exact'}, 'goal_graph_runs.cancel'),
        ('goal_graph_run_advance_plan', {'runId': 'run', 'expectedPlanRevision': 2, 'expectedLedgerRevision': 9, 'expectedRevision': 2, 'idempotencyKey': 'advance:exact'}, 'goal_graph_runs.advance_plan'),
        ('goal_plans_propose', {'scopeId': 'scope', 'planJson': '{"revision":1,"revision":2}', 'expectedRevision': 0, 'idempotencyKey': 'plan:exact'}, 'goal_plans.propose'),
        ('goal_plans_revise', {'scopeId': 'scope', 'planJson': '{}', 'expectedRevision': 1, 'idempotencyKey': 'revise:exact'}, 'goal_plans.revise'),
        ('goal_plans_ready_preview', {'scopeId': 'scope', 'expectedRevision': 1}, 'goal_plans.ready_preview'),
        ('goal_runs_list', {'taskId': 'task'}, 'goal_runs.list')]:
        accepted(command, command, args, method)
    for command in ['goal_graph_run_approve', 'goal_graph_runs_propose', 'goal_node_approve', 'goal_node_reject', 'goal_node_approvals_propose', 'approvals_finalize', 'approvals_decline', 'generic_ipc', 'sessions_create', 'sessions_send', 'goal_runs_dispatch', 'goal_graph_runs_dispatch', 'projects_import', 'connections_propose']:
        refused('private method ' + command, {'command': command, 'args': {}}, 403)
    for key in [None, '', 1, False, [], {}, 'space key', '../foreign', 'k' * 129]:
        refused('invalid cancel key ' + repr(key), {'command': 'goal_graph_runs_cancel', 'args': {'runId': 'run', 'expectedRevision': 2, 'idempotencyKey': key}}, 400)
    for command, args in [
        ('goal_plans_propose', {'scopeId': 'scope', 'planJson': '{}', 'expectedRevision': 0}),
        ('goal_plans_revise', {'scopeId': 'scope', 'planJson': '{}', 'expectedRevision': 1}),
        ('goal_graph_run_advance_plan', {'runId': 'run', 'expectedPlanRevision': 2, 'expectedLedgerRevision': 9, 'expectedRevision': 2})]:
        for key in [None, '', False]:
            refused('required mutation key ' + command + repr(key), {'command': command, 'args': {**args, 'idempotencyKey': key}}, 400)
    for revision in [None, 0, -1, True, 1.5, SAFE_INTEGER + 1]:
        refused('invalid run CAS ' + repr(revision), {'command': 'goal_graph_runs_cancel', 'args': {'runId': 'run', 'expectedRevision': revision, 'idempotencyKey': 'exact'}}, 400)
    for command, args in [('goal_graph_runs_get', {'runId': 'foreign'}), ('goal_graph_runs_list', {'taskId': 'foreign'}), ('goal_scopes_get', {'scopeId': 'foreign'}), ('goal_graph_runs_get', {'runId': 'run', 'token': 'renderer'}), ('goal_graph_runs_get', []), ('system_snapshot', {'method': 'approvals.finalize'})]:
        refused('foreign/extra/type ' + command + repr(args), {'command': command, 'args': args}, 400)
    raw = b'{"command":"goal_graph_runs_get","args":{"runId":"run","runId":"foreign"}}'
    try:
        decode_http(raw, origin='http://127.0.0.1:1', host='127.0.0.1:1', content_type='application/json', expected_origin='http://127.0.0.1:1', bounds=bounds)
    except BoundaryRefusal as failure:
        assert failure.status == 400
        rows.append({'name': 'duplicate JSON outer argument field', 'result': 'passed: refused'})
    else:
        raise AssertionError('Duplicate argument accepted')
    for origin, host, content_type in [('http://foreign.invalid', '127.0.0.1:1', 'application/json'), ('http://127.0.0.1:1', 'foreign.invalid', 'application/json'), ('http://127.0.0.1:1', '127.0.0.1:1', 'text/plain')]:
        try:
            decode_http(b'{"command":"system_snapshot"}', origin=origin, host=host, content_type=content_type, expected_origin='http://127.0.0.1:1', bounds=bounds)
        except BoundaryRefusal as failure:
            assert failure.status == (400 if content_type == 'text/plain' else 403)
            rows.append({'name': 'Origin/Host/content-type refusal ' + repr((origin, host, content_type)), 'result': 'passed: refused'})
        else:
            raise AssertionError('Cross-boundary HTTP admitted')
    opaque = '{"revision":1,"revision":2}'
    actual = decode_envelope({'command': 'goal_plans_propose', 'args': {'scopeId': 'scope', 'planJson': opaque, 'expectedRevision': 0, 'idempotencyKey': 'opaque'}}, bounds)
    assert actual.params['plan_json'] == opaque
    rows.append({'name': 'inner plan JSON stays opaque/exact for actual Core rejection', 'result': 'passed'})
    refused('invalid UTF-8 plan string', {'command': 'goal_plans_propose', 'args': {'scopeId': 'scope', 'planJson': '\ud800', 'expectedRevision': 0, 'idempotencyKey': 'unicode'}}, 400)
    return {'status': 'passed', 'dimension': 'pure outgoing HTTP boundary checks with synthetic arguments; no Core/browser/model/candidate/TaskAccepted execution', 'count': len(rows), 'checks': rows}


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def readonly_reply_abort(failure: BaseException, method: str | None, stage: str) -> bool:
    # Only a known socket disconnect WHILE SENDING an already-observed readonly
    # Core response is harmless to authority. Never classify an IPC call failure,
    # unknown handler exception, denial, or mutation's lost reply as successful.
    return isinstance(failure, (BrokenPipeError, ConnectionResetError)) and method in READ_ONLY_HTTP_METHODS and stage in {'response_headers', 'response_body'}


def transport_self_check() -> dict:
    cases = [
        ('readonly body broken pipe', BrokenPipeError(), 'goal_graph_runs.get', 'response_body', True),
        ('readonly header reset', ConnectionResetError(), 'system.snapshot', 'response_headers', True),
        ('mutation lost body reply', BrokenPipeError(), 'goal_plans.revise', 'response_body', False),
        ('cancel lost reply', ConnectionResetError(), 'goal_graph_runs.cancel', 'response_headers', False),
        ('unknown method broken pipe', BrokenPipeError(), None, 'response_body', False),
        ('read Core transport failure', ConnectionResetError(), 'goal_graph_runs.get', 'core_call', False),
        ('read unexpected error', ValueError(), 'goal_graph_runs.get', 'response_body', False),
        ('read timeout', TimeoutError(), 'goal_graph_runs.get', 'response_body', False),
        ('denial write failure', BrokenPipeError(), None, 'boundary_denial', False)]
    rows = []
    for name, error, method, stage, expected in cases:
        assert readonly_reply_abort(error, method, stage) is expected, name
        rows.append({'name': name, 'method': method, 'stage': stage, 'readonly_reply_abort': expected, 'status': 'passed'})
    return {'status': 'passed', 'dimension': 'static disconnect classification controls, no HTTP/Core/browser execution', 'count': len(rows), 'checks': rows}


def owner_only_plans(scope: dict[str, Any]) -> tuple[dict, dict]:
    """50a53bf sessions_ipc::graph_owner_summary_plan, not a new plan DSL."""
    node = {
        'id': 'produce-report', 'parent_id': None, 'kind': 'work', 'goal': 'goal-report',
        'dependencies': [], 'workspace_id': scope['workspace_id'],
        'executor': {'kind': 'connection', 'binding': scope['binding'], 'program_sha256': scope['program_sha256']},
        'recipients': [], 'required_inputs': [{'goal_task_id': scope['task_id'], 'artifact_id': 'fixed-input',
            'contract_id': scope['inputs']['fixed-input']['contract_id'],
            'source': {'kind': 'approved_external', 'sha256': scope['inputs']['fixed-input']['sha256']}}],
        'outputs': [{'artifact_id': 'report', 'contract_id': scope['outputs']['report']}],
        'required_validators': scope['contracts'][scope['outputs']['report']],
        'side_effect': 'workspace_write', 'write_set': [{'kind': 'workspace', 'path': 'reports/report.txt'}],
        'budget': {'attempts': 1, 'parallel': 1, 'runtime_ms': 5000, 'output_bytes': 2048,
                   'children': 0, 'reworks': 0, 'tokens': None, 'cost_microunits': None},
        'reservation': {'runtime_ms': 4000, 'output_bytes': 2048, 'tokens': None, 'cost_microunits': None},
        'retry': {'max_attempts': 1, 'max_reworks': 0, 'retry_classes': []},
        'idempotency_key': 'produce-report-once', 'external_action_id': None}
    summary = copy.deepcopy(node)
    summary.update(id='independent-2', kind='summary', goal='goal-report-extra-2',
        outputs=[{'artifact_id': 'report-2', 'contract_id': scope['outputs']['report-2']}],
        required_validators=scope['contracts'][scope['outputs']['report-2']],
        write_set=[{'kind': 'workspace', 'path': 'reports/report-2.txt'}], idempotency_key='independent-once-2')
    initial = {'schema_version': 1, 'plan_id': 'runtime-plan:' + scope['task_id'], 'goal': scope['goal'],
        'scope_revision': scope['authority_revision'], 'revision': 1, 'expected_revision': 0,
        'acceptance_rules': scope['required_acceptance_rules'], 'nodes': [node, summary]}
    revised = copy.deepcopy(initial)
    producer, dependent = revised['nodes']
    subject = {'goal_task_id': scope['task_id'], 'artifact_id': 'report', 'contract_id': scope['outputs']['report'],
        'source': {'kind': 'node', 'node_id': producer['id'], 'candidate_for_validation': True}}
    owner = copy.deepcopy(producer)
    owner.update(id='owner-report', kind='owner_approval', goal='Review exact original report',
        executor={'kind': 'owner', 'rule_id': 'goal-owner-review'}, dependencies=[producer['id']],
        required_inputs=[subject], outputs=[], required_validators=['goal-owner-review'],
        side_effect='advisory_read_only', write_set=[], idempotency_key='owner-report-once')
    dependent['dependencies'] = [owner['id']]
    dependent['required_inputs'] = [copy.deepcopy(subject)]
    dependent['required_inputs'][0]['source']['candidate_for_validation'] = False
    final_owner = copy.deepcopy(owner)
    final_owner.update(id='owner-summary', goal='Review exact dependent summary', dependencies=[dependent['id']],
        required_inputs=[{'goal_task_id': scope['task_id'], 'artifact_id': 'report-2',
            'contract_id': scope['outputs']['report-2'], 'source': {'kind': 'node',
            'node_id': dependent['id'], 'candidate_for_validation': True}}], idempotency_key='owner-summary-once')
    revised.update(revision=2, expected_revision=1, nodes=[producer, owner, dependent, final_owner])
    return initial, revised


# Generated account-free child consumes the complete actual Core TaskRequest.
# A normal ACP response is only process output: this program cannot grant,
# validate, accept a Goal, or mint Core candidates/receipts.
GRAPH_ACP = r'''
import hashlib,json,os,re,sys
from pathlib import Path
if sys.argv[1:]==['--version']:
 print('AgentBaton generated Graph ACP 1.0');sys.exit(0)
assert not sys.argv[1:]
def send(value):print(json.dumps(value),flush=True)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-graph-owned'}})
 elif m=='session/prompt':
  text=r['params']['prompt'][0]['text'];v=json.loads(text)
  assert set(v)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location'}
  assert v['schema_version']==1 and v['kind']=='agentbaton_goal_task_request'
  assert v['workspace']['canonical_root']==os.getcwd() and v['node']['workspace_id']==v['workspace']['id']
  assert v['plan_revision']==2 and v['plan_digest'] and v['lineage']['attempt_id']
  assert v['return_location']=={'run_id':v['run_id'],'node_id':v['node']['id'],'attempt_id':v['lineage']['attempt_id']}
  assert v['budget']['limits']==v['node']['budget'] and v['budget']['reservation']==v['node']['reservation']
  assert v['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
  assert len(v['inputs'])==1 and len(v['outputs'])==2
  assert [(x['artifact']['artifact_id'],x['artifact']['relative_path']) for x in v['outputs']]==[('report','reports/report.txt'),('report-2','reports/report-2.txt')]
  binding=v['inputs'][0];artifact=binding['artifact'];p=Path(artifact['relative_path'])
  assert not p.is_absolute() and '..' not in p.parts and not p.is_symlink()
  data=p.read_bytes();assert hashlib.sha256(data).hexdigest()==binding['sha256']
  if v['node']['id']=='produce-report':
   assert v['goal']=='goal-report' and v['node']['kind']=='work'
   assert artifact['relative_path']=='file' and binding['candidate'] is None and data==b'baseline'
   destination=Path('reports/report.txt');content=b'fixed report'
  elif v['node']['id']=='independent-2':
   assert v['goal']=='goal-report-extra-2' and v['node']['kind']=='summary'
   assert v['node']['dependencies']==['owner-report'] and artifact['relative_path']=='reports/report.txt'
   assert binding['candidate']['sha256']==binding['sha256'] and binding['candidate']['relative_path']==str(p)
   assert data==b'fixed report';destination=Path('reports/report-2.txt');content=b'extra report 2'
  else:raise AssertionError('Only the fixed two agent nodes may launch')
  Path('reports').mkdir(exist_ok=True);attempt=v['lineage']['attempt_id']
  assert re.fullmatch(r'[A-Za-z0-9._:-]{1,128}',attempt)
  Path('reports/task-request-'+attempt+'.json').write_text(text)
  ticks=Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19]
  Path('reports/fixture-owned-'+attempt+'.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'start_ticks':ticks}))
  destination.write_bytes(content)
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}})
'''


def verify_candidate(binary: Path, manifest_raw: bytes) -> dict:
    manifest = json.loads(manifest_raw)
    assert manifest.get('schema') == 12, 'Only an explicit frozen schema12 binary may execute Graph'
    entry = manifest['binaries']['batond']
    expected = entry['sha256'] if isinstance(entry, dict) else entry
    assert re.fullmatch('[0-9a-f]{64}', expected) and sha(binary.read_bytes()) == expected
    assert manifest.get('git_head') or manifest.get('source_head'), 'Candidate must bind actual source'
    return manifest


def pinned_artifact(root: Path, reference: dict, expected: bytes) -> dict:
    """Readonly observation; never constructs a Core acceptance/witness row."""
    path = Path(reference['relative_path'])
    assert not path.is_absolute() and path.parts and all(p not in ('.', '..') for p in path.parts)
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    current = root_fd
    try:
        root_stat = os.fstat(root_fd)
        identity = reference['root']
        assert identity['canonical_root'] == str(root) and identity['device'] == root_stat.st_dev and identity['inode'] == root_stat.st_ino
        for component in path.parts[:-1]:
            child = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=current)
            if current != root_fd:
                os.close(current)
            current = child
        fd = os.open(path.parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=current)
        try:
            before = os.fstat(fd)
            assert stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_size <= 1024
            raw = os.read(fd, 1025)
            after = os.fstat(fd)
            stable_fields = ('st_dev', 'st_ino', 'st_size', 'st_mode', 'st_uid', 'st_gid', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns')
            assert all(getattr(before, key) == getattr(after, key) for key in stable_fields) and raw == expected
            pin = reference['file_pin']
            for field, actual in [('device', before.st_dev), ('inode', before.st_ino), ('bytes', before.st_size),
                                  ('uid', before.st_uid), ('gid', before.st_gid), ('mode', before.st_mode),
                                  ('mtime', before.st_mtime_ns // 1_000_000_000), ('mtime_nsec', before.st_mtime_ns % 1_000_000_000),
                                  ('ctime', before.st_ctime_ns // 1_000_000_000), ('ctime_nsec', before.st_ctime_ns % 1_000_000_000)]:
                assert pin[field] == actual, ('Pinned field mismatch', field)
            assert reference['bytes'] == len(raw) and reference['sha256'] == sha(raw)
            return {'relative_path': str(path), 'sha256': sha(raw), 'bytes': len(raw), 'pin': pin}
        finally:
            os.close(fd)
    finally:
        if current != root_fd:
            os.close(current)
        os.close(root_fd)


def driver_self_check() -> dict:
    # Synthetic inputs exercise construction/bounds only, not Core acceptance.
    scope = {'task_id': 't', 'workspace_id': 'w', 'goal': 'g', 'authority_revision': 1,
        'binding': {'fixture': 'opaque'}, 'program_sha256': 'a' * 64,
        'inputs': {'fixed-input': {'contract_id': 'input:fixed-input', 'sha256': sha(b'baseline')}},
        'outputs': {'report': 'output:report', 'report-2': 'output:report-2'},
        'contracts': {'output:report': ['goal-owner-review'], 'output:report-2': ['goal-owner-review']},
        'required_acceptance_rules': ['goal-owner-review']}
    initial, revised = owner_only_plans(scope)
    rows = []
    def check(name, condition):
        assert condition, name
        rows.append({'name': name, 'status': 'passed'})
    check('initial two independent agent nodes preserved', len(initial['nodes']) == 2 and all(not x['dependencies'] for x in initial['nodes']))
    check('revision2 exact four-node chain', [(x['id'], x['kind'], x['dependencies']) for x in revised['nodes']] == [
        ('produce-report', 'work', []), ('owner-report', 'owner_approval', ['produce-report']),
        ('independent-2', 'summary', ['owner-report']), ('owner-summary', 'owner_approval', ['independent-2'])])
    check('Summary requires accepted original output', revised['nodes'][2]['required_inputs'][0]['source']['candidate_for_validation'] is False)
    check('Owner nodes only review and never write/spawn', all(x['executor']['kind'] == 'owner' and not x['outputs'] and not x['write_set'] and x['side_effect'] == 'advisory_read_only' for x in revised['nodes'] if x['kind'] == 'owner_approval'))
    check('both outputs have separate Owner subjects', [revised['nodes'][i]['required_inputs'][0]['artifact_id'] for i in [1, 3]] == ['report', 'report-2'])
    check('no deterministic validation falsely included', not any(x['kind'] == 'validate' for x in revised['nodes']))
    compile(GRAPH_ACP, '<generated-graph-acp>', 'exec')
    check('generated ACP syntax', True)
    for schema in [11, None]:
        try:
            verify_candidate(Path('/must-not-read-a-binary'), json.dumps({'schema': schema}).encode())
        except AssertionError:
            check('old/missing schema refused before binary access ' + str(schema), True)
        else:
            raise AssertionError('Old schema was admitted')
    with tempfile.TemporaryDirectory(prefix='graph-readonly-pin-check-') as temporary:
        root = Path(temporary); folder = root / 'reports'; folder.mkdir(); target = folder / 'candidate'
        target.write_bytes(b'fixed report'); fs = target.stat(); rs = root.stat()
        pin = {'device': fs.st_dev, 'inode': fs.st_ino, 'bytes': fs.st_size, 'uid': fs.st_uid, 'gid': fs.st_gid,
            'mode': fs.st_mode, 'mtime': fs.st_mtime_ns // 1_000_000_000, 'mtime_nsec': fs.st_mtime_ns % 1_000_000_000,
            'ctime': fs.st_ctime_ns // 1_000_000_000, 'ctime_nsec': fs.st_ctime_ns % 1_000_000_000}
        reference = {'relative_path': 'reports/candidate', 'root': {'canonical_root': str(root), 'device': rs.st_dev, 'inode': rs.st_ino},
            'file_pin': pin, 'bytes': len(b'fixed report'), 'sha256': sha(b'fixed report')}
        check('isolated generated file matches exact readonly FD pin/bytes', pinned_artifact(root, reference, b'fixed report')['sha256'] == reference['sha256'])
        def refused_pin(name, altered):
            try:
                pinned_artifact(root, altered, b'fixed report')
            except (AssertionError, OSError):
                check(name, True)
            else:
                raise AssertionError('Unsafe file observation admitted: ' + name)
        refused_pin('foreign file pin refused', {**reference, 'file_pin': {**pin, 'inode': pin['inode'] + 1}})
        refused_pin('parent traversal refused', {**reference, 'relative_path': '../candidate'})
        (folder / 'link').symlink_to(target)
        refused_pin('final-component symlink refused', {**reference, 'relative_path': 'reports/link'})
        (root / 'linked-parent').symlink_to(folder, target_is_directory=True)
        refused_pin('ancestor-component symlink refused', {**reference, 'relative_path': 'linked-parent/candidate'})
        os.link(target, folder / 'hardlink')
        refused_pin('multiple-link file refused', reference)
    return {'status': 'passed', 'dimension': 'static plan constructor/generated Python syntax/manifest negatives and isolated readonly FD controls; no Core/browser execution or minted receipts', 'count': len(rows), 'checks': rows}


def owner_only_driver(args, output: Path) -> int:
    """Runnable real driver. Every receipt comes from the supplied Core12.

    Owner-only is deliberately separate from pending deterministic Validate and
    negative Graph scenarios. An old acceptance projection fails strict checks.
    """
    try:
        assert args.batond and args.candidate_manifest and args.frontend
        binary, frontend = args.batond.resolve(strict=True), args.frontend.resolve(strict=True)
        manifest_raw = args.candidate_manifest.resolve(strict=True).read_bytes()
        manifest = verify_candidate(binary, manifest_raw)  # before any process/token/listener
        assert (frontend / 'index.html').is_file()
        from playwright.sync_api import expect, sync_playwright
    except (AssertionError, OSError, ValueError, KeyError, ImportError) as failure:
        refusal = {'status': 'preflight_refused', 'actual_runtime': 'not_run',
            'harness_sha256': sha(Path(__file__).read_bytes()), 'schema_required': 12,
            'failure': {'type': type(failure).__name__, 'message': str(failure)},
            'no_process_token_read_or_listener': True}
        (output / 'preflight-refusal.json').write_text(json.dumps(refusal, indent=2) + '\n')
        print(json.dumps(refusal)); return 2
    assets = lambda: {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()}
    report = {'status': 'failed_or_interrupted', 'mode': 'owner_only_four_nodes',
        'dimension': 'actual built React/Linux Chromium/finite test HTTP/authenticated Unix Core/generated owned ACP',
        'not_claimed': ['deterministic Validate five-node flow', 'Graph failure matrix', 'native OS Owner dialog',
            'macOS', 'real Provider/model', 'OS sandbox', 'whole product completion'],
        'candidate': manifest, 'manifest_sha256': sha(manifest_raw), 'daemon_sha256': sha(binary.read_bytes()),
        'harness_sha256': sha(Path(__file__).read_bytes()), 'frontend_assets_sha256': assets(),
        'checks': [], 'http_requests': [], 'private_reviews': [], 'observations': {}, 'screenshots': [], 'cleanup': [],
        'http_readonly_reply_aborts': [], 'http_unexpected_errors': []}
    pump = lambda: None
    token = ''
    phase = {'value': 'private_bootstrap'}
    http_lock = threading.Lock()
    http_contexts = {}
    def clean(value):
        text = str(value)
        return text.replace(token, '[redacted generated fixture token]') if token else text
    def passed(name, **facts):
        report['checks'].append({'name': name, 'status': 'passed', **facts})
    def wait(read, predicate, seconds=15):
        deadline = time.monotonic() + seconds
        while True:
            pump()
            value = read()
            if predicate(value):
                return value
            assert time.monotonic() < deadline, clean(('Observation timed out', value))
            time.sleep(.02)
    def ticks(pid):
        try:
            return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
        except FileNotFoundError:
            return None

    with tempfile.TemporaryDirectory(prefix='baton-graph-') as temporary:
        root = Path(temporary).resolve(); root.chmod(0o700)
        runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
        provider = root / 'generated-graph-acp'
        provider.write_text('#!' + sys.executable + '\n' + GRAPH_ACP); provider.chmod(0o700)
        report['generated_program_sha256'] = sha(provider.read_bytes())
        log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        server = thread = browser = None
        workspace = None
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()), lambda v: v[0] and v[1] and v[2] is None)
            host = PrivateUnixFixture(runtime); token = host._token
            def result(method, params, key=None, revision=None):
                response = host.call(FixedCall(method, params, key, revision))
                assert not response.get('error'), clean({'method': method, 'response': response})
                return response['result']
            def approved(method, params, key, kind, revision=None):
                proposal = result(method, params, key, revision)
                assert proposal['kind'] == kind
                assert kind not in GRAPH_KINDS
                raw = json.dumps(proposal['snapshot'], ensure_ascii=False, separators=(',', ':')).encode()
                assert len(raw) <= MAX_LEGACY_REVIEW_BYTES, 'Original legacy review cap is unchanged'
                report['private_reviews'].append({'kind': kind, 'proposal_id': proposal['id'], 'snapshot_utf8_bytes': len(raw), 'snapshot_sha256': sha(raw), 'authority': 'explicit private generated resource fixture; not OS dialog'})
                return result('approvals.finalize', {'proposal_id': proposal['id']}, 'finalize:' + proposal['id'], proposal['revision'])
            connection = approved('connections.propose', {'name': 'Generated Graph account-free ACP', 'provider': 'custom_acp',
                'executable': str(provider), 'arguments': []}, 'graph:connection', 'connection')
            wait(lambda: result('connections.list', {}), lambda rows: any(x['id'] == connection['id'] and x['probe_state'] == 'ready' for x in rows))
            source = root / 'source'; source.mkdir(mode=0o700); (source / 'file').write_bytes(b'baseline')
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}
            def git(*argv):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *argv], env=git_env, text=True).strip()
            git('init', '-q', '--initial-branch=main'); git('add', 'file'); git('commit', '-qm', 'generated bounded Graph input')
            project = result('projects.import', {'path': str(source), 'name': 'Generated Graph Owner-only'}, 'graph:project')
            workspace = result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'graph:workspace', project['revision'])
            workspace_root = Path(workspace['root']).resolve(strict=True)
            assert workspace_root.is_relative_to(runtime / 'workspaces')
            grant = approved('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'],
                'max_attempts': 2, 'max_concurrency': 1, 'max_duration_ms': 600_000}, 'graph:grant', 'grant')
            task = result('tasks.create', {'project_id': project['id'], 'goal': 'Generated Graph Work Owner Summary Owner'}, 'graph:task')
            limits = {'attempts': 2, 'parallel': 1, 'runtime_ms': 10_000, 'output_bytes': 8192, 'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
            scope = approved('goal_scopes.propose', {'task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'],
                'expected_grant_revision': grant['revision'], 'max_nodes': 4, 'new_max_children': 3, 'new_max_depth': 1,
                'root_budget': limits, 'project_budget': limits,
                'inputs': [{'artifact_id': 'fixed-input', 'relative_path': 'file', 'max_bytes': 1024, 'expected_sha256': sha(b'baseline')}],
                'outputs': [{'artifact': {'artifact_id': a, 'relative_path': p, 'max_bytes': 1024, 'expected_sha256': None}, 'validators': []}
                    for a, p in [('report', 'reports/report.txt'), ('report-2', 'reports/report-2.txt')]],
                'writable_prefixes': ['reports'], 'owner_acceptance_criteria': 'Owner reviews the exact report separately from process success'}, 'graph:scope', 'goal')
            known_runs = {}
            bounds = FixtureBounds(frozenset({project['id']}), frozenset({task['id']}), frozenset({scope['id']}), known_runs)
            url = ''
            def record_http_failure(failure, context, *, allow_readonly_abort=False):
                item = {**context, 'exception': type(failure).__name__, 'message': clean(failure),
                    'observed_phase': phase['value'], 'authority_mutation': context.get('method') in MUTATION_METHODS,
                    'readonly_core_response_observed': allow_readonly_abort and context.get('method') in READ_ONLY_HTTP_METHODS}
                expected_abort = allow_readonly_abort and readonly_reply_abort(failure, context.get('method'), context['stage'])
                item['classification'] = 'readonly_reply_aborted; no write action' if expected_abort else 'unexpected_handler_or_mutation_reply_failure'
                with http_lock:
                    report['http_readonly_reply_aborts' if expected_abort else 'http_unexpected_errors'].append(item)
            class FixtureServer(http.server.ThreadingHTTPServer):
                # Join request handlers too, before inspecting the error lists;
                # daemon request threads could otherwise report AFTER a green result.
                daemon_threads = False
                block_on_close = True
                def process_request_thread(self, request, client_address):
                    identity = threading.get_ident()
                    with http_lock:
                        http_contexts[identity] = {'method': None, 'stage': 'request_setup', 'path': None, 'started_phase': phase['value']}
                    try:
                        super().process_request_thread(request, client_address)
                    finally:
                        with http_lock:
                            http_contexts.pop(identity, None)
                def handle_error(self, request, client_address):
                    failure = sys.exc_info()[1]
                    with http_lock:
                        context = dict(http_contexts.get(threading.get_ident(), {'method': None, 'stage': 'unknown_request_thread', 'path': None}))
                    record_http_failure(failure, context)
            class Handler(http.server.SimpleHTTPRequestHandler):
                def setup(self):
                    self.request.settimeout(5)
                    super().setup()
                def log_message(self, *_):
                    pass
                def do_POST(self):
                    context = http_contexts[threading.get_ident()]
                    context.update(path=self.path, stage='boundary_decode')
                    if self.path != '/__test_goal_graph_ipc':
                        self.send_error(404); return
                    try:
                        size_text = self.headers.get('Content-Length', '')
                        require(re.fullmatch('[1-9][0-9]{0,5}', size_text) is not None)
                        size = int(size_text); require(size <= MAX_HTTP_BYTES)
                        call = decode_http(self.rfile.read(size), origin=self.headers.get('Origin', ''), host=self.headers.get('Host', ''),
                            content_type=self.headers.get('Content-Type', ''), expected_origin=url, bounds=bounds)
                    except BoundaryRefusal as failure:
                        context['stage'] = 'boundary_denial'
                        self.send_error(failure.status); return
                    context.update(method=call.method, stage='core_call', idempotency_key=call.key)
                    try:
                        response = host.call(call)
                        if call.method == 'goal_graph_runs.list' and isinstance(response.get('result'), list):
                            for row in response['result']:
                                assert row['run']['task_id'] == task['id'] and row['run']['scope_id'] == scope['id']
                                known_runs[row['run']['id']] = task['id']
                        report['http_requests'].append({'method': call.method, 'params': call.params, 'key': call.key, 'revision': call.revision, 'response': response})
                        raw = json.dumps(response).encode(); assert len(raw) <= MAX_RESPONSE_BYTES
                    except Exception as failure:
                        record_http_failure(failure, dict(context))
                        self.send_error(502); return
                    try:
                        context['stage'] = 'response_headers'
                        self.send_response(200); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(raw))); self.end_headers()
                        context['stage'] = 'response_body'
                        self.wfile.write(raw)
                        context['stage'] = 'response_complete'
                    except (BrokenPipeError, ConnectionResetError) as failure:
                        record_http_failure(failure, dict(context), allow_readonly_abort=True)
                        self.close_connection = True
            server = FixtureServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = 'http://127.0.0.1:' + str(server.server_port)
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            passed('actual private resource approvals and scope prepared; HTTP has no authority path')

            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--no-sandbox'])
                context = browser.new_context(viewport={'width': 1488, 'height': 1200}, locale='en-US')
                context.add_init_script("localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_goal_graph_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed Graph adapter refused request');return r.json();}};")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); errors = []; page.on('pageerror', lambda error: errors.append(str(error)))
                pump = lambda: page.wait_for_timeout(1)
                def expand(detail):
                    if not detail.evaluate('element=>element.open'):
                        detail.locator(':scope > summary').click()
                def shot(name, focus, owner_node_id=None):
                    owner_metrics = []
                    if owner_node_id is not None:
                        owner_button = graph.locator('[data-testid=goal-graph-owner-approve][data-node-id="' + owner_node_id + '"]')
                        expect(owner_button).to_be_visible()
                        owner_button.locator('..').evaluate("element=>element.scrollIntoView({block:'center'})")
                        for action in ['approve', 'reject']:
                            button = graph.locator('[data-testid=goal-graph-owner-' + action + '][data-node-id="' + owner_node_id + '"]')
                            metrics = button.evaluate("""element=>{const r=element.getBoundingClientRect();const points=[[r.left+2,r.top+2],[r.right-2,r.top+2],[r.left+2,r.bottom-2],[r.right-2,r.bottom-2],[r.left+r.width/2,r.top+r.height/2]];return {testid:element.dataset.testid,node:element.dataset.nodeId,rect:{x:r.x,y:r.y,width:r.width,height:r.height},viewport:{width:innerWidth,height:innerHeight},unobscured:points.every(([x,y])=>x>=0&&x<innerWidth&&y>=0&&y<innerHeight&&element.contains(document.elementFromPoint(x,y)))};}""")
                            owner_metrics.append(metrics)
                            assert metrics['unobscured'], ('Owner capture target is occluded/outside viewport', metrics)
                    else:
                        focus.evaluate("element=>element.scrollIntoView({block:'start'})")
                    path = output / (name + '.png'); page.screenshot(path=str(path))
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'viewport': page.viewport_size, 'owner_controls': owner_metrics, 'dimension': 'Linux Chromium; not native/macOS'})
                try:
                    phase['value'] = 'browser_active'
                    page.goto(url)
                    page.get_by_test_id('task-card').filter(has_text=task['goal']).click(); page.locator('#tab-details').click()
                    panel = page.get_by_test_id('goal-planning'); expand(panel.locator(':scope > details'))
                    expect(panel.get_by_role('combobox', name='Selected Goal scope', exact=True)).to_have_value(scope['id'])
                    editor = panel.locator('details').filter(has=page.get_by_text('Versioned plan proposal', exact=True)).last
                    expand(editor)
                    for proposal in owner_only_plans(scope):
                        panel.get_by_role('textbox', name='Plan proposal JSON', exact=True).fill('\n ' + json.dumps(proposal, indent=2) + '\n')
                        panel.get_by_role('button', name='Store initial plan proposal' if proposal['revision'] == 1 else 'Store revised plan proposal', exact=True).click()
                        # The browser mutation is asynchronous. list is a true
                        # readonly empty-until-created projection; get correctly
                        # returns NOT_FOUND before the first store commits.
                        plans = wait(lambda: result('goal_plans.list', {'scope_id': scope['id']}),
                            lambda rows: any(row['proposal']['revision'] == proposal['revision'] for row in rows))
                        stored = next(row for row in plans if row['proposal']['revision'] == proposal['revision'])
                        assert stored['proposal'] == proposal and stored['state'] == 'validated_proposal' and stored['dispatched'] is False
                    outer = panel.get_by_test_id('goal-execution').first; expand(outer.locator(':scope > details'))
                    graph = panel.get_by_test_id('goal-graph-panel')
                    expect(graph).to_have_attribute('data-scope-id', scope['id'])
                    expect(graph).to_have_attribute('aria-busy', 'false')
                    before = result('sessions.list', {}); assert not before
                    denied = []
                    for command in ['goal_graph_run_approve', 'goal_node_approve', 'goal_node_reject', 'approvals_finalize', 'approvals_decline', 'generic_ipc']:
                        status = page.evaluate("async command=>(await fetch('/__test_goal_graph_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args:{}})})).status", command)
                        assert status == 403; denied.append(command)
                    report['observations']['http_owner_denied'] = denied
                    passed('built UI stores exact plan revisions; Graph Owner/finalize/generic HTTP403; no process launched by planning')
                    proposal_response = host.prepare_graph({'scope_id': scope['id'], 'expected_plan_revision': 2, 'expected_ledger_revision': result('system.snapshot', {})['sequence']}, 'graph:execute')
                    assert not proposal_response.get('error'), proposal_response
                    proposal = proposal_response['result']
                    report['private_reviews'].append(check_full_review(proposal, expected_kind='goal_graph_execution'))
                    report['observations']['execution_proposal'] = proposal
                    assert proposal['snapshot']['scope'] == scope and proposal['snapshot']['plan'] == stored
                    assert proposal['snapshot']['scope']['program_sha256'] == sha(provider.read_bytes())
                    started_response = host.finalize_graph(proposal, 'graph:execute-finalize')
                    assert not started_response.get('error'), started_response
                    started = started_response['result']; run_id = started['run']['id']; known_runs[run_id] = task['id']
                    assert started['task_accepted'] is False
                    def view():
                        return result('goal_graph_runs.get', {'run_id': run_id})
                    def ready_owner(node_id):
                        return wait(view, lambda row: any(n['node']['id'] == node_id and n['state'] == 'awaiting_owner' for n in row['nodes']))
                    def refresh():
                        graph.get_by_test_id('goal-graph-refresh').click()
                        expect(graph).to_have_attribute('aria-busy', 'false')
                        target = graph.locator('[data-testid=goal-graph-inspect][data-run-id="' + run_id + '"]')
                        if not graph.get_by_test_id('goal-graph-history').count():
                            target.click(); expect(graph).to_have_attribute('aria-busy', 'false')
                    def owner_decision(detail, node_id, key):
                        selection = {'run_id': run_id, 'node_id': node_id, 'expected_plan_revision': detail['run']['plan_revision'], 'expected_ledger_revision': detail['ledger_revision']}
                        response = host.prepare_owner(selection, detail['run']['revision'], key)
                        assert not response.get('error'), response
                        owner = response['result']
                        report['private_reviews'].append(check_full_review(owner, expected_kind='goal_node_acceptance'))
                        assert owner['snapshot']['candidate_receipts'] == [c for c in detail['candidates'] if c['node_id'] == ('produce-report' if node_id == 'owner-report' else 'independent-2')]
                        decision = host.decide_owner(owner, explicit_reject=False, key=key + ':finalize')
                        assert not decision.get('error'), decision
                        receipt = decision['result']; assert receipt['decision'] == 'accepted' and receipt['approval_id'] == owner['id']
                        replay = host.decide_owner(owner, explicit_reject=False, key=key + ':finalize')
                        assert not replay.get('error') and replay['result'] == receipt
                        opposite = host.decide_owner(owner, explicit_reject=True, key=key + ':opposite')
                        assert opposite['error']['code'] == 'CONFIG_CONFLICT'
                        recovered = host.prepare_owner(selection, detail['run']['revision'], key)
                        assert not recovered.get('error') and recovered['result']['id'] == owner['id'] and recovered['result']['consumed'] is True
                        report['observations'].setdefault('private_owner_decisions', []).append({'proposal': owner, 'decision': receipt, 'replay': replay['result'], 'opposite_error': opposite['error'], 'recovered_proposal': recovered['result']})
                        return receipt
                    initial = ready_owner('owner-report')
                    assert initial['task_accepted'] is False and initial['task_acceptance'] is None and len(initial['candidates']) == 1 and not initial['owner_decisions']
                    assert all(n['session_id'] is None for n in initial['nodes'] if n['node']['id'] != 'produce-report')
                    assert not (workspace_root / 'reports/report-2.txt').exists()
                    first_artifact = initial['candidates'][0]['artifacts'][0]
                    pinned_artifact(workspace_root, first_artifact, b'fixed report')
                    refresh(); expect(graph.get_by_test_id('goal-graph-node')).to_have_count(4)
                    shot('01-work-candidate-before-owner', graph.get_by_test_id('goal-graph-history'), 'owner-report')
                    report['observations']['before_first_owner'] = initial
                    first_decision = owner_decision(initial, 'owner-report', 'graph:owner-report')
                    assert first_decision['task_accepted'] is False
                    after = ready_owner('owner-summary')
                    assert len(after['candidates']) == 2 and after['task_accepted'] is False
                    summary_candidate = next(c for c in after['candidates'] if c['node_id'] == 'independent-2')
                    assert summary_candidate['consumed_inputs'][0]['candidate'] == first_artifact
                    pinned_artifact(workspace_root, summary_candidate['artifacts'][0], b'extra report 2')
                    refresh(); shot('02-summary-candidate-before-final-owner', graph.get_by_test_id('goal-graph-history'), 'owner-summary')
                    report['observations']['before_final_owner'] = after
                    final_decision = owner_decision(after, 'owner-summary', 'graph:owner-summary')
                    assert final_decision['task_accepted'] is True
                    accepted = view(); final_task = result('tasks.get', {'task_id': task['id']}); acceptance = accepted['task_acceptance']
                    assert accepted['task_accepted'] is True and accepted['run']['task_accepted'] is True and accepted['run']['state'] == 'accepted'
                    assert final_task['state'] == 'accepted' and final_task['acceptance_ref'] == acceptance['id']
                    assert final_task['acceptance_sequence'] == acceptance['sequence'] and final_task['revision'] == acceptance['task_revision']
                    assert acceptance['source'] == 'baton_goal_acceptance' and len(acceptance['artifact_refs']) == 2
                    assert acceptance['validation_ids'] == [] and len(acceptance['owner_decision_ids']) == 2
                    assert set(acceptance['owner_decision_ids']) == {d['id'] for d in accepted['owner_decisions']}
                    assert all(d['decision'] == 'accepted' and d['source'] == 'local_owner_confirmation' for d in accepted['owner_decisions'])
                    # OO-WIRE4: do not grandfather old agent/pending projections.
                    for n in accepted['nodes']:
                        assert n['acceptance_state'] == 'accepted', ('Fresh accepted-rule projection required', n)
                        if n['node']['kind'] == 'owner_approval':
                            assert n['execution_kind'] == 'owner' and n['state'] == 'accepted' and n['stop_confirmed'] is False
                            assert n['session_id'] is None and n['attempt_id'] is None
                        else:
                            assert n['execution_kind'] == 'connection' and n['state'] == 'candidate' and n['stop_confirmed'] is True
                    passed('real private Owner decisions gate dependent Summary and all-required TaskAccepted with canonical fresh acceptance projection')
                    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
                        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
                        assert db.execute('PRAGMA user_version').fetchone()[0] == 12
                        event_rows = db.execute('SELECT sequence,document FROM events ORDER BY sequence LIMIT 4097').fetchall()
                        assert len(event_rows) <= 4096 and all(len(raw.encode()) <= MAX_GRAPH_REVIEW_BYTES for _, raw in event_rows)
                        events = [(seq, json.loads(raw)) for seq, raw in event_rows]
                        def matching(kind, source_name, predicate):
                            return [(seq, event) for seq, event in events if event['kind'] == kind and event['source'] == source_name and predicate(event)]
                        for candidate in accepted['candidates']:
                            node = next(n for n in accepted['nodes'] if n['attempt_id'] == candidate['attempt_id'])
                            assert candidate['source'] == 'baton_goal_evidence' and candidate['integrity'] == 'verified' and candidate['task_acceptance'] == 'not_established'
                            assert candidate['stop_receipt_sequence'] < candidate['capture_receipt_sequence']
                            captures = matching('goal.candidate_captured', 'goal_core', lambda e: e['payload'].get('candidate_id') == candidate['id'])
                            assert len(captures) == 1 and captures[0][0] == candidate['capture_receipt_sequence']
                            assert captures[0][1]['payload']['io_completion'] == 'worker_joined'
                            candidate_raw = db.execute('SELECT document FROM goal_graph_candidates WHERE id=?', [candidate['id']]).fetchone()[0]
                            assert json.loads(candidate_raw) == candidate and sha(candidate_raw.encode()) == captures[0][1]['payload']['receipt_digest']
                            stops = matching('session.stopped', 'baton_supervisor', lambda e: e['aggregate_id'] == candidate['session_id'])
                            assert len(stops) == 1 and stops[0][1]['payload']['process_reaped'] is True
                            assert stops[0][0] == candidate['stop_receipt_sequence']
                            session = result('sessions.get', {'session_id': candidate['session_id']}); assert session['state'] == 'stopped'
                            reserved = matching('goal.node_reserved', 'goal_core', lambda e: e['aggregate_id'] == run_id and e['payload'].get('session_id') == candidate['session_id'] and e['payload'].get('attempt_id') == candidate['attempt_id'])
                            intent_stopped = matching('attempt.stopped', 'user_action', lambda e: e['aggregate_id'] == task['id'] and e['payload'].get('intent_id') == session['intent_id'])
                            assert len(reserved) == len(intent_stopped) == 1
                            assert reserved[0][0] < intent_stopped[0][0] < stops[0][0] < captures[0][0]
                            assert intent_stopped[0][1]['payload']['stop_evidence'] == {'kind': 'executor_stop', 'executor_instance': session['id'], 'receipt_ref': 'owned-worker-stop:' + session['id']}
                            raw = (workspace_root / ('reports/task-request-' + candidate['attempt_id'] + '.json')).read_bytes()
                            request = json.loads(raw)
                            assert request['node'] == node['node'] and request['run_id'] == run_id and request['lineage']['attempt_id'] == candidate['attempt_id']
                            assert request['inputs'][0]['candidate'] == candidate['consumed_inputs'][0]['candidate']
                            owned = json.loads((workspace_root / ('reports/fixture-owned-' + candidate['attempt_id'] + '.json')).read_text())
                            assert ticks(owned['pid']) != owned['start_ticks'], 'Actual owned worker must be gone'
                            assert node['actual_output_bytes'] == candidate['artifacts'][0]['bytes'] and node['actual_runtime_ms'] > 0
                            assert node['tokens'] is None and node['cost_microunits'] is None
                            report['observations'].setdefault('owned_task_requests', []).append({'utf8_bytes': len(raw), 'sha256': sha(raw), 'raw_utf8': raw.decode('utf-8'), 'request': request, 'candidate': candidate, 'reserved': reserved[0], 'intent_stopped': intent_stopped[0], 'stop': stops[0], 'capture': captures[0], 'owned_process_identity': owned, 'owned_process_gone': True})
                        for decision in accepted['owner_decisions']:
                            raw_decision = db.execute('SELECT document FROM goal_graph_owner_decisions WHERE id=?', [decision['id']]).fetchone()[0]
                            assert json.loads(raw_decision) == decision
                            decided = matching('goal.owner_decided', 'local_owner_confirmation', lambda e: e['aggregate_id'] == run_id and e['payload'].get('owner_decision_id') == decision['id'])
                            assert len(decided) == 1 and decided[0][1]['payload']['receipt_digest'] == sha(raw_decision.encode())
                            assert decided[0][1]['payload']['approval_id'] == decision['approval_id'] and decided[0][1]['payload']['decision'] == 'accepted'
                            report['observations'].setdefault('durable_owner_decisions', []).append({'decision': decision, 'event': decided[0], 'receipt_sha256': sha(raw_decision.encode())})
                        raw = db.execute('SELECT document FROM goal_graph_acceptances WHERE id=?', [acceptance['id']]).fetchone()[0]
                        assert json.loads(raw) == acceptance
                        commits = matching('task.accepted', 'baton_goal_acceptance', lambda e: e['aggregate_id'] == task['id'])
                        assert len(commits) == 1 and commits[0][0] == acceptance['sequence']
                        commit = commits[0][1]
                        assert commit['revision'] == final_task['revision'] and commit['payload']['receipt_digest'] == sha(raw.encode())
                        assert commit['payload']['criteria_complete'] is True and commit['payload']['task_accepted'] is True
                        assert commit['payload']['acceptance_ref'] == acceptance['id'] and commit['payload']['acceptance_sequence'] == acceptance['sequence']
                        budgets = {scope_name: {'consumed': consumed, 'active': active} for scope_name, consumed, active in db.execute('SELECT scope,consumed,active FROM budgets')}
                        for key in ['root:' + task['id'], 'project:' + project['id']]:
                            assert budgets[key] == {'consumed': 2, 'active': 0}
                        ledgers = {k: json.loads(v) for k, v in db.execute('SELECT scope,document FROM goal_ledgers')}
                        for key in ['root-grant:' + grant['id'], 'project:' + project['id']]:
                            assert ledgers[key]['observed_attempts'] == 2 and ledgers[key]['output_bytes'] == 26
                        report['observations']['durable_acceptance'] = {'event': commits[0], 'receipt_sha256': sha(raw.encode()), 'budgets': budgets, 'ledgers': ledgers}
                    assert len(result('sessions.list', {})) == 2
                    assert result('tasks.cancel', {'task_id': final_task['id']}, 'graph:terminal-cancel', final_task['revision']) == final_task
                    refused = host.call(FixedCall('sessions.create', {'task_id': final_task['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'grant_id': grant['id']}, 'graph:terminal-no-reopen', final_task['revision']))
                    assert refused.get('error')
                    for _ in range(3):
                        stable = view(); assert stable['run'] == accepted['run'] and stable['ledger_revision'] == accepted['ledger_revision']
                    passed('fresh SHA/FD candidates, owned-reaped TaskRequests, durable TaskAccepted event/digest and cumulative two-attempt budgets; terminal task cannot reopen')
                    report['observations']['accepted'] = {'graph': accepted, 'task': final_task, 'ordinary_session_refusal': refused['error']}
                    page.get_by_role('button', name='Refresh', exact=True).click()
                    # Global refresh reloads Task acceptance_ref/sequence; Graph-only refresh cannot prove this.
                    panel = page.get_by_test_id('goal-planning'); outer = panel.get_by_test_id('goal-execution').first
                    expand(panel.locator(':scope > details')); expand(outer.locator(':scope > details'))
                    graph = panel.get_by_test_id('goal-graph-panel'); refresh()
                    history = graph.get_by_test_id('goal-graph-history')
                    expect(history.get_by_text('Task accepted', exact=True)).to_be_visible()
                    expect(history.get_by_test_id('goal-graph-node')).to_have_count(4)
                    shot('03-task-accepted-four-node-owner-chain', history)
                    assert not errors, errors
                    assert (source / 'file').read_bytes() == b'baseline' and assets() == report['frontend_assets_sha256']
                    assert sha(binary.read_bytes()) == report['daemon_sha256'] and sha(Path(__file__).read_bytes()) == report['harness_sha256']
                    passed('actual built UI shows authoritative TaskAccepted four-node history; no browser errors or asset/source drift')
                    report['status'] = 'passed'
                except BaseException:
                    shot('failure-current-ui', page.locator('body'))
                    raise
                finally:
                    phase['value'] = 'browser_closing'
                    browser.close(); browser = None
        except BaseException as failure:
            report['failure'] = {'type': type(failure).__name__, 'message': clean(failure), 'traceback': clean(traceback.format_exc())}
        finally:
            pump = lambda: None
            phase['value'] = 'http_shutdown'
            if server is not None:
                server.shutdown()
            if thread is not None:
                thread.join(timeout=5)
                report['cleanup'].append({'action': 'HTTP serve thread joined', 'thread_alive': thread.is_alive()})
                if thread.is_alive():
                    report['status'] = 'failed_or_interrupted'
            if server is not None:
                server.server_close()  # block_on_close joins every non-daemon handler
                with http_lock:
                    active_handlers = [dict(value) for value in http_contexts.values()]
                report['cleanup'].append({'action': 'HTTP server closed and request handlers joined', 'active_handlers': active_handlers})
                if active_handlers:
                    report['status'] = 'failed_or_interrupted'
            phase['value'] = 'daemon_shutdown'
            termination_requested = daemon.poll() is None
            if daemon.poll() is None:
                daemon.terminate()
            forced_daemon_kill = False
            try:
                daemon_exit = daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                forced_daemon_kill = True
                os.killpg(daemon.pid, signal.SIGKILL); daemon_exit = daemon.wait()
                report['status'] = 'failed_or_interrupted'; report['cleanup'].append({'action': 'owned daemon needed SIGKILL; not stop proof'})
            report['cleanup'].append({'action': 'owned daemon wait completed', 'pid': daemon.pid,
                'termination_requested': termination_requested, 'wait_exitcode': daemon_exit, 'forced_sigkill': forced_daemon_kill,
                'process_not_alive': ticks(daemon.pid) is None})
            if daemon_exit != 0 or ticks(daemon.pid) is not None:
                report['status'] = 'failed_or_interrupted'
            phase['value'] = 'generated_process_identity_check'
            inspected_identities = []
            if workspace is not None:
                for marker in (Path(workspace['root']) / 'reports').glob('fixture-owned-*.json'):
                    try:
                        owned = json.loads(marker.read_text()); pid = owned['pid']
                        assert type(pid) is int and pid > 1 and owned['pgid'] == pid and re.fullmatch('[0-9]+', owned['start_ticks'])
                        expected_records = [r for r in report['observations'].get('owned_task_requests', []) if r['owned_process_identity'] == owned]
                        observed_ticks = ticks(pid)
                        inspected_identities.append(owned)
                        if observed_ticks != owned['start_ticks']:
                            report['cleanup'].append({'action': 'generated process original identity not alive', 'marker': marker.name,
                                'marker_sha256': sha(marker.read_bytes()), 'expected_identity': owned, 'observed_start_ticks': observed_ticks,
                                'original_process_not_alive': True, 'pid_exists': observed_ticks is not None,
                                'matched_real_task_request': len(expected_records) == 1, 'signal_sent': False})
                            continue
                        assert owned['pgid'] == pid and os.getpgid(pid) == pid
                        assert str(provider).encode() in Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
                        os.killpg(pid, signal.SIGKILL)
                        deadline = time.monotonic() + 2
                        while ticks(pid) == owned['start_ticks'] and time.monotonic() < deadline:
                            time.sleep(.02)
                        report['cleanup'].append({'action': 'killed only identity-checked generated private process group; not stop proof',
                            'expected_identity': owned, 'observed_start_ticks_after_kill': ticks(pid), 'original_process_not_alive': ticks(pid) != owned['start_ticks']})
                        report['status'] = 'failed_or_interrupted'
                    except ProcessLookupError:
                        report['cleanup'].append({'action': 'generated process exited during identity check', 'expected_identity': owned,
                            'original_process_not_alive': ticks(owned['pid']) != owned['start_ticks'], 'signal_sent': False})
                    except Exception as failure:
                        report['cleanup'].append({'action': 'refused unsafe cleanup', 'failure': clean(failure)})
                        report['status'] = 'failed_or_interrupted'
            expected_identities = [r['owned_process_identity'] for r in report['observations'].get('owned_task_requests', [])]
            if report['status'] == 'passed':
                if len(expected_identities) != 2 or len(inspected_identities) != 2 or any(i not in inspected_identities for i in expected_identities):
                    report['status'] = 'failed_or_interrupted'
                    report['cleanup'].append({'action': 'required two generated worker identities were not fully inspected', 'expected': expected_identities, 'inspected': inspected_identities})
            if report['http_unexpected_errors']:
                report['status'] = 'failed_or_interrupted'
            if report['status'] == 'passed':
                passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and both exact generated worker identities gone')
            phase['value'] = 'closed'
            log.close()
            if token:
                log_path = output / 'daemon.log'
                log_path.write_bytes(log_path.read_bytes().replace(token.encode(), b'[redacted generated fixture token]'))
            (output / 'report.json').write_text(clean(json.dumps(report, ensure_ascii=False, indent=2)) + '\n')
    print(json.dumps({'mode': report['mode'], 'status': report['status'], 'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'report': str(output / 'report.json')}))
    return 0 if report['status'] == 'passed' else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--driver-self-check', action='store_true')
    parser.add_argument('--transport-self-check', action='store_true')
    parser.add_argument('--owner-only', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--batond', type=Path)
    parser.add_argument('--candidate-manifest', type=Path)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': 12}
    assert sum([args.self_check, args.driver_self_check, args.transport_self_check, args.owner_only]) <= 1, 'Choose one explicit mode'
    if args.transport_self_check:
        report = {**attribution, **transport_self_check()}
        (output / 'transport-static-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'dimension': report['dimension'], 'count': report['count'], 'status': report['status']}))
        return 0
    if args.driver_self_check:
        report = {**attribution, **driver_self_check()}
        (output / 'driver-static-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'dimension': report['dimension'], 'count': report['count'], 'status': report['status']}))
        return 0
    if args.self_check:
        report = {**attribution, **boundary_self_check()}
        (output / 'boundary-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'dimension': report['dimension'], 'count': report['count'], 'status': report['status']}))
        return 0
    if args.owner_only:
        return owner_only_driver(args, output)
    # Full five-node/negative matrix remains pending. Only explicit --owner-only
    # invokes the separate real four-node driver, with a schema12 preflight.
    report = {**attribution, 'status': 'pending_dependencies', 'actual_runtime': 'not_run',
              'pending': list(PENDING_DEPENDENCIES), 'no_successful_graph_execution_or_acceptance_claimed': True}
    if args.candidate_manifest is not None:
        raw = args.candidate_manifest.resolve(strict=True).read_bytes(); manifest = json.loads(raw)
        report['manifest_sha256'] = hashlib.sha256(raw).hexdigest()
        if manifest.get('schema') != 12:
            report['refusal'] = 'Older schema binary is not Graph12 execution evidence'
        else:
            report['manifest_schema'] = 12
    (output / 'pending-runtime.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'actual_runtime': report['actual_runtime'], 'pending': report['pending']}))
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
