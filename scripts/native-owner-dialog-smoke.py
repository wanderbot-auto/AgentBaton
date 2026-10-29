#!/usr/bin/env python3
"""Own-process macOS Grant(default) / Factory planning dialog fixtures.

No production credentials/OS permissions. Default Grant host bootstrap finalizes
only its generated --version connection. Factory mode additionally prepares its
own explicit enclosing nondelegating Grant; the target Factory is finalized ONLY
by the real production factory_scope_approve bridge after actual OS confirmation.
"""
import argparse
from contextlib import contextmanager, ExitStack
import hashlib
import importlib.util
import ctypes
import json
import math
import os
import platform
from pathlib import Path
import re
import signal
import socket
import stat
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid


# Exact schema14 tables: never substitute old test DDL or silently skip absent
# tables. Every Goal store/runtime/graph table is empty in this Grant-only probe.
GOAL_TABLES = ('goal_grant_revisions', 'goal_scopes', 'goal_plan_heads',
               'goal_plan_revisions', 'goal_plan_proposals', 'goal_ledgers',
               'goal_node_evidence', 'goal_external_action_journal', 'goal_runs',
               'goal_run_nodes', 'goal_run_reservations', 'goal_graph_capabilities',
               'goal_graph_jobs', 'goal_graph_candidates', 'goal_graph_validations',
               'goal_graph_owner_decisions', 'goal_graph_acceptances')
FACTORY_TABLES = ('factory_scopes', 'factory_runs', 'factory_task_lineage',
                  'factory_task_bindings', 'factory_delegations', 'factory_attempts',
                  'factory_scoped_handles', 'factory_handoffs', 'factory_budget_reservations',
                  'factory_child_plan_heads', 'factory_child_plan_revisions')
ZERO_TABLES = ('attempts', 'sessions', *GOAL_TABLES, *FACTORY_TABLES)


def save(path, value):
    temporary = path.with_suffix('.json.partial')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(path)


class NativeCoreClient:
    def __init__(self, runtime):
        self.runtime = runtime
        self.connection_proposal = None
        self.transcript = []

    def call(self, method, params, key=None, revision=None):
        allowed = {'system.snapshot', 'projects.import', 'workspaces.create',
                   'connections.propose', 'connections.list', 'grants.propose'}
        if method == 'approvals.finalize':
            if params != {'proposal_id': self.connection_proposal} or self.connection_proposal is None:
                raise ValueError('Host may finalize only its generated version-probe connection; never a Grant')
        elif method not in allowed:
            raise ValueError('Not a fixed bootstrap/read method')
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()),
                 'token': (self.runtime / 'session.token').read_text(), 'method': method, 'params': params}
        if key is not None:
            frame['idempotency_key'] = key
        if revision is not None:
            frame['expected_revision'] = revision
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(20)
            stream.connect(str(self.runtime / 'batond.sock'))
            stream.sendall(json.dumps(frame).encode() + b'\n')
            raw = bytearray()
            while not raw.endswith(b'\n'):
                chunk = stream.recv(65536)
                if not chunk or len(raw) + len(chunk) > 4 * 1024 * 1024:
                    raise RuntimeError('Core response incomplete/over fixture cap')
                raw.extend(chunk)
        reply = json.loads(raw)
        self.transcript.append({'method': method, 'params': params,
                                'idempotency_key': key, 'expected_revision': revision, 'reply': reply})
        if 'error' in reply:
            raise RuntimeError(f'{method}: {reply["error"]}')
        return reply['result']


def database(runtime, proposal_id, *, factory_planning=False):
    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as conn:
        conn.execute('PRAGMA query_only=ON')
        conn.execute('BEGIN')
        schema = conn.execute('PRAGMA user_version').fetchone()[0]
        sequence = conn.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
        rows = conn.execute('SELECT document,result FROM approval_proposals WHERE id=?', (proposal_id,)).fetchall()
        if len(rows) != 1:
            raise ValueError('Missing/ambiguous exact Grant proposal')
        proposal = json.loads(rows[0][0])
        result = None if rows[0][1] is None else json.loads(rows[0][1])
        grants = [json.loads(row[0]) for row in conn.execute('SELECT document FROM execution_grants')]
        events = []
        for seq, raw in conn.execute('SELECT sequence,document FROM events ORDER BY sequence'):
            item = json.loads(raw)
            item['sequence'] = seq
            events.append(item)
        counts = {table: conn.execute(f'SELECT count(*) FROM {table}').fetchone()[0]
                  for table in ZERO_TABLES}
        extra = {}
        if factory_planning:
            extra = {'grant_documents': dict(conn.execute('SELECT id,document FROM execution_grants')),
                     'other_proposals': dict(conn.execute('SELECT id,document FROM approval_proposals WHERE id!=?', (proposal_id,))),
                     'task_documents': dict(conn.execute('SELECT id,document FROM tasks')),
                     'scopes': [{'document': json.loads(doc), 'owner_sequence': seq} for doc, seq in conn.execute('SELECT document,owner_sequence FROM factory_scopes')],
                     'dispatch_intents': conn.execute('SELECT count(*) FROM dispatch_intents').fetchone()[0]}
        conn.rollback()
    return {'read_only': True, 'schema_version': schema, 'sequence': sequence,
            'proposal': proposal, 'proposal_result': result, 'grants': grants, 'events': events, 'counts': counts, **extra}


def json_exact(left, right):
    """JSON value equality with distinct bool/int/float types; no coercion."""
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        return (all(type(key) is str for key in left) and all(type(key) is str for key in right)
                and left.keys() == right.keys()
                and all(json_exact(left[key], right[key]) for key in left))
    if type(left) is list:
        return len(left) == len(right) and all(json_exact(a, b) for a, b in zip(left, right))
    if type(left) is float:
        return math.isfinite(left) and math.isfinite(right) and left == right
    return type(left) in (str, int, bool, type(None)) and left == right


def pending_error(value, proposal, sequence):
    if (type(sequence) is not int or type(proposal.get('revision')) is not int
            or proposal['revision'] <= 0
            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 14)
            or type(value.get('sequence')) is not int or not json_exact(value['sequence'], sequence)
            or not json_exact(value.get('proposal'), proposal) or proposal.get('kind') != 'grant'
            or proposal.get('consumed') is not False or value.get('proposal_result') is not None
            or value.get('grants') != [] or not isinstance(value.get('counts'), dict)
            or set(value['counts']) != set(ZERO_TABLES)
            or any(type(n) is not int or n != 0 for n in value['counts'].values())
            or any(e.get('aggregate_id') == proposal['id'] and e.get('kind') == 'approval.confirmed' for e in value.get('events', []))):
        return 'Pending Grant authority/sequence/schema changed or absent'
    return None


def opaque_id(value):
    if not isinstance(value, str) or not value.strip():
        return False
    try:
        return len(value.encode('utf-8')) <= 512
    except UnicodeEncodeError:
        return False


