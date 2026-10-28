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


VALIDATION_CASES = ('integrity', 'data', 'command', 'data-range', 'command-timeout')
LEGACY_TASK_FIELDS = frozenset({'schema_version', 'kind', 'goal', 'node', 'run_id', 'plan_revision', 'plan_digest',
    'workspace', 'inputs', 'outputs', 'policy', 'lineage', 'budget', 'idempotency_key', 'return_location'})
GRAPH_TASK_FIELDS = LEGACY_TASK_FIELDS | {'validation_contracts', 'task_binding'}


def validation_kind(case: str) -> str:
    assert case in VALIDATION_CASES
    return case.split('-')[0]


def scope_selection(task, grant, *, count, max_nodes, kind=None):
    limits = {'attempts': 8, 'parallel': 1, 'runtime_ms': 10_000, 'output_bytes': 8192,
        'children': 3 if kind == 'data' else max(count, 2) - 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
    return {'task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'],
        'expected_grant_revision': grant['revision'], 'max_nodes': max_nodes, 'new_max_children': max_nodes - 1,
        'new_max_depth': 1, 'root_budget': limits, 'project_budget': limits,
        'inputs': [{'artifact_id': 'fixed-input', 'relative_path': 'inputs/file', 'max_bytes': 1024, 'expected_sha256': sha(b'baseline')}],
        'outputs': [{'artifact': {'artifact_id': 'report' if i == 1 else f'report-{i}',
            'relative_path': 'reports/report.txt' if i == 1 else f'reports/report-{i}.txt', 'max_bytes': 1024, 'expected_sha256': None},
            'validators': []} for i in range(1, count + 1)], 'writable_prefixes': ['reports'],
        'owner_acceptance_criteria': 'Owner reviews the exact report separately from process success'}


def scoped_agent_plan(scope, count, goal):
    contract = scope['outputs']['report']
    producer = {'id': 'produce-report', 'parent_id': None, 'kind': 'work', 'goal': goal, 'dependencies': [],
        'workspace_id': scope['workspace_id'], 'executor': {'kind': 'connection', 'binding': scope['binding'], 'program_sha256': scope['program_sha256']},
        'recipients': [], 'required_inputs': [{'goal_task_id': scope['task_id'], 'artifact_id': 'fixed-input',
            'contract_id': scope['inputs']['fixed-input']['contract_id'], 'source': {'kind': 'approved_external', 'sha256': scope['inputs']['fixed-input']['sha256']}}],
        'outputs': [{'artifact_id': 'report', 'contract_id': contract}], 'required_validators': scope['contracts'][contract],
        'side_effect': 'workspace_write', 'write_set': [{'kind': 'workspace', 'path': 'reports/report.txt'}],
        'budget': {'attempts': 1, 'parallel': 1, 'runtime_ms': 5000, 'output_bytes': 2048, 'children': 0, 'reworks': 0, 'tokens': None, 'cost_microunits': None},
        'reservation': {'runtime_ms': 4000, 'output_bytes': 2048, 'tokens': None, 'cost_microunits': None},
        'retry': {'max_attempts': 1, 'max_reworks': 0, 'retry_classes': []}, 'idempotency_key': 'produce-report-once', 'external_action_id': None}
    initial = {'schema_version': 1, 'plan_id': 'runtime-plan:' + scope['task_id'], 'goal': scope['goal'],
        'scope_revision': scope['authority_revision'], 'revision': 1, 'expected_revision': 0,
        'acceptance_rules': scope['required_acceptance_rules'], 'nodes': []}
    nodes = [producer]
    for index in range(2, count + 1):
        artifact = f'report-{index}'; contract = scope['outputs'][artifact]
        node = copy.deepcopy(producer)
        node.update(id=f'independent-{index}', kind='summary', goal=f'goal-report-extra-{index}',
            outputs=[{'artifact_id': artifact, 'contract_id': contract}], required_validators=scope['contracts'][contract],
            write_set=[{'kind': 'workspace', 'path': f'reports/{artifact}.txt'}], idempotency_key=f'independent-once-{index}')
        nodes.append(node)
    initial['nodes'] = nodes
    return initial


def collapsed_producer(plan, count):
    producer = copy.deepcopy(plan['nodes'][0]); selected = plan['nodes'][:count]
    producer['outputs'] = [copy.deepcopy(o) for n in selected for o in n['outputs']]
    producer['write_set'] = [copy.deepcopy(o) for n in selected for o in n['write_set']]
    producer['required_validators'] = sorted({v for n in selected for v in n['required_validators']})
    return producer


def validation_plans(scope, case):
    kind = validation_kind(case); data = kind == 'data'; count = 4 if data else 2
    initial = scoped_agent_plan(scope, count, 'goal-data-report-bad' if case == 'data-range' else 'goal-data-report' if data else 'goal-report')
    producer = collapsed_producer(initial, 3) if data else copy.deepcopy(initial['nodes'][0])
    summary = copy.deepcopy(initial['nodes'][-1])
    rules = [r for r in scope['contracts'][scope['outputs']['report']] if r.startswith(kind + ':')]
    assert len(rules) == 1
    subjects = [{'goal_task_id': scope['task_id'], 'artifact_id': o['artifact_id'], 'contract_id': o['contract_id'],
        'source': {'kind': 'node', 'node_id': producer['id'], 'candidate_for_validation': True}} for o in producer['outputs']]
    validate = copy.deepcopy(producer)
    validate.update(id='validate-report', kind='validate', executor={'kind': 'validator', 'rule_id': rules[0]},
        goal='Validate exact new JSON schema/ranges/metadata without scientific acceptance' if data else f'Apply exact approved {kind} definition to the new captured producer',
        dependencies=[producer['id']], required_inputs=subjects, outputs=[], write_set=[], side_effect='advisory_read_only',
        required_validators=rules, idempotency_key='data-validate-once' if data else 'validate-owned-report-once')
    owner = copy.deepcopy(validate)
    owner.update(id='owner-report', kind='owner_approval', executor={'kind': 'owner', 'rule_id': 'goal-owner-review'},
        dependencies=[producer['id'], validate['id']], required_validators=['goal-owner-review'],
        idempotency_key='data-owner-report-once' if data else 'owner-report-once')
    if not data:
        owner['goal'] = f'Owner reviews exact report after deterministic {kind} only'
    summary['dependencies'] = [owner['id'], validate['id']] if data else [validate['id'], owner['id']]
    summary['required_inputs'] = [copy.deepcopy(subjects[0])]
    summary['required_inputs'][0]['source']['candidate_for_validation'] = False
    last = copy.deepcopy(owner)
    last.update(id='owner-summary', dependencies=[summary['id']],
        required_inputs=[{'goal_task_id': scope['task_id'], **summary['outputs'][0],
            'source': {'kind': 'node', 'node_id': summary['id'], 'candidate_for_validation': True}}],
        idempotency_key='data-owner-summary-once' if data else 'owner-summary-once')
    revised = {**copy.deepcopy(initial), 'revision': 2, 'expected_revision': 1, 'nodes': [producer, validate, owner, summary, last]}
    return initial, revised


