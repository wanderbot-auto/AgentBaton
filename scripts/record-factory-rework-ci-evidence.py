#!/usr/bin/env python3
"""Finite post-owned-shutdown diagnostics, not a test-result or authority verifier.

Only explicitly retained rework-* directories are inspected. Raw SQLite, IDs,
paths, snapshots, messages and Provider output never enter the public projection.
No Store.open, migration, recovery, daemon or network action is performed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import stat
import sys
import tempfile

CONTRACT = 'agentbaton.factory-rework-progress.v1'
SCHEMA = 17
MAX_DB = 16 * 1024 * 1024
MAX_CASES = 16
MAX_ENTRIES = 64
MAX_ROWS = 512
MAX_PUBLIC = 128 * 1024
MAX_ASSOCIATED_EVENTS = 16
MAX_EVENT_ROWS = 4096
EVENT_KINDS = ('goal.run_blocked', 'goal.node_settled', 'goal.capture_settled',
               'goal.validation_settled', 'goal.owner_node_settled', 'goal.factory_failure_confirmed')
# Optional historical-event diagnostics only; never runtime/admission/stop proof.
CAPTURE_TIMING_FIELDS = ('producer_elapsed_ms', 'runtime_reservation_ms',
    'capture_worker_started_queue_ms', 'capture_worker_completed_queue_ms',
    'capture_join_observed_queue_ms', 'capture_worker_elapsed_ms')
# Same finite production codes as goal_runtime_diagnostics::error_code.
REASON_CODES = frozenset(('WORKSPACE_BUSY', 'STALE_REVISION', 'INPUT_CHANGED',
    'AUTH_REQUIRED', 'RESOURCE_LIMIT', 'DEADLINE_EXCEEDED', 'OUTCOME_UNKNOWN',
    'BUDGET_UNKNOWN', 'INTERNAL_ERROR', 'PERMISSION_DENIED',
    'CAPABILITY_UNSUPPORTED', 'CONFIG_CONFLICT', 'INTERNAL_ADVANCE_INTERRUPTED'))
VALIDATION_IO_STATES = frozenset(('observed', 'observation_rejected',
                                'not_started_or_preflight_rejected'))
VALIDATION_ERROR_CODES = REASON_CODES | frozenset(('CANCEL_REQUESTED',
    'ENFORCEMENT_UNAVAILABLE', 'INVALID_REQUEST'))
RUN_STATES = frozenset(('queued', 'running', 'awaiting_acceptance', 'blocked',
    'failed', 'cancel_requested', 'cancelled', 'outcome_unknown', 'accepted', 'completed'))
NODE_STATES = RUN_STATES | frozenset(('not_started', 'ready', 'reserved', 'stopping',
    'candidate', 'validated', 'awaiting_owner'))
FAILURES = frozenset(('REWORK_LIST_INVALID', 'REWORK_OUTCOME_UNKNOWN',
    'REWORK_EARLY_ACCEPTANCE', 'REWORK_REVISION2_TERMINAL'))
METHODS = ('factory_runs.get', 'factory_child_goals.list', 'factory_child_goals.get')
PHASES = frozenset(('wait_rework_awaiting_owner', 'owner_exact_second_review',
    'wait_parent_resume_completed', 'whitelist_control'))
STAGES = frozenset(('request_begin', 'response_received', 'transport_failure',
    'milestone', 'owner_write_returned'))
TABLES = ('factory_child_goal_runs', 'goal_runs', 'goal_run_nodes', 'goal_graph_validations')
INTEGRATION_TABLES = ('integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences')
NAME = re.compile(r'rework-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z')


class Invalid(Exception):
    """Fixed classification only; never copy an underlying exception message."""


def need(condition):
    if not condition:
        raise Invalid()


def integer(value, maximum=(1 << 64) - 1, nullable=False, signed=False):
    if value is None and nullable:
        return None
    need(type(value) is int and (-(1 << 63) if signed else 0) <= value <= maximum)
    return value


def boolean(value, nullable=False):
    if value is None and nullable:
        return None
    need(type(value) is bool)
    return value


def enum(value, choices, nullable=False):
    if value is None and nullable:
        return None
    need(type(value) is str and value in choices)
    return value


def strict_json(raw):
    def object_pairs(pairs):
        out = {}
        for key, value in pairs:
            need(key not in out)
            out[key] = value
        return out
    return json.loads(raw, object_pairs_hook=object_pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(Invalid()))


def progress(value):
    """Only the producer's finite array projection, never arbitrary list JSON."""
    need(type(value) is dict)
    count = integer(value.get('summary_count'), MAX_ROWS, nullable=True)
    failure = enum(value.get('failure_code'), FAILURES, nullable=True)
    if count != 1:
        need(set(value) == {'summary_count', 'failure_code'})
        need((count == 0 and failure is None) or failure == 'REWORK_LIST_INVALID')
        return {'summary_count': count, 'failure_code': failure}
    fields = ('plan_revision', 'run_plan_revision', 'run_revision')
    out = {'summary_count': count}
    out.update((key, integer(value.get(key), nullable=True)) for key in fields)
    out['run_state'] = enum(value.get('run_state'), RUN_STATES, nullable=True)
    for key in ('task_accepted', 'run_task_accepted', 'cancel_requested', 'plan_digest_matches'):
        out[key] = boolean(value.get(key), nullable=True)
    out['failure_code'] = failure
    need(set(value) == set(out))
    if (out['plan_revision'] not in (1, 2) or out['run_state'] is None
            or out['plan_revision'] != out['run_plan_revision']
            or out['plan_digest_matches'] is not True or out['run_revision'] is None
            or any(out[key] is None for key in ('task_accepted', 'run_task_accepted', 'cancel_requested'))):
        expected = 'REWORK_LIST_INVALID'
    elif out['run_state'] == 'outcome_unknown':
        expected = 'REWORK_OUTCOME_UNKNOWN'
    elif out['task_accepted'] or out['run_task_accepted'] or out['run_state'] in ('accepted', 'completed'):
        expected = 'REWORK_EARLY_ACCEPTANCE'
    elif out['plan_revision'] == 2 and (out['cancel_requested'] or out['run_state'] in (
            'blocked', 'failed', 'cancel_requested', 'cancelled')):
        expected = 'REWORK_REVISION2_TERMINAL'
    else:
        expected = None
    need(failure == expected)
    return out