def grant_scope(intent):
    return {'workspace_ids': [intent['workspace_id']], 'connection_ids': [intent['connection_id']],
            'host_ids': ['local'], 'max_attempts': intent['max_attempts'],
            'max_concurrency': intent['max_concurrency'], 'max_duration_ms': intent['max_duration_ms'],
            'recipients': [], 'network': {'mode': 'deny'}, 'max_depth': 0, 'max_children': 0}


def grant_enforcement():
    return {'baton_mediated_scope': True, 'os_filesystem_sandbox': False,
            'os_network_sandbox': False, 'native_identity_isolation': False}


def approved_error(value, proposal, baseline, response):
    grants = value.get('grants')
    current = value.get('proposal', {})
    if (type(baseline) is not int or type(proposal.get('revision')) is not int
            or proposal['revision'] <= 0
            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 14)
            or not isinstance(grants, list) or len(grants) != 1
            or current.get('id') != proposal['id'] or not json_exact(current.get('snapshot'), proposal['snapshot'])
            or current.get('kind') != 'grant' or current.get('consumed') is not True
            or type(current.get('revision')) is not int or not json_exact(current['revision'], proposal['revision'] + 1)
            or not opaque_id(grants[0].get('id')) or not opaque_id(grants[0].get('approval_id'))
            or not json_exact(value.get('proposal_result'), grants[0]) or not json_exact(response.get('result'), grants[0])
            or response.get('error') is not None or type(value.get('sequence')) is not int or not json_exact(value['sequence'], baseline + 2)
            or not isinstance(value.get('counts'), dict)
            or set(value['counts']) != set(ZERO_TABLES)
            or any(type(n) is not int or n != 0 for n in value['counts'].values())):
        return 'Exact production Grant commit/CAS/result/zero execution evidence missing'
    grant = grants[0]
    intent = proposal['snapshot']['intent']
    if (grant.get('project_id') != intent['project_id']
            or not json_exact(grant.get('scope'), grant_scope(intent))
            or any(type(grant['scope'].get(k)) is not int for k in ['max_attempts', 'max_concurrency', 'max_duration_ms', 'max_depth', 'max_children'])
            or not json_exact(grant.get('enforcement'), grant_enforcement())
            or any(type(grant['enforcement'].get(k)) is not bool for k in grant_enforcement()) or grant.get('parent_id') is not None
            or grant.get('root_grant_id') != grant.get('id') or type(grant.get('depth')) is not int or grant['depth'] != 0
            or type(grant.get('revision')) is not int or grant['revision'] != 1
            or grant.get('revoked') is not False or grant.get('scope', {}).get('workspace_ids') != [intent['workspace_id']]
            or grant['scope'].get('connection_ids') != [intent['connection_id']]
            or any(grant['scope'].get(k) != intent[k] for k in ['max_attempts', 'max_concurrency', 'max_duration_ms'])
            or not json_exact(grant.get('workspace_bindings'), {intent['workspace_id']: proposal['snapshot']['refs']['workspace_identity']})
            or not json_exact(grant.get('connection_bindings'), {intent['connection_id']: proposal['snapshot']['refs']['connection']})):
        return 'Committed root Grant does not match the exact bounded intent and resolved bindings'
    grant_events = [e for e in value.get('events', []) if e.get('kind') == 'execution_grant.approved']
    expected_grant_event = {'grant_id': grant['id'], 'project_id': grant['project_id'],
                            'root_grant_id': grant['root_grant_id'], 'approval_id': grant['approval_id'],
                            'scope': grant['scope'], 'enforcement': grant['enforcement']}
    if (len(grant_events) != 1 or grant_events[0].get('aggregate_id') != grant['id']
            or grant_events[0].get('source') != 'user_action'
            or type(grant_events[0].get('revision')) is not int or not json_exact(grant_events[0]['revision'], grant['revision'])
            or type(grant_events[0].get('sequence')) is not int or not json_exact(grant_events[0]['sequence'], baseline + 1)
            or not json_exact(grant_events[0].get('payload'), expected_grant_event)):
        return 'Exact persisted execution_grant.approved event missing'
    confirmations = [event for event in value.get('events', []) if event.get('aggregate_id') == proposal['id'] and event.get('kind') == 'approval.confirmed']
    if (len(confirmations) != 1 or confirmations[0].get('source') != 'local_owner_confirmation'
            or not json_exact(confirmations[0].get('revision'), current['revision'])
            or type(confirmations[0].get('sequence')) is not int or not json_exact(confirmations[0]['sequence'], baseline + 2)
            or not json_exact(confirmations[0].get('payload'), {'kind': 'grant', 'proposal_id': proposal['id'], 'consumed': True})):
        return 'Exact single Core Owner confirmation event missing'
    return None


def expected_message(proposal, source):
    # Read, do not execute or replace, the actual shipping formatter. Fail closed
    # on refactoring rather than accepting a partial/digest-only message.
    match = re.search(r'let message = format!\(\s*("(?:[^"\\]|\\.)*")\s*,\s*kind.action\(\)\s*\)', source)
    if not match:
        raise ValueError('Production full-message formatter changed; audit before OS click')
    template = json.loads(match.group(1))
    if template.count('{}') != 1 or template.count('{table_explanation}') != 1 or template.count('{snapshot}') != 1:
        raise ValueError('Unexpected production formatter placeholders')
    action = 'Grant this exact recipient and workspace the bounded execution scope below'
    snapshot = json.dumps(proposal['snapshot'], ensure_ascii=False, indent=2, sort_keys=True)
    # serde_json default map is sorted in this workspace; mechanical source/fixture check.
    return template.replace('{}', action, 1).replace('{table_explanation}', '', 1).replace('{snapshot}', snapshot, 1)


def presentation_prompts(message):
    """Independent reversible mirror of native_parent's bounded display format."""
    if not isinstance(message, str) or len(message.encode('utf-8')) > 1_048_576 + 32_768:
        raise ValueError('Complete native message exceeds presentation envelope')
    rows, encoded, start, offset = [], '', 0, 0
    for char in message:
        code = ord(char)
        escaped = (f'\\u{code:04x}' if code in (127, 0x061c) or 128 <= code <= 159 or 0x200b <= code <= 0x200f
                   or 0x2028 <= code <= 0x202e or 0x2060 <= code <= 0x206f or code == 0xfeff
                   else json.dumps(char, ensure_ascii=False)[1:-1])
        if encoded and len(encoded.encode('utf-8')) + len(escaped.encode('utf-8')) > 40:
            rows.append((start, offset, encoded))
            start, encoded = offset, ''
        encoded += escaped
        offset += len(char.encode('utf-8'))
    if encoded or not rows:
        rows.append((start, offset, encoded))
    restored = ''.join(json.loads('"' + row[2] + '"') for row in rows)
    if restored != message:
        raise ValueError('Presentation did not preserve complete original bytes')
    total = (len(rows) + 7) // 8
    pages = []
    for index in range(total):
        page = rows[index * 8:(index + 1) * 8]
        body = '\n'.join(f'L{index * 8 + row + 1:05} "{text}"' for row, (_, _, text) in enumerate(page))
        pages.append(f'Native review page {index + 1}/{total}\nOriginal UTF-8 bytes {page[0][0]}..{page[-1][1]} of {offset}.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\n{body}')
    pages.append(f'FINAL display confirmation\nAll {total} pages / {offset} original UTF-8 bytes were reviewed.\nOK confirms the original request exactly as displayed.\nAny further required reviews still apply.\nCancel stops without private approval.')
    return pages