def definition_selection(kind, session_id, case):
    count = 3 if kind == 'data' else 1
    selected = {'session_id': session_id,
        'inputs': [{'artifact_id': 'fixed-input', 'relative_path': 'inputs/file', 'max_bytes': 1024, 'expected_sha256': sha(b'baseline')}],
        'outputs': [{'artifact_id': 'report' if i == 1 else f'report-{i}',
            'relative_path': 'reports/report.txt' if i == 1 else f'reports/report-{i}.txt', 'max_bytes': 1024, 'expected_sha256': None} for i in range(1, count + 1)]}
    if kind == 'integrity':
        return 'validation_policies.propose', 'validation', {**selected, 'analysis': None}
    if kind == 'data':
        table = {'table_artifact_id': 'report', 'dataset_rows': 1,
            'columns': [{'name': 'x', 'required': True, 'nullable': False, 'scalar': {'type': 'integer', 'min': 0, 'max': 10}}],
            'analysis': {'script_artifact_id': 'report-2', 'environment_artifact_id': 'report-3', 'units': {'x': 'bytes'},
                'timezone': 'UTC', 'seed': 7, 'scope': {'kind': 'full'}},
            'limits': {'max_bytes': 1_048_576, 'max_depth': 16, 'max_values': 100_000, 'max_string_bytes': 4096, 'max_rows': 10_000}}
        return 'data_validation_policies.propose', 'data_validation', {**selected, 'table': table}
    assert kind == 'command'
    executable = Path('/usr/bin/sleep' if case == 'command-timeout' else '/usr/bin/printf').resolve(strict=True)
    return 'command_validation_policies.propose', 'command_validation', {**selected, 'executable': str(executable),
        'mode': 'native_executable', 'script': None, 'arguments': ['3'] if case == 'command-timeout' else ['fixed command observed'],
        'environment': {}, 'expected_exit_codes': [0], 'wall_time_ms': 1500 if case == 'command-timeout' else 500,
        'cleanup_time_ms': 500, 'stdout_limit': 512, 'stderr_limit': 512,
        'require': {k: False for k in ['filesystem_isolation', 'network_isolation', 'process_tree_containment',
            'execution_time_input_immutability', 'auxiliary_source_immutability', 'sealed_executable_image']}}


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