def scalar(value):
    need(type(value) is dict)
    out = {'phase': enum(value.get('phase'), PHASES),
           'method': enum(value.get('method'), METHODS + ('factory_runs.start',)),
           'stage': enum(value.get('stage'), STAGES)}
    for key in ('call_number', 'wall_ms', 'scenario_elapsed_ms', 'call_elapsed_ms',
                'observation_remaining_ms', 'observation_overdue_ms'):
        out[key] = integer(value.get(key))
    out['approved_scope_remaining_ms'] = integer(value.get('approved_scope_remaining_ms'),
                                                (1 << 63) - 1, signed=True)
    for key, fixed in (('configured_client_read_timeout_ms', 5000),
                       ('configured_actor_request_budget_ms', 60000)):
        need(type(value.get(key)) is int and value[key] == fixed)
        out[key] = fixed
    for key in ('reply_object_supplied', 'reply_is_error'):
        out[key] = boolean(value.get(key))
    for key in ('reply_revision', 'run_revision', 'plan_revision', 'node_count'):
        out[key] = integer(value.get(key), nullable=True)
    for key in ('run_completed', 'run_outcome_unknown'):
        out[key] = boolean(value.get(key), nullable=True)
    for key in ('method_calls', 'method_total_elapsed_ms'):
        need(type(value.get(key)) is dict and set(value[key]) == set(METHODS))
        out[key] = {method: integer(value[key][method]) for method in METHODS}
    if 'received_before_original_deadline' in value:
        out['received_before_original_deadline'] = boolean(value['received_before_original_deadline'])
    if 'list_progress' in value:
        out['list_progress'] = None if value['list_progress'] is None else progress(value['list_progress'])
        need(out['list_progress'] is None or out['method'] == 'factory_child_goals.list')
    # Unknown fields are not published or accepted as evidence.
    need(set(value) == set(out))
    return out


def stamp(meta):
    return (meta.st_dev, meta.st_ino, meta.st_mode, meta.st_uid, meta.st_nlink,
            meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns)


def directory(path=None, parent=None, name=None, private=False):
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK
    if parent is None:
        # Walk every ancestor via pinned FDs; no symlink/canonical-path alias.
        need(type(path) is str and 0 < len(os.fsencode(path)) <= 4096)
        parts = Path(path).parts
        need(Path(path).is_absolute() and '..' not in parts)
        fd = os.open('/', flags)
        try:
            for part in parts[1:]:
                next_fd = os.open(part, flags, dir_fd=fd)
                os.close(fd)
                fd = next_fd
        except BaseException:
            os.close(fd)
            raise
    else:
        fd = os.open(name, flags, dir_fd=parent)
    try:
        meta = os.fstat(fd)
        mode = stat.S_IMODE(meta.st_mode)
        need(meta.st_uid == os.geteuid() and stat.S_ISDIR(meta.st_mode))
        need(mode == 0o700 if private else mode in (0o700, 0o755))
        return fd
    except BaseException:
        os.close(fd)
        raise


def read_file(parent, name, limit, db=False):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and before.st_nlink == 1)
        # Existing JSON receipts use ordinary create under a private0700 dir.
        need(stat.S_IMODE(before.st_mode) in ((0o600,) if db else (0o600, 0o644)))
        need(0 < before.st_size <= limit)
        data = bytearray()
        while len(data) <= limit:
            chunk = os.read(fd, min(65536, limit + 1 - len(data)))
            if not chunk:
                break
            data.extend(chunk)
        need(len(data) == before.st_size and len(data) <= limit)
        need(stamp(before) == stamp(os.fstat(fd)))
        need(stamp(before) == stamp(os.stat(name, dir_fd=parent, follow_symlinks=False)))
        return bytes(data)
    finally:
        os.close(fd)


def bucket_revision(value):
    integer(value)
    need(value > 0)
    return str(value) if value in (1, 2) else 'later'