def presentation_error(observed, proposal, prompts, phase, index):
    if (type(index) is not int or not 0 <= index < len(prompts)
            or type(proposal.get('revision')) is not int or proposal['revision'] <= 0
            or observed.get('phase') != phase or not json_exact(observed.get('presentation_index'), index)
            or not json_exact(observed.get('presentation_count'), len(prompts))
            or observed.get('proposal_id') != proposal['id']
            or not json_exact(observed.get('proposal_revision'), proposal['revision'])
            or observed.get('message') != prompts[index]
            or observed.get('title') != 'AgentBaton — Native owner approval'
            or observed.get('parent_matches') is not True
            or observed.get('visible') is not True
            or observed.get('complete_text_visible') is not True):
        return 'Sheet ready does not bind the complete exact visible presentation page'
    return None


def group_helpers():
    # Reuse the independently tested existing ownership/quiescence predicates;
    # the old runner and its readonly evidence contract are never modified.
    spec = importlib.util.spec_from_file_location('native_group_helpers', Path(__file__).with_name('native-run.py'))
    helpers = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helpers)
    return helpers


def exited_without_reaping(child):
    return group_helpers().owned_child_exited(child.pid)


def stop_owned(child, role):
    record = {'role': role, 'started': child is not None, 'ok': False, 'signals': [], 'errors': [], 'returncode': None}
    if child is None:
        return record
    record['pid'] = child.pid
    # Any previous poll/wait would permit PID/group reuse. Do not signal an
    # already-reaped leader, even if an unknown group reused its numeric ID.
    if child.returncode is not None:
        record['errors'].append('leader already reaped; no safe group signal')
        record['returncode'] = child.returncode
        return record
    helpers = group_helpers()
    exited, quiescent = None, False
    try:
        exited = helpers.owned_child_exited(child.pid)
        record['running_before_cleanup'] = not exited
        quiescent = exited and helpers.owned_group_is_quiescent(child.pid)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        record['errors'].append(f'observe:{type(error).__name__}: {error}')
    record['initial_group_quiescent'] = quiescent
    if not quiescent:
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            try:
                os.killpg(child.pid, sig)
                record['signals'].append({'signal': sig.name, 'result': 'signalled'})
            except ProcessLookupError:
                record['signals'].append({'signal': sig.name, 'result': 'absent'})
            except OSError as error:
                record['errors'].append(f'signal:{sig.name}:{type(error).__name__}: {error}')
            until = time.monotonic() + (1.5 if sig == signal.SIGTERM else 1)
            while time.monotonic() < until:
                try:
                    if helpers.owned_child_exited(child.pid) and helpers.owned_group_is_quiescent(child.pid):
                        quiescent = True
                        break
                except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                    record['errors'].append(f'quiescence:{type(error).__name__}: {error}')
                    break
                time.sleep(.02)
            if quiescent:
                break
    record['group_quiescent_before_reaping'] = quiescent
    try:
        record['returncode'] = child.wait(timeout=1)
        reaped = 0
        if sys.platform.startswith('linux'):
            while True:
                try:
                    pid, _ = os.waitpid(-child.pid, os.WNOHANG)
                except ChildProcessError:
                    break
                if not pid:
                    break
                reaped += 1
        record['adopted_owned_group_children_reaped'] = reaped
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        record['errors'].append(f'reap:{type(error).__name__}: {error}')
    normal = (exited is True and record['initial_group_quiescent'] and child.returncode == 0 if role == 'app'
              else exited is False and child.returncode in (0, -signal.SIGTERM))
    forced = any(item['signal'] == 'SIGKILL' for item in record['signals'])
    record['ok'] = normal and quiescent and not forced and not record['errors']
    return record


def enable_owned_subreaper():
    if sys.platform.startswith('linux'):
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), 'owned fixture subreaper failed')


def private_path_record(path):
    record = {'path': str(path), 'utf8_bytes': len(str(path).encode('utf-8'))}
    try:
        info = path.lstat()
        record.update(exists=True, uid=info.st_uid, mode=stat.S_IMODE(info.st_mode),
                      is_directory=stat.S_ISDIR(info.st_mode), is_symlink=stat.S_ISLNK(info.st_mode))
    except FileNotFoundError:
        record['exists'] = False
    return record


def daemon_output_record(path):
    if path is None:
        return None
    digest = hashlib.sha256()
    size = 0
    with path.open('rb') as reader:
        for chunk in iter(lambda: reader.read(65536), b''):
            size += len(chunk)
            digest.update(chunk)
    return {**private_path_record(path), 'bytes': size, 'sha256': digest.hexdigest()}


