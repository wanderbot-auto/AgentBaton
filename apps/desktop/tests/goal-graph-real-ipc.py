#!/usr/bin/env python3
"""Preparation for actual Graph UI/Unix IPC acceptance, never synthetic success.

The finite HTTP boundary permits scoped reads, existing plan storage and Graph
cancel/advance. Actual Graph/Owner proposals and Accept/Reject belong only to
an authenticated private host fixture OUTSIDE HTTP. Runtime awaits the Core12
worker/finalizer/TaskAccepted fixture; --self-check is a pure boundary check.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import socket
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
PENDING_DEPENDENCIES = (
    'Core12 fixed graph execution finalizer/worker and actual generated TaskRequest fixture',
    'Exact approved deterministic Validate definition and producer subject-rebinding fixture',
    'Private Owner Accept/Reject/replay current-ref/CAS fixture with authentic durable decision receipts',
    'Dependent Summary consumption and all-required TaskAccepted durable event/projection fixture',
    'Drift, owned-stop unknown, cancellation, ledger/CAS and explicit rejection actual negative fixtures',
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--batond', type=Path)
    parser.add_argument('--candidate-manifest', type=Path)
    parser.add_argument('--frontend', type=Path)
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': 12}
    if args.self_check:
        report = {**attribution, **boundary_self_check()}
        (output / 'boundary-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'dimension': report['dimension'], 'count': report['count'], 'status': report['status']}))
        return 0
    # Deliberately no daemon invocation/HTTP listener/process until the actual
    # new Core graph fixture is supplied and the complete driver is connected.
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