VALIDATION_ACP = r'''
import hashlib,json,os,re,sys
from pathlib import Path
if sys.argv[1:]==['--version']:
 print('AgentBaton generated Graph validation ACP 1.0');sys.exit(0)
assert not sys.argv[1:]
BASE={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location'}
def send(value):print(json.dumps(value),flush=True)
def no_private_config(value):
 if isinstance(value,dict):
  assert not {'arguments','environment','argv0','environment_sha256'}&set(value)
  for item in value.values():no_private_config(item)
 elif isinstance(value,list):
  for item in value:no_private_config(item)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-graph-validator-owned'}})
 elif m=='session/prompt':
  text=r['params']['prompt'][0]['text'];v=json.loads(text);graph='validation_contracts' in v
  assert set(v)==(BASE|{'validation_contracts','task_binding'} if graph else BASE)
  assert v['schema_version']==1 and v['kind']=='agentbaton_goal_task_request'
  assert v['workspace']['canonical_root']==os.getcwd() and v['workspace']['id']==v['node']['workspace_id']
  assert v['workspace']['input_revision'] and v['plan_digest'] and v['lineage']['attempt_id']
  assert v['return_location']=={'run_id':v['run_id'],'node_id':v['node']['id'],'attempt_id':v['lineage']['attempt_id']}
  assert v['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
  assert v['budget']['limits']==v['node']['budget'] and v['budget']['reservation']==v['node']['reservation']
  assert len(v['inputs'])==1
  binding=v['inputs'][0];artifact=binding['artifact'];p=Path(artifact['relative_path'])
  assert not p.is_absolute() and '..' not in p.parts and not p.is_symlink()
  data=p.read_bytes();assert hashlib.sha256(data).hexdigest()==binding['sha256']
  if graph:
   b=v['task_binding'];assert set(b)=={'task_id','attempt_id','spec_revision','input_revision'}
   assert b['task_id']==v['lineage']['root_task_id'] and b['attempt_id']==v['lineage']['attempt_id'] and b['input_revision']==v['workspace']['input_revision'] and b['spec_revision']>=1
   assert isinstance(v['validation_contracts'],list)
   for contract in v['validation_contracts']:
    assert set(contract)=={'rule_id','definition_digest','artifacts','context','current_subject','task_acceptance'}
    assert re.fullmatch('[0-9a-f]{64}',contract['definition_digest']) and contract['task_acceptance']=='not_established'
    subject=contract['current_subject'];assert set(subject)=={'task_id','attempt_id','spec_revision','input_revision','version'}
    assert all(subject[key]==b[key] for key in b)
    kind=contract['rule_id'].split(':')[0];context=contract['context'];no_private_config(context)
    assert kind in ('integrity','data','command')
    assert set(context)==({'file_contract'} if kind=='integrity' else {'file_contract','table'} if kind=='data' else {'file_contract','script','validator_id'})
    assert set(subject['version'])==({'kind','dataset_revision'} if kind=='data' else {'kind','workspace_id','base_revision'})
    assert subject['version']==({'kind':'data','dataset_revision':b['input_revision']} if kind=='data' else {'kind':'workspace_artifacts','workspace_id':v['workspace']['id'],'base_revision':b['input_revision']})
    expected=['report','report-2','report-3'] if kind=='data' else ['report']
    assert contract['artifacts']==expected and v['node']['id']=='produce-report'
    if kind=='data':
     table=context['table'];assert table['table_artifact_id']=='report' and table['dataset_rows']==1
     assert table['columns']==[{'name':'x','required':True,'nullable':False,'scalar':{'type':'integer','min':0,'max':10}}]
     assert table['analysis']=={'script_artifact_id':'report-2','environment_artifact_id':'report-3','units':{'x':'bytes'},'timezone':'UTC','seed':7,'scope':{'kind':'full'}}
    if kind=='command':assert context['script'] is None and context['validator_id']=='builtin.fixed-command.v1'
  assert len(v['validation_contracts'])==(1 if v['node']['id']=='produce-report' else 0)
  if binding['candidate'] is not None:
   assert binding['candidate']['sha256']==binding['sha256'] and binding['candidate']['relative_path']==str(p)
 else:
  assert artifact['relative_path']=='inputs/file' and data==b'baseline' and 'candidate' not in binding
  Path('reports').mkdir(exist_ok=True);attempt=v['lineage']['attempt_id'];assert re.fullmatch(r'[A-Za-z0-9._:-]{1,128}',attempt)
  Path('reports/task-request-'+attempt+'.json').write_text(text)
  ticks=Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19]
  Path('reports/fixture-owned-'+attempt+'.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'start_ticks':ticks}))
  outputs={x['artifact']['artifact_id']:x['artifact']['relative_path'] for x in v['outputs']}
  assert all(not Path(path).is_absolute() and '..' not in Path(path).parts for path in outputs.values())
  if v['node']['id']=='produce-report':
   assert artifact['relative_path']=='inputs/file' and data==b'baseline'
   if v['goal'] in ('goal-data-report','goal-data-report-bad'):
    assert set(outputs)==({'report','report-2','report-3','report-4'} if graph else {'report','report-2','report-3'})
    assert {x['artifact_id'] for x in v['node']['outputs']}=={'report','report-2','report-3'}
    script=b'# locally generated reproducibility fixture, not executed\n';environment=b'{"fixture":"local generated ACP","seed":7}\n'
    Path(outputs['report-2']).write_bytes(script);Path(outputs['report-3']).write_bytes(environment)
    revision=v['workspace']['input_revision']
    metadata={'input_revision':revision,'dataset_revision':revision,'input_digests':{'fixed-input':binding['sha256']},'table_artifact_id':'report','row_count':1,'dataset_rows':1,'units':{'x':'bytes'},'timezone':'UTC','seed':7,'scope':{'kind':'full'},'script_sha256':hashlib.sha256(script).hexdigest(),'environment_sha256':hashlib.sha256(environment).hexdigest()}
    Path(outputs['report']).write_text(json.dumps({'metadata':metadata,'rows':[{'x':20 if v['goal'].endswith('-bad') else len(data)}]}))
   else:
    assert v['goal']=='goal-report' and set(outputs)==({'report','report-2'} if graph else {'report'})
    Path(outputs['report']).write_bytes(b'fixed report')
  elif v['node']['kind']=='summary':
   assert graph and binding['candidate'] is not None and {'owner-report','validate-report'}==set(v['node']['dependencies'])
   index=v['node']['id'].removeprefix('independent-');assert index in ('2','4') and v['goal']=='goal-report-extra-'+index
   artifact_id='report-'+index;assert v['node']['outputs']==[{'artifact_id':artifact_id,'contract_id':'output:'+artifact_id}]
   Path(outputs[artifact_id]).write_bytes(('extra report '+index).encode())
  else:raise AssertionError('Only fixed provider nodes may execute')
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


def verify_public_graph_request(request, scope, reuse):
    assert set(request) == GRAPH_TASK_FIELDS
    binding = {'task_id': scope['task_id'], 'attempt_id': request['lineage']['attempt_id'],
        'spec_revision': scope['goal']['spec_revision'], 'input_revision': scope['input_revision']}
    assert request['task_binding'] == binding
    public = []
    selected_outputs = {o['artifact_id'] for o in request['node']['outputs']}
    for permission in reuse:
        artifacts = [artifact for artifact in permission['artifact_ids'] if artifact in selected_outputs]
        if not artifacts:
            continue
        definition = permission['definition']; kind = permission['source']['kind']
        if kind == 'integrity':
            context = {'file_contract': definition['contract']}
        elif kind == 'data':
            context = {'file_contract': definition['file_contract'], 'table': definition['table']}
        else:
            assert kind == 'command'
            context = {'file_contract': definition['contract']['file_contract'], 'script': definition['contract']['script'], 'validator_id': definition['validator_id']}
            assert not {'arguments', 'environment', 'argv0', 'environment_sha256'} & set(context)
        version = {'kind': 'data', 'dataset_revision': binding['input_revision']} if kind == 'data' else {'kind': 'workspace_artifacts', 'workspace_id': scope['workspace_id'], 'base_revision': binding['input_revision']}
        public.append({'rule_id': permission['rule_id'], 'definition_digest': permission['definition_digest'], 'artifacts': artifacts,
            'context': context, 'current_subject': {**binding, 'version': version}, 'task_acceptance': 'not_established'})
    assert request['validation_contracts'] == public, 'Complete public constraints must match approved definitions; no generic/extra context'
    return public


def verify_validation_receipt(runtime: Path, detail: dict, definition: dict, case: str) -> dict:
    """Observe actual receipt/owned-job provenance, never execute a legacy policy."""
    kind = validation_kind(case)
    assert len(detail['validations']) == 1
    receipt = detail['validations'][0]
    node = next(n for n in detail['nodes'] if n['node']['id'] == 'validate-report')
    expected = {'integrity': ('artifact_integrity_only', 'not_established'),
        'data': ('data_contract_only', 'approved_schema_ranges_metadata_only'),
        'command': ('approved_deterministic_command_only', 'approved_fixed_command_observations_only')}[kind]
    assert (receipt['scope'], receipt['correctness']) == expected
    assert receipt['source'] == 'baton_goal_validator' and receipt['task_acceptance'] == 'not_established'
    assert receipt['stop_confirmed'] is True and receipt['io_completion'] == 'worker_joined'
    assert receipt['attempt_id'] == node['attempt_id'] and node['session_id'] is None
    assert node['execution_kind'] == 'validator' and node['job_id'] and node['stop_confirmed'] is True
    assert receipt['rule_id'] == node['node']['executor']['rule_id']
    assert receipt['actual_runtime_ms'] > 0 and node['actual_runtime_ms'] == receipt['actual_runtime_ms']
    assert node['tokens'] is None and node['cost_microunits'] is None
    assert receipt['result_digest'] == sha(json.dumps(receipt['observation'], sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode())
    refs = next(c for c in detail['candidates'] if c['node_id'] == 'produce-report')['artifacts']
    assert receipt['candidate_refs'] == refs
    io = receipt['observation']['goal_io']
    assert io['checked_candidate_bytes'] == sum(r['bytes'] for r in refs)
    assert io['new_declared_artifact_bytes'] == 0 and io['total_filesystem_write_bytes'] is None
    assert io['total_filesystem_write_enforcement'] == 'not_enforced'
    expected_new = io['retained_stdout_bytes'] + io['retained_stderr_bytes']
    assert node['actual_output_bytes'] == expected_new
    if kind != 'command':
        assert expected_new == 0
    elif case == 'command':
        assert expected_new == 22 and receipt['observation']['stdout']['captured_bytes'] == 22
        assert receipt['observation']['stdout']['sha256_of_captured'] == sha(b'fixed command observed')
    observed_subject = receipt['observation']['subject'] if kind == 'integrity' else receipt['observation']['file_integrity']['subject']
    producer = next(c for c in detail['candidates'] if c['node_id'] == 'produce-report')
    assert observed_subject['task_id'] == detail['run']['task_id'] and observed_subject['attempt_id'] == producer['attempt_id']
    assert observed_subject['task_id'] != definition['task_id'] and observed_subject['attempt_id'] != definition['attempt_id']
    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        assert db.execute('PRAGMA user_version').fetchone()[0] == 12
        raw = db.execute('SELECT document FROM goal_graph_validations WHERE id=?', [receipt['id']]).fetchone()[0]
        assert json.loads(raw) == receipt
        job_state, origin_raw = db.execute('SELECT state,document FROM goal_graph_jobs WHERE id=?', [node['job_id']]).fetchone()
        origin = json.loads(origin_raw)
        assert origin['job_kind'] == 'validator' and origin['attempt_id'] == node['attempt_id'] and origin['run_id'] == detail['run']['id']
        assert origin['definition_source'] == {'kind': kind, 'policy_id': definition['id'], 'revision': definition['revision']}
        assert origin['definition_digest'] == receipt['definition_digest'] and origin['candidate_refs'] == refs
        assert job_state == ('completed' if receipt['passed'] else receipt['state'])
        intent_state, executor, intent_raw = db.execute('SELECT state,executor_instance,document FROM dispatch_intents WHERE id=?', [origin['intent_id']]).fetchone()
        assert intent_state == 'stopped' and executor == origin['id'] and json.loads(intent_raw)['state'] == 'stopped'
        event_rows = db.execute('SELECT sequence,document FROM events ORDER BY sequence LIMIT 4097').fetchall()
        assert len(event_rows) <= 4096 and all(len(value.encode()) <= MAX_GRAPH_REVIEW_BYTES for _, value in event_rows)
        events = [(seq, json.loads(value)) for seq, value in event_rows]
        def one(event_kind, source, field, value):
            found = [(seq, event) for seq, event in events if event['kind'] == event_kind and event['source'] == source and event['payload'].get(field) == value]
            assert len(found) == 1, (event_kind, found)
            return found[0]
        prepared = one('goal.validator_prepared', 'goal_core', 'job_id', origin['id'])
        started = one('goal.validator_worker_started', 'goal_core', 'job_id', origin['id'])
        stopped = one('attempt.stopped', 'user_action', 'intent_id', origin['intent_id'])
        settled = one('goal.validation_settled', 'goal_core', 'validation_id', receipt['id'])
        assert prepared[0] < started[0] < stopped[0] < settled[0]
        assert prepared[1]['payload']['origin_digest'] == sha(origin_raw.encode())
        assert started[1]['payload']['owned_thread_started'] is True
        assert stopped[1]['payload']['stop_evidence'] == {'kind': 'executor_stop', 'executor_instance': origin['id'], 'receipt_ref': 'owned-goal-validator-stop:' + origin['id']}
        assert settled[1]['payload']['receipt_digest'] == sha(raw.encode()) and settled[1]['payload']['result_digest'] == receipt['result_digest']
        assert settled[1]['payload']['io_completion'] == 'worker_joined' and settled[1]['payload']['actual_output_bytes'] == expected_new
        assert not any(e['kind'] == 'attempt.prepared' and e['payload'].get('attempt_id') == receipt['attempt_id'] and e['payload'].get('executor_kind') != 'validator' for _, e in events)
        assert all(db.execute('SELECT count(*) FROM ' + table).fetchone()[0] == 0 for table in ['validation_runs', 'data_validation_runs', 'command_validation_runs'])
    return {'receipt': receipt, 'node': node, 'origin': origin, 'job_state': job_state, 'dispatch_intent': json.loads(intent_raw),
        'prepared': prepared, 'worker_started': started, 'attempt_stopped': stopped, 'settled': settled,
        'receipt_sha256': sha(raw.encode()), 'checked_candidate_bytes': io['checked_candidate_bytes'], 'new_output_bytes': expected_new,
        'provider_session_created_for_validator': False, 'old_policy_executions': 0}


def owned_command_child(daemon_pid: int) -> dict | None:
    # Observe only children of threads in this private daemon, never global ps.
    children = set()
    for task in Path(f'/proc/{daemon_pid}/task').glob('*/children'):
        try:
            children.update(int(pid) for pid in task.read_text().split())
        except FileNotFoundError:
            continue
    for pid in children:
        try:
            argv = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
            if argv[:2] != [b'/usr/bin/sleep', b'3']:
                continue
            tail = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
            assert int(tail[1]) == daemon_pid and os.getpgid(pid) == pid
            return {'pid': pid, 'pgid': pid, 'parent_pid': daemon_pid, 'start_ticks': tail[19], 'argv': ['/usr/bin/sleep', '3']}
        except FileNotFoundError:
            continue
    return None


def expected_data_files(input_revision, input_sha, *, bad=False):
    script = b'# locally generated reproducibility fixture, not executed\n'
    environment = b'{"fixture":"local generated ACP","seed":7}\n'
    metadata = {'input_revision': input_revision, 'dataset_revision': input_revision, 'input_digests': {'fixed-input': input_sha},
        'table_artifact_id': 'report', 'row_count': 1, 'dataset_rows': 1, 'units': {'x': 'bytes'}, 'timezone': 'UTC', 'seed': 7,
        'scope': {'kind': 'full'}, 'script_sha256': sha(script), 'environment_sha256': sha(environment)}
    return {'report': json.dumps({'metadata': metadata, 'rows': [{'x': 20 if bad else 8}]}).encode(),
        'report-2': script, 'report-3': environment}


def validation_self_check():
    rows = []
    def check(name, condition):
        assert condition, name
        rows.append({'name': name, 'status': 'passed'})
    compile(VALIDATION_ACP, '<generated-graph-validation-acp>', 'exec')
    check('generated validation ACP compiles', True)
    check('legacy base-key set vs exact Graph two-field paired extension', GRAPH_TASK_FIELDS - LEGACY_TASK_FIELDS == {'validation_contracts', 'task_binding'} and not LEGACY_TASK_FIELDS & {'validation_contracts', 'task_binding'})
    for case in VALIDATION_CASES:
        kind = validation_kind(case); count = 4 if kind == 'data' else 2
        outputs = {'report' if i == 1 else f'report-{i}': 'output:report' if i == 1 else f'output:report-{i}' for i in range(1, count + 1)}
        scope = {'task_id': 'fixture-new-task', 'workspace_id': 'fixture-ws', 'goal': {}, 'authority_revision': 1,
            'binding': {'fixture': 'opaque'}, 'program_sha256': 'a' * 64,
            'inputs': {'fixed-input': {'contract_id': 'input:fixed-input', 'sha256': sha(b'baseline')}}, 'outputs': outputs,
            'contracts': {contract: ['goal-owner-review'] + ([kind + ':fixture-definition'] if i == 0 or kind == 'data' and i < 3 else []) for i, contract in enumerate(outputs.values())},
            'required_acceptance_rules': ['goal-owner-review']}
        initial, plan = validation_plans(scope, case)
        check(case + ' exact five node types', [n['kind'] for n in plan['nodes']] == ['work', 'validate', 'owner_approval', 'summary', 'owner_approval'])
        check(case + ' original independent proposal preserved', len(initial['nodes']) == count and all(not n['dependencies'] for n in initial['nodes']))
        check(case + ' Validate is owned executor with no output authority', plan['nodes'][1]['executor']['kind'] == 'validator' and not plan['nodes'][1]['outputs'] and not plan['nodes'][1]['write_set'])
        check(case + ' validation and Owner gate Summary', set(plan['nodes'][3]['dependencies']) == {'validate-report', 'owner-report'} and plan['nodes'][3]['required_inputs'][0]['source']['candidate_for_validation'] is False)
        check(case + ' complete producer subject set', len(plan['nodes'][0]['outputs']) == (3 if kind == 'data' else 1) and len(plan['nodes'][1]['required_inputs']) == (3 if kind == 'data' else 1))
        selection = scope_selection({'id': 'new', 'revision': 1}, {'id': 'grant', 'revision': 1}, count=count, max_nodes=5, kind=kind)
        check(case + ' unchanged bounded shared eight-attempt policy', selection['root_budget'] == selection['project_budget'] and selection['project_budget']['attempts'] == 8 and selection['project_budget']['runtime_ms'] == 10_000 and selection['project_budget']['parallel'] == 1)
        _, _, policy = definition_selection(kind, 'actual-session-required', case)
        if case == 'command-timeout':
            check('timeout keeps1500ms/500cleanup/512x2 and exact sleep3', policy['arguments'] == ['3'] and policy['wall_time_ms'] == 1500 and policy['cleanup_time_ms'] == 500 and policy['stdout_limit'] == policy['stderr_limit'] == 512)
    check('Data bad fixture changes range value only', json.loads(expected_data_files('revision', sha(b'baseline'), bad=True)['report'])['rows'] == [{'x': 20}])
    check('Command new stdout charge22 differs from checked producer12', len(b'fixed command observed') == 22 and len(b'fixed report') == 12)
    return {'status': 'passed', 'dimension': 'static five-node plans/legacy/Graph exact named-key contracts/generated Python/explicit bounded policy controls; no daemon/browser/validator run', 'count': len(rows), 'checks': rows}


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
    case = getattr(args, 'validation_case', None)
    kind = validation_kind(case) if case else None
    data_case = kind == 'data'
    negative_case = case in ('data-range', 'command-timeout')
    try:
        assert args.batond and args.candidate_manifest and args.frontend
        binary, frontend = args.batond.resolve(strict=True), args.frontend.resolve(strict=True)
        manifest_raw = args.candidate_manifest.resolve(strict=True).read_bytes()
        manifest = verify_candidate(binary, manifest_raw)  # before any process/token/listener
        if case:
            assert sha(binary.read_bytes()) != 'fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e', 'Old Owner-only Core5541 is not the new Graph validation-contract API'
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
    report = {'status': 'failed_or_interrupted', 'mode': 'validation_' + case if case else 'owner_only_four_nodes',
        'dimension': 'actual built React/Linux Chromium/finite test HTTP/authenticated Unix Core/generated owned ACP',
        'not_claimed': ([] if case else ['deterministic Validate five-node flow']) + ['full Graph failure matrix', 'native OS Owner dialog',
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
        provider.write_text('#!' + sys.executable + '\n' + (VALIDATION_ACP if case else GRAPH_ACP)); provider.chmod(0o700)
        report['generated_program_sha256'] = sha(provider.read_bytes())
        log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        server = thread = browser = None
        workspace = None
        command_identity = None
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
                if case:
                    assert len(json.dumps(proposal, ensure_ascii=False, separators=(',', ':')).encode()) <= MAX_LEGACY_REVIEW_BYTES
                    report['observations'].setdefault('complete_private_resource_proposals', []).append(proposal)
                report['private_reviews'].append({'kind': kind, 'proposal_id': proposal['id'], 'snapshot_utf8_bytes': len(raw), 'snapshot_sha256': sha(raw), 'authority': 'explicit private generated resource fixture; not OS dialog'})
                return result('approvals.finalize', {'proposal_id': proposal['id']}, 'finalize:' + proposal['id'], proposal['revision'])
            connection = approved('connections.propose', {'name': 'Generated Graph account-free ACP', 'provider': 'custom_acp',
                'executable': str(provider), 'arguments': []}, 'graph:connection', 'connection')
            wait(lambda: result('connections.list', {}), lambda rows: any(x['id'] == connection['id'] and x['probe_state'] == 'ready' for x in rows))
            source = root / 'source'; source.mkdir(mode=0o700); (source / 'file').write_bytes(b'baseline')
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}
            def git(*argv):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *argv], env=git_env, text=True).strip()
            if not case:
                git('init', '-q', '--initial-branch=main'); git('add', 'file'); git('commit', '-qm', 'generated bounded Graph input')
            project = result('projects.import', {'path': str(source), 'name': 'Generated Graph Owner-only'}, 'graph:project')
            if case:
                preview = result('data_workspace_previews.start', {'project_id': project['id'],
                    'files': [{'artifact_id': 'source', 'relative_path': 'file', 'max_bytes': 1024, 'expected_sha256': None}],
                    'limits': {'max_file_bytes': 1024, 'max_total_bytes': 1024, 'max_files': 1}}, 'graph:preview', project['revision'])
                preview = wait(lambda: result('data_workspace_previews.get', {'preview_id': preview['id']}), lambda p: p['state'] == 'verified')
                creation = approved('data_workspaces.propose', {'preview_id': preview['id']}, 'graph:copy', 'data_workspace', preview['revision'])
                copied = wait(lambda: result('data_workspaces.get', {'creation_id': creation['id']}), lambda p: p['state'] == 'ready')
                assert copied['io_completion'] == 'worker_joined'
                workspace = next(w for w in result('workspaces.list', {'project_id': project['id']}) if w['id'] == copied['workspace_id'])
                assert workspace['identity']['kind']['kind'] == 'directory'
            else:
                workspace = result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'graph:workspace', project['revision'])
            workspace_root = Path(workspace['root']).resolve(strict=True)
            assert workspace_root.is_relative_to(runtime / 'workspaces')
            grant = approved('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'],
                'max_attempts': 8 if case else 2, 'max_concurrency': 1,
                'max_duration_ms': (10_000 if kind == 'integrity' else 30_000) if case else 600_000}, 'graph:grant', 'grant')
            task = result('tasks.create', {'project_id': project['id'], 'goal': 'Generated Graph Work Owner Summary Owner'}, 'graph:task')
            limits = {'attempts': 2, 'parallel': 1, 'runtime_ms': 10_000, 'output_bytes': 8192, 'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
            bootstrap = None
            definition = None
            bootstrap_bytes = 0
            if case:
                selection = scope_selection(task, grant, count=3 if data_case else 1, max_nodes=5 if data_case else 2, kind=kind)
                bootstrap_scope = approved('goal_scopes.propose', selection, 'graph:bootstrap-scope', 'goal')
                bootstrap_plan = scoped_agent_plan(bootstrap_scope, 3 if data_case else 1, 'goal-data-report' if data_case else 'goal-report')
                result('goal_plans.propose', {'scope_id': bootstrap_scope['id'], 'plan_json': json.dumps(bootstrap_plan)}, 'graph:bootstrap-plan1', 0)
                if data_case:
                    bootstrap_plan = {**bootstrap_plan, 'revision': 2, 'expected_revision': 1, 'nodes': [collapsed_producer(bootstrap_plan, 3)]}
                    result('goal_plans.propose', {'scope_id': bootstrap_scope['id'], 'plan_json': json.dumps(bootstrap_plan)}, 'graph:bootstrap-plan2', 1)
                legacy = approved('goal_runs.propose', {'scope_id': bootstrap_scope['id'], 'expected_plan_revision': bootstrap_plan['revision'],
                    'expected_ledger_revision': result('system.snapshot', {})['sequence']}, 'graph:bootstrap-execute', 'goal_execution')
                bootstrap = wait(lambda: result('goal_runs.get', {'run_id': legacy['id']}), lambda d: d['run']['state'] == 'awaiting_acceptance')
                old_node = bootstrap['nodes'][0]
                assert old_node['stop_confirmed'] is True and bootstrap['task_accepted'] is False
                assert result('sessions.get', {'session_id': old_node['session_id']})['state'] == 'stopped'
                bootstrap_raw = (workspace_root / ('reports/task-request-' + old_node['attempt_id'] + '.json')).read_bytes()
                bootstrap_request = json.loads(bootstrap_raw)
                assert set(bootstrap_request) == LEGACY_TASK_FIELDS
                owned = json.loads((workspace_root / ('reports/fixture-owned-' + old_node['attempt_id'] + '.json')).read_text())
                assert ticks(owned['pid']) != owned['start_ticks']
                bootstrap_files = {o['artifact_id']: (workspace_root / ('reports/report.txt' if o['artifact_id'] == 'report' else 'reports/' + o['artifact_id'] + '.txt')).read_bytes() for o in old_node['node']['outputs']}
                bootstrap_bytes = sum(map(len, bootstrap_files.values()))
                assert old_node['actual_output_bytes'] == bootstrap_bytes
                report['observations']['owned_task_requests'] = [{'origin': 'real_legacy_bootstrap', 'utf8_bytes': len(bootstrap_raw),
                    'sha256': sha(bootstrap_raw), 'raw_utf8': bootstrap_raw.decode(), 'request': bootstrap_request, 'owned_process_identity': owned, 'owned_process_gone': True}]
                method, approval_kind, policy_selection = definition_selection(kind, old_node['session_id'], case)
                definition = approved(method, policy_selection, 'graph:source-definition', approval_kind)
                result('goal_runs.cancel', {'run_id': legacy['id']}, 'graph:cancel-bootstrap', bootstrap['run']['revision'])
                cancelled = wait(lambda: result('goal_runs.get', {'run_id': legacy['id']}), lambda d: d['run']['state'] == 'cancelled')
                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
                    db.execute('PRAGMA query_only=ON')
                    assert all(db.execute('SELECT count(*) FROM ' + table).fetchone()[0] == 0 for table in ['validation_runs', 'data_validation_runs', 'command_validation_runs'])
                    assert db.execute('SELECT consumed FROM budgets WHERE scope=?', ['project:' + project['id']]).fetchone()[0] == 1
                    assert db.execute('SELECT count(*) FROM attempts').fetchone()[0] == 1
                report['observations']['bootstrap'] = {'scope': bootstrap_scope, 'run': bootstrap, 'cancelled': cancelled,
                    'definition': definition, 'definition_selection': policy_selection,
                    'outputs': {k: {'bytes': len(v), 'sha256': sha(v)} for k, v in bootstrap_files.items()},
                    'output_bytes': bootstrap_bytes, 'legacy_policy_executions': 0, 'project_consumed_attempts': 1,
                    'task_id': task['id'], 'provider_sessions': 1, 'new_task_must_not_reset_project_history': True}
                grant = next(g for g in result('grants.list', {'project_id': project['id']}) if g['id'] == grant['id'])
                task = result('tasks.create', {'project_id': project['id'], 'goal': 'Generated five-node ' + case + ' Graph with real bootstrap history'}, 'graph:new-task')
                assert task['id'] != bootstrap_scope['task_id']
                selection = scope_selection(task, grant, count=4 if data_case else 2, max_nodes=5, kind=kind)
                reference = {'kind': kind, 'policy_id': definition['id'], 'revision': definition['revision']}
                for i, output in enumerate(selection['outputs']):
                    output['validators'] = [reference] if i == 0 or data_case and i < 3 else []
                scope = approved('goal_scopes.propose', selection, 'graph:new-scope', 'goal')
                assert scope['id'] != bootstrap_scope['id'] and scope['workspace_id'] == bootstrap_scope['workspace_id']
            else:
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
                context = browser.new_context(viewport={'width': 1488, 'height': 1400 if case else 1200}, locale='en-US')
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
                    for proposal in validation_plans(scope, case) if case else owner_only_plans(scope):
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
                    before = result('sessions.list', {}); assert len(before) == (1 if case else 0)
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
                    if case:
                        reuse = proposal['snapshot']['definition_reuse']; rebind = proposal['snapshot']['subject_rebinding']
                        assert len(reuse) == 1 and reuse[0]['source'] == {'kind': kind, 'policy_id': definition['id'], 'revision': definition['revision']}
                        assert reuse[0]['definition'] == definition and reuse[0]['goal_acceptance'] is False
                        assert reuse[0]['artifact_ids'] == (['report', 'report-2', 'report-3'] if data_case else ['report'])
                        assert rebind['mode'] == 'this_run_current_plan_owned_producer_only' and rebind['workspace_id'] == workspace['id']
                        assert rebind['allows_legacy_candidate_backfill'] is False and rebind['allows_old_policy_execution_reuse'] is False and rebind['allows_new_workspace'] is False
                        assert rebind['requires_current_scope_and_inputs'] is True and rebind['requires_owned_stop_and_capture'] is True and rebind['requires_current_definition_digest'] is True
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
                    if negative_case:
                        if case == 'command-timeout':
                            command_identity = wait(lambda: owned_command_child(daemon.pid), lambda p: p is not None, seconds=5)
                            report['observations']['owned_command_child'] = command_identity
                        failed = wait(view, lambda d: len(d['validations']) == 1)
                        proof = verify_validation_receipt(runtime, failed, definition, case)
                        assert proof['receipt']['definition_digest'] == proposal['snapshot']['definition_reuse'][0]['definition_digest']
                        assert proof['receipt']['passed'] is False and proof['receipt']['state'] == 'failed'
                        assert failed['run']['state'] == 'blocked' and failed['task_accepted'] is False and failed['task_acceptance'] is None
                        assert not failed['owner_decisions'] and len(failed['candidates']) == 1
                        assert all(n['state'] == 'not_started' and n['attempt_id'] is None and n['session_id'] is None for n in failed['nodes'] if n['node']['kind'] in ('owner_approval', 'summary'))
                        candidate = failed['candidates'][0]
                        expected_files = expected_data_files(scope['input_revision'], scope['inputs']['fixed-input']['sha256'], bad=True) if data_case else {'report': b'fixed report'}
                        for artifact in candidate['artifacts']:
                            pinned_artifact(workspace_root, artifact, expected_files[artifact['artifact_id']])
                        if case == 'data-range':
                            assert proof['receipt']['observation']['failure'] is not None
                            assert json.loads((workspace_root / 'reports/report.txt').read_bytes())['rows'] == [{'x': 20}]
                        else:
                            assert proof['receipt']['observation']['reason'] == 'deadline_exceeded'
                            assert proof['receipt']['observation']['process_started'] is True
                            assert proof['receipt']['observation']['cleanup']['owned_leader_reaped'] is True
                            assert ticks(command_identity['pid']) != command_identity['start_ticks']
                        session = result('sessions.get', {'session_id': candidate['session_id']})
                        assert session['state'] == 'stopped'
                        raw = (workspace_root / ('reports/task-request-' + candidate['attempt_id'] + '.json')).read_bytes()
                        request = json.loads(raw); assert set(request) == GRAPH_TASK_FIELDS
                        verify_public_graph_request(request, scope, proposal['snapshot']['definition_reuse'])
                        assert request['run_id'] == run_id and request['lineage']['attempt_id'] == candidate['attempt_id']
                        owned = json.loads((workspace_root / ('reports/fixture-owned-' + candidate['attempt_id'] + '.json')).read_text())
                        assert ticks(owned['pid']) != owned['start_ticks']
                        report['observations']['owned_task_requests'].append({'origin': 'real_graph_failure_producer', 'utf8_bytes': len(raw), 'sha256': sha(raw), 'raw_utf8': raw.decode(), 'request': request, 'candidate': candidate, 'owned_process_identity': owned, 'owned_process_gone': True})
                        with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
                            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
                            assert db.execute('SELECT count(*) FROM attempts').fetchone()[0] == 3
                            assert db.execute('SELECT count(*) FROM sessions').fetchone()[0] == 2
                            assert db.execute('SELECT count(*) FROM goal_graph_acceptances').fetchone()[0] == 0
                            for scope_name, consumed in [('project:' + project['id'], 3), ('root:' + task['id'], 2)]:
                                assert db.execute('SELECT consumed,active FROM budgets WHERE scope=?', [scope_name]).fetchone() == (consumed, 0)
                            new_bytes = sum(a['bytes'] for a in candidate['artifacts']) + proof['new_output_bytes']
                            for key in ['project:' + project['id'], 'root-grant:' + grant['id']]:
                                ledger = json.loads(db.execute('SELECT document FROM goal_ledgers WHERE scope=?', [key]).fetchone()[0])
                                assert ledger['observed_attempts'] == 3 and ledger['output_bytes'] == bootstrap_bytes + new_bytes
                            capture_doc = db.execute('SELECT document FROM goal_graph_candidates WHERE id=?', [candidate['id']]).fetchone()[0]
                            capture = db.execute("SELECT sequence,document FROM events WHERE json_extract(document,'$.kind')='goal.candidate_captured' AND json_extract(document,'$.payload.candidate_id')=?", [candidate['id']]).fetchall()
                            assert len(capture) == 1 and capture[0][0] == candidate['capture_receipt_sequence']
                            assert json.loads(capture[0][1])['payload']['receipt_digest'] == sha(capture_doc.encode())
                            assert candidate['stop_receipt_sequence'] < candidate['capture_receipt_sequence']
                            report['observations']['negative_capture_event'] = [capture[0][0], json.loads(capture[0][1])]
                        report['observations']['real_validator'] = proof
                        report['observations']['blocked_graph'] = failed
                        refresh(); history = graph.get_by_test_id('goal-graph-history')
                        expect(history.get_by_test_id('goal-graph-node')).to_have_count(5)
                        expect(graph.get_by_test_id('goal-graph-owner-approve')).to_have_count(0)
                        expect(history.get_by_text('Task acceptance pending', exact=True)).to_be_visible()
                        expect(history.locator('[data-node-id="validate-report"] > details > summary')).to_contain_text('Failed')
                        shot('01-five-node-' + case + '-blocked-before-owner', history)
                        assert not errors, errors
                        assert (source / 'file').read_bytes() == b'baseline' and assets() == report['frontend_assets_sha256']
                        assert sha(binary.read_bytes()) == report['daemon_sha256'] and sha(Path(__file__).read_bytes()) == report['harness_sha256']
                        passed('real ' + case + ' validation failure joins actual worker and blocks Owner/Summary/TaskAccepted without refund/reset')
                        passed('actual built UI shows failed Validate and pending acceptance; original sources/assets unchanged')
                        report['status'] = 'passed'
                    else:
                        initial = ready_owner('owner-report')
                        assert initial['task_accepted'] is False and initial['task_acceptance'] is None and len(initial['candidates']) == 1 and not initial['owner_decisions']
                        assert all(n['session_id'] is None for n in initial['nodes'] if n['node']['id'] != 'produce-report')
                        assert not (workspace_root / ('reports/report-4.txt' if data_case else 'reports/report-2.txt')).exists()
                        first_artifact = initial['candidates'][0]['artifacts'][0]
                        expected_first = expected_data_files(scope['input_revision'], scope['inputs']['fixed-input']['sha256']) if data_case else {'report': b'fixed report'}
                        for artifact in initial['candidates'][0]['artifacts']:
                            pinned_artifact(workspace_root, artifact, expected_first[artifact['artifact_id']])
                        if case:
                            validation_proof = verify_validation_receipt(runtime, initial, definition, case)
                            assert validation_proof['receipt']['definition_digest'] == proposal['snapshot']['definition_reuse'][0]['definition_digest']
                            assert validation_proof['receipt']['passed'] is True and validation_proof['node']['state'] == 'validated'
                            assert validation_proof['node']['acceptance_state'] == 'validated'
                            assert next(n for n in initial['nodes'] if n['node']['id'] == 'produce-report')['acceptance_state'] == 'validated'
                            report['observations']['real_validator'] = validation_proof
                        refresh(); expect(graph.get_by_test_id('goal-graph-node')).to_have_count(5 if case else 4)
                        shot('01-work-candidate-before-owner', graph.get_by_test_id('goal-graph-history'), 'owner-report')
                        report['observations']['before_first_owner'] = initial
                        first_decision = owner_decision(initial, 'owner-report', 'graph:owner-report')
                        assert first_decision['task_accepted'] is False
                        after = ready_owner('owner-summary')
                        assert len(after['candidates']) == 2 and after['task_accepted'] is False
                        summary_candidate = next(c for c in after['candidates'] if c['node_id'] == ('independent-4' if data_case else 'independent-2'))
                        assert summary_candidate['consumed_inputs'][0]['candidate'] == first_artifact
                        pinned_artifact(workspace_root, summary_candidate['artifacts'][0], b'extra report 4' if data_case else b'extra report 2')
                        refresh(); shot('02-summary-candidate-before-final-owner', graph.get_by_test_id('goal-graph-history'), 'owner-summary')
                        report['observations']['before_final_owner'] = after
                        final_decision = owner_decision(after, 'owner-summary', 'graph:owner-summary')
                        assert final_decision['task_accepted'] is True
                        accepted = view(); final_task = result('tasks.get', {'task_id': task['id']}); acceptance = accepted['task_acceptance']
                        assert accepted['task_accepted'] is True and accepted['run']['task_accepted'] is True and accepted['run']['state'] == 'accepted'
                        assert final_task['state'] == 'accepted' and final_task['acceptance_ref'] == acceptance['id']
                        assert final_task['acceptance_sequence'] == acceptance['sequence'] and final_task['revision'] == acceptance['task_revision']
                        assert acceptance['source'] == 'baton_goal_acceptance' and len(acceptance['artifact_refs']) == (4 if data_case else 2)
                        assert acceptance['validation_ids'] == ([initial['validations'][0]['id']] if case else []) and len(acceptance['owner_decision_ids']) == 2
                        assert set(acceptance['owner_decision_ids']) == {d['id'] for d in accepted['owner_decisions']}
                        assert all(d['decision'] == 'accepted' and d['source'] == 'local_owner_confirmation' for d in accepted['owner_decisions'])
                        # OO-WIRE4: do not grandfather old agent/pending projections.
                        for n in accepted['nodes']:
                            assert n['acceptance_state'] == ('validated' if n['node']['kind'] == 'validate' else 'accepted'), ('Fresh rule-specific projection required', n)
                            if n['node']['kind'] == 'owner_approval':
                                assert n['execution_kind'] == 'owner' and n['state'] == 'accepted' and n['stop_confirmed'] is False
                                assert n['session_id'] is None and n['attempt_id'] is None
                            elif n['node']['kind'] == 'validate':
                                assert case and n['execution_kind'] == 'validator' and n['state'] == 'validated' and n['stop_confirmed'] is True
                                assert n['session_id'] is None and n['attempt_id'] and n['job_id']
                            else:
                                assert n['execution_kind'] == 'connection' and n['state'] == 'candidate' and n['stop_confirmed'] is True
                        passed('real private Owner decisions and declared checks gate dependent Summary and all-required TaskAccepted with canonical fresh projections')
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
                                assert set(request) == (GRAPH_TASK_FIELDS if case else LEGACY_TASK_FIELDS)
                                if case:
                                    verify_public_graph_request(request, scope, proposal['snapshot']['definition_reuse'])
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
                                assert budgets[key] == {'consumed': (4 if key.startswith('project:') else 3) if case else 2, 'active': 0}
                            ledgers = {k: json.loads(v) for k, v in db.execute('SELECT scope,document FROM goal_ledgers')}
                            produced_bytes = sum(a['bytes'] for c in accepted['candidates'] for a in c['artifacts'])
                            validator_output = validation_proof['new_output_bytes'] if case else 0
                            expected_project_output = bootstrap_bytes + produced_bytes + validator_output
                            for key in ['root-grant:' + grant['id'], 'project:' + project['id']]:
                                assert ledgers[key]['observed_attempts'] == (4 if case else 2) and ledgers[key]['output_bytes'] == expected_project_output
                            if case:
                                assert db.execute('SELECT count(*) FROM attempts').fetchone()[0] == 4
                                assert db.execute('SELECT count(*) FROM sessions').fetchone()[0] == 3
                                assert db.execute('SELECT sum(json_extract(document,\'$.actual_output_bytes\')) FROM goal_run_reservations WHERE run_id=?', [run_id]).fetchone()[0] == produced_bytes + validator_output
                                if kind == 'integrity':
                                    assert expected_project_output == 38
                                elif kind == 'command':
                                    assert expected_project_output == 60
                            report['observations']['durable_acceptance'] = {'event': commits[0], 'receipt_sha256': sha(raw.encode()), 'budgets': budgets, 'ledgers': ledgers}
                        assert len(result('sessions.list', {})) == (3 if case else 2)
                        assert result('tasks.cancel', {'task_id': final_task['id']}, 'graph:terminal-cancel', final_task['revision']) == final_task
                        refused = host.call(FixedCall('sessions.create', {'task_id': final_task['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'grant_id': grant['id']}, 'graph:terminal-no-reopen', final_task['revision']))
                        assert refused.get('error')
                        for _ in range(3):
                            stable = view(); assert stable['run'] == accepted['run'] and stable['ledger_revision'] == accepted['ledger_revision']
                        passed('fresh SHA/FD candidates, owned-reaped TaskRequests, durable TaskAccepted event/digest and cumulative bootstrap plus run budgets; terminal task cannot reopen')
                        report['observations']['accepted'] = {'graph': accepted, 'task': final_task, 'ordinary_session_refusal': refused['error']}
                        page.get_by_role('button', name='Refresh', exact=True).click()
                        # Global refresh reloads Task acceptance_ref/sequence; Graph-only refresh cannot prove this.
                        panel = page.get_by_test_id('goal-planning'); outer = panel.get_by_test_id('goal-execution').first
                        expand(panel.locator(':scope > details')); expand(outer.locator(':scope > details'))
                        graph = panel.get_by_test_id('goal-graph-panel'); refresh()
                        history = graph.get_by_test_id('goal-graph-history')
                        expect(history.get_by_text('Task accepted', exact=True)).to_be_visible()
                        expect(history.get_by_test_id('goal-graph-node')).to_have_count(5 if case else 4)
                        shot('03-task-accepted-five-node-' + kind if case else '03-task-accepted-four-node-owner-chain', history)
                        assert not errors, errors
                        assert (source / 'file').read_bytes() == b'baseline' and assets() == report['frontend_assets_sha256']
                        assert sha(binary.read_bytes()) == report['daemon_sha256'] and sha(Path(__file__).read_bytes()) == report['harness_sha256']
                        passed('actual built UI shows authoritative TaskAccepted complete scoped history; no browser errors or asset/source drift')
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
            if case == 'command-timeout' and command_identity is None and daemon.poll() is None:
                command_identity = owned_command_child(daemon.pid)
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
            if command_identity is not None:
                observed = ticks(command_identity['pid'])
                if observed == command_identity['start_ticks']:
                    pid = command_identity['pid']
                    try:
                        assert command_identity['pgid'] == pid and os.getpgid(pid) == pid
                        assert Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')[:2] == [b'/usr/bin/sleep', b'3']
                        os.killpg(pid, signal.SIGKILL)
                        report['cleanup'].append({'action': 'failed owned Command cleanup required exact private PID/group kill; not stop proof', 'identity': command_identity})
                    except (OSError, AssertionError) as failure:
                        report['cleanup'].append({'action': 'refused unsafe Command cleanup', 'identity': command_identity, 'failure': clean(failure)})
                    report['status'] = 'failed_or_interrupted'
                else:
                    report['cleanup'].append({'action': 'actual owned Command original identity not alive', 'expected_identity': command_identity,
                        'observed_start_ticks': observed, 'pid_exists': observed is not None, 'original_process_not_alive': True, 'signal_sent': False})
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
            expected_provider_count = (2 if negative_case else 3) if case else 2
            if report['status'] == 'passed':
                if len(expected_identities) != expected_provider_count or len(inspected_identities) != expected_provider_count or any(i not in inspected_identities for i in expected_identities):
                    report['status'] = 'failed_or_interrupted'
                    report['cleanup'].append({'action': 'required generated worker identities were not fully inspected', 'required_count': expected_provider_count, 'expected': expected_identities, 'inspected': inspected_identities})
            if report['http_unexpected_errors']:
                report['status'] = 'failed_or_interrupted'
            if report['status'] == 'passed':
                passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and all exact generated worker identities gone')
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
    parser.add_argument('--validation-self-check', action='store_true')
    parser.add_argument('--owner-only', action='store_true')
    parser.add_argument('--validation-case', choices=VALIDATION_CASES)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--batond', type=Path)
    parser.add_argument('--candidate-manifest', type=Path)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': 12}
    assert sum([args.self_check, args.driver_self_check, args.transport_self_check, args.validation_self_check, args.owner_only, bool(args.validation_case)]) <= 1, 'Choose one explicit mode'
    if args.validation_self_check:
        report = {**attribution, **validation_self_check()}
        (output / 'validation-static-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'dimension': report['dimension'], 'count': report['count'], 'status': report['status']}))
        return 0
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
    if args.owner_only or args.validation_case:
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