@contextmanager
def bootstrap(daemon_path, output, *, factory_planning=False):
    """Actual Core/generated --version fixture only; no GUI or Grant finalize.

    Linux controls use this same pre-OS path, never pretending they ran macOS.
    The caller must keep the yielded context alive while observing the database.
    """
    if type(factory_planning) is not bool:
        raise ValueError('Fixed internal Factory fixture selector must be boolean')
    # 103 UTF-8 bytes leaves the trailing NUL in Darwin's sockaddr_un.sun_path.
    # This prepares only our new fixture paths; the production guard is unchanged.
    socket_limit = 103
    output = Path(output).resolve(strict=True)
    output_info = output.stat()
    if not output.is_dir() or output_info.st_uid != os.geteuid() or stat.S_IMODE(output_info.st_mode) & 0o077:
        raise ValueError('Bootstrap evidence directory must be private and owned')
    if any(output.iterdir()):
        raise ValueError('Use a fresh empty bootstrap evidence directory')
    daemon, client, stdout_path, stderr_path = None, None, None, None
    generated, paths, failure, last_readiness_error = {}, {}, None, None
    phase = 'private_path_preparation'
    with ExitStack() as owned:
        try:
            temporary = owned.enter_context(tempfile.TemporaryDirectory(prefix='ab-own-'))
            root = Path(temporary).resolve(strict=True)
            initial_socket = root / 'runtime' / 'batond.sock'
            paths.update(requested_root=str(temporary), initial_canonical_root=str(root),
                         initial_socket_utf8_bytes=len(str(initial_socket).encode('utf-8')),
                         socket_limit_utf8_bytes=socket_limit, private_short_fallback=False)
            if paths['initial_socket_utf8_bytes'] > socket_limit:
                # Fixed system temp parent, canonicalized; never chmod a shared
                # parent or reuse an existing runtime. mkdtemp owns the new leaf.
                short_parent = Path('/tmp').resolve(strict=True)
                temporary = owned.enter_context(tempfile.TemporaryDirectory(prefix='ab-own-', dir=short_parent))
                root = Path(temporary).resolve(strict=True)
                paths['private_short_fallback'] = True
            root.chmod(0o700)
            runtime = root / 'runtime'
            runtime.mkdir(mode=0o700)
            socket_path = runtime / 'batond.sock'
            paths.update(root=private_path_record(root), runtime=private_path_record(runtime),
                         socket=private_path_record(socket_path), effective_uid=os.geteuid())
            save(output / 'bootstrap-paths.json', paths)
            if paths['socket']['utf8_bytes'] > socket_limit:
                raise ValueError('Private fixture socket exceeds portable address capacity')
            env = os.environ.copy()
            env['BATON_RUNTIME_DIR'] = str(runtime)
            # Own generated fixture, never private Provider state or a real engine.
            executable = root / 'probe'
            executable.write_text('#!/bin/sh\nif [ "$#" = 1 ] && [ "$1" = --version ]; then echo baton-owner-fixture-1; exit 0; fi\nexit 64\n')
            executable.chmod(0o700)
            project = root / 'source'
            project.mkdir()
            git_env = {k: v for k, v in env.items() if not k.startswith('GIT_')}
            git_env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null')
            phase = 'generated_project_preparation'
            for args in [['init', '-q'], ['-c', 'user.name=Native Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'local owner fixture']]:
                if factory_planning and args[0] != 'init':
                    (project / 'input.txt').write_bytes(b'fixed input\n')
                    subprocess.run(['git', '-C', str(project), '-c', 'core.hooksPath=/dev/null', 'add', 'input.txt'], env=git_env, check=True, timeout=10, capture_output=True)
                subprocess.run(['git', '-C', str(project), '-c', 'core.hooksPath=/dev/null', '-c', 'init.templateDir=', *args], env=git_env, check=True, timeout=10, capture_output=True)
            enable_owned_subreaper()
            streams = []
            for name in ['daemon-stdout.log', 'daemon-stderr.log']:
                path = output / name
                descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0), 0o600)
                if name == 'daemon-stdout.log':
                    stdout_path = path
                else:
                    stderr_path = path
                streams.append(owned.enter_context(os.fdopen(descriptor, 'wb', buffering=0)))
            phase = 'daemon_spawn'
            daemon = subprocess.Popen([str(daemon_path.resolve())], env=env, start_new_session=True, stdout=streams[0], stderr=streams[1])
            client = NativeCoreClient(runtime)
            phase = 'daemon_readiness'
            deadline = time.monotonic() + 10
            while True:
                if exited_without_reaping(daemon):
                    raise RuntimeError('Fixture daemon exited before readiness')
                try:
                    client.call('system.snapshot', {})
                    break
                except (OSError, RuntimeError) as error:
                    last_readiness_error = {'type': type(error).__name__, 'message': str(error)}
                    if time.monotonic() >= deadline:
                        raise
                    time.sleep(.05)
            phase = 'generated_connection_preparation'
            imported = client.call('projects.import', {'path': str(project), 'name': 'Owner fixture'}, 'owner-project')
            workspace = client.call('workspaces.create', {'project_id': imported['id'], 'expected_head': imported['identity']['kind']['head']}, 'owner-workspace', imported['revision'])
            connection_proposal = client.call('connections.propose', {'name': 'Generated version-only fixture', 'provider': 'custom_acp', 'executable': str(executable), 'arguments': []}, 'owner-connection')
            client.connection_proposal = connection_proposal['id']
            connection = client.call('approvals.finalize', {'proposal_id': connection_proposal['id']}, 'proposal:' + connection_proposal['id'], connection_proposal['revision'])
            phase = 'generated_version_probe'
            deadline = time.monotonic() + 10
            while True:
                connections = client.call('connections.list', {})
                actual = next(c for c in connections if c['id'] == connection['id'])
                if actual['probe_state'] != 'pending':
                    if actual['probe_state'] != 'ready':
                        raise ValueError('Generated version probe did not reach ready')
                    break
                if time.monotonic() >= deadline:
                    raise TimeoutError('Bounded generated version probe')
                time.sleep(.05)
            params = {'project_id': imported['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 1, 'max_concurrency': 1, 'max_duration_ms': 300000 if factory_planning else 1000}
            proposal = client.call('grants.propose', params, 'owner-os-grant')
            invoke = {'projectId': imported['id'], 'workspaceId': workspace['id'], 'connectionId': connection['id'], 'maxAttempts': 1, 'maxConcurrency': 1, 'maxDurationMs': 300000 if factory_planning else 1000, 'idempotencyKey': 'owner-os-grant'}
            generated = {'runtime': runtime, 'env': env, 'client': client, 'proposal': proposal, 'invoke': invoke}
            phase = 'yielded_before_native_owner'
            yield generated
        except BaseException as error:
            failure = {'type': type(error).__name__, 'message': str(error)}
            raise
        finally:
            try:
                save(output / 'core-bootstrap-transcript.json', client.transcript if client else [])
            finally:
                cleanup = stop_owned(daemon, 'daemon')
                generated['daemon_cleanup'] = cleanup
                save(output / 'daemon-cleanup.json', cleanup)
                if 'runtime' in paths:
                    paths['runtime_after_cleanup'] = private_path_record(Path(paths['runtime']['path']))
                    paths['socket_after_cleanup'] = private_path_record(Path(paths['socket']['path']))
                save(output / 'bootstrap-state.json', {
                    'status': 'completed' if failure is None and cleanup['ok'] else 'failed',
                    'phase': phase, 'paths': paths, 'original_failure': failure,
                    'last_readiness_error': last_readiness_error,
                    'daemon_exitcode': cleanup['returncode'], 'cleanup_ok': cleanup['ok'],
                    'stdout': daemon_output_record(stdout_path), 'stderr': daemon_output_record(stderr_path)})
                # Keep the original error when one is already propagating, but
                # never allow unexpected exit/forced cleanup to become success.
                if not cleanup['ok'] and failure is None:
                    raise RuntimeError('Owned daemon exited unexpectedly or required forced cleanup; no successful result')


def run(binary, daemon_path, output):
    if sys.platform != 'darwin':
        raise RuntimeError('NOT_RUN: real macOS GUI required; Linux/browser results do not qualify')
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    output = output.resolve()
    if any(output.iterdir()):
        raise ValueError('Use a fresh empty evidence directory; old evidence is never reused')
    with bootstrap(daemon_path, output) as generated:
        runtime, env, proposal = generated['runtime'], generated['env'], generated['proposal']
        app = None
        try:
            baseline = database(runtime, proposal['id'])
            error = pending_error(baseline, proposal, baseline['sequence'])
            if error:
                raise ValueError(error)
            message = expected_message(proposal, (Path(__file__).resolve().parents[1] / 'apps/desktop/src-tauri/src/approval.rs').read_text())
            invoke = generated['invoke']
            fixture = {'kind': 'native_grant_os_dialog_fixture', 'runtime': str(runtime), 'output': str(output), 'proposal': proposal, 'message': message, 'presentation': presentation_prompts(message), 'invoke': invoke, 'baseline_sequence': baseline['sequence']}
            save(output / 'fixture.json', fixture)
            save(output / 'before.json', baseline)
            app = subprocess.Popen([str(binary.resolve()), str(output / 'fixture.json')], env=env, start_new_session=True)
            deadline = time.monotonic() + 85
            permits = set()
            while not exited_without_reaping(app) and time.monotonic() < deadline:
                for phase in ['cancel', 'approve']:
                    for index, prompt in enumerate(fixture['presentation']):
                        key = f'{phase}-page-{index + 1:04}'
                        ready = output / f'{key}-sheet-ready.json'
                        if ready.exists() and key not in permits:
                            observed = json.loads(ready.read_text())
                            error = presentation_error(observed, proposal, fixture['presentation'], phase, index)
                            if error:
                                raise ValueError(error)
                            if index and f'{phase}-page-{index:04}' not in permits:
                                raise ValueError('Presentation page arrived out of order')
                            check = database(runtime, proposal['id'])
                            error = pending_error(check, proposal, baseline['sequence'])
                            if error:
                                raise ValueError(error)
                            if phase == 'approve':
                                cancel_reply = json.loads((output / 'cancel-reply.json').read_text())
                                if cancel_reply.get('error', {}).get('code') != 'PERMISSION_DENIED':
                                    raise ValueError('Prior real Cancel path not recorded')
                            save(output / f'{key}-before-click-database.json', check)
                            save(output / f'{key}-database-permit.json', {'phase': phase, 'presentation_index': index, 'presentation_count': len(fixture['presentation']), 'proposal_id': proposal['id'], 'proposal_revision': proposal['revision'], 'before_click_sequence': baseline['sequence']})
                            permits.add(key)
                time.sleep(.05)
            if not exited_without_reaping(app):
                raise TimeoutError('Real OS dialog process did not finish')
            expected_permits = {f'{phase}-page-{index + 1:04}' for phase in ['cancel', 'approve'] for index in range(len(fixture['presentation']))}
            if permits != expected_permits:
                raise RuntimeError('Real dialog process failed; inspect retained native report')
            after = database(runtime, proposal['id'])
            save(output / 'after.json', after)
            error = approved_error(after, proposal, baseline['sequence'], json.loads((output / 'approve-reply.json').read_text()))
            if error:
                raise ValueError(error)
            images = {}
            for key in sorted(expected_permits):
                raw = (output / f'{key}-sheet.png').read_bytes()
                if len(raw) < 64 or raw[:8] != b'\x89PNG\r\n\x1a\n' or raw[12:16] != b'IHDR':
                    raise ValueError('Missing actual sheet PNG')
                width, height = int.from_bytes(raw[16:20], 'big'), int.from_bytes(raw[20:24], 'big')
                if min(width, height) < 20:
                    raise ValueError('Actual sheet image is empty/tiny')
                images[key] = {'sha256': hashlib.sha256(raw).hexdigest(), 'width': width, 'height': height, 'visual_review': 'pending_human_or_independent_image_inspection'}
            result = {'status': 'automated_os_and_database_checks_passed_visual_review_pending', 'dimension': 'actual_macos_grant_cancel_then_same_key_real_ok', 'proposal_id': proposal['id'], 'proposal_revision': proposal['revision'], 'grant_id': after['grants'][0]['id'], 'before_sequence': baseline['sequence'], 'after_sequence': after['sequence'], 'images': images, 'presentation_pages_per_phase': len(fixture['presentation']), 'original_message_sha256': hashlib.sha256(message.encode()).hexdigest(), 'model_calls': 'not_run', 'graph_os_owner': 'not_run', 'human_owner_review': 'not_claimed', 'os': sys.platform, 'architecture': platform.machine(), 'harness_binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'daemon_binary_sha256': hashlib.sha256(daemon_path.read_bytes()).hexdigest()}
        finally:
            cleanup = stop_owned(app, 'app')
            save(output / 'app-cleanup.json', cleanup)
            if not cleanup['ok']:
                raise RuntimeError('Native app did not exit cleanly; no successful result')
    result['process_cleanup'] = {'app': cleanup, 'daemon': generated['daemon_cleanup']}
    save(output / 'result.json', result)


FACTORY_KIND = 'native_factory_planning_os_dialog_fixture'
FACTORY_KEY = 'owner-os-factory-planning'
FACTORY_TITLE = 'AgentBaton — Independent Factory approval'


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8')


def digest(value):
    return hashlib.sha256(compact(value)).hexdigest()


def factory_messages(proposal, source):
    """Full-snapshot independent formatter mirror; every byte is reconstructed.

    Literal captions are extracted from the frozen shipping source, not supplied
    by the renderer. Fail on a formatter refactor; never accept a digest-only UI.
    """
    body = source.split('pub fn confirm_factory_and_finalize(', 1)[1].split('\n#[cfg(test)]', 1)[0]
    action = re.search(r'let action = if .*?\{\s*("(?:[^"\\]|\\.)*")', body, re.S)
    page = re.search(r'let message\s*=\s*format!\(\s*("(?:[^"\\]|\\.)*")', body)
    final = re.search(r'if !confirm_native\(&format!\(\s*("(?:[^"\\]|\\.)*")', body)
    if not action or not page or not final:
        raise ValueError('Audit changed production Factory formatter before OS clicks')
    action, page, final = [json.loads(m.group(1)) for m in (action, page, final)]
    raw = compact(proposal['snapshot'])
    if not raw or len(raw) > 1048576:
        raise ValueError('Complete Factory snapshot exceeds original 1MiB bound')
    chunks, offset = [], 0
    while offset < len(raw):
        end = min(offset + 12288, len(raw))
        while True:
            try:
                text = raw[offset:end].decode('utf-8')
                break
            except UnicodeDecodeError:
                end -= 1
        if end == offset:
            raise ValueError('No complete UTF8 scalar in Factory page')
        chunks.append(text)
        offset = end
    if ''.join(chunks).encode('utf-8') != raw:
        raise ValueError('Factory snapshot was not completely reconstructed')
    messages, offset = [], 0
    for index, chunk in enumerate(chunks):
        end = offset + len(chunk.encode())
        replacements = {'action': action, 'id': proposal['id'], 'revision': proposal['revision'],
                        'offset': offset, 'end': end, 'total': len(raw), 'page': chunk}
        template = page
        for positional in (index + 1, len(chunks), offset, end, len(raw)):
            template = template.replace('{}', str(positional), 1)
        # Substitute from a single regex pass: original JSON may contain caption
        # placeholders verbatim and must not be interpreted a second time.
        messages.append(re.sub(r'\{(action|id|revision|offset|end|total|page)\}', lambda m: str(replacements[m[1]]), template))
        offset = end
    template = final.replace('{}', str(len(chunks)), 1)
    values = {'action': action, 'id': proposal['id'], 'revision': proposal['revision'], 'total': len(raw)}
    messages.append(re.sub(r'\{(action|id|revision|total)\}', lambda m: str(values[m[1]]), template))
    if any(len(message.encode()) > 16384 for message in messages):
        raise ValueError('Factory outer message exceeds original16KiB bound')
    return messages


def factory_presentation(proposal, source):
    messages = factory_messages(proposal, source)
    pages, contexts = [], []
    for outer, message in enumerate(messages):
        inner = presentation_prompts(message)
        for index, page in enumerate(inner):
            pages.append(page)
            contexts.append({'outer_index': outer, 'outer_count': len(messages),
                             'inner_index': index, 'inner_count': len(inner),
                             'outer_final': outer + 1 == len(messages), 'inner_final': index + 1 == len(inner),
                             'message_sha256': hashlib.sha256(message.encode()).hexdigest()})
    if not 2 <= len(pages) <= 1024 or any(len(p.encode()) > 2048 for p in pages):
        raise ValueError('Factory display exceeds original bounded page envelope')
    return messages, pages, contexts


def factory_presentation_error(observed, fixture, pages, contexts, index):
    # Retain the original strict text/layout predicate, with only the fixed
    # Factory title and the additional immutable two-layer nonce/context binding.
    title = observed.get('title')
    normalized = dict(observed, title='AgentBaton — Native owner approval')
    return (presentation_error(normalized, fixture['proposal'], pages, fixture['phase'], index)
            or (None if title == FACTORY_TITLE
                and observed.get('review_nonce') == fixture['review_nonce']
                and json_exact(observed.get('factory_page'), contexts[index]) else 'Factory outer/inner/nonce/title mismatch'))


class FactoryBootstrapClient:
    """Generated-fixture Owner setup only. No target Factory finalize method.

    This is not product authorization or a public endpoint. The pre-existing
    Grant-mode client remains restricted to connection bootstrap only.
    """
    def __init__(self, client, grant_proposal):
        self.client = client
        self.grant_proposal = grant_proposal
        self.finalized_grant = False

    def request(self, method, params, key, revision=None):
        if method == 'approvals.finalize':
            if (self.finalized_grant or self.grant_proposal.get('kind') != 'grant'
                    or not json_exact(params, {'proposal_id': self.grant_proposal['id']})
                    or not json_exact(revision, self.grant_proposal['revision'])
                    or key != 'proposal:' + self.grant_proposal['id']):
                raise ValueError('Only the single generated enclosing Grant bootstrap; NEVER Factory finalize')
        elif method not in {'tasks.create', 'factory_scopes.propose'}:
            raise ValueError('Not a fixed Factory fixture preparation method')
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()),
                 'token': (self.client.runtime / 'session.token').read_text(),
                 'method': method, 'params': params, 'idempotency_key': key}
        if revision is not None:
            frame['expected_revision'] = revision
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(20)
            stream.connect(str(self.client.runtime / 'batond.sock'))
            stream.sendall(json.dumps(frame).encode() + b'\n')
            raw = bytearray()
            while not raw.endswith(b'\n'):
                chunk = stream.recv(65536)
                if not chunk or len(raw) + len(chunk) > 4 * 1024 * 1024:
                    raise RuntimeError('Incomplete or oversized generated Core reply')
                raw.extend(chunk)
        reply = json.loads(raw)
        self.client.transcript.append({'method': method, 'params': params, 'idempotency_key': key,
                                       'expected_revision': revision, 'reply': reply,
                                       'dimension': 'generated enclosing resources only, not OS approval'})
        if 'error' in reply:
            raise RuntimeError(f'{method}: {reply["error"]}')
        if method == 'approvals.finalize':
            self.finalized_grant = True
        return reply['result']


def prepare_factory(generated):
    p = generated['proposal']
    initial = database(generated['runtime'], p['id'])
    if error := pending_error(initial, p, initial['sequence']):
        raise ValueError(error)
    # Exact live14/30zero BEFORE any enclosing Grant finalization.
    client = FactoryBootstrapClient(generated['client'], p)
    grant = client.request('approvals.finalize', {'proposal_id': p['id']}, 'proposal:' + p['id'], p['revision'])
    intent = p['snapshot']['intent']
    if grant['scope']['max_children'] != 0 or grant['scope']['max_depth'] != 0:
        raise ValueError('Generated enclosing Grant cannot delegate')
    task = client.request('tasks.create', {'project_id': intent['project_id'], 'goal': 'Review bounded 子计划 planning only'}, 'owner-factory-task')
    budget = {'attempts': 1, 'parallel': 1, 'runtime_ms': 1000, 'output_bytes': 1024,
              'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
    output = {'artifact_id': 'report', 'relative_path': 'report.txt', 'max_bytes': 1024, 'expected_sha256': None}
    selection = {'root_task_id': task['id'], 'expected_task_revision': task['revision'],
        'grant_id': grant['id'], 'expected_grant_revision': grant['revision'],
        'connection_ids': [intent['connection_id']], 'workspace_ids': [intent['workspace_id']],
        'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'max_bytes': 1024,
                    'expected_sha256': hashlib.sha256(b'fixed input\n').hexdigest()}],
        'outputs': [output], 'recipients': [], 'max_depth': 1, 'max_children': 1, 'max_total_nodes': 2,
        'root_budget': budget, 'project_budget': dict(budget), 'ttl_ms': 300000,
        'capabilities': {'delegate_task': True, 'resume_parent_as_new_attempt': True,
                         'child_planning': True, 'child_goal_execution': False, 'create_workspace': False},
        'child_planning_policy': {'max_nodes': 1, 'max_revisions': 2, 'max_plan_bytes': 1024,
            'outputs': [{'artifact': output, 'validators': []}], 'writable_prefixes': ['report.txt'],
            'owner_acceptance_criteria': 'Review complete report / 核对完整报告; stored plan is not acceptance.'}}
    proposal = client.request('factory_scopes.propose', selection, FACTORY_KEY)
    if proposal.get('kind') != 'factory' or not json_exact(proposal['snapshot']['selection'], selection):
        raise ValueError('Real Core did not return the exact Factory proposal')
    return proposal, {'selection': selection, 'idempotencyKey': FACTORY_KEY}