def validation_observations(conn, selected):
    """Existing receipt scalars only, for this fixture's two plan versions.

    Missing library fields do not mean the command did not start. In particular,
    a joined preflight rejection has no CommandValidationView at all. These
    post-join facts are not a new receipt authenticator or deadline observation.
    """
    result = {'1': None, '2': None}
    seen = set()
    for run_id, revision, raw in conn.execute(
            'SELECT run_id,plan_revision,document FROM goal_graph_validations '
            f'WHERE run_id IN ({selected}) AND plan_revision IN (1,2) ORDER BY plan_revision'):
        key = str(integer(revision, 2))
        need(key in result and key not in seen)
        seen.add(key)
        value = strict_json(raw)
        need(type(value) is dict and type(value.get('plan_revision')) is int
             and value['plan_revision'] == revision)
        if 'run_id' not in value:
            # Old incomplete evidence cannot attribute even real scalar values.
            # It remains an unavailable slot, not a zero/false observation.
            continue
        need(type(run_id) is str and type(value['run_id']) is str and value['run_id'] == run_id)
        out = {'actual_runtime_ms': integer(value.get('actual_runtime_ms'), nullable=True),
               'goal_io_state': None, 'error_code': None,
               'process_started': None, 'exit_code': None}
        view = value.get('observation')
        # A missing/non-object historical observation is unavailable, not a
        # fabricated false process-start or exit-code observation.
        if type(view) is dict:
            out['process_started'] = boolean(view.get('process_started'), nullable=True)
            exit_code = integer(view.get('exit_code'), (1 << 31) - 1, nullable=True, signed=True)
            need(exit_code is None or exit_code >= -(1 << 31))
            out['exit_code'] = exit_code
            goal_io = view.get('goal_io')
            if goal_io is not None:
                need(type(goal_io) is dict)
                state = goal_io.get('state')
                if state is not None:
                    need(type(state) is str)
                    out['goal_io_state'] = state if state in VALIDATION_IO_STATES else 'unrecognized'
                code = goal_io.get('error_code')
                if code is not None:
                    need(type(code) is str)
                    out['error_code'] = code if code in VALIDATION_ERROR_CODES else 'unrecognized'
        result[key] = out
    return result


def budget_usage(conn):
    """Post-join existing ledger/child-record facts, not admission or authority.

    Graph reservations are read once from their original table. Factory's
    runtime-derived Graph mirror is deliberately not reconstructed or summed.
    Missing old evidence stays pending; no limit or remaining budget is inferred.
    """
    out = {'status': 'pending', 'code': 'unknown_not_available',
           'snapshot_time': 'post_owned_shutdown_not_admission_instant',
           'not_an_authority_or_admission_verdict': True,
           'root': None, 'project': None, 'child': None}
    try:
        for table in ('goal_run_reservations', 'factory_budget_reservations', 'goal_ledgers'):
            row = conn.execute('SELECT type,sql FROM sqlite_master WHERE name=?', (table,)).fetchone()
            if row is None:
                return out
            need(row[0] == 'table' and type(row[1]) is str
                 and re.match(r'CREATE\s+TABLE\s', row[1], re.I))
            count, total, maximum = conn.execute(
                f'SELECT count(*),coalesce(sum(length(CAST(document AS BLOB))),0),'
                f'coalesce(max(length(CAST(document AS BLOB))),0) FROM {table}').fetchone()
            integer(count, MAX_ROWS)
            integer(total, 4 * 1024 * 1024)
            integer(maximum, 1024 * 1024)
        rows = conn.execute('SELECT goal_run_id,factory_scope_id,task_id,document '
                            'FROM factory_child_goal_runs').fetchall()
        need(len(rows) == 1)
        goal, scope, child, raw = rows[0]
        need(all(type(key) is str and 0 < len(key.encode()) <= 512 for key in (goal, scope, child)))
        record = strict_json(raw)
        initial, origin = record['initial_run'], record['origin']
        need(record['run_id'] == goal and initial['id'] == goal and initial['task_id'] == child
             and origin['factory_scope_id'] == scope and origin['initial_plan']['task_id'] == child)
        root, project = initial['root_grant_id'], initial['project_id']
        need(all(type(key) is str and 0 < len(key.encode()) <= 512 for key in (root, project)))
        current_raw = conn.execute('SELECT document FROM goal_runs WHERE id=?', (goal,)).fetchone()
        if current_raw is None:
            return out
        current = strict_json(current_raw[0])
        need(current['id'] == goal and current['task_id'] == child
             and current['root_grant_id'] == root and current['project_id'] == project)
        for name, key in (('root', 'root-grant:' + root), ('project', 'project:' + project)):
            rows = conn.execute('SELECT document FROM goal_ledgers WHERE scope=? LIMIT 2', (key,)).fetchall()
            if not rows:
                return out
            need(len(rows) == 1)
            ledger = strict_json(rows[0][0])
            out[name] = {'actual_runtime_ms': integer(ledger['runtime_ms'], nullable=True),
                         'observed_attempts': integer(ledger['observed_attempts'], (1 << 32) - 1)}
            out[name]['runtime_unknown'] = out[name]['actual_runtime_ms'] is None
        totals = {'settled_count': 0, 'unsettled_count': 0,
                  'settled_actual_runtime_ms': 0, 'unsettled_reserved_runtime_ms': 0,
                  'settled_runtime_unknown': False}
        seen = set()

        def add(attempt, value):
            need(type(attempt) is str and 0 < len(attempt.encode()) <= 512 and attempt not in seen)
            seen.add(attempt)
            settled = boolean(value['settled'])
            if settled:
                totals['settled_count'] += 1
                actual = integer(value['actual_runtime_ms'], nullable=True)
                if actual is None:
                    totals['settled_runtime_unknown'] = True
                    totals['settled_actual_runtime_ms'] = None
                elif totals['settled_actual_runtime_ms'] is not None:
                    totals['settled_actual_runtime_ms'] = integer(totals['settled_actual_runtime_ms'] + actual)
            else:
                totals['unsettled_count'] += 1
                reserved = integer(value['reservation']['runtime_ms'])
                totals['unsettled_reserved_runtime_ms'] = integer(totals['unsettled_reserved_runtime_ms'] + reserved)

        for attempt, run, raw in conn.execute(
                'SELECT attempt_id,run_id,document FROM goal_run_reservations WHERE run_id=?', (goal,)):
            value = strict_json(raw)
            need(run == goal and value['run_id'] == goal)
            add(attempt, value)
        for attempt, row_scope, raw in conn.execute(
                'SELECT attempt_id,scope_id,document FROM factory_budget_reservations WHERE scope_id=?', (scope,)):
            value = strict_json(raw)
            ancestors = value['ancestors']
            need(type(ancestors) is list and 0 < len(ancestors) <= 33
                 and all(type(key) is str and 0 < len(key.encode()) <= 512 for key in ancestors)
                 and len(set(ancestors)) == len(ancestors))
            need(value['authority_kind'] == 'factory_task' and row_scope == scope == value['scope_id']
                 and value['attempt_id'] == attempt and value['root_grant_id'] == root
                 and value['project_id'] == project and value['task_id'] in ancestors)
            if child in ancestors:
                add(attempt, value)
        out['child'] = totals
        unknown = (out['root']['runtime_unknown'] or out['project']['runtime_unknown']
                   or totals['settled_runtime_unknown'])
        out['status'] = 'pending' if unknown else 'collected_post_shutdown_budget_facts'
        out['code'] = 'unknown_runtime_observation' if unknown else 'existing_ledger_and_child_records'
        return out
    except (sqlite3.Error, KeyError, TypeError):
        # Old/missing metadata is not a zero consumption or a successful test.
        return out


