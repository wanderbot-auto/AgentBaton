#!/usr/bin/env python3
"""Built Factory UI -> fixed test HTTP -> actual authenticated Core13 Unix IPC.

Owner review/finalization and resource creation exist only in the private host
fixture. Generated ACP/MCP is an account-free test program, never a real model.
Default invocation records pending dependencies and exits2 without a process.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from functools import partial
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import shutil
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import uuid

SUPPORT_PATH = Path(__file__).with_name('goal-graph-real-ipc.py')
_spec = importlib.util.spec_from_file_location('_factory_existing_ipc_support', SUPPORT_PATH)
support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = support
_spec.loader.exec_module(support)
FixedCall = support.FixedCall
BoundaryRefusal = support.BoundaryRefusal
require = support.require
safe_integer = support.safe_integer
unique_object = support.unique_object
MAX_HTTP_BYTES, MAX_IPC_BYTES, MAX_RESPONSE_BYTES = 270_336, 262_144, 4_194_304
MAX_FACTORY_REVIEW_BYTES, MAX_LEGACY_REVIEW_BYTES = 1_048_576, 16_384
MUTATIONS = frozenset({'factory_runs.start', 'factory_runs.cancel'})
READS = frozenset({'system.snapshot', 'projects.list', 'connections.list', 'sessions.list',
    'workspaces.list', 'grants.list', 'events.subscribe', 'goal_scopes.list',
    'factory_scopes.get', 'factory_scopes.list', 'factory_runs.get'})
PENDING = ('Root immutable matching schema13 batond and sibling baton with manifest hashes',
    'Root actual composed built Factory frontend and asset hashes',
    'Core-author confirmation of actual Factory runtime/ledger and private MCP contract')


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def install_private_tools(raw_daemon, raw_companion, installed, record):
    """Copy both pinned files; strip ONLY the private CLI, never frozen tools."""
    installed.mkdir(mode=0o700)
    assert stat.S_IMODE(installed.stat().st_mode) == 0o700
    record.update(directory=str(installed), operation='copy both; strip -S private baton only', tools={})
    for name, source in [('batond', raw_daemon), ('baton', raw_companion)]:
        before = sha(source.read_bytes()); destination = installed / name
        with source.open('rb') as reader, destination.open('xb') as writer:
            shutil.copyfileobj(reader, writer, 1024 * 1024)
        destination.chmod(0o700)
        assert not destination.samefile(source) and destination.stat().st_nlink == 1
        assert sha(destination.read_bytes()) == before == sha(source.read_bytes())
        record['tools'][name] = {'original_path': str(source), 'original_sha256': before,
            'original_bytes': source.stat().st_size, 'installed_path': str(destination),
            'copy_sha256': before}
    strip = Path('/usr/bin/strip').resolve(strict=True)
    argv = [str(strip), '-S', str(installed / 'baton')]
    record['strip_argv'] = argv
    result = subprocess.run(argv, capture_output=True, timeout=30)
    record['strip'] = {'exitcode': result.returncode, 'stdout': result.stdout.decode(errors='replace'),
        'stderr': result.stderr.decode(errors='replace'), 'tool_sha256': sha(strip.read_bytes())}
    assert len(result.stdout) + len(result.stderr) <= 16384 and result.returncode == 0
    for name, source in [('batond', raw_daemon), ('baton', raw_companion)]:
        destination = installed / name
        info = record['tools'][name]
        info.update(installed_sha256=sha(destination.read_bytes()), installed_bytes=destination.stat().st_size,
            original_after_sha256=sha(source.read_bytes()))
        assert info['original_after_sha256'] == info['original_sha256']
        if name == 'batond':
            assert info['installed_sha256'] == info['original_sha256']
    return installed / 'batond', installed / 'baton'


def bounded_file(path, limit, expected_parent=None):
    parent = path.parent
    if expected_parent is not None:
        assert parent == expected_parent
    parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent_fd)
        try:
            before = os.fstat(fd)
            assert stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_uid == os.getuid()
            assert before.st_size <= limit
            chunks = []; remaining = limit + 1
            while remaining:
                chunk = os.read(fd, min(65536, remaining))
                if not chunk:
                    break
                chunks.append(chunk); remaining -= len(chunk)
            raw = b''.join(chunks); after = os.fstat(fd)
            fields = ('st_dev', 'st_ino', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
            assert len(raw) == before.st_size <= limit
            assert all(getattr(before, key) == getattr(after, key) for key in fields)
            return raw, {'device': before.st_dev, 'inode': before.st_ino, 'bytes': len(raw),
                'mode': stat.S_IMODE(before.st_mode), 'sha256': sha(raw)}
        finally:
            os.close(fd)
    finally:
        os.close(parent_fd)


def write_private_json(path, value):
    raw = (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as writer:
        writer.write(raw)
    return sha(raw)


def diagnostic_snapshot(runtime, workspace, output, owner_token):
    """Bounded complete readonly transaction, never claims a retained DB copy."""
    result = {'dimension': 'generated-fixture-only readonly query snapshot, NOT retained/replayable SQLite bytes',
        'complete': True, 'tables': {}, 'generated_stderr': [], 'errors': []}
    secrets = {owner_token} if owner_token else set()
    credentials = runtime / 'factory-scoped-handles'
    if credentials.is_dir():
        paths = []
        for path in credentials.iterdir():
            paths.append(path)
            if len(paths) > 256:
                result['complete'] = False
                result['errors'].append({'stage': 'credential', 'type': 'EntryLimit', 'message': 'more than256 generated credential entries'})
                break
        for path in paths[:256]:
            try:
                raw, _ = bounded_file(path, 8192, credentials); value = json.loads(raw)
                assert value['socket_path'] == str(runtime / 'batond.sock')
                if isinstance(value.get('scoped_token'), str):
                    secrets.add(value['scoped_token'])
            except FileNotFoundError:
                # A revoked/deleted credential race does not remove journal data.
                continue
            except Exception as failure:
                result['complete'] = False
                result['errors'].append({'stage': 'credential', 'name': path.name, 'type': type(failure).__name__})
    def find_secrets(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key in {'token', 'scoped_token'} and isinstance(item, str): secrets.add(item)
                else: find_secrets(item)
        elif isinstance(value, list):
            for item in value: find_secrets(item)
        elif isinstance(value, str) and value.startswith(('{', '[')):
            try: find_secrets(json.loads(value))
            except ValueError: pass
    database = runtime / 'baton.sqlite3'
    try:
        with sqlite3.connect('file:' + str(database) + '?mode=ro', uri=True, timeout=2) as db:
            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
            result['schema'] = db.execute('PRAGMA user_version').fetchone()[0]
            names = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")]
            fixed = {'events','sessions','session_turns','attempts','dispatch_intents','workspace_leases',
                'execution_grants','budgets','tasks','projects','managed_workspaces','connections','session_runtime_snapshots'}
            total = 0
            for name in sorted(n for n in names if n in fixed or n.startswith('factory_')):
                assert re.fullmatch('[a-z_]+', name)
                count = db.execute('SELECT count(*) FROM "' + name + '"').fetchone()[0]
                assert count <= 2048, ('diagnostic row cap', name, count)
                cursor = db.execute('SELECT * FROM "' + name + '" ORDER BY rowid')
                columns = [c[0] for c in cursor.description]; rows = []
                for values in cursor:
                    row = dict(zip(columns, values))
                    assert all(not isinstance(v, bytes) for v in values), 'Unexpected binary diagnostic field'
                    encoded = json.dumps(row).encode(); assert len(encoded) <= MAX_RESPONSE_BYTES
                    total += len(encoded); assert total <= 16 * 1024 * 1024
                    find_secrets(row); rows.append(row)
                result['tables'][name] = {'query': 'SELECT * FROM "' + name + '" ORDER BY rowid',
                    'row_count': count, 'rows': rows}
            db.rollback()
    except Exception as failure:
        result['complete'] = False; result['errors'].append({'stage': 'readonly_query', 'type': type(failure).__name__, 'message': str(failure)})
    if workspace is not None:
        directory = Path(workspace['root']) / 'reports'
        if directory.is_dir():
            paths = sorted(directory.glob('fixture-stderr-*.txt'))
            if len(paths) > 8:
                result['complete'] = False; result['errors'].append({'stage': 'stderr', 'message': 'more than8 stderr files'})
            for path in paths[:8]:
                try:
                    assert re.fullmatch('fixture-stderr-[0-9]+\\.txt', path.name)
                    raw, pin = bounded_file(path, 1048576, directory)
                    result['generated_stderr'].append({'name': path.name, 'original_pin': pin, 'text': raw.decode('utf-8', 'replace')})
                except Exception as failure:
                    result['complete'] = False; result['errors'].append({'stage': 'stderr', 'name': path.name, 'message': str(failure)})
    def sanitize(value):
        if isinstance(value, dict):
            return {k: '[redacted generated fixture secret]' if k in {'token','scoped_token'} else sanitize(v) for k,v in value.items()}
        if isinstance(value, list): return [sanitize(v) for v in value]
        if isinstance(value, str):
            for secret in secrets:
                if secret: value = value.replace(secret, '[redacted generated fixture secret]')
            return value
        return value
    for table in result['tables'].values():
        table['raw_rows_sha256_before_redaction'] = sha(json.dumps(table['rows'], separators=(',', ':')).encode())
    result = sanitize(result)
    result['redaction'] = 'Only generated Owner/scoped token values/keys; other evidence retained. Hashes refer to before-redaction query encoding, not DB bytes.'
    path = output / ('diagnostics-' + uuid.uuid4().hex + '.json')
    digest = write_private_json(path, result)
    return {'path': str(path), 'sha256': digest, 'complete': result['complete'], 'schema': result.get('schema'),
        'table_rows': {k:v['row_count'] for k,v in result['tables'].items()}, 'retained_database_bytes': False}, result


REQUEST_FIELDS = frozenset({'schema_version','kind','task_id','root_task_id','parent_task_id','goal',
    'input_artifacts','output_artifacts','handoff_result','handoff_results','handoff_digest','context_continuation','acceptance_state'})


class FixtureControllerReaper:
    """Linux test-controller only; wait just a bound adopted MCP zombie, never signal."""
    def __init__(self, record):
        import ctypes
        self.ctypes = ctypes
        self.libc = ctypes.CDLL(None, use_errno=True)
        self.enabled = False
        self.record = record

    def get(self):
        value = self.ctypes.c_int()
        assert self.libc.prctl(37, self.ctypes.byref(value), 0, 0, 0) == 0
        return value.value

    def enable(self):
        assert sys.platform == 'linux'
        self.prior = self.get()
        assert self.libc.prctl(36, 1, 0, 0, 0) == 0
        self.enabled = True
        self.pid = os.getpid()
        self.identity = support.proc_identity(self.pid)
        self.record.update(controller_pid=self.pid, controller_start_ticks=self.identity['start_ticks'],
            prior_subreaper=self.prior, enabled=self.get() == 1, restored=False, adopted_mcp_waits=[],
            dimension='Linux test controller reaps only bound adopted MCP children; NOT Core descendant join')
        assert self.record['enabled']

    def reap(self, fixed):
        assert self.enabled and os.getpid() == self.pid
        assert support.proc_identity(self.pid)['start_ticks'] == self.identity['start_ticks']
        raw, pin = bounded_file(Path(fixed['path']), MAX_HTTP_BYTES)
        assert pin == fixed['file_pin'] and sha(raw) == fixed['sha256']
        saved = json.loads(raw, object_pairs_hook=unique_object)
        assert saved['owned_companion'] == fixed['owned_companion'] and saved['owned_provider'] == fixed['owned_provider']
        expected = fixed['owned_companion']; pid = expected['pid']
        assert type(pid) is int and pid > 1 and re.fullmatch('[0-9]+', expected['start_ticks'])
        try:
            current = support.proc_identity(pid)
        except FileNotFoundError:
            return
        assert current['start_ticks'] == expected['start_ticks'], 'Bound companion PID identity changed; no wait'
        if current['parent_pid'] != self.pid or current['state'] != 'Z':
            return  # Live/non-adopted identities still face the original argv/gone checks.
        assert os.getpgid(pid) == expected['pgid'] == fixed['owned_provider']['pid']
        assert support.proc_identity(pid) == current
        waited_pid, status = os.waitpid(pid, os.WNOHANG)
        assert waited_pid == pid, 'Bound adopted zombie was not actually waited'
        try:
            after = support.proc_identity(pid)
        except FileNotFoundError:
            after = None
        assert after is None or after['start_ticks'] != expected['start_ticks'], 'Wait did not remove original identity'
        self.record['adopted_mcp_waits'].append({'role': 'owned_companion', 'expected': dict(expected),
            'observed_before_wait': current, 'waited_pid': waited_pid, 'raw_wait_status': status,
            'exitcode': os.waitstatus_to_exitcode(status), 'original_identity_gone_after_wait': True,
            'session_id': fixed['session_id'], 'attempt_id': fixed['attempt_id'], 'record_sha256': fixed['sha256'],
            'reaped_by': 'test_controller', 'Core_reaped_this_descendant': False, 'signal_sent': False})

    def restore(self):
        if self.enabled:
            assert self.libc.prctl(36, self.prior, 0, 0, 0) == 0
            assert self.get() == self.prior
            self.record.update(restored=True, restored_subreaper=self.prior)
            self.enabled = False


def collect_started_records(runtime, workspace, scope, context, verified, output, reaper=None):
    """A discovery glob cannot authorize cleanup: bind every FD to actual Core text/session."""
    if workspace is None or scope is None: return []
    directory = Path(workspace['root']) / 'reports'
    if not directory.exists(): return []
    paths = sorted(directory.glob('fixture-request-*.json')); assert len(paths) <= 8
    assert sha(Path(context['provider']).read_bytes()) == context['program_sha256']
    assert sha(Path(context['settings']).read_bytes()) == context['settings_sha256']
    assert sha(Path(context['companion']).read_bytes()) == context['companion_sha256']
    with sqlite3.connect('file:' + str(runtime / 'baton.sqlite3') + '?mode=ro', uri=True, timeout=2) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
        sessions = {json.loads(raw)['native_session_id']:json.loads(raw) for raw, in db.execute('SELECT document FROM sessions') if json.loads(raw).get('native_session_id')}
        turns = [(sid, json.loads(raw)['text']) for sid,raw in db.execute('SELECT session_id,document FROM session_turns')]
        attempts = {sid:json.loads(raw) for sid,raw in db.execute('SELECT session_id,document FROM factory_attempts WHERE scope_id=?', (scope['id'],))}
        discovered = []
        for path in paths:
            raw, pin = bounded_file(path, MAX_HTTP_BYTES, directory); row = json.loads(raw, object_pairs_hook=unique_object)
            assert pin['mode'] == 0o600 and row['fixture_nonce'] == context['nonce']
            assert row['program_sha256'] == context['program_sha256'] and row['settings_sha256'] == context['settings_sha256']
            request = json.loads(row['request_text'], object_pairs_hook=unique_object)
            assert set(request) == REQUEST_FIELDS and sha(row['request_text'].encode()) == row['request_sha256']
            assert request['kind'] == 'agentbaton_factory_task_request' and request['schema_version'] == 1
            assert request['root_task_id'] == scope['root_task_id'] and request['context_continuation'] == 'new_attempt_explicit_handoff_only'
            assert request['acceptance_state'] == 'not_established'
            session = sessions[row['native_session_id']]
            assert session['task_id'] == request['task_id'] and session['workspace_id'] == workspace['id']
            assert session['connection_id'] == context['connection_id'] and session['grant_id'] == scope['root_grant_id']
            assert sum(sid==session['id'] and text==row['request_text'] for sid,text in turns) == 1
            attempt = attempts[session['id']]
            assert attempt['attempt_id'] == session['attempt_id'] and attempt['task_id'] == request['task_id']
            assert attempt['root_task_id'] == scope['root_task_id'] and attempt['run_id'] == context['run_id']
            expected_stage = 'child' if request['parent_task_id'] is not None else ('parent-resumed' if request['handoff_results'] else 'parent-initial')
            assert row['stage'] == expected_stage
            for field in ['owned_provider','owned_companion']:
                identity = row[field]
                assert type(identity['pid']) is int and identity['pid']>1 and type(identity['pgid']) is int and identity['pgid']>1
                assert re.fullmatch('[0-9]+', identity['start_ticks'])
            pid = row['owned_provider']['pid']
            assert path.name == 'fixture-request-' + str(pid) + '.json'
            assert row['native_session_id'] == 'generated-factory-' + str(pid) + '-' + context['nonce']
            assert row['owned_provider']['pgid'] == pid and row['owned_companion']['pgid'] == pid
            assert row['provider_argv'] == [context['provider'], '--fixture-settings', context['settings']]
            argv = row['companion_argv']; assert len(argv)==3 and argv[:2]==[context['companion'],'factory-mcp']
            assert Path(argv[2]).resolve().is_relative_to(runtime / 'factory-scoped-handles')
            fixed = {'path':str(path),'sha256':sha(raw),'file_pin':pin,'session_id':session['id'],
                'attempt_id':session['attempt_id'],'stage':row['stage'],'native_session_id':row['native_session_id'],
                'owned_provider':dict(row['owned_provider']),'owned_companion':dict(row['owned_companion'])}
            old = next((r for r in verified if r['path']==fixed['path']),None)
            assert old is None or old==fixed, 'Previously bound generated record changed'
            if reaper is not None and old is not None:
                reaper.reap(old)  # Only previously bound identities after fresh full Core/record validation.
            for field, expected_argv in [('owned_provider', row['provider_argv']), ('owned_companion', argv)]:
                expected = row[field]
                try: current = support.proc_identity(expected['pid'])
                except (FileNotFoundError, ProcessLookupError): current = None
                if current is not None and current['start_ticks']==expected['start_ticks']:
                    with Path('/proc',str(expected['pid']),'cmdline').open('rb') as reader:
                        observed = reader.read(8193)
                    assert len(observed)<=8192 and observed.endswith(b'\0')
                    actual = [x.decode() for x in observed[:-1].split(b'\0')]
                    assert actual[-len(expected_argv):] == expected_argv and os.getpgid(expected['pid'])==expected['pgid']
                    assert support.proc_identity(expected['pid'])['start_ticks']==expected['start_ticks']
            if old is None:
                copy = output / ('verified-' + str(pid) + '.json')
                write_private_json(copy, {'dimension':'verified generated fixture record, not authority from an Agent',
                    'core_session_id':session['id'],'core_attempt_id':session['attempt_id'],'original_file_pin':pin,'record':row})
                verified.append(fixed)
            row['request'] = request; discovered.append(row)
        db.rollback()
    return discovered


def note_failure(report, failure, stage, clean=str):
    report['status'] = 'failed_or_interrupted'
    entry = {'stage': stage, 'type': type(failure).__name__, 'message': clean(failure),
        'traceback': clean(''.join(traceback.format_exception(type(failure), failure, failure.__traceback__)))}
    if 'failure' not in report:
        report['failure'] = entry
    else:
        report.setdefault('secondary_failures', []).append(entry)


def cleanup_call(report, stage, action, clean=str):
    try:
        return action()
    except BaseException as failure:
        note_failure(report, failure, stage, clean)
        return None


def runtime_exit(report):
    return 0 if (report.get('status') == 'passed' and report.get('cleanup_complete') is True
        and not report.get('failure') and not report.get('http_unexpected_errors')) else 1


def inspect_owned_records(report, verified, required_count, provider, settings, companion, runtime, clean=str, require_full=True):
    """Only previously checked fixed identities; a new empty glob is not proof."""
    complete = True; observations = []; identities = set()
    if require_full and len(verified) != required_count:
        note_failure(report, AssertionError(('Missing required verified fixture records', len(verified), required_count)), 'cleanup_identity_count', clean)
        complete = False
    for row in verified:
        try:
            path = Path(row['path'])
            raw, pin = bounded_file(path, MAX_HTTP_BYTES)
            assert sha(raw) == row['sha256'] and pin == row['file_pin'], 'Previously verified fixture record missing or changed'
        except BaseException as failure:
            note_failure(report, failure, 'cleanup_record_integrity', clean); complete = False
        for field in ['owned_provider', 'owned_companion']:
            try:
                expected = row[field]; pid = expected['pid']
                assert type(pid) is int and pid > 1 and type(expected['pgid']) is int and expected['pgid'] > 1
                assert re.fullmatch('[0-9]+', expected['start_ticks'])
                pair = (pid, expected['start_ticks']); assert pair not in identities; identities.add(pair)
                try:
                    current = support.proc_identity(pid)
                except FileNotFoundError:
                    current = None
                alive = current is not None and current['start_ticks'] == expected['start_ticks']
                observation = {'role': field, 'expected': expected, 'observed': current,
                    'original_identity_gone': not alive, 'signal_sent': False}
                if alive:
                    # A signal can only target this saved identity and this exact
                    # private fixture executable/config/credential path, never ps/pkill.
                    assert os.getpgid(pid) == expected['pgid']
                    raw = Path(f'/proc/{pid}/cmdline').read_bytes()
                    assert len(raw) <= 8192 and raw.endswith(b'\0')
                    argv = [part.decode() for part in raw[:-1].split(b'\0')]
                    if field == 'owned_provider':
                        assert expected['pgid'] == pid and str(provider) in argv
                        assert argv[-2:] == ['--fixture-settings', str(settings)]
                    else:
                        assert expected['pgid'] == row['owned_provider']['pid']
                        assert len(argv) == 3 and argv[:2] == [str(companion), 'factory-mcp']
                        assert Path(argv[2]).resolve().is_relative_to(runtime / 'factory-scoped-handles')
                    assert support.proc_identity(pid)['start_ticks'] == expected['start_ticks']
                    os.kill(pid, signal.SIGKILL); observation['signal_sent'] = True
                    observations.append(observation)
                    raise AssertionError('Exact private fixture identity needed forced cleanup; not Core stop proof')
                observations.append(observation)
            except BaseException as failure:
                note_failure(report, failure, 'cleanup_owned_identity', clean); complete = False
    inspected_required = required_count if require_full else len(verified)
    if len(identities) != inspected_required * 2 or len(observations) != inspected_required * 2:
        note_failure(report, AssertionError(('Incomplete required identity inspection', len(identities), len(observations), inspected_required * 2)), 'cleanup_identity_coverage', clean)
        complete = False
    if not require_full:
        complete = False  # An early failed workflow never acquires successful-cleanup status.
    report['cleanup'].append({'action': 'fixed previously verified generated identities inspected',
        'required_records_for_success': required_count, 'required_identities_for_success': required_count * 2,
        'workflow_successful_before_cleanup': require_full, 'actual_bound_records': len(verified),
        'actual_bound_identities': len(verified) * 2,
        'verified_records': len(verified), 'inspected_identities': len(observations), 'observations': observations,
        'complete': complete})
    return complete


def verify_attempt_budgets(rows, task_id, project_id, expected_attempts, expected_active=0):
    """Exact isolated fixture keys from sessions.rs; no absent project meter green."""
    expected_keys = {'root:' + task_id, 'project:' + project_id}
    assert len(rows) == 2 and {r['scope'] for r in rows} == expected_keys, ('Exact root/project rows required', rows, expected_keys)
    for row in rows:
        assert type(row['consumed']) is int and row['consumed'] == expected_attempts, ('Cumulative attempts must not refund/reset', row, expected_attempts)
        assert type(row['active']) is int and row['active'] == expected_active, ('Exact observed active count required', row, expected_active)


@dataclass(frozen=True)
class Bounds:
    # Only filled from private authenticated actual Core results, never HTTP.
    project_id: str
    task_id: str
    scope_id: str


def decode_envelope(value, bounds: Bounds):
    require(type(value) is dict and set(value) <= {'command', 'args'} and 'command' in value)
    command, args = value['command'], value.get('args')
    require(isinstance(command, str))
    if args is None:
        args = {}
    require(type(args) is dict)
    simple = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list',
        'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}
    scoped = {'workspaces_list': ('workspaces.list', 'projectId', 'project_id', bounds.project_id),
        'grants_list': ('grants.list', 'projectId', 'project_id', bounds.project_id),
        'goal_scopes_list': ('goal_scopes.list', 'taskId', 'task_id', bounds.task_id),
        'factory_scopes_list': ('factory_scopes.list', 'rootTaskId', 'root_task_id', bounds.task_id),
        'factory_scopes_get': ('factory_scopes.get', 'scopeId', 'scope_id', bounds.scope_id),
        'factory_runs_get': ('factory_runs.get', 'scopeId', 'scope_id', bounds.scope_id)}
    if command in simple:
        require(not args)
        return FixedCall(simple[command], {})
    if command in scoped:
        method, camel, snake, expected = scoped[command]
        require(set(args) == {camel} and isinstance(args[camel], str) and args[camel] == expected)
        return FixedCall(method, {snake: expected})
    if command == 'events_subscribe':
        require(set(args) <= {'cursor', 'limit'})
        cursor, limit = args.get('cursor', 0), args.get('limit', 100)
        require(safe_integer(cursor) and safe_integer(limit, 1) and limit <= 100)
        return FixedCall('events.subscribe', {'cursor': cursor, 'limit': limit})
    if command in {'factory_runs_start', 'factory_runs_cancel'}:
        require(set(args) == {'scopeId', 'expectedRevision', 'idempotencyKey'})
        require(isinstance(args['scopeId'], str) and args['scopeId'] == bounds.scope_id)
        require(safe_integer(args['expectedRevision'], 1))
        key = args['idempotencyKey']
        require(isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key) is not None)
        return FixedCall('factory_runs.start' if command.endswith('start') else 'factory_runs.cancel',
            {'scope_id': bounds.scope_id}, key, args['expectedRevision'])
    # Includes Owner approve/propose/finalize/decline, revoke (host-only here),
    # generic IPC, session dispatch and all four scoped Agent tools.
    raise BoundaryRefusal(403, 'Outside fixed Factory browser fixture boundary')


def decode_http(raw, *, origin, host, content_type, expected_origin, bounds):
    require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}', expected_origin) is not None)
    require(1 <= int(expected_origin.rsplit(':', 1)[1]) <= 65535)
    if origin != expected_origin or host != expected_origin.removeprefix('http://'):
        raise BoundaryRefusal(403, 'Exact private loopback Origin and Host required')
    require(content_type == 'application/json' and 0 < len(raw) <= MAX_HTTP_BYTES)
    try:
        envelope = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
            parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
    except (UnicodeError, ValueError) as failure:
        raise BoundaryRefusal(400, 'Malformed complete JSON request') from failure
    return decode_envelope(envelope, bounds)


class PrivateHost:
    """Private Owner token stays outside HTTP/results/ACP settings."""
    def __init__(self, runtime):
        self.runtime = runtime.resolve(strict=True)
        token_path = self.runtime / 'session.token'
        require(stat.S_IMODE(token_path.stat().st_mode) == 0o600)
        self.token = token_path.read_text()

    def wire(self, call):
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': self.token,
            'method': call.method, 'params': call.params}
        if call.key is not None:
            frame['idempotency_key'] = call.key
        if call.revision is not None:
            frame['expected_revision'] = call.revision
        raw = json.dumps(frame, separators=(',', ':'), allow_nan=False).encode() + b'\n'
        require(len(raw) <= MAX_IPC_BYTES)
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(5)
            stream.connect(str(self.runtime / 'batond.sock'))
            stream.sendall(raw)
            with stream.makefile('rb') as reader:
                response = reader.readline(MAX_RESPONSE_BYTES + 1)
        require(0 < len(response) <= MAX_RESPONSE_BYTES and response.endswith(b'\n'))
        value = json.loads(response, object_pairs_hook=unique_object)
        require(type(value) is dict)
        return value, response  # original Core response bytes for HTTP, no u64 rewrite

    def result(self, method, params, key=None, revision=None):
        value, _ = self.wire(FixedCall(method, params, key, revision))
        assert not value.get('error'), {'method': method, 'response': value}
        return value['result']


CONNECTION_PIN_FIELDS = frozenset({'protocol_route', 'revision', 'environment_id', 'identity_ref',
    'host_id', 'config_revision', 'executable_sha256', 'capability_snapshot_id'})


def review_factory(proposal, selection, project, task, grant, connection, workspace, companion):
    """Python keeps original serde integers; representation hash is not Core digest."""
    require(proposal.get('kind') == 'factory' and isinstance(proposal.get('id'), str)
        and safe_integer(proposal.get('revision'), 1))
    snapshot = proposal['snapshot']
    require(set(snapshot) == {'schema_version', 'selection', 'bindings', 'contract_digest'})
    require(snapshot['schema_version'] == 1 and snapshot['selection'] == selection)
    require(re.fullmatch('[a-f0-9]{64}', snapshot['contract_digest']) is not None)
    b = snapshot['bindings']
    require(set(b) == {'project', 'root_task', 'approved_grant', 'connections', 'workspaces',
        'inputs', 'outputs', 'companion', 'guarantees', 'start_contract'})
    require(b['project'] == project and b['root_task'] == task and b['approved_grant'] == grant)
    require(task['state'] == 'draft' and not task.get('acceptance_ref') and not task.get('acceptance_sequence'))
    require(b['connections'] == [connection] and b['workspaces'] == [workspace])
    pin = grant['connection_bindings'][connection['id']]
    require(set(pin) == CONNECTION_PIN_FIELDS and all(pin[k] == connection[k] for k in pin))
    require(safe_integer(pin['revision'], 1) and all(isinstance(pin[k], str) and pin[k]
        for k in CONNECTION_PIN_FIELDS - {'revision'}))
    require(re.fullmatch('[a-f0-9]{64}', pin['executable_sha256']) is not None)
    require(workspace['identity'] == grant['workspace_bindings'][workspace['id']])
    require(connection['host_id'] in grant['scope']['host_ids'])
    require(all(r in grant['scope']['recipients'] for r in selection['recipients']))
    for rows, requirements, is_input in [(b['inputs'], selection['inputs'], True), (b['outputs'], selection['outputs'], False)]:
        require(len(rows) == len(requirements))
        for row, expected in zip(rows, requirements):
            contract = row['contract'] if is_input else row
            require(contract['requirement'] == expected and contract['canonical_root'] == workspace['root'])
            require(contract['root_device'] == workspace['identity']['device'] and contract['root_inode'] == workspace['identity']['inode'])
            if is_input:
                require(row['sha256'] == expected['expected_sha256'] and row['bytes'] == len(b'fixed input\n'))
    require(b['companion'] == {'executable': str(companion), 'executable_sha256': sha(companion.read_bytes()),
        'argument_template': ['factory-mcp', '${SESSION_HANDLE_FILE}'],
        'credential_transport': 'private_0600_file_reference', 'global_daemon_token_available': False})
    require(set(b['guarantees']) == {'filesystem_sandbox', 'network_sandbox', 'account_isolation',
        'output_quota', 'continuous_input_immutability', 'complete_process_tree_containment'}
        and all(v is False for v in b['guarantees'].values()))
    start = b['start_contract']
    require(set(start) == {'mode', 'root_task_request_digest', 'context_continuation', 'implicit_start_on_approval', 'new_workspace_authority'})
    require(start['mode'] == 'single_scope_single_run' and start['context_continuation'] == 'new_attempt_explicit_handoff_only'
        and start['implicit_start_on_approval'] is False and start['new_workspace_authority'] is False
        and re.fullmatch('[a-f0-9]{64}', start['root_task_request_digest']) is not None)
    raw = json.dumps(proposal, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()
    require(len(raw) <= MAX_FACTORY_REVIEW_BYTES)
    return {'authority': 'private authenticated generated Owner fixture, NOT real OS dialog',
        'proposal_id': proposal['id'], 'complete_review_encoding_bytes': len(raw),
        'complete_review_encoding_sha256': sha(raw), 'snapshot': snapshot}


def preflight(args):
    assert args.batond and args.candidate_manifest and args.frontend, 'Supply frozen batond/manifest and composed built frontend'
    binary = args.batond.resolve(strict=True)
    raw = args.candidate_manifest.resolve(strict=True).read_bytes()
    manifest = json.loads(raw, object_pairs_hook=unique_object)
    assert manifest['schema'] == 13, 'Only actual schema13 candidate is Factory runtime evidence'
    companion = binary.with_name('baton').resolve(strict=True)
    for name, path in [('batond', binary), ('baton', companion)]:
        assert manifest['binaries'][name] == sha(path.read_bytes()), (name, 'frozen binary SHA mismatch')
        assert path.is_file() and os.access(path, os.X_OK)
    assert companion.stat().st_size <= 128 * 1024 * 1024, 'Core companion original128MiB binding cap; never strip frozen executable'
    frontend = args.frontend.resolve(strict=True)
    assert (frontend / 'index.html').is_file()
    paths = list(frontend.rglob('*'))
    assert len(paths) <= 4096 and not any(p.is_symlink() for p in paths)
    assert sum(p.stat().st_size for p in paths if p.is_file()) <= 64 * 1024 * 1024
    return binary, companion, frontend, manifest, raw


GENERATED_ACP = r'''import hashlib,json,os,signal,stat,subprocess,sys
if '--version' in sys.argv:
 print('Generated account-free Factory ACP 1');sys.exit(0)
settings_path=sys.argv[sys.argv.index('--fixture-settings')+1]
mcp=None;mid=0
def emit(value):print(json.dumps(value),flush=True)
def response(request,value):emit({'jsonrpc':'2.0','id':request['id'],'result':value})
def identity(pid):
 tail=open('/proc/'+str(pid)+'/stat').read().rsplit(')',1)[1].split()
 return {'pid':pid,'pgid':os.getpgid(pid),'start_ticks':tail[19]}
def mcp_call(method,params,allow_error=False):
 global mid
 mid+=1;frame={'jsonrpc':'2.0','id':mid,'method':method,'params':params}
 raw=json.dumps(frame);assert len(raw.encode())<=65536
 mcp.stdin.write(raw+'\n');mcp.stdin.flush()
 raw=mcp.stdout.readline(1048577);assert raw.endswith('\n') and len(raw.encode())<=1048576
 value=json.loads(raw);assert value['id']==mid
 if not allow_error:assert 'error' not in value,value
 return value if allow_error else value['result']
def tool(name,args):
 packet=mcp_call('tools/call',{'name':name,'arguments':args})
 value=json.loads(packet['content'][0]['text']);assert 'error' not in value,value
 return value['result']
def close_mcp():
 global mcp
 if mcp is not None:
  try:mcp.stdin.close()
  except BaseException as failure:
   try:mcp.wait(timeout=1)
   except BaseException as wait_failure:failure.add_note('MCP wait also failed: '+repr(wait_failure))
   raise
  mcp.wait(timeout=1);mcp=None
def stop_on_signal(signum,_frame):
 signal.signal(signum,signal.SIG_IGN)
 raise SystemExit(128+signum)
signal.signal(signal.SIGTERM,stop_on_signal)
def file_hash(path):return hashlib.sha256(open(path,'rb').read()).hexdigest()
try:
 for line in sys.stdin:
  request=json.loads(line);method=request.get('method')
  if method=='initialize':
   assert not any(k in os.environ for k in ['GH_TOKEN','GITHUB_TOKEN','OPENAI_API_KEY','ANTHROPIC_API_KEY'])
   response(request,{'protocolVersion':1,'agentCapabilities':{}})
  elif method=='session/new':
   settings=json.load(open(settings_path));servers=request['params']['mcpServers'];assert len(servers)==1
   os.makedirs('reports',exist_ok=True)
   stderr_fd=os.open('reports/fixture-stderr-'+str(os.getpid())+'.txt',os.O_WRONLY|os.O_CREAT|os.O_APPEND|os.O_NOFOLLOW,0o600)
   sys.stderr=os.fdopen(stderr_fd,'a',buffering=1)
   server=servers[0];assert server['name']=='agentbaton-factory' and server['env']==[]
   assert server['command']==settings['companion'] and server['args'][0]=='factory-mcp' and len(server['args'])==2
   credential=server['args'][1];assert os.path.isabs(credential)
   assert stat.S_IMODE(os.stat(credential).st_mode)==0o600 and stat.S_IMODE(os.stat(os.path.dirname(credential)).st_mode)==0o700
   # Never open the scoped credential or owner session.token; only Baton reads it.
   mcp=subprocess.Popen([server['command']]+server['args'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=sys.stderr,text=True)
   mcp_call('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'generated-browser-fixture','version':'1'}})
   assert [t['name'] for t in mcp_call('tools/list',{})['tools']]==['delegate_task','get_task_status','get_task_result','cancel_task']
   native_session_id='generated-factory-'+str(os.getpid())+'-'+settings['nonce']
   response(request,{'sessionId':native_session_id})
  elif method=='session/prompt':
   raw=request['params']['prompt'][0]['text'];assert len(raw.encode())<=131072;c=json.loads(raw)
   assert set(c)=={'schema_version','kind','task_id','root_task_id','parent_task_id','goal','input_artifacts','output_artifacts','handoff_result','handoff_results','handoff_digest','context_continuation','acceptance_state'}
   assert c['schema_version']==1 and c['kind']=='agentbaton_factory_task_request'
   assert c['context_continuation']=='new_attempt_explicit_handoff_only' and c['acceptance_state']=='not_established'
   assert c['root_task_id']==settings['task_id']
   status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']
   denied=mcp_call('tools/call',{'name':'approvals.finalize','arguments':{'proposal_id':'forbidden'}},True)
   assert 'error' in denied
   assert len(c['input_artifacts'])==1
   source=c['input_artifacts'][0];contract=source['contract'];requirement=contract['requirement']
   assert requirement['relative_path']=='input.txt' and requirement['artifact_id']=='input'
   assert open('input.txt','rb').read()==b'fixed input\n' and file_hash('input.txt')==source['sha256']==requirement['expected_sha256']
   root_stat=os.stat('.');assert os.path.realpath('.')==contract['canonical_root'] and root_stat.st_dev==contract['root_device'] and root_stat.st_ino==contract['root_inode']
   stage='child' if c['parent_task_id'] is not None else ('parent-resumed' if c['handoff_results'] else 'parent-initial')
   os.makedirs('reports',exist_ok=True)
   proof={'stage':stage,'request_text':raw,'request_sha256':hashlib.sha256(raw.encode()).hexdigest(),
    'owned_provider':identity(os.getpid()),'owned_companion':identity(mcp.pid),'denied_owner_tool':denied,
    'fixture_nonce':settings['nonce'],'native_session_id':native_session_id,
    'program_sha256':file_hash(__file__),'settings_sha256':file_hash(settings_path),
    'provider_argv':sys.argv,'companion_argv':[server['command']]+server['args']}
   record_path='reports/fixture-request-'+str(os.getpid())+'.json'
   record_fd=os.open(record_path+'.tmp',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
   with os.fdopen(record_fd,'w')as f:json.dump(proof,f)
   os.replace(record_path+'.tmp',record_path)
   with open('reports/fixture-stages.txt','a')as f:f.write(stage+'\n')
   if settings['case']=='cancel':
    assert stage=='parent-initial'
    continue  # actual live Provider awaits real session/cancel; no end_turn success
   if settings['case']=='unknown':
    close_mcp();print('generated malformed ACP protocol',flush=True);continue
   if stage=='parent-initial':
    child=tool('delegate_task',{'goal':'Write the fixed generated child report','connection_id':settings['connection_id'],
     'workspace_id':settings['workspace_id'],'input_artifact_ids':['input'],'output_artifact_ids':['report-child'],
     'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},
     'wait_for_child':True,'idempotency_key':'generated-child'})
    assert child['parent_task_id']==c['task_id'] and child['state']=='blocked_on_live_parent'
    early=tool('get_task_result',{'task_id':child['task_id']});assert early['task']['result_available'] is False and early['acceptance_state']=='not_established'
   else:
    if stage=='parent-resumed':
     assert len(c['handoff_results'])==1 and c['handoff_digest'] and c['handoff_result']['handoff_digest']==c['handoff_digest']
     result=c['handoff_results'][0];assert result['task']['confirmed_stop'] is True and result['task']['result_available'] is True
     assert result['task']['parent_task_id']==c['task_id'] and result['acceptance_state']=='not_established'
     assert tool('get_task_result',{'task_id':result['task']['task_id']})['task']['task_id']==result['task']['task_id']
     for artifact in result['artifacts']:
      assert file_hash(artifact['relative_path'])==artifact['sha256'] and os.stat(artifact['relative_path']).st_size==artifact['bytes']
    artifact_id='report-child' if stage=='child' else 'report-parent'
    output=next(a for a in c['output_artifacts'] if a['requirement']['artifact_id']==artifact_id)
    path=output['requirement']['relative_path'];assert path=='reports/'+artifact_id+'.txt'
    assert os.path.realpath(path).startswith(os.path.realpath('.')+'/')
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_TRUNC|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb')as f:f.write((stage+' '+artifact_id+' report\n').encode())
   close_mcp()
   response(request,{'stopReason':'end_turn'})
  elif method=='session/cancel':
   close_mcp()
finally:
 close_mcp()
'''


def runtime_driver(args, output, attribution, prepared):
    raw_binary, raw_companion, frontend, manifest, manifest_raw = prepared
    try:
        from playwright.sync_api import expect, sync_playwright
    except ImportError as failure:
        (output / 'preflight-refusal.json').write_text(json.dumps({**attribution, 'status': 'preflight_refused',
            'actual_runtime': 'not_run', 'reason': str(failure), 'no_process_token_read_or_listener': True}, indent=2) + '\n')
        return 2
    compile(GENERATED_ACP, '<generated-account-free-acp>', 'exec')
    assets = {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()}
    report = {**attribution, 'status': 'failed_or_interrupted', 'case': args.case,
        'dimension': 'actual built UI/Linux Chromium/fixed HTTP/authenticated Core13/generated ACP and real private Baton MCP',
        'not_claimed': ['Native Owner OS dialog', 'macOS', 'real Provider/model', 'true response-drop recovery',
            'Provider disconnect side-effect recovery', 'complete Factory fault matrix', 'full product completion'],
        'candidate': manifest, 'candidate_manifest_sha256': sha(manifest_raw), 'frontend_assets_sha256': assets,
        'checks': [], 'private_owner_reviews': [], 'http': [], 'http_unexpected_errors': [], 'readonly_aborts': [],
        'observations': {}, 'screenshots': [], 'cleanup': [], 'cleanup_complete': False}
    token = ''; pump = lambda: None; observe_records = lambda: []
    def clean(value):
        return str(value).replace(token, '[redacted generated Owner token]') if token else str(value)
    def wait(read, predicate, seconds=15):
        deadline = time.monotonic() + seconds
        while True:
            pump(); value = read(); observe_records()
            if predicate(value):
                return value
            assert time.monotonic() < deadline, clean(('Observation timed out', value))
            time.sleep(.02)
    def passed(name):
        report['checks'].append({'name': name, 'status': 'passed'})
    temporary_owner = tempfile.TemporaryDirectory(prefix='baton-factory-')
    controller_reaper = None
    try:
        report['test_controller_reaper'] = {}
        controller_reaper = FixtureControllerReaper(report['test_controller_reaper'])
        controller_reaper.enable()
        temporary = temporary_owner.name
        root = Path(temporary).resolve(); root.chmod(0o700)
        report['private_installation'] = {}
        binary, companion = install_private_tools(raw_binary, raw_companion, root / 'installed', report['private_installation'])
        runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
        settings = root / 'generated-settings.json'
        provider = root / 'generated-factory-acp'
        provider.write_text('#!' + sys.executable + '\n' + GENERATED_ACP); provider.chmod(0o700)
        report['generated_program_sha256'] = sha(provider.read_bytes())
        log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        server = thread = browser = None; workspace = host = scope = None; verified_records = []
        record_context = {}; diagnostic_captured = False
        def diagnose():
            nonlocal diagnostic_captured
            if diagnostic_captured: return
            diagnostic_captured = True
            try:
                discovered = observe_records()
                report['observations']['failure_actual_task_requests'] = discovered
            except Exception as record_failure:
                report['observations']['failure_record_verification_error'] = clean(record_failure)
            try:
                info, data = diagnostic_snapshot(runtime, workspace, output, token)
                report['observations']['failure_diagnostics'] = info
                table = data['tables'].get('factory_attempts', {}).get('rows', [])
                events = data['tables'].get('events', {}).get('rows', [])
                decoded = [json.loads(e['document']) for e in events]
                never_started = [e['aggregate_id'] for e in decoded if e['kind']=='session.stopped' and e['payload'].get('no_process_started') is True]
                bound = {r['session_id'] for r in verified_records}
                report['observations']['failed_record_inventory'] = {'actual_bound_records':len(verified_records),
                    'actual_bound_identities':len(verified_records)*2,'verified_session_ids':sorted(bound),
                    'reserved_session_ids':[r['session_id'] for r in table],
                    'known_never_started_session_ids':never_started,
                    'reserved_without_verified_request_record':[r['session_id'] for r in table if r['session_id'] not in bound],
                    'expected_records_if_workflow_successful':3 if args.case=='normal' else 1,
                    'meaning':'reserved/never-started sessions are not asserted to be spawned processes'}
            except Exception as diagnostic_failure:
                report['observations']['failure_diagnostic_capture_error'] = clean(diagnostic_failure)
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()),
                lambda v: v[0] and v[1] and v[2] is None)
            host = PrivateHost(runtime); token = host.token
            def owner_resource(method, params, kind, key, revision=None):
                proposal = host.result(method, params, key, revision)
                assert proposal['kind'] == kind
                raw = json.dumps(proposal, ensure_ascii=False, separators=(',', ':')).encode()
                assert len(raw) <= MAX_LEGACY_REVIEW_BYTES
                report['private_owner_reviews'].append({'kind': kind, 'proposal': proposal,
                    'authority': 'explicit private generated-resource Owner fixture; not OS dialog'})
                return host.result('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
            created = owner_resource('connections.propose', {'name': 'Generated Factory account-free ACP', 'provider': 'custom_acp',
                'executable': str(provider), 'arguments': ['--fixture-settings', str(settings)]}, 'connection', 'factory:connection')
            connection = wait(lambda: host.result('connections.list', {}),
                lambda rows: any(c['id'] == created['id'] and c['probe_state'] == 'ready' for c in rows))
            connection = next(c for c in connection if c['id'] == created['id'])
            source = root / 'source'; source.mkdir(mode=0o700)
            (source / 'input.txt').write_bytes(b'fixed input\n'); (source / '.gitignore').write_text('reports/\n')
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}
            def git(*argv):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Generated fixture',
                    '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *argv], env=git_env, text=True).strip()
            git('init', '-q', '--initial-branch=main'); git('add', 'input.txt', '.gitignore'); git('commit', '-qm', 'generated fixed Factory input')
            project = host.result('projects.import', {'path': str(source), 'name': 'Generated Factory runtime'}, 'factory:project')
            workspace = host.result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'factory:workspace', project['revision'])
            workspace_root = Path(workspace['root']).resolve(strict=True)
            assert workspace_root.is_relative_to(runtime / 'workspaces')
            grant = owner_resource('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'],
                'connection_id': connection['id'], 'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 30_000}, 'grant', 'factory:grant')
            task = host.result('tasks.create', {'project_id': project['id'], 'goal': 'Generated bounded parent child new-parent Factory'}, 'factory:task')
            settings_value = {'case': args.case, 'task_id': task['id'], 'connection_id': connection['id'],
                'workspace_id': workspace['id'], 'companion': str(companion), 'nonce':uuid.uuid4().hex}
            settings.write_text(json.dumps(settings_value)); settings.chmod(0o600)
            report['observations']['nonsecret_generated_settings'] = settings_value
            report['generated_settings_sha256'] = sha(settings.read_bytes())
            budget = {'attempts': 8, 'parallel': 1, 'runtime_ms': 30_000, 'output_bytes': 12288,
                'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
            selection = {'root_task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'],
                'expected_grant_revision': grant['revision'], 'connection_ids': [connection['id']], 'workspace_ids': [workspace['id']],
                'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'max_bytes': 1024, 'expected_sha256': sha(b'fixed input\n')}],
                'outputs': [{'artifact_id': key, 'relative_path': 'reports/' + key + '.txt', 'max_bytes': 4096, 'expected_sha256': None}
                    for key in ['report-child', 'report-parent']], 'recipients': [], 'max_depth': 2, 'max_children': 1, 'max_total_nodes': 3,
                'root_budget': budget, 'project_budget': dict(budget), 'ttl_ms': 30_000,
                'capabilities': {'delegate_task': True, 'resume_parent_as_new_attempt': True, 'child_planning': False,
                    'child_goal_execution': False, 'create_workspace': False}}
            proposal = host.result('factory_scopes.propose', selection, 'factory:scope')
            report['private_owner_reviews'].append(review_factory(proposal, selection, project, task, grant, connection, workspace, companion))
            scope = host.result('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
            assert scope['id'] == scope['approval_id'] == proposal['id'] and scope['selection'] == selection
            assert scope['bindings'] == proposal['snapshot']['bindings'] and scope['contract_digest'] == proposal['snapshot']['contract_digest']
            record_context.update(nonce=settings_value['nonce'], program_sha256=report['generated_program_sha256'],
                settings_sha256=report['generated_settings_sha256'], connection_id=connection['id'],
                provider=str(provider), settings=str(settings), companion=str(companion),
                companion_sha256=report['private_installation']['tools']['baton']['installed_sha256'])
            def observe():
                directory = workspace_root / 'reports'
                if not directory.is_dir() or not any(directory.glob('fixture-request-*.json')): return []
                current = host.result('factory_runs.get', {'scope_id':scope['id']})
                record_context['run_id'] = current['run']['id']
                return collect_started_records(runtime, workspace, scope, record_context, verified_records, output, controller_reaper)
            observe_records = observe
            assert host.result('sessions.list', {}) == []
            stale, _ = host.wire(FixedCall('factory_runs.start', {'scope_id': scope['id']}, 'factory:wrong-cas', scope['revision'] + 1))
            assert stale['error']['code'] == 'STALE_REVISION' and host.result('sessions.list', {}) == []
            report['observations']['actual_wrong_start_CAS_refusal'] = stale
            bounds = Bounds(project['id'], task['id'], scope['id'])
            passed('actual generated resources and complete private Factory approval; approval starts no Provider')
            lock = threading.Lock(); active = {}; phase = {'value': 'serving'}
            class Server(http.server.ThreadingHTTPServer):
                daemon_threads = False; block_on_close = True
                def handle_error(self, request, client_address):
                    with lock:
                        report['http_unexpected_errors'].append({'stage': 'uncaught_handler', 'traceback': clean(traceback.format_exc())})
            class Handler(http.server.SimpleHTTPRequestHandler):
                def setup(self):
                    super().setup(); self.connection.settimeout(5)
                def log_message(self, *_):
                    pass
                def list_directory(self, _):
                    self.send_error(403); return None
                def do_POST(self):
                    context = {'stage': 'headers', 'method': None, 'phase': phase['value']}
                    identity = id(self)
                    with lock:
                        active[identity] = context
                    try:
                        if self.path != '/__test_factory_ipc':
                            self.send_error(404); return
                        try:
                            require(self.headers.get('Transfer-Encoding') is None)
                            require(len(self.headers.get_all('Content-Length', [])) == 1)
                            require(all(len(self.headers.get_all(k, [])) == 1 for k in ['Origin', 'Host', 'Content-Type']))
                            length = self.headers['Content-Length']
                            require(re.fullmatch('[1-9][0-9]{0,5}', length) is not None and int(length) <= MAX_HTTP_BYTES)
                            raw = self.rfile.read(int(length)); require(len(raw) == int(length))
                            call = decode_http(raw, origin=self.headers.get('Origin', ''), host=self.headers.get('Host', ''),
                                content_type=self.headers.get('Content-Type', ''), expected_origin=url, bounds=bounds)
                        except BoundaryRefusal as failure:
                            self.send_error(failure.status); return
                        context.update(method=call.method, stage='core_call')
                        try:
                            value, reply = host.wire(call)
                            with lock:
                                report['http'].append({'method': call.method, 'params': call.params, 'key': call.key, 'revision': call.revision,
                                    'request_body_sha256': sha(raw), 'original_core_reply_sha256': sha(reply), 'response': value})
                            context['stage'] = 'response_headers'
                            self.send_response(200); self.send_header('Content-Type', 'application/json')
                            self.send_header('Content-Length', str(len(reply))); self.end_headers()
                            context['stage'] = 'response_body'; self.wfile.write(reply); context['stage'] = 'complete'
                        except (BrokenPipeError, ConnectionResetError) as failure:
                            if call.method in READS and context['stage'] == 'response_body':
                                report['readonly_aborts'].append({**context, 'type': type(failure).__name__})
                            else:
                                report['http_unexpected_errors'].append({**context, 'type': type(failure).__name__, 'message': clean(failure)})
                        except Exception as failure:
                            report['http_unexpected_errors'].append({**context, 'type': type(failure).__name__, 'message': clean(failure)})
                            self.send_error(502)
                    finally:
                        with lock:
                            active.pop(identity, None)
            server = Server(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = 'http://127.0.0.1:' + str(server.server_port)
            thread = threading.Thread(target=server.serve_forever); thread.start()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--no-sandbox'])
                context = browser.new_context(viewport={'width': 1488, 'height': 1400}, locale='en-US')
                context.add_init_script("localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_factory_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed Factory fixture refused request');return r.json();}};")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); errors = []; page.on('pageerror', lambda failure: errors.append(str(failure)))
                pump = lambda: page.wait_for_timeout(1)
                def shot(name, focus):
                    focus.evaluate("e=>e.scrollIntoView({block:'start'})")
                    path = output / (name + '.png'); page.screenshot(path=str(path))
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'dimension': 'Linux Chromium; not native/Mac'})
                try:
                    page.goto(url); page.get_by_test_id('task-card').filter(has_text=task['goal']).click(); page.locator('#tab-details').click()
                    panel = page.get_by_test_id('factory-runtime')
                    assert not panel.locator(':scope > details').evaluate('e=>e.open')
                    assert not any(r['method'].startswith('factory_') for r in report['http'])
                    panel.locator(':scope > details > summary').click()
                    expect(panel.get_by_role('button', name='Start approved Factory run', exact=True)).to_be_enabled()
                    expect(panel.get_by_text('Approval is recorded; execution has not started.', exact=True)).to_be_visible()
                    denied = []
                    for command in ['factory_scope_approve', 'factory_scopes_propose', 'factory_scopes_revoke',
                            'approvals_finalize', 'approvals_decline', 'generic_ipc', 'delegate_task', 'get_task_status', 'get_task_result', 'cancel_task']:
                        status = page.evaluate("async command=>(await fetch('/__test_factory_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args:{}})})).status", command)
                        assert status == 403; denied.append(command)
                    report['observations']['http_authority_denied'] = denied
                    shot('01-real-approved-scope-not-started', panel.get_by_role('button', name='Start approved Factory run', exact=True))
                    passed('built UI reads genuine scope; folded load and explicit start; HTTP denies Owner and Agent tools')
                    panel.get_by_role('button', name='Start approved Factory run', exact=True).click()
                    if args.case == 'cancel':
                        wait(lambda: list((workspace_root / 'reports').glob('fixture-request-*.json')), bool)
                        panel.get_by_role('button', name='Refresh Factory records', exact=True).click()
                        expect(panel.get_by_role('button', name='Request Factory cancellation', exact=True)).to_be_enabled()
                        panel.get_by_role('button', name='Request Factory cancellation', exact=True).click()
                        requested = wait(lambda: [r for r in report['http'] if r['method'] == 'factory_runs.cancel'], bool)[0]['response']
                        assert not requested.get('error') and requested['result']['state'] == 'cancel_requested'
                        assert requested['result']['cancel_requested'] is True
                        report['observations']['actual_cancel_intent_receipt'] = requested
                    terminal = {'normal': 'completed', 'cancel': 'cancelled', 'unknown': 'outcome_unknown'}[args.case]
                    detail = wait(lambda: host.result('factory_runs.get', {'scope_id': scope['id']}), lambda d: d['run']['state'] == terminal)
                    report['observations']['terminal'] = detail
                    assert detail['run']['scope_id'] == scope['id'] and detail['run']['context_continuation'] == 'new_attempt_explicit_handoff_only'
                    assert len(detail['tasks']) == (2 if args.case == 'normal' else 1)
                    if args.case in {'normal', 'cancel'}:
                        assert detail['run']['active_session_id'] is None
                        assert all(t['attempt_id'] is not None and t['confirmed_stop'] is True for t in detail['tasks'])
                    if args.case == 'cancel':
                        assert detail['run']['cancel_requested'] is True
                    records = observe_records()
                    expected_stages = ['child', 'parent-initial', 'parent-resumed'] if args.case == 'normal' else ['parent-initial']
                    assert sorted(r['stage'] for r in records) == expected_stages
                    for row in records:
                        assert sha(row['request_text'].encode()) == row['request_sha256']
                        row['request'] = json.loads(row['request_text'])
                        assert row['request']['root_task_id'] == task['id']
                        for field in ['owned_provider', 'owned_companion']:
                            identity = row[field]
                            assert type(identity['pid']) is int and identity['pid'] > 1 and type(identity['pgid']) is int and identity['pgid'] > 1
                            assert re.fullmatch('[0-9]+', identity['start_ticks'])
                        assert row['owned_provider']['pgid'] == row['owned_provider']['pid']
                        assert row['owned_companion']['pgid'] == row['owned_provider']['pid']
                    report['observations']['actual_task_requests'] = records
                    if args.case == 'normal':
                        assert (workspace_root / 'reports/fixture-stages.txt').read_text() == 'parent-initial\nchild\nparent-resumed\n'
                        for artifact_id, content in [('report-child', b'child report-child report\n'), ('report-parent', b'parent-resumed report-parent report\n')]:
                            path = workspace_root / 'reports' / (artifact_id + '.txt')
                            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
                            try:
                                before = os.fstat(fd); actual = os.read(fd, 4097); after = os.fstat(fd)
                                assert stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and actual == content
                                assert (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
                            finally:
                                os.close(fd)
                            report['observations'].setdefault('declared_files', []).append({'artifact_id': artifact_id, 'bytes': len(content),
                                'sha256': sha(content), 'observed_device': before.st_dev, 'observed_inode': before.st_ino,
                                'evidence_kind': 'independent bounded nofollow FD read; not a fabricated Core receipt'})
                        parent = [r for r in records if r['stage'].startswith('parent')]
                        assert len({r['owned_provider']['pid'] for r in parent}) == 2
                        assert parent[0]['request']['task_id'] == parent[1]['request']['task_id'] == task['id']
                    with sqlite3.connect('file:' + str(runtime / 'baton.sqlite3') + '?mode=ro', uri=True) as db:
                        report['observations']['actual_sqlite_schema'] = db.execute('PRAGMA user_version').fetchone()[0]
                        assert report['observations']['actual_sqlite_schema'] == 13
                        attempts = db.execute('SELECT count(*) FROM factory_attempts').fetchone()[0]
                        assert attempts == (3 if args.case == 'normal' else 1)
                        assert db.execute('SELECT count(*) FROM execution_grants').fetchone()[0] == 1
                        assert db.execute("SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'").fetchone()[0] == 0
                        budgets = [dict(zip(['scope', 'consumed', 'active'], row)) for row in db.execute('SELECT scope,consumed,active FROM budgets')]
                        report['observations']['budgets'] = budgets
                        verify_attempt_budgets(budgets, task['id'], project['id'], 3 if args.case == 'normal' else 1)
                        if args.case in {'normal', 'cancel'}:
                            assert db.execute("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'").fetchone()[0] == 0
                            assert db.execute("SELECT count(*) FROM workspace_leases WHERE holder!=''").fetchone()[0] == 0
                            assert db.execute('SELECT count(*) FROM factory_scoped_handles WHERE revoked=0').fetchone()[0] == 0
                        events = [json.loads(raw) for raw, in db.execute('SELECT document FROM events ORDER BY sequence')]
                        report['observations']['events'] = events
                        owned_stops = [e for e in events if e['kind'] == 'session.stopped' and e['payload'].get('process_reaped') is True and e['payload'].get('no_process_started') is False]
                        report['observations']['actual_owned_session_stops'] = owned_stops
                        # Even malformed-result unknown cannot claim active slots
                        # closed without these actual owned-reap event receipts.
                        assert len(owned_stops) == attempts
                    starts = [r for r in report['http'] if r['method'] == 'factory_runs.start']
                    assert len(starts) == 1 and not starts[0]['response'].get('error')
                    original_start = starts[0]
                    replay, _ = host.wire(FixedCall('factory_runs.start', original_start['params'], original_start['key'], original_start['revision']))
                    assert not replay.get('error') and replay['result'] == original_start['response']['result']
                    conflict, _ = host.wire(FixedCall('factory_runs.start', original_start['params'], original_start['key'], original_start['revision'] + 1))
                    assert conflict['error']['code'] == 'CONFIG_CONFLICT'
                    report['observations']['same_key_historical_start_replay'] = {'reply': replay, 'different_payload_error': conflict,
                        'dimension': 'ordinary same-key receipt replay; NOT a dropped-response test'}
                    passed('actual scoped MCP/owned attempts persist exact lineage and cumulative budgets; no TaskAccepted or new Grant')
                    panel.get_by_role('button', name='Refresh Factory records', exact=True).click()
                    region = panel.get_by_role('region', name='Factory run', exact=True)
                    expect(region).to_contain_text(terminal)
                    expect(panel.get_by_text('Completed Factory processing does not accept the task.', exact=True)).to_be_visible()
                    shot('02-real-' + terminal + '-task-history', region)
                    assert not errors and not report['http_unexpected_errors'], (errors, report['http_unexpected_errors'])
                    assert sha(Path(__file__).read_bytes()) == attribution['harness_sha256']
                    assert {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()} == assets
                    assert sha(binary.read_bytes()) == manifest['binaries']['batond']
                    assert sha(companion.read_bytes()) == report['private_installation']['tools']['baton']['installed_sha256']
                    assert sha(raw_binary.read_bytes()) == manifest['binaries']['batond'] and sha(raw_companion.read_bytes()) == manifest['binaries']['baton']
                    passed('actual built Factory UI shows terminal history without acceptance or source/asset drift')
                    report['status'] = 'passed'
                except BaseException as failure:
                    note_failure(report, failure, 'browser_runtime', clean)
                    diagnose()  # daemon still alive; first cause precedes capture/teardown
                    try:
                        shot('failure-current-ui', page.locator('body'))
                    except Exception as capture_failure:
                        report['failure_capture_error'] = clean(capture_failure)
                    raise
                finally:
                    phase['value'] = 'browser_closing'
                    cleanup_call(report, 'browser_close', browser.close, clean); browser = None
        except BaseException as failure:
            note_failure(report, failure, 'runtime_outer', clean)
            diagnose()
            if host is not None:
                try:
                    report['observations']['failure_snapshot'] = host.result('system.snapshot', {})
                    if scope is not None:
                        report['observations']['failure_factory_response'] = host.wire(FixedCall('factory_runs.get', {'scope_id': scope['id']}))[0]
                except Exception as read_failure:
                    report['observations']['failure_diagnostic_error'] = clean(read_failure)
        finally:
            pump = lambda: None
            if browser is not None:
                cleanup_call(report, 'browser_close_after_failure', browser.close, clean)
            if server is not None:
                cleanup_call(report, 'http_shutdown', server.shutdown, clean)
            http_complete = True
            if thread is not None:
                cleanup_call(report, 'http_thread_join', lambda: thread.join(timeout=5), clean)
                alive = cleanup_call(report, 'http_thread_inspection', thread.is_alive, clean)
                report['cleanup'].append({'action': 'HTTP thread join', 'thread_alive_after_join': alive})
                if alive is not False:
                    http_complete = False
                    note_failure(report, AssertionError('HTTP serve thread did not join'), 'http_thread_join_result', clean)
            if server is not None:
                cleanup_call(report, 'http_handlers_close', server.server_close, clean)
                remaining = list(active.values())
                report['cleanup'].append({'action': 'HTTP handlers joined', 'active_handlers': remaining})
                if remaining:
                    http_complete = False
                    note_failure(report, AssertionError('HTTP handlers remain active'), 'http_handlers_join_result', clean)
            def finish_daemon():
                if daemon.poll() is None:
                    daemon.terminate()
                forced = False
                try:
                    exitcode = daemon.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    forced = True; os.killpg(daemon.pid, signal.SIGKILL); exitcode = daemon.wait()
                report['cleanup'].append({'action': 'private daemon waited', 'pid': daemon.pid,
                    'exitcode': exitcode, 'forced_sigkill': forced})
                assert exitcode == 0 and not forced, 'Private daemon cleanup failed or needed forced kill'
                return True
            daemon_complete = cleanup_call(report, 'daemon_wait', finish_daemon, clean)
            cleanup_call(report, 'controller_bound_adopted_mcp_waits',
                lambda: [controller_reaper.reap(fixed) for fixed in verified_records], clean)
            identity_complete = cleanup_call(report, 'fixed_owned_identity_inspection',
                lambda: inspect_owned_records(report, verified_records, 3 if args.case == 'normal' else 1,
                    provider, settings, companion, runtime, clean,
                    require_full=report['status']=='passed' and not report.get('failure')), clean)
            if report['http_unexpected_errors']:
                note_failure(report, AssertionError('Unexpected HTTP errors'), 'http_final_error_check', clean)
            cleanup_call(report, 'daemon_log_close', log.close, clean)
            def redact_log():
                path = output / 'daemon.log'
                if token:
                    path.write_bytes(path.read_bytes().replace(token.encode(), b'[redacted generated Owner token]'))
            cleanup_call(report, 'daemon_log_redaction', redact_log, clean)
            report['cleanup_complete'] = bool(http_complete and daemon_complete is True and identity_complete is True
                and not report.get('failure') and not report['http_unexpected_errors'])
            if not report['cleanup_complete']:
                report['status'] = 'failed_or_interrupted'
    except BaseException as failure:
        note_failure(report, failure, 'runtime_setup_or_cleanup', clean)
    finally:
        if controller_reaper is not None:
            cleanup_call(report, 'test_controller_subreaper_restore', controller_reaper.restore, clean)
        cleanup_call(report, 'temporary_directory_cleanup', temporary_owner.cleanup, clean)
        report['cleanup_complete'] = report['cleanup_complete'] and not report.get('failure')
        if not report['cleanup_complete']:
            report['status'] = 'failed_or_interrupted'
        try:
            (output / 'report.json').write_text(clean(json.dumps(report, ensure_ascii=False, indent=2)) + '\n')
        except BaseException as write_failure:
            note_failure(report, write_failure, 'report_persistence', clean)
            raise RuntimeError('Could not persist failed report; first failure: ' + clean(report['failure'])) from write_failure
    print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'report': str(output / 'report.json')}))
    return runtime_exit(report)


def boundary_self_check():
    bounds = Bounds('project', 'task', 'scope')
    rows = []
    def valid(name, command, args, expected):
        actual = decode_envelope({'command': command, 'args': args}, bounds)
        assert actual == expected
        rows.append({'name': name, 'status': 'passed'})
    def invalid(name, envelope, status):
        try:
            decode_envelope(envelope, bounds)
        except BoundaryRefusal as failure:
            assert failure.status == status, (name, failure.status)
        else:
            raise AssertionError(name)
        rows.append({'name': name, 'status': 'passed'})
    for command, method in [('system_snapshot', 'system.snapshot'), ('projects_list', 'projects.list'),
            ('connections_list', 'connections.list'), ('sessions_list', 'sessions.list')]:
        valid(command, command, {}, FixedCall(method, {}))
    for command, camel, snake, identity, method in [
            ('factory_scopes_list', 'rootTaskId', 'root_task_id', 'task', 'factory_scopes.list'),
            ('factory_scopes_get', 'scopeId', 'scope_id', 'scope', 'factory_scopes.get'),
            ('factory_runs_get', 'scopeId', 'scope_id', 'scope', 'factory_runs.get'),
            ('workspaces_list', 'projectId', 'project_id', 'project', 'workspaces.list'),
            ('grants_list', 'projectId', 'project_id', 'project', 'grants.list'),
            ('goal_scopes_list', 'taskId', 'task_id', 'task', 'goal_scopes.list')]:
        valid(command, command, {camel: identity}, FixedCall(method, {snake: identity}))
        invalid(command + ' foreign', {'command': command, 'args': {camel: 'foreign'}}, 400)
        invalid(command + ' authority extra', {'command': command, 'args': {camel: identity, 'token': 'untrusted'}}, 400)
    for command, method in [('factory_runs_start', 'factory_runs.start'), ('factory_runs_cancel', 'factory_runs.cancel')]:
        args = {'scopeId': 'scope', 'expectedRevision': 3, 'idempotencyKey': 'fixed-key'}
        valid(command, command, args, FixedCall(method, {'scope_id': 'scope'}, 'fixed-key', 3))
        for label, patch in [('null key', {'idempotencyKey': None}), ('empty key', {'idempotencyKey': ''}),
                ('bad key', {'idempotencyKey': 'a\nkey'}), ('long key', {'idempotencyKey': 'a' * 129}),
                ('bool CAS', {'expectedRevision': True}), ('zero CAS', {'expectedRevision': 0}),
                ('float CAS', {'expectedRevision': 1.0}), ('unsafe CAS', {'expectedRevision': 2**53}),
                ('foreign scope', {'scopeId': 'foreign'}), ('opaque authority', {'token': 'bad'})]:
            invalid(command + ' ' + label, {'command': command, 'args': {**args, **patch}}, 400)
    for command in ['factory_scope_approve', 'factory_scopes_propose', 'factory_scopes_revoke',
            'approvals_finalize', 'approvals_decline', 'generic_ipc', 'sessions_dispatch',
            'delegate_task', 'get_task_status', 'get_task_result', 'cancel_task']:
        invalid('deny ' + command, {'command': command, 'args': {}}, 403)
    origin = 'http://127.0.0.1:43123'
    def web(raw, **patch):
        return decode_http(raw, origin=patch.get('origin', origin), host=patch.get('host', '127.0.0.1:43123'),
            content_type=patch.get('content_type', 'application/json'), expected_origin=origin, bounds=bounds)
    assert web(b'{"command":"system_snapshot"}') == FixedCall('system.snapshot', {})
    rows.append({'name': 'exact loopback HTTP positive', 'status': 'passed'})
    for name, raw, patch, expected in [('duplicate', b'{"command":"system_snapshot","command":"generic_ipc"}', {}, 400),
            ('UTF8', b'\xff', {}, 400), ('nonfinite', b'{"command":"x","args":{"x":NaN}}', {}, 400),
            ('oversize', b'x' * (MAX_HTTP_BYTES + 1), {}, 400), ('foreign Origin', b'{}', {'origin': 'http://example.invalid'}, 403),
            ('foreign Host', b'{}', {'host': 'example.invalid'}, 403), ('MIME', b'{}', {'content_type': 'text/plain'}, 400)]:
        try:
            web(raw, **patch)
        except BoundaryRefusal as failure:
            assert failure.status == expected
        else:
            raise AssertionError(name)
        rows.append({'name': name, 'status': 'passed'})
    return {'dimension': 'pure fixed HTTP boundary controls; no Core/browser/Provider',
        'status': 'passed', 'count': len(rows), 'checks': rows}


def fixture_self_check():
    """Pure review/AST and version-only local generated process, not Core."""
    rows = []
    def passed(name):
        rows.append({'name': name, 'status': 'passed'})
    compile(GENERATED_ACP, '<generated-account-free-acp>', 'exec')
    passed('generated ACP strict Python compiles')
    wire_path = Path(__file__).parents[1] / 'src-tauri/tests/fixtures/factory-wire-bundle-0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64.json'
    raw = wire_path.read_bytes()
    assert sha(raw) == '0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64'
    passed('saved actual Store bundle original UTF8 SHA verified, not a runtime replay')
    with tempfile.TemporaryDirectory(prefix='factory-review-unit-') as temporary:
        directory = Path(temporary); companion = directory / 'generated-companion-binding-unit'
        companion.write_bytes(b'Unit-only companion binding, never executed\n')
        wire = json.loads(raw); proposal, selection = wire['proposal'], wire['selection']
        bindings = proposal['snapshot']['bindings']
        # This modified DTO is explicitly a unit fixture, never a Core result.
        bindings['companion']['executable'] = str(companion)
        bindings['companion']['executable_sha256'] = sha(companion.read_bytes())
        expected = {key: json.loads(json.dumps(bindings[key])) for key in ['project', 'root_task', 'approved_grant', 'connections', 'workspaces']}
        def review(value):
            return review_factory(value, selection, expected['project'], expected['root_task'], expected['approved_grant'],
                expected['connections'][0], expected['workspaces'][0], companion)
        assert review(proposal)['proposal_id'] == proposal['id']
        passed('complete private Factory review unit accepts exact bound typed DTO')
        for name in ['kind', 'task', 'grant-pin', 'project', 'host', 'output-inode', 'implicit-start', 'input-hash', 'oversize']:
            changed = json.loads(json.dumps(proposal)); b = changed['snapshot']['bindings']
            if name == 'kind': changed['kind'] = 'goal'
            if name == 'task': b['root_task']['id'] = 'foreign'
            if name == 'grant-pin': b['approved_grant']['connection_bindings'][b['connections'][0]['id']]['config_revision'] = 'foreign'
            if name == 'project': del b['project']
            if name == 'host': b['connections'][0]['host_id'] = 'foreign'
            if name == 'output-inode': b['outputs'][0]['root_inode'] += 1
            if name == 'implicit-start': b['start_contract']['implicit_start_on_approval'] = True
            if name == 'input-hash': b['inputs'][0]['sha256'] = 'a' * 64
            if name == 'oversize': changed['padding'] = 'x' * MAX_FACTORY_REVIEW_BYTES
            try:
                review(changed)
            except (BoundaryRefusal, KeyError):
                passed('full private review refuses ' + name)
            else:
                raise AssertionError(name)
        program = directory / 'generated-acp-unit.py'; program.write_text(GENERATED_ACP)
        result = subprocess.run([sys.executable, str(program), '--version'], capture_output=True, timeout=2, check=True)
        assert result.stdout == b'Generated account-free Factory ACP 1\n' and result.stderr == b''
        passed('actual generated program version-only process exits; no MCP/Core/model started')
    return {'dimension': 'review DTO derivative + source compile + generated version-only unit; NOT actual Core/browser/Factory',
        'status': 'passed', 'count': len(rows), 'checks': rows}


def cleanup_self_check():
    rows = []
    def check(name, condition):
        assert condition, name
        rows.append({'name': name, 'status': 'passed'})
    def report():
        return {'status': 'passed', 'cleanup_complete': True, 'cleanup': [], 'http_unexpected_errors': []}
    def throw(message):
        raise RuntimeError(message)
    value = report()
    cleanup_call(value, 'browser_close', lambda: throw('unit-only browser.close failed'))
    check('original late-exception counterexample now fails and returns nonzero', value['status'] == 'failed_or_interrupted' and runtime_exit(value) == 1)
    value = report(); note_failure(value, ValueError('original failure'), 'body')
    cleanup_call(value, 'browser_close', lambda: throw('secondary cleanup failed'))
    check('primary failure survives later cleanup exception', value['failure']['message'] == 'original failure' and value['secondary_failures'][0]['message'] == 'secondary cleanup failed')
    value = report(); value['cleanup_complete'] = False
    check('no complete cleanup cannot return green', runtime_exit(value) == 1)
    value = report(); value['failure'] = {'message': 'recorded error'}
    check('recorded failure forbids green even if status was incorrectly reset', runtime_exit(value) == 1)
    with tempfile.TemporaryDirectory(prefix='factory-cleanup-unit-') as temporary:
        root = Path(temporary); path = root / 'fixture-record.json'; path.write_text('{"unit":true}\n')
        fixed = {'path': str(path), 'sha256': sha(path.read_bytes()),
            'file_pin': bounded_file(path, MAX_HTTP_BYTES)[1],
            'owned_provider': {'pid': 800000001, 'pgid': 800000001, 'start_ticks': '1'},
            'owned_companion': {'pid': 800000002, 'pgid': 800000001, 'start_ticks': '2'}}
        original = support.proc_identity
        support.proc_identity = lambda _: None  # Explicit metadata stub, never process-stop proof.
        try:
            def inspect(saved, required=1):
                value = report()
                complete = inspect_owned_records(value, saved, required, root/'provider', root/'settings', root/'baton', root/'runtime')
                value['cleanup_complete'] = complete
                return value
            value = inspect([])
            check('original empty-cleanup counterexample denies missing required records', runtime_exit(value) == 1 and value['cleanup'][-1]['complete'] is False)
            value = inspect([fixed])
            check('complete fixed record inspections pass only explicit mocked-gone identities', runtime_exit(value) == 0 and value['cleanup'][-1]['inspected_identities'] == 2)
            path.unlink(); value = inspect([fixed])
            check('removed previously verified file fails instead of empty glob success', runtime_exit(value) == 1)
            path.write_text('changed'); value = inspect([fixed])
            check('changed verified record hash fails', runtime_exit(value) == 1)
            path.write_text('{"unit":true}\n'); missing = dict(fixed); del missing['owned_companion']
            value = inspect([missing])
            check('incomplete fixed identity pair fails', runtime_exit(value) == 1)
            value = inspect([fixed], required=3)
            check('normal requires all three provider/companion records', runtime_exit(value) == 1)
        finally:
            support.proc_identity = original
    for expected in [1, 3]:
        valid = [{'scope': 'root:task', 'consumed': expected, 'active': 0}, {'scope': 'project:project', 'consumed': expected, 'active': 0}]
        verify_attempt_budgets(valid, 'task', 'project', expected)
        check('exact root+project positive consumed' + str(expected), True)
        for name in ['missing-project', 'wrong-project-key', 'wrong-project-consumed', 'wrong-project-active', 'extra-row', 'missing-root', 'root-refund', 'bool-consumed']:
            changed = json.loads(json.dumps(valid))
            if name == 'missing-project': changed.pop()
            if name == 'wrong-project-key': changed[1]['scope'] = 'project:foreign'
            if name == 'wrong-project-consumed': changed[1]['consumed'] = expected + 1
            if name == 'wrong-project-active': changed[1]['active'] = 1
            if name == 'extra-row': changed.append(dict(changed[1]))
            if name == 'missing-root': changed.pop(0)
            if name == 'root-refund': changed[0]['consumed'] = 0
            if name == 'bool-consumed': changed[0]['consumed'] = True
            try:
                verify_attempt_budgets(changed, 'task', 'project', expected)
            except AssertionError:
                check(name + ' consumed' + str(expected), True)
            else:
                raise AssertionError(name)
    return {'dimension': 'real report/cleanup/budget helper control execution, explicit gone-process metadata stub; no Core/browser/daemon',
        'status': 'passed', 'count': len(rows), 'checks': rows}


def diagnostic_self_check():
    checks=[]
    def check(name, condition=True):
        assert condition, name; checks.append({'name':name,'status':'passed'})
    with tempfile.TemporaryDirectory(prefix='factory-diagnostic-unit-') as temporary:
        root=Path(temporary); output=root/'evidence'; output.mkdir(mode=0o700)
        fifo=root/'record-fifo'; os.mkfifo(fifo,0o600)
        try: bounded_file(fifo,1024,root)
        except AssertionError: check('FIFO without writer opens nonblocking then fails regular-file check')
        else: raise AssertionError('FIFO was accepted')
        with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as listener:
            unix_socket=root/'record-socket'; listener.bind(str(unix_socket))
            try: bounded_file(unix_socket,1024,root)
            except OSError: check('Unix socket path cannot be accepted as record file')
            else: raise AssertionError('Socket was accepted')
        installed={}; source=Path('/usr/bin/true').resolve(strict=True); original=sha(source.read_bytes())
        daemon,cli=install_private_tools(source,source,root/'installed',installed)
        check('real strip tool acts only on exclusive private CLI copy', installed['strip_argv'][-1]==str(cli) and not cli.samefile(source))
        check('original public ELF source unchanged', sha(source.read_bytes())==original)
        check('private daemon copy bytes unchanged', sha(daemon.read_bytes())==original)
        check('private installed tools/directory modes700', all(stat.S_IMODE(p.stat().st_mode)==0o700 for p in [daemon,cli,cli.parent]))
        runtime=root/'synthetic-runtime';runtime.mkdir(mode=0o700)
        workspace=root/'workspace';workspace.mkdir(mode=0o700);reports=workspace/'reports';reports.mkdir(mode=0o700)
        provider=root/'synthetic-provider';provider.write_bytes(b'Not executed: unit fixture only\n')
        settings=root/'synthetic-settings';settings.write_text('{"nonce":"unit-nonce"}\n')
        request={'schema_version':1,'kind':'agentbaton_factory_task_request','task_id':'task','root_task_id':'task',
            'parent_task_id':None,'goal':'unit','input_artifacts':[],'output_artifacts':[],'handoff_result':None,
            'handoff_results':[],'handoff_digest':None,'context_continuation':'new_attempt_explicit_handoff_only','acceptance_state':'not_established'}
        text=json.dumps(request)
        pid=800000001; native='generated-factory-'+str(pid)+'-unit-nonce'
        session={'id':'session','native_session_id':native,'task_id':'task','workspace_id':'workspace',
            'connection_id':'connection','grant_id':'grant','attempt_id':'attempt'}
        attempt={'attempt_id':'attempt','task_id':'task','root_task_id':'task','run_id':'run','session_id':'session'}
        database=runtime/'baton.sqlite3'
        # Deliberately synthetic schema0 data, solely testing readonly helpers;
        # never a real Core/approval/Provider fixture or execution success.
        with sqlite3.connect(database) as db:
            db.executescript('CREATE TABLE sessions(id TEXT,document TEXT); CREATE TABLE session_turns(session_id TEXT,document TEXT); CREATE TABLE factory_attempts(session_id TEXT,scope_id TEXT,document TEXT); CREATE TABLE events(sequence INTEGER,document TEXT);')
            db.execute('INSERT INTO sessions VALUES(?,?)',('session',json.dumps(session)))
            db.execute('INSERT INTO session_turns VALUES(?,?)',('session',json.dumps({'text':text})))
            db.execute('INSERT INTO factory_attempts VALUES(?,?,?)',('session','scope',json.dumps(attempt)))
            db.execute('INSERT INTO events VALUES(?,?)',(1,json.dumps({'kind':'unit','token':'unit-owner-secret','fencing_token':7})))
        stderr=reports/'fixture-stderr-800000001.txt';stderr.write_text('unit-owner-secret trace preserved\n')
        before=sha(database.read_bytes());info,data=diagnostic_snapshot(runtime,{'root':str(workspace)},output,'unit-owner-secret')
        saved=Path(info['path']).read_text()
        check('bounded readonly diagnostics retain all synthetic journal/session rows', info['complete'] and data['tables']['events']['row_count']==1 and data['tables']['sessions']['row_count']==1)
        check('generated stderr retained', len(data['generated_stderr'])==1 and 'trace preserved' in data['generated_stderr'][0]['text'])
        check('only generated token redacted, business fencing token retained', 'unit-owner-secret' not in saved and '"fencing_token": 7' in data['tables']['events']['rows'][0]['document'])
        check('diagnostic query does not change database bytes', sha(database.read_bytes())==before)
        check('diagnostic artifact private600 and not database replay', stat.S_IMODE(Path(info['path']).stat().st_mode)==0o600 and info['retained_database_bytes'] is False)
        credentials=runtime/'factory-scoped-handles'; credentials.mkdir(mode=0o700)
        credential=credentials/'unit.json'; credential.write_text('invalid-sensitive-unit-text')
        info,data=diagnostic_snapshot(runtime,{'root':str(workspace)},output,'unit-owner-secret')
        check('invalid credential marks diagnostics incomplete without raw content', not info['complete'] and any(e.get('type')=='JSONDecodeError' for e in data['errors']) and 'invalid-sensitive-unit-text' not in json.dumps(data))
        original_reader=bounded_file
        try:
            def denied_reader(path,*args):
                if path==credential: raise PermissionError('sensitive-permission-unit-text')
                return original_reader(path,*args)
            globals()['bounded_file']=denied_reader
            info,data=diagnostic_snapshot(runtime,{'root':str(workspace)},output,'unit-owner-secret')
            check('credential permission error is incomplete and does not leak exception content', not info['complete'] and any(e.get('type')=='PermissionError' for e in data['errors']) and 'sensitive-permission-unit-text' not in json.dumps(data))
            def missing_reader(path,*args):
                if path==credential: raise FileNotFoundError('deleted generated credential')
                return original_reader(path,*args)
            globals()['bounded_file']=missing_reader
            info,data=diagnostic_snapshot(runtime,{'root':str(workspace)},output,'unit-owner-secret')
            check('deleted credential race does not erase otherwise complete diagnostics', info['complete'] and not data['errors'])
        finally: globals()['bounded_file']=original_reader
        credential.unlink()
        for i in range(257):
            write_private_json(credentials/(str(i)+'.json'),{'socket_path':str(runtime/'batond.sock'),'scoped_token':'unit-scoped-secret'})
        info,data=diagnostic_snapshot(runtime,{'root':str(workspace)},output,'unit-owner-secret')
        check('credential entry cap explicitly marks diagnostics incomplete', not info['complete'] and any(e.get('type')=='EntryLimit' for e in data['errors']))
        scope={'id':'scope','root_task_id':'task','root_grant_id':'grant'}
        context={'nonce':'unit-nonce','program_sha256':sha(provider.read_bytes()),'settings_sha256':sha(settings.read_bytes()),
            'connection_id':'connection','run_id':'run','provider':str(provider),'settings':str(settings),
            'companion':str(cli),'companion_sha256':sha(cli.read_bytes())}
        row={'fixture_nonce':'unit-nonce','program_sha256':context['program_sha256'],'settings_sha256':context['settings_sha256'],
            'native_session_id':native,'request_text':text,'request_sha256':sha(text.encode()),'stage':'parent-initial',
            'owned_provider':{'pid':pid,'pgid':pid,'start_ticks':'1'},'owned_companion':{'pid':pid+1,'pgid':pid,'start_ticks':'2'},
            'provider_argv':[str(provider),'--fixture-settings',str(settings)],
            'companion_argv':[str(cli),'factory-mcp',str(runtime/'factory-scoped-handles/unit.json')]}
        record=reports/('fixture-request-'+str(pid)+'.json');write_private_json(record,row)
        original_probe=support.proc_identity;support.proc_identity=lambda _:None
        try:
            verified=[];rows=collect_started_records(runtime,{'id':'workspace','root':str(workspace)},scope,context,verified,output)
            check('record accepted only after matching synthetic Core session/attempt/raw turn', len(rows)==1 and len(verified)==1 and verified[0]['session_id']=='session')
            collect_started_records(runtime,{'id':'workspace','root':str(workspace)},scope,context,verified,output)
            check('same FD/content record is not counted twice', len(verified)==1)
            failed={'status':'failed_or_interrupted','cleanup':[],'cleanup_complete':False,'http_unexpected_errors':[],
                'failure':{'message':'original failed workflow'}}
            complete=inspect_owned_records(failed,verified,3,provider,settings,cli,runtime,require_full=False)
            check('failed one-start workflow reports actual2 identities, not fabricated6', complete is False and failed['cleanup'][-1]['actual_bound_identities']==2 and failed['cleanup'][-1]['inspected_identities']==2)
            check('partial failed workflow never becomes cleanup complete or green', runtime_exit(failed)==1 and failed['failure']['message']=='original failed workflow')
            for name in ['nonce','program','request','argv','symlink']:
                changed=json.loads(json.dumps(row)); record.unlink()
                if name=='nonce':changed['fixture_nonce']='foreign'
                if name=='program':changed['program_sha256']='f'*64
                if name=='request':changed['request_text']=text+' ';changed['request_sha256']=sha(changed['request_text'].encode())
                if name=='argv':changed['provider_argv']=['foreign']
                if name=='symlink':record.symlink_to(stderr)
                else:write_private_json(record,changed)
                try:collect_started_records(runtime,{'id':'workspace','root':str(workspace)},scope,context,[],output)
                except (AssertionError,OSError,KeyError):check('unverified '+name+' record cannot authorize identity cleanup')
                else:raise AssertionError(name)
            record.unlink();write_private_json(record,row)
            try:collect_started_records(runtime,{'id':'workspace','root':str(workspace)},scope,context,verified,output)
            except AssertionError:check('same-content replacement cannot replace previously bound FD identity')
            else:raise AssertionError('replacement FD')
        finally:support.proc_identity=original_probe
    return {'dimension':'real private copy/strip and readonly helper execution; synthetic schema0 records + explicit gone-metadata stub, NO Core/browser/Provider',
        'status':'passed','count':len(checks),'checks':checks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--fixture-self-check', action='store_true')
    parser.add_argument('--cleanup-self-check', action='store_true')
    parser.add_argument('--diagnostic-self-check', action='store_true')
    parser.add_argument('--case', choices=['normal', 'cancel', 'unknown'])
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--candidate-manifest', type=Path)
    parser.add_argument('--batond', type=Path)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    assert sum([args.self_check, args.fixture_self_check, args.cleanup_self_check, args.diagnostic_self_check, bool(args.case)]) <= 1
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    attribution = {'harness_sha256': sha(Path(__file__).read_bytes()), 'support_sha256': sha(SUPPORT_PATH.read_bytes()), 'schema_required': 13}
    if args.self_check:
        report = {**attribution, **boundary_self_check()}
        (output / 'boundary-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.fixture_self_check:
        report = {**attribution, **fixture_self_check()}
        (output / 'fixture-unit-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.cleanup_self_check:
        report = {**attribution, **cleanup_self_check()}
        (output / 'cleanup-unit-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.diagnostic_self_check:
        report = {**attribution, **diagnostic_self_check()}
        (output / 'diagnostic-unit-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.case:
        try:
            prepared = preflight(args)
        except (AssertionError, OSError, ValueError, KeyError) as failure:
            report = {**attribution, 'status': 'preflight_refused', 'actual_runtime': 'not_run',
                'reason': str(failure), 'no_process_token_read_or_listener': True}
            (output / 'preflight-refusal.json').write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps(report)); return 2
        return runtime_driver(args, output, attribution, prepared)
    report = {**attribution, 'status': 'pending_dependencies', 'actual_runtime': 'not_run', 'pending': list(PENDING)}
    (output / 'pending-runtime.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report)); return 2


if __name__ == '__main__':
    raise SystemExit(main())