def factory_database(runtime, proposal_id):
    # One readonly SQLite transaction binds sequence, receipt and ALL raw docs.
    return database(runtime, proposal_id, factory_planning=True)


def factory_pending_error(value, fixture, before):
    proposal = fixture['proposal']
    if (type(before.get('sequence')) is not int or type(proposal.get('revision')) is not int
            or proposal['revision'] <= 0 or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 14)
            or not json_exact(value.get('sequence'), before['sequence'])
            or proposal.get('kind') != 'factory' or proposal.get('consumed') is not False
            or not json_exact(value.get('proposal'), proposal) or value.get('proposal_result') is not None
            or not json_exact(value.get('grant_documents'), before['grant_documents'])
            or not json_exact(value.get('other_proposals'), before['other_proposals'])
            or not json_exact(value.get('task_documents'), before['task_documents'])
            or not json_exact(value.get('events'), before['events'])
            or not json_exact(value.get('grants'), before['grants'])
            or value.get('scopes') != [] or not json_exact(value.get('dispatch_intents'), 0)
            or not isinstance(value.get('counts'), dict) or set(value['counts']) != set(ZERO_TABLES)
            or any(type(n) is not int or n != 0 for n in value['counts'].values())):
        return 'Factory pending proposal/sequence/original Grant/raw documents/zero execution changed'
    if (len(before['grants']) != 1 or not json_exact(before['grants'][0]['scope']['max_children'], 0)
            or not json_exact(before['grants'][0]['scope']['max_depth'], 0)
            or not json_exact(proposal['snapshot']['bindings']['approved_grant'], before['grants'][0])):
        return 'Factory proposal not bound to the original nondelegating Grant'
    return None