# Optional original-guard sample, not reconstructed remaining budget/authority.
ANCESTOR_ADMISSION_GUARDS = frozenset(('attempts', 'parallel', 'runtime', 'output',
    'runtime_overflow', 'output_overflow'))
ANCESTOR_ADMISSION_FIELDS = frozenset(('guard_kind', 'run_revision', 'plan_revision',
    'relevant_count', 'active_count', 'attempt_cap', 'parallel_cap',
    'runtime_accounted_count', 'output_accounted_count',
    'settled_actual_runtime_ms', 'unsettled_reserved_runtime_ms',
    'requested_runtime_ms', 'runtime_cap_ms', 'settled_actual_output_bytes',
    'unsettled_reserved_output_bytes', 'requested_output_bytes', 'output_cap_bytes',
    'aggregation_complete'))


def ancestor_admission(value):
    need(type(value) is dict and set(value) == ANCESTOR_ADMISSION_FIELDS)
    out = {'guard_kind': enum(value['guard_kind'], ANCESTOR_ADMISSION_GUARDS)}
    for key in ('run_revision', 'plan_revision', 'attempt_cap', 'parallel_cap',
                'requested_runtime_ms', 'runtime_cap_ms', 'requested_output_bytes', 'output_cap_bytes'):
        out[key] = integer(value[key])
    need(out['run_revision'] > 0 and out['plan_revision'] > 0)
    for key in ('relevant_count', 'active_count', 'runtime_accounted_count', 'output_accounted_count'):
        out[key] = integer(value[key], 100_000)
    need(out['active_count'] <= out['relevant_count']
         and out['output_accounted_count'] <= out['runtime_accounted_count'] <= out['relevant_count'])
    for key in ('settled_actual_runtime_ms', 'unsettled_reserved_runtime_ms',
                'settled_actual_output_bytes', 'unsettled_reserved_output_bytes'):
        out[key] = integer(value[key], nullable=True)
    out['aggregation_complete'] = boolean(value['aggregation_complete'])
    if out['guard_kind'] in ('attempts', 'parallel'):
        need(not out['aggregation_complete'] and out['runtime_accounted_count'] == 0
             and out['output_accounted_count'] == 0
             and all(out[key] is None for key in ('settled_actual_runtime_ms',
                 'unsettled_reserved_runtime_ms', 'settled_actual_output_bytes',
                 'unsettled_reserved_output_bytes')))
        if out['guard_kind'] == 'attempts':
            need(out['relevant_count'] >= out['attempt_cap'])
        else:
            need(out['relevant_count'] < out['attempt_cap'] and out['active_count'] >= out['parallel_cap'])
    elif out['guard_kind'] in ('runtime', 'output'):
        need(out['aggregation_complete']
             and out['runtime_accounted_count'] == out['relevant_count']
             and out['output_accounted_count'] == out['relevant_count'])
        for key in ('settled_actual_runtime_ms', 'unsettled_reserved_runtime_ms',
                    'settled_actual_output_bytes', 'unsettled_reserved_output_bytes'):
            need(out[key] is not None)
        runtime = integer(out['settled_actual_runtime_ms'] + out['unsettled_reserved_runtime_ms'] + out['requested_runtime_ms'])
        output = integer(out['settled_actual_output_bytes'] + out['unsettled_reserved_output_bytes'] + out['requested_output_bytes'])
        need((out['guard_kind'] == 'runtime' and runtime > out['runtime_cap_ms'])
             or (out['guard_kind'] == 'output' and runtime <= out['runtime_cap_ms'] and output > out['output_cap_bytes']))
    else:
        # An overflow stops the original loop. Remaining rows are unobserved,
        # and a missing partial subtotal is never published as a zero.
        need(not out['aggregation_complete'] and out['runtime_accounted_count'] > 0)
        if out['guard_kind'] == 'output_overflow':
            need(out['output_accounted_count'] == out['runtime_accounted_count'])
        else:
            need(out['runtime_accounted_count'] == out['output_accounted_count'] + 1)
    out['snapshot_time'] = 'original_ancestor_reservation_guard'
    out['not_an_authority_or_stop_proof'] = True
    return out


