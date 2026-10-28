#!/usr/bin/env python3
"""Own-process macOS Grant dialog fixture. No production credentials/OS permissions.

The only approvals.finalize host bootstrap is for this generated local executable's
--version connection. The target Grant is finalized ONLY by production Tauri UI.
"""
import argparse
from contextlib import contextmanager, ExitStack
import hashlib
import importlib.util
import ctypes
import json
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


# Exact schema12 tables: never substitute old test DDL or silently skip absent
# tables. Every Goal store/runtime/graph table is empty in this Grant-only probe.
GOAL_TABLES = ('goal_grant_revisions', 'goal_scopes', 'goal_plan_heads',
               'goal_plan_revisions', 'goal_plan_proposals', 'goal_ledgers',
               'goal_node_evidence', 'goal_external_action_journal', 'goal_runs',
               'goal_run_nodes', 'goal_run_reservations', 'goal_graph_capabilities',
               'goal_graph_jobs', 'goal_graph_candidates', 'goal_graph_validations',
               'goal_graph_owner_decisions', 'goal_graph_acceptances')
ZERO_TABLES = ('attempts', 'sessions', *GOAL_TABLES)


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


def database(runtime, proposal_id):
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
        conn.rollback()
    return {'read_only': True, 'schema_version': schema, 'sequence': sequence,
            'proposal': proposal, 'proposal_result': result, 'grants': grants, 'events': events, 'counts': counts}


def pending_error(value, proposal, sequence):
    if (value.get('read_only') is not True or value.get('schema_version') != 12
            or type(value.get('sequence')) is not int or value['sequence'] != sequence
            or value.get('proposal') != proposal or proposal.get('kind') != 'grant'
            or proposal.get('consumed') is not False or value.get('proposal_result') is not None
            or value.get('grants') != [] or not isinstance(value.get('counts'), dict)
            or set(value['counts']) != set(ZERO_TABLES)
            or any(type(n) is not int or n != 0 for n in value['counts'].values())
            or any(e.get('aggregate_id') == proposal['id'] and e.get('kind') == 'approval.confirmed' for e in value.get('events', []))):
        return 'Pending Grant authority/sequence/schema changed or absent'
    return None


def approved_error(value, proposal, baseline, response):
    grants = value.get('grants')
    current = value.get('proposal', {})
    if (value.get('read_only') is not True or value.get('schema_version') != 12
            or not isinstance(grants, list) or len(grants) != 1
            or current.get('id') != proposal['id'] or current.get('snapshot') != proposal['snapshot']
            or current.get('kind') != 'grant' or current.get('consumed') is not True
            or type(current.get('revision')) is not int or current['revision'] != proposal['revision'] + 1
            or grants[0].get('approval_id') != proposal['id']
            or value.get('proposal_result') != grants[0] or response.get('result') != grants[0]
            or response.get('error') is not None or type(value.get('sequence')) is not int or value['sequence'] != baseline + 2
            or not isinstance(value.get('counts'), dict)
            or set(value['counts']) != set(ZERO_TABLES)
            or any(type(n) is not int or n != 0 for n in value['counts'].values())):
        return 'Exact production Grant commit/CAS/result/zero execution evidence missing'
    grant = grants[0]
    intent = proposal['snapshot']['intent']
    if (grant.get('project_id') != intent['project_id'] or grant.get('parent_id') is not None
            or grant.get('root_grant_id') != grant.get('id') or grant.get('depth') != 0 or grant.get('revision') != 1
            or grant.get('revoked') is not False or grant.get('scope', {}).get('workspace_ids') != [intent['workspace_id']]
            or grant['scope'].get('connection_ids') != [intent['connection_id']]
            or any(grant['scope'].get(k) != intent[k] for k in ['max_attempts', 'max_concurrency', 'max_duration_ms'])
            or grant.get('workspace_bindings', {}).get(intent['workspace_id']) != proposal['snapshot']['refs']['workspace_identity']
            or grant.get('connection_bindings', {}).get(intent['connection_id']) != proposal['snapshot']['refs']['connection']):
        return 'Committed root Grant does not match the exact bounded intent and resolved bindings'
    grant_events = [e for e in value.get('events', []) if e.get('aggregate_id') == grant['id'] and e.get('kind') == 'execution_grant.approved']
    if len(grant_events) != 1 or grant_events[0].get('source') != 'user_action' or grant_events[0].get('revision') != grant['revision'] or type(grant_events[0].get('sequence')) is not int or grant_events[0]['sequence'] != baseline + 1:
        return 'Exact persisted execution_grant.approved event missing'
    confirmations = [event for event in value.get('events', []) if event.get('aggregate_id') == proposal['id'] and event.get('kind') == 'approval.confirmed']
    if (len(confirmations) != 1 or confirmations[0].get('source') != 'local_owner_confirmation'
            or confirmations[0].get('revision') != current['revision']
            or type(confirmations[0].get('sequence')) is not int or confirmations[0]['sequence'] != baseline + 2
            or confirmations[0].get('payload') != {'kind': 'grant', 'proposal_id': proposal['id'], 'consumed': True}):
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
        escaped = (f'\\u{code:04x}' if code == 127 or 128 <= code <= 159 or 0x200b <= code <= 0x200f
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
    if (observed.get('phase') != phase or observed.get('presentation_index') != index
            or observed.get('presentation_count') != len(prompts)
            or observed.get('proposal_id') != proposal['id']
            or observed.get('proposal_revision') != proposal['revision']
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
def bootstrap(daemon_path, output):
    """Actual Core/generated --version fixture only; no GUI or Grant finalize.

    Linux controls use this same pre-OS path, never pretending they ran macOS.
    The caller must keep the yielded context alive while observing the database.
    """
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
            params = {'project_id': imported['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 1, 'max_concurrency': 1, 'max_duration_ms': 1000}
            proposal = client.call('grants.propose', params, 'owner-os-grant')
            invoke = {'projectId': imported['id'], 'workspaceId': workspace['id'], 'connectionId': connection['id'], 'maxAttempts': 1, 'maxConcurrency': 1, 'maxDurationMs': 1000, 'idempotencyKey': 'owner-os-grant'}
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


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--daemon', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.binary, args.daemon, args.output)
    except Exception as error:
        args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
        save(args.output / 'runner-failure.json', {'status': 'failed_or_not_run', 'error': str(error), 'os': sys.platform})
        raise