def factory_approved_error(value, fixture, before, response):
    proposal = fixture['proposal']
    current = dict(proposal, consumed=True, revision=proposal['revision'] + 1)
    scope = response.get('result')
    counts = {name: int(name == 'factory_scopes') for name in ZERO_TABLES}
    if (type(before.get('sequence')) is not int or type(proposal.get('revision')) is not int
            or proposal['revision'] <= 0 or not isinstance(scope, dict) or response.get('error') is not None
            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 14)
            or not json_exact(value.get('sequence'), before['sequence'] + 1)
            or not json_exact(value.get('proposal'), current)
            or not json_exact(value.get('proposal_result'), scope)
            or not json_exact(value.get('scopes'), [{'document': scope, 'owner_sequence': before['sequence'] + 1}])
            or not json_exact(value.get('counts'), counts) or not json_exact(value.get('dispatch_intents'), 0)
            or any(not json_exact(value.get(key), before[key]) for key in ['grant_documents', 'grants', 'other_proposals', 'task_documents'])):
        return 'Factory exact persisted scope/result/CAS/sequence/unchanged original documents missing'
    selection, bindings = proposal['snapshot']['selection'], proposal['snapshot']['bindings']
    if (scope.get('id') != proposal['id'] or scope.get('approval_id') != proposal['id']
            or scope.get('root_task_id') != selection['root_task_id']
            or scope.get('root_grant_id') != selection['grant_id']
            or not json_exact(scope.get('root_grant_revision'), selection['expected_grant_revision'])
            or scope.get('project_id') != bindings['root_task']['project_id']
            or not json_exact(scope.get('revision'), 1) or scope.get('revoked') is not False
            or not json_exact(scope.get('selection'), selection) or not json_exact(scope.get('bindings'), bindings)
            or scope.get('contract_digest') != proposal['snapshot']['contract_digest']
            or selection['capabilities']['child_planning'] is not True
            or selection['capabilities']['child_goal_execution'] is not False
            or selection['capabilities']['create_workspace'] is not False
            or not json_exact(bindings['child_planning']['policy'], selection['child_planning_policy'])
            or bindings['child_planning'].get('goal_execution_authorized') is not False
            or bindings['child_planning'].get('new_workspace_authorized') is not False
            or bindings['child_planning'].get('authority_kind') != 'factory_child_planning_only'):
        return 'Factory approval response expanded/mismatched the complete approved planning contract'
    event = {'aggregate_id': proposal['id'], 'kind': 'approval.confirmed', 'source': 'local_owner_confirmation',
             'revision': proposal['revision'] + 1, 'sequence': before['sequence'] + 1,
             'payload': {'kind': 'factory', 'factory_scope_id': proposal['id'], 'contract_digest': scope['contract_digest'],
                         'root_task_id': scope['root_task_id'], 'root_grant_id': scope['root_grant_id'],
                         'original_grant_revision_unchanged': scope['root_grant_revision'], 'spawned': False,
                         'authority': 'independent_task_delegation_only'}}
    # Events have additional server metadata (timestamp/id), but every original
    # event remains byte-value exact and there is exactly ONE new matching event.
    events = value.get('events', [])
    if len(events) != len(before['events']) + 1 or not json_exact(events[:-1], before['events']) or any(not json_exact(events[-1].get(k), v) for k, v in event.items()):
        return 'Factory unique exact Owner event/old history mismatch'
    return None