def event_fields(event, subject, observed_plan_revision):
    """Exact known scalars only. No arbitrary payload key/value is published."""
    kind = enum(event.get('kind'), EVENT_KINDS)
    payload = event.get('payload')
    need(type(payload) is dict)
    out = {'kind': kind}
    out['task_accepted'] = boolean(payload.get('task_accepted'))
    if kind == 'goal.run_blocked':
        reason = payload.get('reason_code')
        need(type(reason) is str)
        out['reason_code'] = reason if reason in REASON_CODES else 'unrecognized'
        out['automatic_retry'] = boolean(payload.get('automatic_retry'))
        need(out['automatic_retry'] is False and out['task_accepted'] is False)
        if 'budget_admission' in payload:
            need(reason == 'RESOURCE_LIMIT')
            out['budget_admission'] = ancestor_admission(payload['budget_admission'])
            need(integer(event.get('revision')) == out['budget_admission']['run_revision'] + 1
                 and observed_plan_revision == out['budget_admission']['plan_revision'])
    elif kind == 'goal.factory_failure_confirmed':
        plan = integer(payload.get('plan_revision'))
        proof = payload.get('proof')
        need(type(proof) is dict and proof.get('run_id') == subject)
        need(plan == observed_plan_revision and integer(proof.get('plan_revision')) == plan)
        refs = proof.get('failure_refs')
        need(type(refs) is list and 1 <= len(refs) <= MAX_ROWS)
        need(all(type(ref) is str and 0 < len(ref.encode()) <= 512 for ref in refs))
        need(out['task_accepted'] is False and payload.get('reservation_refunded') is False)
        out.update(plan_revision=plan, failure_ref_count=len(refs), reservation_refunded=False)
    elif kind == 'goal.owner_node_settled':
        out['decision'] = enum(payload.get('decision'), ('accepted', 'rejected'))
    else:
        for key in ('actual_runtime_ms', 'actual_output_bytes'):
            out[key] = integer(payload.get(key), nullable=True)
        if kind in ('goal.node_settled', 'goal.validation_settled'):
            out['budget_overrun'] = boolean(payload.get('budget_overrun'))
            out['stop_confirmed'] = boolean(payload.get('stop_confirmed'))
            out['observed_runtime_lower_bound_ms'] = integer(payload.get('observed_runtime_lower_bound_ms'))
        if kind == 'goal.node_settled':
            out['node_state'] = enum(payload.get('state'), NODE_STATES)
        if kind == 'goal.validation_settled':
            out['passed'] = boolean(payload.get('passed'))
        if kind == 'goal.capture_settled':
            out['capture_state'] = enum(payload.get('state'), ('completed', 'failed', 'outcome_unknown'))
            for key in ('capture_worker_joined', 'producer_stop_confirmed'):
                out[key] = boolean(payload.get(key))
            for key in CAPTURE_TIMING_FIELDS:
                out[key] = integer(payload.get(key), nullable=True)
            # A partial observed timing is not evidence for the missing segments.
            out['capture_timing_status'] = ('observed' if any(out[key] is not None
                for key in CAPTURE_TIMING_FIELDS) else 'not_available')
        if kind in ('goal.capture_settled', 'goal.validation_settled'):
            out['io_completion'] = enum(payload.get('io_completion'), ('worker_joined', 'not_started'))
    return out


