#!/usr/bin/env python3
"""Built React/Linux Chromium -> finite test bridge -> actual Core17 Unix IPC.

Only --run starts processes. The private host, outside HTTP, reviews complete
Core proposals and finalizes generated fixture permissions. This is NOT the
production Tauri Native formatter, an OS dialog, macOS or a live Provider test.
No normal authority, pass, acceptance, lease or stop is inserted through SQL.
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import copy
from functools import partial
import hashlib
import http.server
import json
import os
from pathlib import Path
import queue
import re
import shutil
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
import types
import uuid

ROOT = Path(__file__).resolve().parents[3]
ENDPOINT = '/__test_integration_ipc'
MAX_BODY, MAX_REPLY, MAX_REVIEW = 270336, 4194304, 1048576
SAFE = 9007199254740991
MAX_CHROMIUM_IMAGE = 536870912  # Actual installed browser ELF; other tool caps stay256MiB.
TARGET = 'refs/heads/integration-browser'
REPORT, FAILURE_STDERR = b'fixed report', b'fixed correctness failure\n'
SELECTION_KEYS = frozenset(('task_id', 'expected_task_revision', 'expected_spec_revision',
    'goal_scope_id', 'expected_scope_revision', 'host_id', 'target_ref', 'expected_target_head',
    'max_rounds', 'on_target_advance', 'ttl_ms', 'stage_reservations'))
PROPOSAL_KEYS = frozenset(('id', 'kind', 'revision', 'snapshot', 'created_at', 'expires_at_ms', 'consumed'))
SNAPSHOT_KEYS = frozenset(('intent', 'bindings', 'as_of_sequence', 'shared_usage', 'authority_kind', 'guarantees', 'warning'))
GUARANTEES = {'local_unchecked_out_ref_only': True, 'merged_candidate_revalidation_required': True,
    'original_index_and_working_files_unchanged': True, 'output_subject_rebinding_same_paths_only': True,
    'fixed_inputs_and_scripts_unchanged': True, 'os_filesystem_sandbox': False,
    'os_network_sandbox': False, 'whole_filesystem_output_quota': False, 'remote_delivery_authorized': False}
BASE_READS = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list',
    'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}
TASK_READS = {'tasks_get': 'tasks.get', 'goal_scopes_list': 'goal_scopes.list'}
ID_READS = {'integration_policies_get': ('integration_policies.get', 'policyId', 'policy_id', 'policies'),
    'integrations_get': ('integrations.get', 'integrationId', 'integration_id', 'runs')}
WRITES = {'integration_policies_revoke': ('integration_policies.revoke', 'policyId', 'policy_id', 'policies'),
    'integrations_start': ('integrations.start', 'policyId', 'policy_id', 'policies'),
    'integrations_cancel': ('integrations.cancel', 'integrationId', 'integration_id', 'runs'),
    'integrations_reconcile': ('integrations.reconcile', 'integrationId', 'integration_id', 'runs')}


class Refusal(ValueError):
    pass


def require(value, message='Fixed integration fixture refused'):
    if not value:
        raise Refusal(message)


def integer(value, minimum=0, maximum=SAFE):
    return type(value) is int and minimum <= value <= maximum


def identifier(value):
    return type(value) is str and re.fullmatch(r'[A-Za-z0-9:._-]{1,256}', value) is not None


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def exact(a, b):
    if type(a) is not type(b):
        return False
    if type(a) is dict:
        return a.keys() == b.keys() and all(exact(a[k], b[k]) for k in a)
    if type(a) is list:
        return len(a) == len(b) and all(exact(x, y) for x, y in zip(a, b))
    return a == b


def unique(pairs):
    out = {}
    for k, v in pairs:
        require(k not in out, 'Duplicate JSON field')
        out[k] = v
    return out


def decode(raw):
    return json.loads(raw, object_pairs_hook=unique,
        parse_constant=lambda _: (_ for _ in ()).throw(Refusal('Nonfinite JSON')))


def private_json(path, value):
    raw = json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False).encode() + b'\n'
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def pinned_file(path, limit=268435456):
    """Whole bounded bytes and stable identity; no symlinks or empty images."""
    path = Path(path)
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= limit)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        signature = lambda m: (m.st_dev, m.st_ino, m.st_size, m.st_mode, m.st_uid,
            m.st_nlink, m.st_mtime_ns, m.st_ctime_ns)
        require(signature(before) == signature(os.fstat(stream.fileno())))
        h, size = hashlib.sha256(), 0
        while block := stream.read(min(1048576, before.st_size - size + 1)):
            size += len(block)
            require(size <= before.st_size)
            h.update(block)
        require(size == before.st_size and signature(before) == signature(os.fstat(stream.fileno()))
            == signature(path.lstat()))
    return {'sha256': h.hexdigest(), 'bytes': size, 'mode': stat.S_IMODE(before.st_mode),
        'device': str(before.st_dev), 'inode': str(before.st_ino), 'uid': before.st_uid, 'links': before.st_nlink}


def assets(frontend):
    require(frontend.is_dir() and not frontend.is_symlink())
    result, total = {}, 0
    for path in sorted(frontend.rglob('*')):
        require(not path.is_symlink())
        if path.is_dir():
            continue
        require(len(result) < 512)
        record = pinned_file(path, 16777216)
        total += record['bytes']
        require(total <= 67108864)
        result[path.relative_to(frontend).as_posix()] = record
    require('index.html' in result)
    return result


def preflight(args):
    head = args.expected_source_head
    require(type(head) is str and re.fullmatch('[a-f0-9]{40}', head) is not None)
    binary = args.batond.absolute()
    companion = binary.with_name('baton')
    tools = {k: pinned_file(v) for k, v in [('batond', binary), ('baton', companion)]}
    require(all(os.access(p, os.X_OK) for p in (binary, companion)))
    require(all(v['uid'] in (0, os.geteuid()) and v['mode'] & 0o022 == 0 for v in tools.values()))
    manifest_raw = args.candidate_manifest.read_bytes()
    require(len(manifest_raw) <= 2097152)
    manifest = decode(manifest_raw)
    require(type(manifest.get('schema')) is int and manifest['schema'] == 17)
    aliases = [manifest[k] for k in ('source_head', 'git_head', 'compiled_core_source_head') if k in manifest]
    require(aliases and all(v == head for v in aliases))
    require(manifest.get('dirty', False) is False and manifest.get('tracked_clean', True) is True)
    require(all(manifest['binaries'][k] == tools[k]['sha256'] for k in tools))
    build_raw = args.frontend_build_record.read_bytes()
    require(len(build_raw) <= MAX_REVIEW)
    build = decode(build_raw)
    require(build.get('source') == head and type(build.get('exit')) is int and build['exit'] == 0
        and build.get('command') == ['npm', '--prefix', 'apps/desktop', 'run', 'build']
        and build.get('source_head_and_tracked_unchanged') is True
        and build.get('dirty', False) is False and build.get('tracked_clean', True) is True)
    frontend = args.frontend.resolve(strict=True)
    args.chromium = args.chromium.resolve(strict=True)
    runtime_tools = {'chromium': pinned_file(args.chromium, MAX_CHROMIUM_IMAGE),
        'python': pinned_file(Path(sys.executable).resolve(strict=True))}
    require(all(v['uid'] in (0, os.geteuid()) and v['mode'] & 0o022 == 0 for v in runtime_tools.values())
        and os.access(args.chromium, os.X_OK))
    source = source_guard(head)
    return binary, companion, frontend, {'source': head, 'tools': tools, 'assets': assets(frontend),
        'manifest_sha256': digest(manifest_raw), 'build_record_sha256': digest(build_raw),
        'driver_sha256': digest(Path(__file__).read_bytes()), 'source_guard': source, 'runtime_tools': runtime_tools}


def source_guard(expected):
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    status = subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=no'], cwd=ROOT)
    require(head == expected and status == b'', 'Actual source must be the clean declared commit')
    tracked = subprocess.check_output(['git', 'show', 'HEAD:apps/desktop/tests/integration-real-ipc.py'], cwd=ROOT)
    require(tracked == Path(__file__).read_bytes(), 'Driver must match the exact tracked source')
    return {'head': head, 'tracked_clean': True, 'driver_sha256': digest(tracked)}


def notification_probe_bytes(expected):
    # Fixed repository path only; never accept a module/path from CLI or HTTP.
    relative = 'apps/desktop/tests/task_notifications_real_ipc_probe.py'
    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == expected
        and subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=no'], cwd=ROOT) == b'')
    path = Path(__file__).with_name('task_notifications_real_ipc_probe.py')
    pin = pinned_file(path, MAX_REVIEW)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        raw = stream.read(MAX_REVIEW + 1)
    tracked = subprocess.check_output(['git', 'show', 'HEAD:' + relative], cwd=ROOT)
    require(0 < len(raw) <= MAX_REVIEW and len(raw) == pin['bytes']
        and raw == tracked and digest(raw) == pin['sha256'])
    return {'source': expected, 'relative_path': relative, 'sha256': pin['sha256'], 'bytes': pin['bytes']}, raw


def notification_probe_source(expected):
    return notification_probe_bytes(expected)[0]


def load_notification_probe(expected):
    record, raw = notification_probe_bytes(expected)
    path = Path(__file__).with_name('task_notifications_real_ipc_probe.py')
    # Compile the exact reviewed bytes. SourceFileLoader may instead execute an
    # untracked timestamp/size-matching .pyc even when the .py hash is correct.
    module = types.ModuleType('_fixed_task_notifications_real_ipc_probe')
    module.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    require(notification_probe_source(expected) == record)
    return module, record


def reservation(value):
    require(type(value) is dict and set(value) == {'runtime_ms', 'output_bytes', 'tokens', 'cost_microunits'})
    require(integer(value['runtime_ms'], 1) and integer(value['output_bytes'], 1))
    require(all(value[k] is None or integer(value[k]) for k in ('tokens', 'cost_microunits')))


def validator(value):
    require(type(value) is dict and set(value) == {'kind', 'policy_id', 'revision'})
    require(value['kind'] in ('integrity', 'data', 'command') and identifier(value['policy_id'])
        and integer(value['revision'], 1))


def selection(value, bounds):
    require(type(value) is dict and set(value) == SELECTION_KEYS)
    require(value['task_id'] == bounds.task and value['goal_scope_id'] == bounds.scope
        and value['host_id'] == 'local' and value['target_ref'] == TARGET
        and value['expected_target_head'] == bounds.target_head)
    require(all(integer(value[k], 1) for k in ('expected_task_revision', 'expected_spec_revision',
        'expected_scope_revision', 'ttl_ms')) and integer(value['max_rounds'], 1, 8))
    require(value['on_target_advance'] in ('block', 'bounded_descendant_reprepare'))
    stages = value['stage_reservations']
    require(type(stages) is dict and set(stages) == {'preparation', 'validators', 'cas'})
    reservation(stages['preparation']); reservation(stages['cas'])
    require(type(stages['validators']) is list and 1 <= len(stages['validators']) <= 64)
    refs = []
    for row in stages['validators']:
        require(type(row) is dict and set(row) == {'validator', 'reservation'})
        validator(row['validator']); reservation(row['reservation'])
        require(any(exact(row['validator'], v) for v in bounds.validators))
        require(not any(exact(row['validator'], v) for v in refs))
        refs.append(row['validator'])
    require(any(v['kind'] in ('data', 'command') for v in refs))
    require(all(any(exact(v, r) for r in refs) for v in bounds.required_validators))
    return copy.deepcopy(value)


class Boundary:
    """Only actual generated fixture IDs may enter fixed typed invokes."""
    def __init__(self, task, project, scope, target_head, validators, required=()):
        self.task, self.project, self.scope, self.target_head = task, project, scope, target_head
        self.validators, self.required_validators = copy.deepcopy(validators), copy.deepcopy(list(required))
        self.policies, self.runs, self.writes = set(), set(), {}
        self.lock = threading.Lock()

    def route(self, envelope):
        require(type(envelope) is dict and set(envelope) <= {'command', 'args'} and 'command' in envelope)
        command, args = envelope['command'], envelope.get('args')
        if args is None:
            args = {}
        require(type(command) is str and type(args) is dict)
        key = revision = None
        if command in BASE_READS:
            require(not args); return BASE_READS[command], {}, None, None
        if command == 'events_subscribe':
            require(set(args) <= {'cursor', 'limit'} and integer(args.get('cursor', 0))
                and integer(args.get('limit', 100), 1, 100))
            return 'events.subscribe', {'cursor': args.get('cursor', 0), 'limit': args.get('limit', 100)}, None, None
        if command in TASK_READS:
            require(set(args) == {'taskId'} and args['taskId'] == self.task)
            return TASK_READS[command], {'task_id': self.task}, None, None
        if command == 'integration_context_get':
            require(set(args) == {'taskId', 'goalScopeId'} and args['taskId'] == self.task
                and args['goalScopeId'] == self.scope)
            return 'integration_context.get', {'task_id': self.task, 'goal_scope_id': self.scope}, None, None
        if command in ('integration_targets_list', 'integration_policies_list', 'integrations_list'):
            cursor = 'afterRef' if command == 'integration_targets_list' else 'afterId'
            require(set(args) <= {'taskId', cursor, 'limit'} and args.get('taskId') == self.task
                and integer(args.get('limit', 100), 1, 100))
            if cursor in args:
                require(identifier(args[cursor]) if cursor == 'afterId' else args[cursor] == TARGET)
            method = {'integration_targets_list': 'integration_targets.list',
                'integration_policies_list': 'integration_policies.list', 'integrations_list': 'integrations.list'}[command]
            params = {'task_id': self.task, 'limit': args.get('limit', 100)}
            if cursor in args:
                params['after_ref' if cursor == 'afterRef' else 'after_id'] = args[cursor]
            return method, params, None, None
        if command == 'integration_policy_approve':
            require(set(args) == {'selection', 'idempotencyKey'})
            params = selection(args['selection'], self)
            method, key = 'integration_policies.propose', args['idempotencyKey']
        elif command in ID_READS or command in WRITES:
            method, camel, snake, collection = (ID_READS | WRITES)[command]
            write = command in WRITES
            require(set(args) == {camel} | ({'expectedRevision', 'idempotencyKey'} if write else set()))
            with self.lock:
                require(identifier(args[camel]) and args[camel] in getattr(self, collection))
            params = {snake: args[camel]}
            if write:
                key, revision = args['idempotencyKey'], args['expectedRevision']
                require(integer(revision, 1))
        else:
            # Includes approvals.*, renderer authority, generic/session/task writes.
            raise Refusal('Command outside fixed integration boundary')
        if command == 'integration_policy_approve' or command in WRITES:
            require(identifier(key))
            with self.lock:
                prior = self.writes.get((command, key))
                payload = (copy.deepcopy(params), revision)
                require(prior is None or exact(prior[0], params) and prior[1] == revision,
                    'Same key cannot acquire a changed payload or domain revision')
                self.writes[(command, key)] = payload
        return method, params, key, revision

    def observe(self, method, response):
        if 'error' in response:
            return
        value = response['result']
        with self.lock:
            if method == 'integration_policies.list':
                require(type(value) is list and len(value) <= 100)
                for p in value:
                    require(p['selection']['task_id'] == self.task and p['selection']['goal_scope_id'] == self.scope)
                    self.policies.add(p['id'])
            elif method == 'integrations.list':
                require(type(value) is list and len(value) <= 100)
                for r in value:
                    require(r['task_id'] == self.task and r['policy_id'] in self.policies)
                    self.runs.add(r['id'])


def parse_http(raw, headers, origin, bounds):
    require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}', origin) is not None
        and 1 <= int(origin.rsplit(':', 1)[1]) <= 65535)
    require(headers.get('Origin') == origin and headers.get('Host') == origin[7:]
        and headers.get('Content-Type') == 'application/json' and not headers.get('Transfer-Encoding'))
    require(0 < len(raw) <= MAX_BODY)
    return bounds.route(decode(raw.decode('utf-8')))


def readonly_abort(error, method, stage):
    return isinstance(error, (BrokenPipeError, ConnectionResetError)) and method is not None \
        and method in set(BASE_READS.values()) | set(TASK_READS.values()) | {
            'events.subscribe', 'integration_context.get', 'integration_targets.list',
            'integration_policies.get', 'integration_policies.list', 'integrations.get', 'integrations.list'} \
        and stage in ('response_headers', 'response_body')


@contextmanager
def database(runtime):
    db = sqlite3.connect((runtime / 'baton.sqlite3').resolve().as_uri() + '?mode=ro', uri=True, timeout=2)
    try:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        require(db.execute('PRAGMA user_version').fetchone() == (17,))
        yield db
    finally:
        db.close()


def rows(db, table, *, policy=None):
    require(table in ('command_validation_policies', 'command_validation_runs'))
    require(policy is not None and identifier(policy))
    column = 'id' if table == 'command_validation_policies' else 'policy_id'
    return db.execute(f'SELECT rowid,* FROM {table} WHERE {column}=? ORDER BY rowid', (policy,)).fetchall()


def integration_counts(runtime):
    with database(runtime) as db:
        return tuple(db.execute('SELECT count(*) FROM ' + t).fetchone()[0] for t in
            ('integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences'))


class Host:
    """Private token transport. No token or private frame is sent to the browser."""
    def __init__(self, runtime):
        self.runtime = runtime
        self.token = (runtime / 'session.token').read_text()
        self.audit, self.lock = [], threading.Lock()

    def call(self, method, params, key=None, revision=None):
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': self.token,
            'method': method, 'params': params}
        if key is not None: frame['idempotency_key'] = key
        if revision is not None: frame['expected_revision'] = revision
        raw = compact(frame) + b'\n'
        require(len(raw) <= 262144)
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(20)
            stream.connect(str(self.runtime / 'batond.sock')); stream.sendall(raw)
            with stream.makefile('rb') as reader:
                reply = reader.readline(MAX_REPLY + 1)
        require(0 < len(reply) <= MAX_REPLY and reply.endswith(b'\n'))
        value = decode(reply)
        require(type(value) is dict and ('result' in value) != ('error' in value))
        with self.lock:
            self.audit.append({'method': method, 'params': copy.deepcopy(params), 'key': key,
                'revision': revision, 'response': value})
        return value

    def result(self, method, params, key=None, revision=None):
        value = self.call(method, params, key, revision)
        require('error' not in value, 'Actual private Core request failed')
        return value['result']

    def approved(self, method, params, key, kind, revision=None):
        require(method in ('connections.propose', 'grants.propose', 'goal_scopes.propose',
            'goal_graph_runs.propose', 'goal_node_approvals.propose', 'command_validation_policies.propose'))
        proposal = self.result(method, params, key, revision)
        require(proposal['kind'] == kind and proposal['consumed'] is False
            and len(compact(proposal)) <= (MAX_REVIEW if kind in ('goal_graph_execution', 'goal_node_acceptance') else 16384))
        return self.result('approvals.finalize', {'proposal_id': proposal['id']},
            'proposal:' + proposal['id'], proposal['revision'])


def review_proposal(proposal, selected, context, expected_definition):
    require(type(proposal) is dict and set(proposal) == PROPOSAL_KEYS and proposal['kind'] == 'integration'
        and proposal['consumed'] is False and integer(proposal['revision'], 1) and proposal['revision'] == 1
        and identifier(proposal['id']) and integer(proposal['expires_at_ms'], 1)
        and type(proposal['created_at']) is str and bool(proposal['created_at'])
        and len(compact(proposal)) <= MAX_REVIEW)
    snap = proposal['snapshot']
    require(type(snap) is dict and set(snap) == SNAPSHOT_KEYS and len(compact(snap)) <= MAX_REVIEW
        and exact(snap['intent'], selected) and snap['authority_kind'] == 'authorized_local_integration'
        and integer(snap['as_of_sequence']) and type(snap['warning']) is str and bool(snap['warning'])
        and exact(snap['guarantees'], GUARANTEES))
    bindings, observed = snap['bindings'], context['bindings']
    require(context['preview_only'] is True and bindings['task_id'] == selected['task_id']
        and bindings['goal_scope_id'] == selected['goal_scope_id']
        and bindings['goal_scope_revision'] == selected['expected_scope_revision']
        and bindings['spec_revision'] == selected['expected_spec_revision'])
    # Selected definitions legitimately change only validators/binding_digest.
    for field in observed:
        if field not in ('validators', 'binding_digest'):
            require(exact(bindings[field], observed[field]), 'Complete original static binding changed')
    require(set(bindings) == set(observed) and len(bindings['validators']) == 1)
    definition = bindings['validators'][0]
    require(exact(definition, expected_definition)
        and exact(selected['stage_reservations']['validators'][0]['validator'], definition['reference']))
    require(exact(snap['shared_usage'], context['shared_usage']))
    require(bindings['workspace_binding']['kind'] == 'task_managed'
        and bindings['workspace_binding']['receipt']['task_id'] == selected['task_id']
        and bindings['grant_lineage'] and bindings['factory_scope_id'] is None)
    return {'proposal_sha256': digest(compact(proposal)), 'snapshot_sha256': digest(compact(snap)),
        'proposal_bytes': len(compact(proposal)), 'snapshot_bytes': len(compact(snap))}


def verify_policy(policy, proposal, selected):
    require(type(policy) is dict and set(policy) == {'id', 'approval_id', 'revision', 'revoked', 'selection',
        'bindings', 'snapshot_digest', 'created_at', 'expires_at_ms'}
        and policy['id'] == proposal['id'] == policy['approval_id'] and integer(policy['revision'], 1) and policy['revision'] == 1
        and type(policy['created_at']) is str and bool(policy['created_at'])
        and policy['revoked'] is False and exact(policy['selection'], selected)
        and exact(policy['bindings'], proposal['snapshot']['bindings'])
        and policy['snapshot_digest'] == digest(compact(proposal['snapshot']))
        and integer(policy['expires_at_ms'], 1) and policy['expires_at_ms'] <= int(time.time() * 1000) + selected['ttl_ms']
        and policy['expires_at_ms'] <= policy['bindings']['goal_scope_expires_at_ms']
        and all(policy['expires_at_ms'] <= g['expires_at_ms'] for g in policy['bindings']['grant_lineage']))


def verify_consumed(document, stored, proposal, policy):
    consumed = copy.deepcopy(proposal)
    consumed['consumed'] = True
    require(exact(decode(document), consumed) and exact(decode(stored), policy),
        'Consumed original proposal/result mismatch')


def require_budget_refusal(response):
    require(type(response) is dict and 'result' not in response and type(response.get('error')) is dict)
    error = response['error']
    require(error.get('code') == 'RESOURCE_LIMIT' and error.get('side_effect_state') == 'not_started'
        and error.get('retryable') is False and type(error.get('message')) is str)



def expected_selection(context, target, reference):
    # Private test-host intent is fixed BEFORE HTTP; it is not a renderer grant.
    b = context['bindings']
    reserve = lambda output: {'runtime_ms': 5000, 'output_bytes': output, 'tokens': None, 'cost_microunits': None}
    return {'task_id': b['task_id'], 'expected_task_revision': context['task_revision'],
        'expected_spec_revision': b['spec_revision'], 'goal_scope_id': b['goal_scope_id'],
        'expected_scope_revision': b['goal_scope_revision'], 'host_id': 'local', 'target_ref': TARGET,
        'expected_target_head': target, 'max_rounds': 1, 'on_target_advance': 'block', 'ttl_ms': 60000,
        'stage_reservations': {'preparation': reserve(1024),
        'validators': [{'validator': copy.deepcopy(reference), 'reservation': reserve(256)}], 'cas': reserve(1024)}}



class PendingReview:
    def __init__(self, selected, response, key):
        self.selected, self.response, self.key = copy.deepcopy(selected), copy.deepcopy(response), key
        self.proposal = self.response['result']
        self.done, self.reply = threading.Event(), None


class Reviews:
    """Browser asks for a proposal only; main private host decides outside HTTP."""
    def __init__(self, host, bounds, context, definition, expected):
        self.expected = copy.deepcopy(expected)
        self.host, self.bounds, self.context, self.definition = host, bounds, context, definition
        self.pending, self.completed, self.stopping = queue.Queue(), [], threading.Event()

    def propose(self, selected, key):
        response = self.host.call('integration_policies.propose', selected, key)
        if 'error' in response:
            return response
        item = PendingReview(selected, response, key)
        self.pending.put(item)
        require(item.done.wait(20) and item.reply is not None, 'Private review was not completed')
        return item.reply

    def service(self):
        require(threading.current_thread() is threading.main_thread(), 'Review cannot execute in an HTTP handler')
        while True:
            try: item = self.pending.get_nowait()
            except queue.Empty: return
            try:
                require(not self.stopping.is_set() and exact(item.selected, self.expected)
                    and len(compact(item.response)) <= MAX_REVIEW,
                    'Private host never approves an unplanned renderer selection')
                # Fresh actual context, never the browser's proposed snapshot/hash.
                context = self.host.result('integration_context.get',
                    {'task_id': self.bounds.task, 'goal_scope_id': self.bounds.scope})
                facts = review_proposal(item.proposal, item.selected, context, self.definition)
                facts.update(whole_response_bytes=len(compact(item.response)),
                    whole_response_sha256=digest(compact(item.response)))
                reply = self.host.call('approvals.finalize', {'proposal_id': item.proposal['id']},
                    'proposal:' + item.proposal['id'], item.proposal['revision'])
                require('error' not in reply, 'Actual independent approval refused')
                verify_policy(reply['result'], item.proposal, item.selected)
                with database(self.host.runtime) as db:
                    document, stored = db.execute('SELECT document,result FROM approval_proposals WHERE id=?',
                        (item.proposal['id'],)).fetchone()
                    verify_consumed(document, stored, item.proposal, reply['result'])
                with self.bounds.lock: self.bounds.policies.add(reply['result']['id'])
                self.completed.append({'proposal': item.proposal, 'selection': item.selected,
                    'policy': reply['result'], 'facts': facts, 'authority': 'private_test_host_not_OS_dialog'})
                item.reply = reply
            finally:
                item.done.set()

    def stop(self):
        self.stopping.set()
        while True:
            try: item = self.pending.get_nowait()
            except queue.Empty: return
            item.done.set()


PROVIDER = r'''import json,os,pathlib,sys
if '--version' in sys.argv:
 print('account-free integration browser ACP 1');sys.exit(0)
def send(v):print(json.dumps(v),flush=True)
for line in sys.stdin:
 q=json.loads(line);m=q.get('method');i=q.get('id')
 if m=='initialize':send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert q['params']['cwd']==os.getcwd() and q['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-integration-browser'}})
 elif m=='session/prompt':
  c=json.loads(''.join(p.get('text','') for p in q['params']['prompt']))
  assert set(c)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location','task_binding','validation_contracts'}
  assert c['schema_version']==1 and c['kind']=='agentbaton_goal_task_request'
  assert c['workspace']['canonical_root']==os.getcwd() and c['node']['id']=='produce-report'
  assert c['lineage']['attempt_id'] and c['inputs'][0]['candidate'] is None
  assert c['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
  assert c['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
  assert pathlib.Path('inputs/file').read_bytes()==b'baseline'
  pathlib.Path(OBSERVATION).write_text(json.dumps(c))
  pathlib.Path('reports').mkdir(exist_ok=True);pathlib.Path('reports/report.txt').write_bytes(b'fixed report')
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}})
 else:raise AssertionError('Unexpected fixed generated ACP method')
'''


def plan(scope):
    output = scope['outputs']['report']
    require(scope['contracts'][output] == ['goal-owner-review'])
    node = {'id': 'produce-report', 'parent_id': None, 'kind': 'work', 'goal': 'integration-generated-report',
        'dependencies': [], 'workspace_id': scope['workspace_id'], 'executor': {'kind': 'connection',
        'binding': scope['binding'], 'program_sha256': scope['program_sha256']}, 'recipients': [],
        'required_inputs': [{'goal_task_id': scope['task_id'], 'artifact_id': 'fixed-input',
        'contract_id': scope['inputs']['fixed-input']['contract_id'], 'source': {'kind': 'approved_external',
        'sha256': scope['inputs']['fixed-input']['sha256']}}], 'outputs': [{'artifact_id': 'report', 'contract_id': output}],
        'required_validators': scope['contracts'][output], 'side_effect': 'workspace_write',
        'write_set': [{'kind': 'workspace', 'path': 'reports/report.txt'}],
        'budget': {'attempts': 1, 'parallel': 1, 'runtime_ms': 5000, 'output_bytes': 2048,
        'children': 0, 'reworks': 0, 'tokens': None, 'cost_microunits': None},
        'reservation': {'runtime_ms': 4000, 'output_bytes': 2048, 'tokens': None, 'cost_microunits': None},
        'retry': {'max_attempts': 1, 'max_reworks': 0, 'retry_classes': []},
        'idempotency_key': 'producer-once', 'external_action_id': None}
    owner = copy.deepcopy(node)
    owner.update(id='owner-report', kind='owner_approval', goal='Owner reviews exact generated report',
        dependencies=['produce-report'], executor={'kind': 'owner', 'rule_id': 'goal-owner-review'},
        required_inputs=[{'goal_task_id': scope['task_id'], 'artifact_id': 'report', 'contract_id': output,
        'source': {'kind': 'node', 'node_id': 'produce-report', 'candidate_for_validation': True}}],
        outputs=[], side_effect='advisory_read_only', write_set=[], idempotency_key='owner-once')
    return {'schema_version': 1, 'plan_id': 'integration-browser-plan', 'goal': scope['goal'],
        'scope_revision': scope['authority_revision'], 'revision': 1, 'expected_revision': 0,
        'acceptance_rules': scope['required_acceptance_rules'], 'nodes': [node, owner]}


def observe(read, ready, *, pump=lambda: None, seconds=15):
    deadline = time.monotonic() + seconds
    while True:
        pump(); value = read()
        if ready(value): return value
        require(time.monotonic() < deadline, 'Actual bounded observation expired')
        time.sleep(.02)


def git(root, *args):
    env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
    env.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0', GIT_NO_LAZY_FETCH='1')
    result = subprocess.run(['git', '-C', str(root), '-c', 'user.name=Generated integration browser',
        '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', '-c', 'init.templateDir=',
        '-c', 'gc.auto=0', '-c', 'maintenance.auto=false', *args], env=env, capture_output=True, timeout=5, check=True)
    return result.stdout.decode().strip()


def source_image(root):
    result = {}
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root)
        if relative.parts[0] == '.git': continue
        require(not path.is_symlink())
        if path.is_file():
            require(len(result) < 256 and path.stat().st_size <= 16777216)
            result[relative.as_posix()] = path.read_bytes()
    index = Path(git(root, 'rev-parse', '--git-path', 'index'))
    if not index.is_absolute(): index = root / index
    return {'head': git(root, 'rev-parse', 'HEAD'), 'refs': [r for r in git(root, 'for-each-ref',
        '--format=%(refname) %(objectname)').splitlines() if not r.startswith(TARGET + ' ')],
        'index': index.read_bytes(), 'staged': git(root, 'diff', '--cached', '--binary'),
        'unstaged': git(root, 'diff', '--binary'), 'status': git(root, 'status', '--porcelain=v1', '--untracked-files=all'), 'files': result}


def bootstrap(host, root, failing):
    source = root / 'project'; source.mkdir(mode=0o700)
    (source / 'inputs').mkdir(); (source / 'inputs/file').write_bytes(b'baseline')
    (source / 'unselected.txt').write_bytes(b'unselected baseline\n')
    (source / '.gitignore').write_text('/reports/\n')
    git(source, 'init', '-q', '--initial-branch=main'); git(source, 'add', '.')
    git(source, 'commit', '-qm', 'Generated integration browser input'); git(source, 'branch', TARGET[11:])
    target = git(source, 'rev-parse', TARGET)
    provider = root / 'generated-acp'
    provider.write_text('#!' + sys.executable + '\nOBSERVATION=' + repr(str(root / 'request.json')) + '\n' + PROVIDER)
    provider.chmod(0o700)
    shell = root / 'fixed-sh'; shutil.copyfile(Path('/bin/sh').resolve(strict=True), shell); shell.chmod(0o700)
    require(shell.stat().st_nlink == 1)
    require(host.result('system.snapshot', {})['database_schema_version'] == 17)
    project = host.result('projects.import', {'path': str(source), 'name': 'Generated integration browser'}, 'project')
    task = host.result('tasks.create', {'project_id': project['id'], 'goal': 'Generated integration browser ' + ('failure' if failing else 'normal')}, 'task')
    resolved = host.result('workspaces.resolve', {'task_id': task['id'], 'expected_task_revision': task['revision'],
        'expected_project_revision': project['revision'], 'host_id': 'local'})
    require(resolved['execution_authorized'] is False)
    pin = resolved['source']
    ready = host.result('workspaces.prepare_task_git', {'task_id': task['id'],
        'expected_spec_revision': resolved['spec_revision'], 'expected_project_revision': project['revision'], 'host_id': 'local',
        'expected_source_pin': {'canonical_root': pin['canonical_root'], 'device': pin['device'], 'inode': pin['inode'],
        'common_dir': pin['kind']['common_dir'], 'head': resolved['source_head']},
        'preparation_key': resolved['preparation_key']}, 'prepare', task['revision'])
    require(ready['binding']['state'] == 'ready' and ready['identity_revalidated'] is True and ready['execution_authorized'] is False)
    workspace = [w for w in host.result('workspaces.list', {'project_id': project['id']})
        if w['id'] == ready['binding']['receipt']['workspace_id']]
    require(len(workspace) == 1); workspace = workspace[0]
    task = host.result('tasks.get', {'task_id': task['id']})
    require(exact(task['code_workspace_binding'], ready['binding']))
    connection = host.approved('connections.propose', {'name': 'Generated browser ACP', 'provider': 'custom_acp',
        'executable': str(provider), 'arguments': []}, 'connection', 'connection')
    observe(lambda: host.result('connections.list', {}), lambda v: any(
        c['id'] == connection['id'] and c['probe_state'] == 'ready' for c in v))
    grant = host.approved('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'],
        'connection_id': connection['id'], 'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 30000}, 'grant', 'grant')
    budget = {'attempts': 8, 'parallel': 1, 'runtime_ms': 20000, 'output_bytes': 8192,
        'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
    scope = host.approved('goal_scopes.propose', {'task_id': task['id'], 'expected_task_revision': task['revision'],
        'grant_id': grant['id'], 'expected_grant_revision': grant['revision'], 'max_nodes': 2,
        'new_max_children': 1, 'new_max_depth': 1, 'root_budget': budget, 'project_budget': budget,
        'inputs': [{'artifact_id': 'fixed-input', 'relative_path': 'inputs/file', 'max_bytes': 1024, 'expected_sha256': digest(b'baseline')}],
        'outputs': [{'artifact': {'artifact_id': 'report', 'relative_path': 'reports/report.txt', 'max_bytes': 1024, 'expected_sha256': None},
        'validators': []}], 'writable_prefixes': ['reports'], 'owner_acceptance_criteria': 'Owner accepts exact generated report independently'}, 'scope', 'goal')
    host.result('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': compact(plan(scope)).decode()}, 'plan', 0)
    graph = host.approved('goal_graph_runs.propose', {'scope_id': scope['id'], 'expected_plan_revision': 1,
        'expected_ledger_revision': host.result('system.snapshot', {})['sequence']}, 'graph', 'goal_graph_execution')
    detail = observe(lambda: host.result('goal_graph_runs.get', {'run_id': graph['run']['id']}),
        lambda d: any(n['node']['id'] == 'owner-report' and n['state'] == 'awaiting_owner' for n in d['nodes']))
    require(detail['task_accepted'] is False and len(detail['candidates']) == 1)
    candidate = detail['candidates'][0]
    require(candidate['source'] == 'baton_goal_evidence' and candidate['stop_receipt_sequence'] < candidate['capture_receipt_sequence']
        and candidate['artifacts'][0]['sha256'] == digest(REPORT) and candidate['artifacts'][0]['bytes'] == len(REPORT))
    require(host.result('sessions.get', {'session_id': candidate['session_id']})['state'] == 'stopped')
    request = decode((root / 'request.json').read_bytes())
    require(request['run_id'] == graph['run']['id'] and request['plan_digest'] == candidate['plan_digest']
        and request['lineage']['attempt_id'] == candidate['attempt_id'] and type(request['task_binding']) is dict)
    command = host.approved('command_validation_policies.propose', {'session_id': candidate['session_id'],
        'executable': str(shell), 'mode': 'native_executable', 'script': None,
        'inputs': [{'artifact_id': 'fixed-input', 'relative_path': 'inputs/file', 'max_bytes': 1024, 'expected_sha256': digest(b'baseline')}],
        'outputs': [{'artifact_id': 'report', 'relative_path': 'reports/report.txt', 'max_bytes': 1024, 'expected_sha256': None}],
        'arguments': ['-c', "printf '%s\\n' 'fixed correctness failure' >&2; exit 7" if failing else 'test -s reports/report.txt'],
        'environment': {}, 'expected_exit_codes': [0], 'wall_time_ms': 500, 'cleanup_time_ms': 500, 'stdout_limit': 128, 'stderr_limit': 128,
        'require': {k: False for k in ('filesystem_isolation', 'network_isolation', 'process_tree_containment',
        'execution_time_input_immutability', 'auxiliary_source_immutability', 'sealed_executable_image')}}, 'correctness', 'command_validation')
    context = host.result('integration_context.get', {'task_id': task['id'], 'goal_scope_id': scope['id']})
    ref = {'kind': 'command', 'policy_id': command['id'], 'revision': command['revision']}
    definitions = [v for v in context['available_validators'] if exact(v['reference'], ref)]
    require(len(definitions) == 1 and context['bindings']['validators'] == [])
    with database(host.runtime) as db:
        original = (rows(db, 'command_validation_policies', policy=command['id']), rows(db, 'command_validation_runs', policy=command['id']))
        require(len(original[0]) == 1 and original[1] == [])
    managed = Path(workspace['root'])
    (managed / 'unselected.txt').write_bytes(b'staged owner change\n'); git(managed, 'add', 'unselected.txt')
    (managed / 'unselected.txt').write_bytes(b'unstaged owner change\n')
    (managed / 'owner-untracked.txt').write_bytes(b'keep owner bytes\n')
    return {'source': source, 'managed': managed, 'project': project, 'task': task, 'scope': scope,
        'graph': graph, 'candidate': candidate, 'context': context, 'definition': definitions[0],
        'command': command, 'original_command': original, 'target': target,
        'source_image': source_image(managed), 'project_image': source_image(source)}


def accept_graph(host, fixture):
    d = host.result('goal_graph_runs.get', {'run_id': fixture['graph']['run']['id']})
    proposal = host.result('goal_node_approvals.propose', {'run_id': d['run']['id'], 'node_id': 'owner-report',
        'expected_plan_revision': d['run']['plan_revision'], 'expected_ledger_revision': d['ledger_revision']}, 'owner-accept', d['run']['revision'])
    require(proposal['kind'] == 'goal_node_acceptance' and exact(proposal['snapshot']['candidate_receipts'], d['candidates'])
        and len(compact(proposal)) <= MAX_REVIEW)
    decision = host.result('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
    require(decision['decision'] == 'accepted' and decision['task_accepted'] is True)
    accepted = host.result('goal_graph_runs.get', {'run_id': d['run']['id']})
    task = host.result('tasks.get', {'task_id': fixture['task']['id']})
    require(accepted['run']['state'] == 'accepted' and accepted['task_acceptance']['source'] == 'baton_goal_acceptance'
        and task['state'] == 'accepted' and task['acceptance_ref'] == accepted['task_acceptance']['id']
        and task['acceptance_sequence'] == accepted['task_acceptance']['sequence'])
    return accepted


def terminal_evidence(host, fixture, accepted, policy, done, failing):
    run, attempts = done['run'], done['attempts']
    require(run['state'] == ('failed' if failing else 'confirmed_local_ref')
        and run['policy_id'] == policy['id'] and run['policy_revision'] == policy['revision']
        and run['task_id'] == fixture['task']['id'] and run['round'] == 1
        and run['acceptance_ref'] == accepted['task_acceptance']['id']
        and run['acceptance_digest'] == digest(compact(accepted['task_acceptance']))
        and run['expected_target_head'] == fixture['target'] and run['target_ref'] == TARGET
        and run['active_stage'] is None and run['io_completion'] == 'worker_joined'
        and done['fence']['state'] == 'released')
    require((run['last_error']['code'] == 'VALIDATION_FAILED') if failing else run['last_error'] is None)
    candidate = run['integration_commit']
    require(candidate != fixture['target'] and run['source_commit'] != fixture['target'])
    require(git(fixture['source'], 'rev-parse', TARGET) == (fixture['target'] if failing else candidate))
    require(git(fixture['managed'], 'rev-parse', candidate + '^{tree}') == run['integration_tree'])
    require(git(fixture['managed'], 'rev-parse', run['source_commit'] + '^{tree}') == run['source_tree'])
    require(git(fixture['managed'], 'diff', '--name-only', fixture['target'], candidate) == 'reports/report.txt')
    require(git(fixture['managed'], 'show', candidate + ':reports/report.txt') == REPORT.decode())
    require(git(fixture['managed'], 'show', run['source_commit'] + ':unselected.txt') == 'unselected baseline')
    require(source_image(fixture['managed']) == fixture['source_image']
        and source_image(fixture['source']) == fixture['project_image'])
    require(len(attempts) == (2 if failing else 3)
        and [a['kind'] for a in attempts] == (['preparation', 'validator'] if failing else ['preparation', 'validator', 'cas'])
        and len({a['attempt_id'] for a in attempts}) == len(attempts))
    for a in attempts:
        require(a['state'] == 'finished' and a['settled'] is True and a['io_completion'] == 'worker_joined'
            and integer(a['actual_runtime_ms']) and a['actual_runtime_ms'] <= a['reservation']['runtime_ms']
            and integer(a['actual_output_bytes']) and a['actual_output_bytes'] <= a['reservation']['output_bytes'])
    receipts = run['validation_receipts']; require(len(receipts) == 1)
    receipt = receipts[0]
    require(exact(receipt['validator'], fixture['definition']['reference'])
        and receipt['definition_digest'] == fixture['definition']['definition_digest']
        and receipt['integration_commit'] == candidate and receipt['integration_tree'] == run['integration_tree']
        and receipt['passed'] is (not failing) and receipt['stop_confirmed'] is True
        and receipt['worker_joined'] is True and receipt['attempt_id'] == attempts[1]['attempt_id'])
    require(attempts[0]['actual_output_bytes'] == len(REPORT)
        and attempts[1]['actual_output_bytes'] == (len(FAILURE_STDERR) if failing else 0))
    with database(host.runtime) as db:
        events = [(n, decode(d)) for n, d in db.execute('SELECT sequence,document FROM events ORDER BY sequence')]
        def event(kind, aggregate):
            values = [(n, e) for n, e in events if e['kind'] == kind and e['aggregate_id'] == aggregate]
            require(len(values) == 1); return values[0]
        require(exact(rows(db, 'command_validation_policies', policy=fixture['command']['id']), fixture['original_command'][0])
            and exact(rows(db, 'command_validation_runs', policy=fixture['command']['id']), fixture['original_command'][1]))
        stopped = event('session.stopped', fixture['candidate']['session_id'])
        require(stopped[1]['source'] == 'baton_supervisor' and stopped[1]['payload']['process_reaped'] is True
            and stopped[0] == fixture['candidate']['stop_receipt_sequence']
            and stopped[0] < fixture['candidate']['capture_receipt_sequence'])
        queued = event('integration.queued', run['id'])
        require(accepted['task_acceptance']['sequence'] < queued[0]
            and queued[1]['payload']['acceptance_ref'] == accepted['task_acceptance']['id'])
        owner = event('approval.confirmed', policy['id'])
        granted = event('integration.policy_approved', policy['id'])
        require(owner[0] < granted[0] and owner[1]['source'] == 'local_owner_confirmation'
            and granted[1]['payload']['owner_sequence'] == owner[0])
        observed = event('integration.validator_observed', run['id'])
        require(observed[0] == receipt['sequence'])
        view = observed[1]['payload']['view']
        require(digest(compact(view)) == receipt['result_digest'] and view['process_started'] is True
            and view['exit_code'] == (7 if failing else 0) and view['exit_signal'] is None
            and view['passed'] is (not failing) and view['exit_observed_within_budget'] is True
            and view['cleanup_completed_within_budget'] is True and view['cleanup']['owned_leader_reaped'] is True
            and view['cleanup']['all_capture_pipes_observed_eof'] is True and view['goal_io']['state'] == 'observed'
            and view['task_acceptance'] == 'not_established' and view['integration_authorized'] is False)
        for name, raw in [('stdout', b''), ('stderr', FAILURE_STDERR if failing else b'')]:
            capture = view[name]
            require(capture['captured_bytes'] == len(raw) and capture['observed_bytes_at_least'] == len(raw)
                and capture['sha256_of_captured'] == digest(raw) and capture['truncated'] is False
                and capture['eof_observed'] is True)
        subject = {'task_id': fixture['task']['id'], 'attempt_id': receipt['attempt_id'],
            'spec_revision': policy['selection']['expected_spec_revision'], 'input_revision': policy['bindings']['source']['head'],
            'version': {'kind': 'git', 'target_ref': TARGET, 'expected_target_head': fixture['target'],
            'source_candidate': run['source_commit'], 'integration_candidate': candidate, 'tree': run['integration_tree']}}
        contract, old = view['contract'], fixture['command']['contract']
        require(exact(contract['file_contract']['subject'], subject)
            and exact(view['file_integrity']['subject'], subject) and view['file_integrity']['passed'] is True
            and exact(view['file_integrity']['policy_contract'], contract['file_contract'])
            and view['approval_id'] == policy['approval_id'] and view['policy_revision'] == receipt['definition_digest'])
        for field in ('arguments', 'mode', 'script', 'argv0', 'image_policy', 'environment_sha256',
            'expected_exit_codes', 'wall_time_ms', 'cleanup_time_ms', 'stdout_limit', 'stderr_limit', 'require'):
            require(exact(contract[field], old[field]))
        for field in ('canonical_root', 'root_device', 'root_inode'):
            require(exact(contract['program'][field], old['program'][field]))
        for field in ('artifact_id', 'relative_path', 'expected_sha256'):
            require(exact(contract['program']['requirement'][field], old['program']['requirement'][field]))
        cwd = contract['cwd']
        managed = decode(db.execute('SELECT document FROM managed_workspaces WHERE root=?',
            (cwd['canonical_root'],)).fetchone()[0])
        require(exact(managed['identity'], cwd) and managed['base_revision'] == candidate
            and cwd['canonical_root'] != str(fixture['managed']) and cwd['kind']['head'] == candidate)
        for key, name, path, raw in [('inputs', 'fixed-input', 'inputs/file', b'baseline'),
            ('outputs', 'report', 'reports/report.txt', REPORT)]:
            values = [v for v in view['file_integrity'][key] if v['artifact_id'] == name]
            bounds = [v for v in contract['file_contract'][key] if v['requirement']['artifact_id'] == name]
            require(len(values) == len(bounds) == 1); value, bound = values[0], bounds[0]
            require(value['relative_path'] == path and value['size'] == len(raw)
                and value['expected_sha256'] == value['observed_sha256'] == digest(raw) and value['state'] == 'passed'
                and value['root_device'] == cwd['device'] and value['root_inode'] == cwd['inode']
                and bound['canonical_root'] == cwd['canonical_root'] and bound['requirement']['expected_sha256'] == digest(raw))
        cases = [(n, e) for n, e in events if e['kind'] == 'integration.cas_observed' and e['aggregate_id'] == run['id']]
        if failing:
            require(cases == [] and db.execute("SELECT count(*) FROM integration_attempts WHERE kind='cas'").fetchone() == (0,))
        else:
            require(len(cases) == 1 and observed[0] < cases[0][0])
            cas = cases[0][1]['payload']; io = cas['io']; record = cas['record']
            require(cas['historical_cas_confirmed'] is True and io['stdout_eof'] is True
                and io['stderr_eof'] is True and io['owned_leader_reaped'] is True
                and attempts[2]['actual_output_bytes'] == io['stdout_retained_bytes'] + io['stderr_retained_bytes']
                and record['grant_ref'] == policy['approval_id'] and record['expected_target_head'] == fixture['target']
                and record['candidate_commit'] == candidate and record['target_ref'] == TARGET and record['state'] == 'confirmed_local_ref')
        for a in attempts:
            found = [(n, e) for n, e in events if e['kind'] == 'integration.attempt_joined'
                and e['aggregate_id'] == run['id'] and e['payload']['attempt_id'] == a['attempt_id']]
            require(len(found) == 1 and found[0][0] == a['receipt_sequence']
                and found[0][1]['payload']['reservation_refunded'] is False)
        source_usage = decode(db.execute('SELECT document FROM goal_run_reservations WHERE run_id=?',
            (fixture['graph']['run']['id'],)).fetchone()[0])
        require(source_usage['settled'] is True and source_usage['actual_output_bytes'] == len(REPORT))
        total_ms = source_usage['actual_runtime_ms'] + sum(a['actual_runtime_ms'] for a in attempts)
        total_bytes = source_usage['actual_output_bytes'] + sum(a['actual_output_bytes'] for a in attempts)
        for domain, root_id in [('root', policy['bindings']['root_task_id']), ('project', policy['bindings']['project_id'])]:
            usage = done['shared_usage'][domain]
            require(usage['attempts'] == len(attempts) + 1 and usage['active'] == 0 and usage['runtime_ms'] == total_ms
                and usage['output_bytes'] == total_bytes and usage['tokens'] is None and usage['cost_microunits'] is None)
            require(db.execute('SELECT consumed,active FROM budgets WHERE scope=?', (domain + ':' + root_id,)).fetchone() == (len(attempts) + 1, 0))
        require(db.execute("SELECT count(*) FROM workspace_leases WHERE holder!=''").fetchone() == (0,)
            and db.execute("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'").fetchone() == (0,))
    policies = host.result('command_validation_policies.list', {'task_id': fixture['task']['id']})
    require([p for p in policies if p['id'] == fixture['command']['id']] == [fixture['command']])
    graph = host.result('goal_graph_runs.get', {'run_id': fixture['graph']['run']['id']})
    require(all(exact(graph[k], accepted[k]) for k in ('run', 'candidates', 'validations', 'owner_decisions', 'task_acceptance')))
    return {'state': run['state'], 'stages': len(attempts), 'command_exit': view['exit_code'],
        'cas_attempts': 0 if failing else 1, 'source_and_index_unchanged': True, 'original_command_unchanged': True,
        'actual_owned_reap_and_join': True, 'shared_runtime_ms': total_ms, 'shared_output_bytes': total_bytes,
        'tokens': None, 'cost_microunits': None}


class FixtureServer(http.server.ThreadingHTTPServer):
    daemon_threads = False
    block_on_close = True

    def handle_error(self, request, client_address):
        self.faults.append({'stage': 'handler', 'kind': type(sys.exc_info()[1]).__name__})


def serve(frontend, host, bounds, reviews, attribution):
    class Handler(http.server.SimpleHTTPRequestHandler):
        def setup(self):
            self.request.settimeout(5); super().setup()

        def log_message(self, *_):
            pass

        def do_GET(self):
            relative = 'index.html' if self.path == '/' else self.path.lstrip('/')
            if relative not in attribution['assets']:
                self.send_error(404); return
            super().do_GET()

        def do_POST(self):
            method, stage = None, 'boundary_decode'
            if self.path != ENDPOINT:
                self.send_error(404); return
            try:
                for name in ('Host', 'Origin', 'Content-Type', 'Content-Length'):
                    require(len(self.headers.get_all(name, [])) == 1)
                size = self.headers['Content-Length']
                require(re.fullmatch('[1-9][0-9]{0,5}', size) is not None and int(size) <= MAX_BODY)
                method, params, key, revision = parse_http(self.rfile.read(int(size)), self.headers, self.server.origin, bounds)
                stage = 'core_call'
                if method == 'integration_policies.propose':
                    response = reviews.propose(params, key)
                else:
                    response = host.call(method, params, key, revision)
                bounds.observe(method, response)
                self.server.calls.append({'method': method, 'error_code': response.get('error', {}).get('code')})
                raw = compact(response); require(len(raw) <= MAX_REPLY)
                stage = 'response_headers'
                self.send_response(200); self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(raw))); self.end_headers()
                stage = 'response_body'; self.wfile.write(raw)
            except Refusal:
                self.server.faults.append({'stage': stage, 'kind': 'Refusal'})
                self.send_error(403)
            except BaseException as error:
                if readonly_abort(error, method, stage):
                    self.server.aborts.append({'method': method, 'stage': stage})
                else:
                    self.server.faults.append({'stage': stage, 'kind': type(error).__name__})
                self.close_connection = True
    server = FixtureServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
    server.origin = 'http://127.0.0.1:' + str(server.server_port)
    server.calls, server.faults, server.aborts = [], [], []
    thread = threading.Thread(target=server.serve_forever, name='integration-fixture-http')
    thread.start()
    return server, thread


def run_case(args, binary, companion, frontend, attribution, output, failing):
    # Imports happen only for an explicitly authorized real run, never pure tests.
    from playwright.sync_api import sync_playwright, expect
    report = {'case': 'command_exit7' if failing else 'normal', 'status': 'failed_or_interrupted',
        'dimension': 'Linux built React/Chromium + finite TEST bridge + actual Core17 Unix IPC + generated ACP + private trusted test host',
        'not_claimed': ['production Tauri Native formatter', 'OS dialog', 'macOS', 'live Provider/model',
            'OS sandbox', 'whole-filesystem output quota', 'complete failure matrix', 'whole product completion'],
        'checks': [], 'screenshots': [], 'cleanup': {}, 'inputs': attribution}
    notification_probe = getattr(args, '_notification_probe', None) if not failing else None
    notification_report = None
    notification_passed = False
    if notification_probe is not None:
        notification_output = output / 'notification-probe'; notification_output.mkdir(mode=0o700)
        notification_report = notification_probe.pending_report(attribution['source'], args._notification_probe_source)
    private = output / 'private'; private.mkdir(mode=0o700)
    root = Path(tempfile.mkdtemp(prefix='ab-integ-ui-', dir='/tmp')).resolve(); root.chmod(0o700)
    runtime = root / 'r'; runtime.mkdir(mode=0o700)
    require(len(str(runtime / 'batond.sock').encode()) < 104)
    daemon = engine = browser = server = thread = host = reviews = None
    log = (private / 'daemon.log').open('xb')
    phase, passed, cleanup_errors = 'startup', False, []
    def checked(name, **facts): report['checks'].append({'name': name, 'status': 'passed', **facts})
    def cleanup(name, fn):
        try: report['cleanup'][name] = fn()
        except BaseException as error:
            cleanup_errors.append(name); report['cleanup'][name] = {'error': type(error).__name__}
    try:
        # Start Chromium before the finite Grant lifetime begins, without changing it.
        engine = sync_playwright().start()
        browser = engine.chromium.launch(executable_path=str(args.chromium), headless=True,
            args=['--no-sandbox', '--disable-dev-shm-usage'])
        report['browser_version'] = browser.version
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        observe(lambda: (daemon.poll(), (runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists()),
            lambda v: v == (None, True, True), seconds=10)
        require(stat.S_IMODE((runtime / 'session.token').stat().st_mode) == 0o600
            and stat.S_ISSOCK((runtime / 'batond.sock').lstat().st_mode)
            and stat.S_IMODE((runtime / 'batond.sock').stat().st_mode) == 0o600)
        host = Host(runtime); phase = 'real_private_graph_bootstrap'
        fixture = bootstrap(host, root, failing)
        bounds = Boundary(fixture['task']['id'], fixture['project']['id'], fixture['scope']['id'], fixture['target'],
            [fixture['definition']['reference']])
        reviews = Reviews(host, bounds, fixture['context'], fixture['definition'],
            expected_selection(fixture['context'], fixture['target'], fixture['definition']['reference']))
        server, thread = serve(frontend, host, bounds, reviews, attribution)
        context = browser.new_context(viewport={'width': 1440, 'height': 1100}, locale='en-US')
        context.add_init_script("localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('" + ENDPOINT + "',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw Error('Finite integration TEST bridge refused');return r.json();}};")
        context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(server.origin + '/') else route.abort())
        page = context.new_page(); page_errors = []
        page.on('pageerror', lambda _: page_errors.append('uncaught_browser_error'))
        def pump():
            reviews.service(); page.wait_for_timeout(10)
        def shot(name, focus):
            focus.scroll_into_view_if_needed()
            expect(focus).to_be_in_viewport(ratio=1)
            focus_box_before_capture = focus.bounding_box()
            path = output / (name + '.png'); page.screenshot(path=str(path))
            report['screenshots'].append({'name': name, 'sha256': digest(path.read_bytes()),
                'bytes': path.stat().st_size, 'viewport': page.viewport_size,
                'focus_box_before_capture_observation': focus_box_before_capture,
                'focus_viewport_asserted_before_capture': True, 'pixel_synchronized': False,
                'dimension': 'Linux Chromium only'})
        phase = 'actual_frontend'
        page.goto(server.origin + '/', wait_until='networkidle')
        page.locator('[data-testid="task-card"][data-task-id="' + bounds.task + '"]').click()
        page.get_by_role('tab', name='Details', exact=True).click()
        panel = page.get_by_test_id('task-integration')
        panel.locator(':scope > summary').click()
        expect(panel.get_by_role('button', name='Refresh integration records')).to_be_enabled()
        panel.get_by_label('Existing local target branch').select_option(TARGET)
        expect(panel.get_by_text(fixture['target'], exact=True)).to_be_visible()
        panel.get_by_text('Fresh candidate correctness checks', exact=True).click()
        panel.get_by_role('checkbox', name=re.compile('Add check command:')).check()
        panel.get_by_text('Stage reservation', exact=True).click()
        runtimes = panel.get_by_label(re.compile(r' · Runtime reservation \(milliseconds\)$'))
        require(runtimes.count() == 3)
        for i in range(3): expect(runtimes.nth(i)).to_have_value('5000')
        expect(panel.get_by_text('Task acceptance: Task acceptance pending', exact=True)).to_be_visible()
        checked('real managed Task/Graph before Task acceptance; visible new5s draft; optional original Command selected')
        if not failing:
            phase = 'actual_default_budget_refusal'
            before = integration_counts(runtime)
            panel.get_by_role('button', name='Review branch permission in native dialog').click()
            observe(lambda: [v for v in server.calls if v['method'] == 'integration_policies.propose'],
                lambda v: len(v) == 1, pump=pump)
            rejected = [v for v in host.audit if v['method'] == 'integration_policies.propose']
            require(len(rejected) == 1)
            require_budget_refusal(rejected[0]['response'])
            expect(panel.get_by_role('alert')).to_contain_text('Integration record or shared resource bound exceeded')
            require(integration_counts(runtime) == before == (0, 0, 0, 0)
                and reviews.completed == [] and git(fixture['source'], 'rev-parse', TARGET) == fixture['target'])
            checked('actual Core RESOURCE_LIMIT for oversized default output; no target/fence/policy/run/attempt authority')
            # The product intentionally requires a read-only refresh after every write/refusal.
            panel.get_by_role('button', name='Refresh integration records').click()
            expect(panel.get_by_role('button', name='Review branch permission in native dialog')).to_be_enabled()
        # Explicit user selection fits the ORIGINAL8192-byte shared budget.
        outputs = panel.get_by_label(re.compile(r' · Output reservation \(bytes\)$'))
        require(outputs.count() == 3)
        for i, value in enumerate(('1024', '256', '1024')): outputs.nth(i).fill(value)
        phase = 'private_independent_review'
        panel.get_by_role('button', name='Review branch permission in native dialog').click()
        observe(lambda: len(reviews.completed), lambda n: n == 1, pump=pump)
        expect(panel.get_by_role('button', name='Refresh integration records')).to_be_enabled()
        panel.get_by_role('button', name='Refresh integration records').click()
        expect(panel.get_by_text('Permission armed; waiting for verified task acceptance. No branch update is confirmed.', exact=True)).to_be_visible()
        policy = reviews.completed[0]['policy']
        require(integration_counts(runtime) == (1, 0, 0, 0)
            and git(fixture['source'], 'rev-parse', TARGET) == fixture['target'])
        checked('complete real pending proposal and consumed receipt matched outside HTTP before TaskAccepted',
            **reviews.completed[0]['facts'])
        shot('task-pending-before-acceptance', panel.get_by_text('Task acceptance: Task acceptance pending', exact=True))
        shot('permission-before-task-acceptance', panel.get_by_text('Permission armed; waiting for verified task acceptance. No branch update is confirmed.', exact=True))
        phase = 'real_owner_acceptance_and_watcher'
        accepted = accept_graph(host, fixture)
        require(not any(v['method'] == 'integrations.start' for v in server.calls))
        def read_terminal():
            runs = host.result('integrations.list', {'task_id': bounds.task})
            require(len(runs) <= 1)
            return None if not runs else host.result('integrations.get', {'integration_id': runs[0]['id']})
        def finished(value):
            if value is None: return False
            state = value['run']['state']
            require(state not in ('outcome_unknown', 'stale_target', 'blocked', 'cancelled'), 'Actual runtime did not establish expected outcome')
            require(state != ('confirmed_local_ref' if failing else 'failed'), 'Actual runtime reached opposite terminal outcome')
            return state == ('failed' if failing else 'confirmed_local_ref')
        done = observe(read_terminal, finished, pump=pump)
        facts = terminal_evidence(host, fixture, accepted, policy, done, failing)
        checked('actual watcher fresh merged Command/owned stop/join/shared fees/source preservation and exact local ref result', **facts)
        require(not any(v['method'] == 'integrations.start' for v in server.calls))
        panel.get_by_role('button', name='Refresh integration records').click()
        history = panel.get_by_role('region', name='Integration history ' + done['run']['id'], exact=True)
        expect(history.get_by_text(done['run']['state'], exact=True)).to_be_visible()
        expect(history.get_by_text('No confirmed local branch update' if failing else 'Confirmed local branch update', exact=True)).to_be_visible()
        if failing: expect(history.get_by_role('alert')).to_contain_text('VALIDATION_FAILED')
        history.get_by_text('Actual validation receipts', exact=True).click()
        expect(history.get_by_text(re.compile(('failed' if failing else 'passed') + ' · stop_confirmed=true · worker_joined=true'))).to_be_visible()
        shot('terminal-state-desktop', history.get_by_text(done['run']['state'], exact=True))
        shot('terminal-receipt-desktop', history.get_by_text(re.compile(('failed' if failing else 'passed') + ' · stop_confirmed=true · worker_joined=true')))
        page.set_viewport_size({'width': 390, 'height': 844})
        shot('terminal-state-mobile', history.get_by_text(done['run']['state'], exact=True))
        shot('terminal-receipt-mobile', history.get_by_text(re.compile(('failed' if failing else 'passed') + ' · stop_confirmed=true · worker_joined=true')))
        require(page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'))
        checked('actual built component displays separate Task/integration result and real receipt; desktop/mobile screenshots')
        if notification_probe is not None:
            phase = 'notification_probe'
            require(notification_probe.run_probe(page, host, bounds, fixture, accepted, server,
                notification_report, notification_output, private_json), 'Real notification probe failed')
        require(not page_errors and not server.faults)
        report['http_method_counts'] = dict(Counter(v['method'] for v in server.calls))
        report['readonly_reply_aborts'] = len(server.aborts)
        passed = True
    except BaseException as error:
        report['failure'] = {'stage': phase, 'kind': type(error).__name__}
        private_json(private / 'failure.json', {'stage': phase, 'traceback': traceback.format_exc()})
    finally:
        # Unblock any held proposal first, join every HTTP handler before teardown.
        if reviews is not None: cleanup('pending_reviews_stopped', lambda: (reviews.stop(), True)[1])
        if browser is not None: cleanup('browser_closed', lambda: (browser.close(), True)[1])
        if engine is not None: cleanup('playwright_stopped', lambda: (engine.stop(), True)[1])
        if server is not None:
            cleanup('http_shutdown', lambda: (server.shutdown(), server.server_close(), True)[2])
        if thread is not None:
            def join_server():
                thread.join(5); require(not thread.is_alive()); return True
            cleanup('http_thread_joined', join_server)
        if daemon is not None:
            def stop_daemon():
                require(daemon.poll() is None, 'Daemon exited before owned shutdown')
                daemon.send_signal(signal.SIGTERM)
                try: code = daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    daemon.kill(); daemon.wait(timeout=5)
                    raise Refusal('Forced kill cannot count as clean owned shutdown')
                require(code == 0, 'Nonzero daemon shutdown')
                return {'exit': code, 'joined': True, 'forced_kill': False}
            cleanup('daemon_joined', stop_daemon)
        log.close()
        if host is not None: cleanup('private_transcript_saved', lambda: (private_json(private / 'transcript.json', host.audit), True)[1])
        def unchanged_inputs():
            require(pinned_file(binary) == attribution['tools']['batond']
                and pinned_file(companion) == attribution['tools']['baton'] and assets(frontend) == attribution['assets']
                and source_guard(attribution['source']) == attribution['source_guard']
                and pinned_file(args.chromium, MAX_CHROMIUM_IMAGE) == attribution['runtime_tools']['chromium']
                and pinned_file(Path(sys.executable).resolve(strict=True)) == attribution['runtime_tools']['python'])
            if notification_probe is not None:
                require(notification_probe_source(attribution['source']) == args._notification_probe_source)
            return True
        cleanup('tools_assets_unchanged', unchanged_inputs)
        if server is not None and server.faults:
            passed = False; report['http_faults'] = server.faults
        if passed and not cleanup_errors:
            cleanup('fixture_removed', lambda: (shutil.rmtree(root), True)[1])
        if not passed or cleanup_errors:
            private_json(private / 'retained.json', {'root': str(root), 'state': 'after_owned_cleanup_attempts',
                'not_a_panic_instant_snapshot': True, 'cleanup_errors': cleanup_errors})
            report['fixture_retained_privately'] = root.exists()
        report['status'] = 'passed' if passed and not cleanup_errors else 'failed_or_interrupted'
        private_json(output / 'report.json', report)
        if notification_probe is not None:
            notification_passed = notification_probe.seal_report(notification_report, report)
            private_json(notification_output / 'report.json', notification_report)
    return report['status'] == 'passed' and (notification_probe is None or notification_passed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--candidate-manifest', type=Path, required=True)
    parser.add_argument('--frontend', type=Path, required=True)
    parser.add_argument('--frontend-build-record', type=Path, required=True)
    parser.add_argument('--expected-source-head', required=True)
    parser.add_argument('--chromium', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--case', choices=('normal', 'command-failure', 'all'), default='all')
    parser.add_argument('--notification-probe', action='store_true',
        help='After the original normal-case checks only, observe real in-app notification UI with existing read-only IPC')
    parser.add_argument('--run', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    require(stat.S_IMODE(args.output.stat().st_mode) == 0o700 and args.output.stat().st_uid == os.geteuid()
        and not args.output.is_symlink())
    try:
        binary, companion, frontend, attribution = preflight(args)
    except BaseException as error:
        private_json(args.output / 'report.json', {'status': 'preflight_refused', 'actual_runtime': 'NOT_RUN',
            'kind': type(error).__name__, 'no_process_token_or_listener': True})
        return 2
    if not args.run:
        private_json(args.output / 'report.json', {'status': 'NOT_RUN', 'reason': '--run required', 'inputs': attribution})
        return 2
    try:
        from playwright.sync_api import sync_playwright  # noqa: F401; dependency check only
        if args.notification_probe and args.case != 'command-failure':
            args._notification_probe, args._notification_probe_source = load_notification_probe(args.expected_source_head)
    except (ImportError, Refusal, OSError) as error:
        private_json(args.output / 'report.json', {'status': 'preflight_refused', 'actual_runtime': 'NOT_RUN',
            'kind': type(error).__name__, 'no_process_token_or_listener': True})
        return 2
    result = True
    for case in (('normal', 'command-failure') if args.case == 'all' else (args.case,)):
        output = args.output / case; output.mkdir(mode=0o700)
        result = run_case(args, binary, companion, frontend, attribution, output, case == 'command-failure') and result
    print(json.dumps({'status': 'passed' if result else 'failed_or_interrupted', 'dimension': 'Linux/browser/generatedACP/private_test_host_not_OS'}))
    return 0 if result else 1


if __name__ == '__main__':
    raise SystemExit(main())