def factory_fixture(proposal, invoke, runtime, output, before, source, phase, cancel_at=None):
    messages, pages, contexts = factory_presentation(proposal, source)
    cancel_at = len(pages) - 1 if cancel_at is None else cancel_at
    if phase not in ('cancel', 'approve') or type(cancel_at) is not int or not 0 <= cancel_at < len(pages):
        raise ValueError('Fixed Factory phase/cancel index required')
    fixture = {'kind': FACTORY_KIND, 'runtime': str(runtime), 'output': str(output), 'proposal': proposal,
               'invoke': invoke, 'baseline_sequence': before['sequence'], 'phase': phase, 'cancel_at': cancel_at,
               'review_nonce': uuid.uuid4().hex, 'snapshot_sha256': digest(proposal['snapshot']),
               'outer_message_sha256': [hashlib.sha256(m.encode()).hexdigest() for m in messages],
               'presentation_sha256': digest(pages), 'presentation_count': len(pages)}
    # Store the whole proposal ONCE, not duplicated projected/digest-only policy.
    # Runtime expected text is rebuilt independently from this complete object.
    if len(json.dumps(fixture, ensure_ascii=False, indent=2).encode()) > 65536:
        raise ValueError('Complete generated Factory fixture exceeds unchanged64KiB bound')
    return fixture, pages, contexts