def project_events(conn, observation):
    result = {'status': 'pending', 'code': 'unknown_not_available',
              'snapshot_time': 'post_owned_shutdown_not_deadline_instant',
              'events': [], 'previous_node_settlement': None,
              'prior_capture_settlements': []}
    # A global IPC revision or Factory.get run is not a child Graph observation.
    if not (type(observation) is dict
            and observation.get('method') == 'factory_child_goals.list'
            and observation.get('stage') == 'response_received'
            and observation.get('reply_object_supplied') is True
            and observation.get('reply_is_error') is False):
        return result
    observed = observation.get('list_progress')
    if not (type(observed) is dict and observed.get('summary_count') == 1
            and observed.get('plan_digest_matches') is True
            and observed.get('plan_revision') in (1, 2)
            and observed.get('run_plan_revision') == observed.get('plan_revision')
            and observed.get('failure_code') != 'REWORK_LIST_INVALID'):
        return result
    revision = integer(observed.get('run_revision'), (1 << 63) - 1)
    need(revision > 0)
    result.update(observed_run_revision=revision, observed_plan_revision=observed['plan_revision'])
    table = conn.execute("SELECT type,sql FROM sqlite_master WHERE name='events'").fetchone()
    if table is None:
        return result
    need(table[0] == 'table' and type(table[1]) is str
         and re.match(r'CREATE\s+TABLE\s', table[1], re.I))
    columns = conn.execute('PRAGMA table_info(events)').fetchall()
    need([(c[1], c[2], c[3], c[5]) for c in columns] ==
         [('sequence', 'INTEGER', 0, 1), ('document', 'TEXT', 1, 0)])
    count, total, maximum = conn.execute(
        'SELECT count(*),coalesce(sum(length(CAST(document AS BLOB))),0),'
        'coalesce(max(length(CAST(document AS BLOB))),0) FROM events').fetchone()
    integer(count, MAX_EVENT_ROWS)
    integer(total, 4 * 1024 * 1024)
    integer(maximum, 1024 * 1024)
    need(conn.execute('SELECT count(*) FROM events WHERE NOT json_valid(document)').fetchone() == (0,))
    subject = conn.execute('SELECT goal_run_id FROM factory_child_goal_runs').fetchone()[0]
    need(type(subject) is str and 0 < len(subject.encode()) <= 128)
    where = ("json_extract(document,'$.aggregate_id')=? AND json_extract(document,'$.source')='goal_core' "
             "AND json_extract(document,'$.revision')=? AND json_extract(document,'$.kind') IN (?,?,?,?,?,?)")
    params = (subject, revision, *EVENT_KINDS)
    matched = conn.execute('SELECT count(*) FROM events WHERE ' + where, params).fetchone()[0]
    integer(matched, MAX_ASSOCIATED_EVENTS)
    if matched == 0:
        return result
    seen = set()
    first_sequence = None
    for sequence, raw in conn.execute('SELECT sequence,document FROM events WHERE ' + where +
                                      ' ORDER BY sequence LIMIT 17', params):
        integer(sequence, (1 << 63) - 1)
        need(sequence > 0)
        if first_sequence is None:
            first_sequence = sequence
        event = strict_json(raw)
        need(type(event) is dict and event.get('aggregate_id') == subject
             and event.get('source') == 'goal_core')
        need(integer(event.get('revision'), (1 << 63) - 1) == revision)
        kind = enum(event.get('kind'), EVENT_KINDS)
        need((kind, revision) not in seen)
        seen.add((kind, revision))
        projected = event_fields(event, subject, observed['plan_revision'])
        projected['ordinal'] = len(result['events']) + 1
        result['events'].append(projected)
    # A capture settlement can follow a prior work settlement. Keep that event
    # distinct: its earlier run revision does NOT prove the same plan revision.
    if any(event['kind'] == 'goal.capture_settled' for event in result['events']):
        prior_where = ("json_extract(document,'$.aggregate_id')=? "
            "AND json_extract(document,'$.source')='goal_core' "
            "AND json_extract(document,'$.kind')='goal.node_settled' "
            "AND json_extract(document,'$.revision')<? AND sequence<?")
        prior_revision = conn.execute("SELECT max(json_extract(document,'$.revision')) FROM events WHERE " +
                                      prior_where, (subject, revision, first_sequence)).fetchone()[0]
        if prior_revision is not None:
            integer(prior_revision, (1 << 63) - 1)
            need(prior_revision > 0)
            prior_params = (subject, prior_revision, *EVENT_KINDS)
            prior = conn.execute('SELECT sequence,document FROM events WHERE ' + where +
                " AND json_extract(document,'$.kind')='goal.node_settled' AND sequence<? ORDER BY sequence LIMIT 2", prior_params + (first_sequence,)).fetchall()
            need(len(prior) == 1 and len(result['events']) < MAX_ASSOCIATED_EVENTS)
            integer(prior[0][0], (1 << 63) - 1)
            need(prior[0][0] > 0)
            event = strict_json(prior[0][1])
            need(type(event) is dict and event.get('aggregate_id') == subject
                 and event.get('source') == 'goal_core' and event.get('kind') == 'goal.node_settled')
            need(integer(event.get('revision'), (1 << 63) - 1) == prior_revision)
            result['previous_node_settlement'] = {'run_revision': prior_revision,
                'same_observed_revision': False, 'same_plan_not_established': True,
                'event': event_fields(event, subject, None)}
    # A later admission refusal is not a capture event. Retain at most two
    # earlier capture settlements from this exact private child subject, so
    # producer/queue/join costs remain observable without relabelling history
    # as the current revision, same plan, admission verdict or new authority.
    if any(event['kind'] == 'goal.run_blocked' for event in result['events']):
        prior = conn.execute(
            "SELECT sequence,document FROM events WHERE "
            "json_extract(document,'$.aggregate_id')=? "
            "AND json_extract(document,'$.source')='goal_core' "
            "AND json_extract(document,'$.kind')='goal.capture_settled' "
            "AND json_extract(document,'$.revision')<? AND sequence<? "
            "ORDER BY sequence DESC LIMIT 2", (subject, revision, first_sequence)).fetchall()
        revisions = set()
        for sequence, raw in reversed(prior):
            integer(sequence, (1 << 63) - 1)
            need(0 < sequence < first_sequence)
            event = strict_json(raw)
            need(type(event) is dict and event.get('aggregate_id') == subject
                 and event.get('source') == 'goal_core'
                 and event.get('kind') == 'goal.capture_settled')
            earlier = integer(event.get('revision'), (1 << 63) - 1)
            need(0 < earlier < revision and earlier not in revisions)
            # LIMIT bounds publication, not uniqueness: a duplicate of this
            # selected revision may lie just outside the two-row window.
            duplicates = conn.execute(
                "SELECT count(*) FROM events WHERE "
                "json_extract(document,'$.aggregate_id')=? "
                "AND json_extract(document,'$.source')='goal_core' "
                "AND json_extract(document,'$.kind')='goal.capture_settled' "
                "AND json_extract(document,'$.revision')=? AND sequence<?",
                (subject, earlier, first_sequence)).fetchone()[0]
            need(duplicates == 1)
            need(len(result['events']) + len(result['prior_capture_settlements'])
                 < MAX_ASSOCIATED_EVENTS)
            revisions.add(earlier)
            result['prior_capture_settlements'].append({
                'run_revision': earlier, 'same_observed_revision': False,
                'same_plan_not_established': True,
                'event': event_fields(event, subject, None)})
    unknown = any(event.get('reason_code') == 'unrecognized' for event in result['events'])
    result['status'] = 'pending' if unknown else 'collected_historical_events'
    result['code'] = 'unrecognized_reason' if unknown else 'same_observed_run_revision'
    return result