def run_factory(binary, daemon_path, companion_path, output, cancel_at=None):
    if sys.platform != 'darwin':
        raise RuntimeError('NOT_RUN: real macOS GUI required; Linux/browser results do not qualify')
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    output = output.resolve()
    if any(output.iterdir()):
        raise ValueError('Use a fresh empty Factory evidence directory')
    installed = output / 'private-tools'
    installed.mkdir(mode=0o700)
    tool_record = {}
    for name, raw in [('batond', daemon_path), ('baton', companion_path)]:
        if raw.is_symlink():
            raise ValueError('Normal tool input must not be a symlink')
        raw = raw.resolve(strict=True)
        if not raw.is_file():
            raise ValueError('Need actual matching normal14 tools')
        destination = installed / name
        data = raw.read_bytes()
        destination.write_bytes(data)
        destination.chmod(0o500)
        tool_record[name] = {'source': str(raw), 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
        if (hashlib.sha256(destination.read_bytes()).hexdigest() != tool_record[name]['sha256']
                or hashlib.sha256(raw.read_bytes()).hexdigest() != tool_record[name]['sha256']):
            raise ValueError('Private tool copy mismatch')
    save(output / 'tool-sources.json', tool_record)
    bootstrap_output = output / 'bootstrap'
    bootstrap_output.mkdir(mode=0o700)
    with bootstrap(installed / 'batond', bootstrap_output, factory_planning=True) as generated:
        runtime = generated['runtime']
        proposal, invoke = prepare_factory(generated)
        before = factory_database(runtime, proposal['id'])
        source = (Path(__file__).resolve().parents[1] / 'apps/desktop/src-tauri/src/approval.rs').read_text()
        phase_results = []
        for phase in ('cancel', 'approve'):
            phase_out = output / phase
            phase_out.mkdir(mode=0o700)
            fixture, pages, contexts = factory_fixture(proposal, invoke, runtime, phase_out, before, source, phase, cancel_at)
            if error := factory_pending_error(factory_database(runtime, proposal['id']), fixture, before):
                raise ValueError(error)
            save(phase_out / 'fixture.json', fixture)
            save(phase_out / 'before.json', before)
            save(phase_out / 'expected-display.json', {'pages': pages, 'contexts': contexts})
            app = None
            permits = set()
            limit = fixture['cancel_at'] + 1 if phase == 'cancel' else len(pages)
            phase_failure = None
            try:
                app = subprocess.Popen([str(binary.resolve()), str(phase_out / 'fixture.json')], env=generated['env'], start_new_session=True)
                deadline = time.monotonic() + 85
                while not exited_without_reaping(app) and time.monotonic() < deadline:
                    for index in range(limit):
                        key = f'{phase}-page-{index + 1:04}'
                        ready = phase_out / f'{key}-sheet-ready.json'
                        if ready.exists() and index not in permits:
                            observed = json.loads(ready.read_text())
                            error = factory_presentation_error(observed, fixture, pages, contexts, index)
                            if error:
                                raise ValueError(error)
                            if index and index - 1 not in permits:
                                raise ValueError('Factory nested page arrived out of order')
                            check = factory_database(runtime, proposal['id'])
                            if error := factory_pending_error(check, fixture, before):
                                raise ValueError(error)
                            save(phase_out / f'{key}-before-click-database.json', check)
                            save(phase_out / f'{key}-database-permit.json', {
                                'phase': phase, 'presentation_index': index, 'presentation_count': len(pages),
                                'proposal_id': proposal['id'], 'proposal_revision': proposal['revision'],
                                'before_click_sequence': before['sequence'], 'review_nonce': fixture['review_nonce'],
                                'factory_page': contexts[index]})
                            permits.add(index)
                    time.sleep(.05)
                if not exited_without_reaping(app) or permits != set(range(limit)):
                    raise RuntimeError('Factory real dialog deadline/missing exact-page permits')
                reply = json.loads((phase_out / (phase + '-reply.json')).read_text())
                after = factory_database(runtime, proposal['id'])
                save(phase_out / 'after.json', after)
                error = (factory_pending_error(after, fixture, before) if phase == 'cancel'
                         else factory_approved_error(after, fixture, before, reply))
                if error:
                    raise ValueError(error)
                if phase == 'cancel' and (reply.get('error', {}).get('code') != 'PERMISSION_DENIED'
                        or reply.get('error', {}).get('side_effect_state') != 'not_started' or reply.get('result') is not None):
                    raise ValueError('Factory cancel did not remain zero finalize/not_started')
                images = {}
                for index in range(limit):
                    name = f'{phase}-page-{index + 1:04}-sheet.png'
                    raw = (phase_out / name).read_bytes()
                    if len(raw) < 64 or raw[:8] != b'\x89PNG\r\n\x1a\n' or raw[12:16] != b'IHDR' or min(int.from_bytes(raw[16:20], 'big'), int.from_bytes(raw[20:24], 'big')) < 20:
                        raise ValueError('Missing actual complete Factory sheet pixels')
                    images[name] = {'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw), 'visual_review': 'pending'}
                phase_results.append({'phase': phase, 'pages_observed': limit, 'total_pages': len(pages), 'images': images})
            except BaseException as error:
                phase_failure = {'type': type(error).__name__, 'message': str(error)}
                save(phase_out / 'original-failure.json', phase_failure)
                try:
                    save(phase_out / 'failure-database.json', factory_database(runtime, proposal['id']))
                except Exception as secondary:
                    save(phase_out / 'failure-database-error.json', {'type': type(secondary).__name__, 'message': str(secondary)})
                raise
            finally:
                cleanup = stop_owned(app, 'app')
                save(phase_out / 'app-cleanup.json', cleanup)
                if not cleanup['ok'] and phase_failure is None:
                    raise RuntimeError('Factory OS app did not exit cleanly')
    save(output / 'result.json', {'status': 'actual_factory_planning_cancel_and_same_key_approval_visual_review_pending',
         'proposal_id': proposal['id'], 'phases': phase_results, 'daemon_cleanup': generated['daemon_cleanup'],
         'dimension': 'actual_macos_factory_planning_only_no_execution', 'real_provider': 'not_run',
         'human_owner_approval': 'not_claimed', 'os': sys.platform, 'architecture': platform.machine()})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--daemon', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--kind', choices=['grant', 'factory_planning'], default='grant')
    parser.add_argument('--companion', type=Path)
    parser.add_argument('--factory-cancel-at', type=int)
    args = parser.parse_args()
    if args.kind == 'factory_planning' and args.companion is None:
        parser.error('factory_planning requires the matching actual baton companion')
    if args.kind == 'grant' and (args.companion is not None or args.factory_cancel_at is not None):
        parser.error('Factory-only options are not Grant fixture authority')
    try:
        if args.kind == 'grant':
            run(args.binary, args.daemon, args.output)
        else:
            run_factory(args.binary, args.daemon, args.companion, args.output, args.factory_cancel_at)
    except Exception as error:
        args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
        save(args.output / 'runner-failure.json', {'status': 'failed_or_not_run', 'error': str(error), 'os': sys.platform})
        raise