def project_db(raw, observation=None):
    """Read a bounded private derivative of the FD-verified, joined snapshot.

    SQLite opens only this owned copy with mode=ro, immutable and query_only.
    This avoids reopening a source path that could now resolve to another inode.
    Main-file capture after join is not a live/WAL snapshot or receipt verifier.
    """
    with tempfile.TemporaryDirectory(prefix='baton-rework-projection-') as temp:
        path = Path(temp) / 'snapshot.sqlite3'
        with path.open('xb') as stream:
            stream.write(raw)
        path.chmod(0o600)
        conn = sqlite3.connect(path.as_uri() + '?mode=ro&immutable=1', uri=True)
        try:
            conn.execute('PRAGMA query_only=ON')
            conn.execute('PRAGMA trusted_schema=OFF')
            conn.set_progress_handler(lambda: 1, 2_000_000)
            need(conn.execute('PRAGMA user_version').fetchone() == (SCHEMA,))
            need(conn.execute('PRAGMA query_only').fetchone() == (1,))
            for table in TABLES:
                # Never evaluate a view/virtual table from a damaged fixture.
                row = conn.execute('SELECT type,sql FROM sqlite_master WHERE name=?', (table,)).fetchone()
                need(row is not None and row[0] == 'table' and type(row[1]) is str
                     and re.match(r'CREATE\s+TABLE\s', row[1], re.I))
                count, total, maximum = conn.execute(
                    f'SELECT count(*),coalesce(sum(length(CAST(document AS BLOB))),0),'
                    f'coalesce(max(length(CAST(document AS BLOB))),0) FROM {table}').fetchone()
                integer(count, MAX_ROWS)
                integer(total, 4 * 1024 * 1024)
                integer(maximum, 1024 * 1024)
            integration_counts = {}
            for table in INTEGRATION_TABLES:
                row = conn.execute('SELECT type,sql FROM sqlite_master WHERE name=?', (table,)).fetchone()
                need(row is not None and row[0] == 'table' and type(row[1]) is str
                     and re.match(r'CREATE\s+TABLE\s', row[1], re.I))
                count = conn.execute('SELECT count(*) FROM "' + table + '"').fetchone()[0]
                need(type(count) is int and count == 0)
                integration_counts[table] = count
            # One rework test owns one child Graph. Empty capture is pending,
            # never evidence of successful progress or confirmed process stop.
            count = conn.execute('SELECT count(*) FROM factory_child_goal_runs').fetchone()[0]
            need(count == 1)
            selected = 'SELECT goal_run_id FROM factory_child_goal_runs'
            runs = []
            for row in conn.execute(f"SELECT state,json_extract(document,'$.state'),json_extract(document,'$.plan_revision'),"
                    "json_type(document,'$.task_accepted'),json_extract(document,'$.task_accepted'),"
                    "json_type(document,'$.cancel_requested'),json_extract(document,'$.cancel_requested') "
                    f'FROM goal_runs WHERE id IN ({selected})'):
                state, document_state, revision, accepted_type, accepted, cancel_type, cancel = row
                enum(state, RUN_STATES)
                need(state == document_state)
                need(accepted_type in ('true', 'false') and cancel_type in ('true', 'false'))
                runs.append({'plan_revision': bucket_revision(revision), 'state': state,
                             'task_accepted': bool(accepted), 'cancel_requested': bool(cancel)})
            need(len(runs) == 1)
            nodes = {}
            for revision, state, kind, document_revision, document_state, count in conn.execute(
                    "SELECT plan_revision,state,json_extract(document,'$.node.kind'),"
                    "json_extract(document,'$.plan_revision'),json_extract(document,'$.state'),count(*) "
                    f'FROM goal_run_nodes WHERE run_id IN ({selected}) GROUP BY 1,2,3,4,5'):
                rev = bucket_revision(revision)
                integer(document_revision)
                need(document_revision == revision and document_state == state)
                enum(state, NODE_STATES)
                enum(kind, ('work', 'validate', 'owner_approval', 'summary'))
                integer(count, MAX_ROWS)
                nodes[f'{rev}:{kind}:{state}'] = count
            validations = {}
            for revision, document_revision, state, passed_type, stopped_type, io, count in conn.execute(
                    "SELECT plan_revision,json_extract(document,'$.plan_revision'),"
                    "json_extract(document,'$.state'),json_type(document,'$.passed'),"
                    "json_type(document,'$.stop_confirmed'),json_extract(document,'$.io_completion'),count(*) "
                    f'FROM goal_graph_validations WHERE run_id IN ({selected}) GROUP BY 1,2,3,4,5,6'):
                rev = bucket_revision(revision)
                integer(document_revision)
                need(document_revision == revision)
                enum(state, ('passed', 'failed', 'cancelled', 'outcome_unknown'))
                need(passed_type in ('true', 'false') and stopped_type in ('true', 'false'))
                enum(io, ('worker_joined', 'not_started'))
                integer(count, MAX_ROWS)
                validations[f'{rev}:{state}:passed_{passed_type}:stopped_{stopped_type}:{io}'] = count
            return {'schema': SCHEMA, 'integration_counts': integration_counts, 'child_graph_count': 1, 'runs': runs,
                    'node_counts': dict(sorted(nodes.items())),
                    'validation_counts': dict(sorted(validations.items())),
                    'validation_observations': validation_observations(conn, selected),
                    'budget_usage': budget_usage(conn),
                    'associated_events': project_events(conn, observation)}
        finally:
            conn.close()


def case(parent, name):
    result = {'status': 'pending', 'code': 'MISSING_OWNED_EVIDENCE',
              'snapshot_time': 'post_owned_shutdown_not_deadline_instant'}
    fd = directory(parent=parent, name=name, private=True)
    try:
        # Scalars can remain useful if Drop could not retain a joined DB.
        try:
            result['last_readonly_observation'] = scalar(strict_json(read_file(
                fd, 'readonly-observation-last.json', 4096)))
        except FileNotFoundError:
            pass
        cleanup = strict_json(read_file(fd, 'owned-cleanup.json', 16384))
        need(type(cleanup) is dict)
        joined = boolean(cleanup.get('joined'))
        daemon = enum(cleanup.get('daemon_result'), ('ok', 'error', 'panic', 'missing_owned_thread'))
        result['actor_thread_joined'] = joined
        result['daemon_result'] = daemon
        if not joined or daemon != 'ok':
            result['code'] = 'OWNED_SHUTDOWN_UNCONFIRMED'
            return result
        checkpoint = strict_json(read_file(fd, 'post-shutdown-snapshot.json', 4096))
        need(type(checkpoint) is dict and set(checkpoint) == {'contract', 'status',
            'main_file_copy_consistent', 'snapshot_time',
            'not_a_general_sqlite_snapshot_or_process_stop_proof'})
        need(checkpoint['contract'] == 'rework-post-join-main-file.v1'
             and checkpoint['snapshot_time'] == 'post_owned_shutdown_not_deadline_instant'
             and checkpoint['not_a_general_sqlite_snapshot_or_process_stop_proof'] is True)
        consistent = boolean(checkpoint['main_file_copy_consistent'])
        enum(checkpoint['status'], ('observed_consistent', 'incomplete'))
        need((checkpoint['status'] == 'observed_consistent') == consistent)
        if not consistent:
            result['code'] = 'MAIN_FILE_CAPTURE_INCOMPLETE'
            return result
        for suffix in ('-wal', '-shm', '-journal'):
            try:
                os.stat('runtime.raw.sqlite3' + suffix, dir_fd=fd, follow_symlinks=False)
            except FileNotFoundError:
                continue
            raise Invalid()
        raw = read_file(fd, 'runtime.raw.sqlite3', MAX_DB, db=True)
        need(type(cleanup.get('raw_db_bytes')) is int and cleanup['raw_db_bytes'] == len(raw))
        digest = cleanup.get('raw_db_sha256')
        need(type(digest) is str and re.fullmatch('[a-f0-9]{64}', digest) is not None)
        need(hashlib.sha256(raw).hexdigest() == digest)
        result['post_shutdown'] = project_db(raw, result.get('last_readonly_observation'))
        result['test_panicking'] = boolean(cleanup.get('test_panicking'))
        if 'last_readonly_observation' in result:
            result['status'], result['code'] = 'collected_post_shutdown', 'FINITE_PROJECTION_COLLECTED'
        else:
            result['code'] = 'MISSING_READONLY_OBSERVATION'
        return result
    except FileNotFoundError:
        return result
    finally:
        os.close(fd)


def render_public(report):
    # This exact serialization (including newline) is also the stdout budget.
    return json.dumps(report, sort_keys=True, indent=2) + '\n'


def collect(evidence_root):
    report = {'contract': CONTRACT, 'status': 'pending', 'code': 'NO_RETAINED_REWORK_CASE',
              'not_a_product_or_test_pass': True, 'cases': []}
    try:
        fd = directory(path=evidence_root)
        try:
            names = []
            with os.scandir(fd) as entries:
                for index, entry in enumerate(entries):
                    need(index < MAX_ENTRIES and len(os.fsencode(entry.name)) <= 128)
                    if NAME.fullmatch(entry.name):
                        names.append(entry.name)
                        need(len(names) <= MAX_CASES)
            for name in sorted(names):
                try:
                    report['cases'].append(case(fd, name))
                except (Invalid, OSError, ValueError, sqlite3.Error, RecursionError, OverflowError):
                    report['cases'].append({'status': 'invalid', 'code': 'INVALID_PRIVATE_EVIDENCE',
                        'snapshot_time': 'post_owned_shutdown_not_deadline_instant'})
        finally:
            os.close(fd)
        if report['cases']:
            report['status'] = ('collected_post_shutdown' if all(
                c['status'] == 'collected_post_shutdown' for c in report['cases']) else 'invalid_or_pending')
            report['code'] = ('FINITE_PROJECTION_COLLECTED' if report['status'] == 'collected_post_shutdown'
                              else 'INCOMPLETE_PRIVATE_EVIDENCE')
    except FileNotFoundError:
        pass
    except (Invalid, OSError, ValueError, RecursionError, OverflowError):
        report['status'], report['code'] = 'invalid', 'INVALID_EVIDENCE_ROOT'
    # Output bound fails closed; never truncate individual progress fields.
    if len(render_public(report).encode('utf-8')) > MAX_PUBLIC:
        return {'contract': CONTRACT, 'status': 'invalid', 'code': 'PUBLIC_PROJECTION_LIMIT',
                'not_a_product_or_test_pass': True, 'cases': []}
    return report


class FixedParser(argparse.ArgumentParser):
    def error(self, _message):
        # argparse's default includes unknown argv (possibly credentials).
        sys.stderr.write('INVALID_COLLECTOR_ARGUMENTS\n')
        raise SystemExit(2)


def main(argv=None):
    parser = FixedParser(prog='record-factory-rework-ci-evidence',
                         description=__doc__, allow_abbrev=False)
    parser.add_argument('--evidence-root', required=True)
    args = parser.parse_args(argv)
    report = collect(args.evidence_root)
    sys.stdout.write(render_public(report))
    return 0 if report['status'] == 'collected_post_shutdown' else 1


if __name__ == '__main__':
    raise SystemExit(main())
