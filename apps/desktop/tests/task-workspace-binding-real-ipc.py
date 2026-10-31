#!/usr/bin/env python3
"""Private built-App/Chromium -> fixed HTTP -> real schema16 Owner Unix IPC.

No Provider, approval, generic RPC, fake success, or public evidence projection.
Only --run starts processes. Default preflight exits 2 (runtime NOT_RUN).
All output is private generated-fixture evidence, never an upload allowlist.
"""
from __future__ import annotations
import argparse
import ctypes
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
import tempfile
import threading
import time
import uuid
from functools import partial

MAX_BODY = 32_768
MAX_REPLY = 4_194_304
MAX_SAFE = 9_007_199_254_740_991
BASE = '840334e2507c7b2b39e89917b491e75c1ca3de18'
ENDPOINT = '/__test_task_workspace'
SELECTION_KEYS = {'task_id', 'expected_task_revision', 'expected_spec_revision',
                  'expected_project_revision', 'host_id', 'expected_source_pin', 'preparation_key'}
PIN_KEYS = {'canonical_root', 'device', 'inode', 'common_dir', 'head'}
READS = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list',
         'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def unique(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate JSON key')
        out[key] = value
    return out


def decode(raw):
    return json.loads(raw, object_pairs_hook=unique,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))


def exact(a, b):
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(exact(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(exact(x, y) for x, y in zip(a, b))
    return a == b


def integer(value, minimum=1):
    return type(value) is int and minimum <= value <= MAX_SAFE


def require(value, message='Fixed fixture boundary refused'):
    if not value:
        raise ValueError(message)


def private_json(path, value):
    raw = (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def file_digest(path, limit=134_217_728, *, dir_fd=None, allow_empty=False, expected_identity=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=dir_fd)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and (0 if allow_empty else 1) <= before.st_size <= limit)
        require(expected_identity is None or expected_identity == (
            before.st_dev, before.st_ino, before.st_mode, before.st_uid, before.st_nlink,
            before.st_size, before.st_mtime_ns, before.st_ctime_ns))
        h = hashlib.sha256()
        if expected_identity is None:
            while block := stream.read(1_048_576):
                h.update(block)
        else:
            # Only the new image path: bound actual reads even if a file grows.
            # One extra byte detects overflow; never follow an unbounded EOF.
            observed = 0
            while block := stream.read(min(1_048_576, before.st_size - observed + 1)):
                observed += len(block)
                require(observed <= before.st_size and observed <= limit)
                h.update(block)
            require(observed == before.st_size)
        after = os.fstat(stream.fileno())
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) ==
                (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns))
        return {'sha256': h.hexdigest(), 'bytes': before.st_size, 'mode': stat.S_IMODE(before.st_mode)}


def assets(frontend):
    require(frontend.is_dir() and not frontend.is_symlink())
    paths = sorted(frontend.rglob('*'))
    require(len(paths) <= 4096 and not any(p.is_symlink() for p in paths))
    rows = {str(p.relative_to(frontend)): file_digest(p, 64 * 1024 * 1024)
            for p in paths if p.is_file()}
    require('index.html' in rows and sum(v['bytes'] for v in rows.values()) <= 64 * 1024 * 1024)
    return rows


class Boundary:
    """Finite fixture scope; never chooses an RPC method from browser text."""
    def __init__(self, project, tasks, source_pin):
        self.project = project
        self.tasks = {task['id']: task for task in tasks}
        self.pin = source_pin
        self.resolved = {}
        self.writes = {}
        self.lock = threading.Lock()

    def route(self, command, args):
        require(type(command) is str and type(args) is dict)
        if command in READS:
            require(args == {})
            return READS[command], {}, None, None
        if command == 'events_subscribe':
            require(set(args) == {'cursor', 'limit'} and integer(args['cursor'], 0)
                    and type(args['limit']) is int and args['limit'] == 100)
            return 'events.subscribe', dict(args), None, None
        if command in ('workspaces_list', 'grants_list'):
            require(set(args) == {'projectId'} and args['projectId'] == self.project['id'])
            method = 'workspaces.list' if command == 'workspaces_list' else 'grants.list'
            return method, {'project_id': self.project['id']}, None, None
        if command == 'tasks_get':
            require(set(args) == {'taskId'} and args['taskId'] in self.tasks)
            return 'tasks.get', {'task_id': args['taskId']}, None, None
        if command == 'workspaces_resolve':
            require(set(args) == {'selection'})
            s = args['selection']
            require(type(s) is dict and set(s) == {'task_id', 'expected_task_revision',
                                                'expected_project_revision', 'host_id'})
            require(s['task_id'] in self.tasks and s['host_id'] == 'local')
            task = self.tasks[s['task_id']]
            require(integer(s['expected_task_revision']) and s['expected_task_revision'] == task['revision'])
            require(integer(s['expected_project_revision']) and s['expected_project_revision'] == self.project['revision'])
            return 'workspaces.resolve', dict(s), None, None
        if command != 'workspaces_prepare_task_git':
            raise PermissionError('Command not in fixed bridge')
        require(set(args) == {'selection', 'idempotencyKey'})
        s, key = args['selection'], args['idempotencyKey']
        require(type(s) is dict and set(s) == SELECTION_KEYS)
        require(s['task_id'] in self.tasks and s['host_id'] == 'local')
        require(type(key) is str and bool(re.fullmatch(r'[A-Za-z0-9:._-]{1,256}', key)))
        require(type(s['expected_source_pin']) is dict and set(s['expected_source_pin']) == PIN_KEYS
                and exact(s['expected_source_pin'], self.pin))
        require(all(integer(s[k]) for k in ('expected_task_revision', 'expected_spec_revision', 'expected_project_revision')))
        require(type(s['preparation_key']) is str and bool(re.fullmatch(r'code-prep:[0-9a-f]{64}', s['preparation_key'])))
        with self.lock:
            old = self.writes.get(s['task_id'])
            if old is not None:
                # No upgraded Task CAS, alternative key, or changed recovery body.
                require(key == old['key'] and exact(s, old['selection']))
            else:
                p = self.resolved.get(s['task_id'])
                require(p is not None and p['state'] == 'needs_git_workspace')
                require(s['expected_task_revision'] == p['task_revision']
                        and s['expected_spec_revision'] == p['spec_revision']
                        and s['expected_project_revision'] == p['project_revision']
                        and s['preparation_key'] == p['preparation_key'])
                self.writes[s['task_id']] = {'key': key, 'selection': decode(json.dumps(s))}
        params = {k: v for k, v in s.items() if k != 'expected_task_revision'}
        return 'workspaces.prepare_task_git', params, key, s['expected_task_revision']

    def observe(self, method, params, reply):
        if 'error' in reply:
            return
        result = reply['result']
        if method == 'workspaces.resolve':
            require(result['execution_authorized'] is False and result['creates_workspace'] is False)
            require(result['task_id'] == params['task_id'])
            with self.lock:
                self.resolved[params['task_id']] = result
        if method == 'workspaces.prepare_task_git':
            require(result['identity_revalidated'] is True and result['execution_authorized'] is False)
            require(result['binding']['state'] == 'ready')
            require(result['original_receipt']['task_id'] == params['task_id'])



class ManagedFixturePaths:
    """Constrain test FS effects independently of any daemon-returned pathname.

    This is a generated Linux fixture namespace, not a product authority/sandbox.
    Parent directory descriptors/identity and atomic NOREPLACE protect writes;
    returned strings alone never grant access to another directory.
    """
    def __init__(self, runtime, project_id, source_pin):
        self.runtime = runtime
        self.managed = runtime / 'workspaces'
        self.project_id = project_id
        self.source_pin = source_pin
        self.parents = {}
        for path in (runtime.parent, runtime, self.managed):
            info = path.lstat()
            require(stat.S_ISDIR(info.st_mode) and not path.is_symlink()
                    and path.resolve(strict=True) == path
                    and stat.S_IMODE(info.st_mode) == 0o700)
            self.parents[path] = (info.st_dev, info.st_ino)

    def check_parents(self):
        for path, pin in self.parents.items():
            info = path.lstat()
            require(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
                    and (info.st_dev, info.st_ino) == pin and path.resolve(strict=True) == path)

    def workspace(self, binding, task_id):
        self.check_parents()
        require(type(binding) is dict and binding.get('state') == 'ready')
        receipt, pin = binding['receipt'], binding['managed_pin']
        require(receipt['task_id'] == task_id and receipt['project_id'] == self.project_id
                and receipt['host_id'] == 'local' and exact(receipt['source_pin'], self.source_pin)
                and receipt['managed_root'] == str(self.managed))
        workspace_id = receipt['workspace_id']
        require(type(workspace_id) is str and bool(re.fullmatch(r'[A-Za-z0-9_-]{1,128}', workspace_id)))
        expected = self.managed / workspace_id
        require(type(pin) is dict and pin['canonical_root'] == str(expected)
                and pin['common_dir'] == self.source_pin['common_dir'])
        info = expected.lstat()
        require(stat.S_ISDIR(info.st_mode) and not expected.is_symlink()
                and expected.resolve(strict=True) == expected)
        # Original opaque canonical strings, never JSON numeric/coercion equality.
        require(type(pin['device']) is str and type(pin['inode']) is str
                and pin['device'] == str(info.st_dev) and pin['inode'] == str(info.st_ino))
        return expected

    def directory_fd(self, path, expected_pin):
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            info = os.fstat(fd)
            require(stat.S_ISDIR(info.st_mode) and (info.st_dev, info.st_ino) == expected_pin)
            return fd
        except BaseException:
            os.close(fd)
            raise

    def workspace_fd(self, binding, task_id):
        path = self.workspace(binding, task_id)
        pin = binding['managed_pin']
        return self.directory_fd(path, (int(pin['device']), int(pin['inode'])))

    def image(self, binding, task_id):
        """Bounded own-worktree bytes, including the .git file, never its target.

        Shared common-dir refs/worktrees legitimately grow when Task3 is prepared;
        they are outside these two generated worktree roots and are not traversed.
        """
        rows, total = {}, 0
        def pin(info):
            return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink,
                    info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        def visit(fd, prefix):
            nonlocal total
            before = os.fstat(fd)
            require(stat.S_ISDIR(before.st_mode) and before.st_uid == os.geteuid())
            names = os.listdir(fd)
            require(len(names) <= 4096)
            for name in sorted(names):
                require(type(name) is str and name not in ('', '.', '..') and '/' not in name)
                relative = prefix + name
                require(len(relative.encode()) <= 4096 and len(rows) < 4096)
                info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                require(info.st_uid == os.geteuid())
                if stat.S_ISDIR(info.st_mode):
                    child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        require(pin(os.fstat(child)) == pin(info))
                        rows[relative] = {'directory': True, 'pin': pin(info)}
                        visit(child, relative + '/')
                    finally:
                        os.close(child)
                else:
                    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1)
                    total += info.st_size
                    require(total <= 64 * 1024 * 1024)
                    rows[relative] = {**file_digest(name, 64 * 1024 * 1024, dir_fd=fd, allow_empty=True, expected_identity=pin(info)), 'pin': pin(info)}
                require(pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == pin(info))
            require(pin(os.fstat(fd)) == pin(before) and sorted(os.listdir(fd)) == sorted(names))
        fd = self.workspace_fd(binding, task_id)
        try:
            visit(fd, '')
            self.workspace(binding, task_id)
            require('.git' in rows and rows['.git'].get('directory') is not True)
            return rows
        finally:
            os.close(fd)

    def committed_input(self, binding, task_id):
        fd = self.workspace_fd(binding, task_id)
        try:
            git_info = os.stat('.git', dir_fd=fd, follow_symlinks=False)
            require(stat.S_ISREG(git_info.st_mode), 'Generated worktree must have a regular gitfile')
            input_fd = os.open('input.txt', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
            with os.fdopen(input_fd, 'rb') as stream:
                info = os.fstat(stream.fileno())
                require(stat.S_ISREG(info.st_mode) and info.st_size <= 1024)
                require(stream.read(1025) == b'committed source\n')
            try:
                os.stat('untracked.txt', dir_fd=fd, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise ValueError('Source untracked file was copied to managed worktree')
        finally:
            os.close(fd)

    def write_output(self, binding, task_id):
        fd = self.workspace_fd(binding, task_id)
        try:
            out = os.open('own-output.txt', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                          0o600, dir_fd=fd)
            with os.fdopen(out, 'wb') as stream:
                stream.write(b'only Task one')
                stream.flush()
                os.fsync(stream.fileno())
        finally:
            os.close(fd)

    def output_absent(self, binding, task_id):
        fd = self.workspace_fd(binding, task_id)
        try:
            try:
                os.stat('own-output.txt', dir_fd=fd, follow_symlinks=False)
            except FileNotFoundError:
                return True
            return False
        finally:
            os.close(fd)

    def replace_for_drift(self, binding, task_id):
        original = self.workspace(binding, task_id)
        retained = original.with_name(original.name + '-retained-fixture')
        require(retained.parent == self.managed and retained != original)
        parent_fd = self.directory_fd(self.managed, self.parents[self.managed])
        original_fd = None
        try:
            original_fd = self.workspace_fd(binding, task_id)
            try:
                os.stat(retained.name, dir_fd=parent_fd, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise ValueError('Retained fixture destination already exists')
            # No overwrite window: Linux renameat2 RENAME_NOREPLACE, never fallback
            # to ordinary replace/rename on unsupported systems.
            libc = ctypes.CDLL(None, use_errno=True)
            rename = libc.renameat2
            rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
            rename.restype = ctypes.c_int
            if rename(parent_fd, os.fsencode(original.name), parent_fd, os.fsencode(retained.name), 1) != 0:
                error = ctypes.get_errno()
                raise OSError(error, 'Atomic private fixture rename refused')
            # The inode actually moved must be the one verified/opened above.
            moved = os.stat(retained.name, dir_fd=parent_fd, follow_symlinks=False)
            pinned = os.fstat(original_fd)
            require((moved.st_dev, moved.st_ino) == (pinned.st_dev, pinned.st_ino))
            os.mkdir(original.name, 0o700, dir_fd=parent_fd)
            self.check_parents()
            return retained
        finally:
            if original_fd is not None:
                os.close(original_fd)
            os.close(parent_fd)


def parse_http(raw, headers, origin):
    require(headers.get('Origin') == origin and headers.get('Host') == origin.removeprefix('http://'))
    require(headers.get('Content-Type') == 'application/json')
    require(0 < len(raw) <= MAX_BODY)
    body = decode(raw)
    require(type(body) is dict and set(body) <= {'command', 'args'} and 'command' in body)
    return body['command'], body.get('args', {})


def db_image(runtime):
    """All relational rows, including empty tables, sequence and FK state; never export DB bytes."""
    with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True, timeout=3) as db:
        db.execute('PRAGMA query_only=ON')
        db.execute('BEGIN')
        require(db.execute('PRAGMA user_version').fetchone()[0] == 16)
        names = [r[0] for r in db.execute("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")]
        rows = {}
        for name in names:
            require(bool(re.fullmatch(r'[A-Za-z0-9_]+', name)))
            data = db.execute('SELECT * FROM "' + name + '"').fetchall()
            require(len(data) <= 10_000)
            rows[name] = sorted(repr(row) for row in data)
        require(not db.execute('PRAGMA foreign_key_check').fetchall())
        return rows


def no_execution(runtime):
    rows = db_image(runtime)
    for name in ('connections', 'execution_grants', 'attempts', 'dispatch_intents', 'sessions',
                 'workspace_leases', 'budgets', 'factory_scopes', 'factory_runs', 'approval_proposals'):
        require(name in rows and rows[name] == [], 'Unexpected execution/authority row')
    return {'schema': 16, 'no_execution_or_grants': True,
            'table_counts': {k: len(v) for k, v in rows.items()}}


def source_image(source, git):
    # Registered source content/index/HEAD, excluding legitimate linked-worktree metadata.
    rows = {}
    for p in sorted(source.rglob('*')):
        rel = str(p.relative_to(source))
        if rel == '.git' or rel.startswith('.git/'):
            continue
        require(not p.is_symlink())
        rows[rel] = 'directory' if p.is_dir() else file_digest(p)['sha256']
    rows['git:index'] = file_digest(source / '.git/index')['sha256']
    rows['git:HEAD'] = git('rev-parse', 'HEAD')
    rows['git:status'] = git('status', '--porcelain=v1', '--untracked-files=all')
    return rows



def task_read_projection(task):
    require(type(task) is dict and type(task.get('id')) is str
            and integer(task.get('revision')) and integer(task.get('spec_revision')))
    binding = task.get('code_workspace_binding')
    workspace_id = None
    if binding is not None:
        require(type(binding) is dict and type(binding.get('receipt')) is dict)
        require(binding['receipt']['task_id'] == task['id'])
        workspace_id = binding['receipt']['workspace_id']
        require(type(workspace_id) is str)
    return {'task_id': task['id'], 'revision': task['revision'], 'workspace_id': workspace_id,
            'task_sha256': sha(json.dumps(task, sort_keys=True).encode())}


def confirmed_canvas_snapshot(entries, start, task):
    """Explicit post-setup App refresh must really deliver this exact new Task."""
    rows = [entry for entry in entries[start:] if entry['method'] == 'system.snapshot']
    if not rows:
        return None
    latest = rows[-1]
    if latest.get('reply_complete') is not True or latest.get('phase') != 'complete':
        return None
    require(latest.get('result') == 'result')
    tasks = latest.get('snapshot_task_projections')
    require(type(tasks) is list and len(tasks) <= 32 and all(type(row) is dict for row in tasks))
    matching = [row for row in tasks if row.get('task_id') == task['id']]
    if not matching:
        return None
    require(len(matching) == 1 and exact(matching[0], task_read_projection(task)),
            'New canvas snapshot Task differs from real private setup')
    return matching[0]


def confirmed_dialog_task_read(entries, start, task):
    """Require a new, fully delivered real TaskGet; never infer from snapshot/old UI state."""
    relevant = [entry for entry in entries[start:] if entry['method'] == 'tasks.get' and entry['task_id'] == task['id']]
    if not relevant:
        return None
    latest = relevant[-1]
    if latest.get('reply_complete') is not True or latest.get('phase') != 'complete':
        return None
    require(latest.get('result') == 'result' and exact(latest.get('task_projection'), task_read_projection(task)),
            'Fresh dialog TaskGet does not match current real Task/binding')
    return latest['task_projection']


def confirmed_dialog_scope_reads(entries, start, project_id, workspace_id):
    """New delivered scope reads, not a parent snapshot or an in-flight reply."""
    proofs = {}
    for method in ('workspaces.list', 'grants.list'):
        matching = [e for e in entries[start:] if e['method'] == method and e.get('project_id') == project_id]
        if not matching:
            return None
        row = matching[-1]
        if row.get('reply_complete') is not True or row.get('phase') != 'complete':
            return None
        require(row.get('result') == 'result')
        proof = row.get('scope_projection')
        require(type(proof) is dict and proof.get('project_id') == project_id)
        if method == 'workspaces.list':
            require(type(proof.get('workspace_ids')) is list
                    and all(type(i) is str for i in proof['workspace_ids'])
                    and proof['workspace_ids'].count(workspace_id) == 1)
        else:
            require(type(proof.get('grant_count')) is int and proof['grant_count'] == 0)
        proofs[method] = proof
    return proofs


def confirmed_entry_replay(entries, start, task, binding):
    """Only the observed original prepare and its explicit post-loss exact replay."""
    rows = [e for e in entries[start:] if e['method'] == 'workspaces.prepare_task_git']
    require(len(rows) == 2 and all(e.get('task_id') == task['id'] for e in rows))
    first, second = rows
    digest = sha(json.dumps(binding['receipt'], sort_keys=True).encode())
    require(type(first.get('request_sha256')) is str and re.fullmatch(r'[0-9a-f]{64}', first['request_sha256'])
            and first['request_sha256'] == second.get('request_sha256'))
    require(first.get('phase') == 'intentional_post_commit_response_drop' and first.get('reply_complete') is False
            and second.get('phase') == 'complete' and second.get('reply_complete') is True
            and all(e.get('result') == 'result' and e.get('receipt_sha256') == digest for e in rows))
    require(task['state'] == 'draft' and binding['state'] == 'ready'
            and binding['receipt']['task_id'] == task['id'] and exact(task['code_workspace_binding'], binding))
    return {'prepare_requests': 2, 'response_drops': 1, 'execution_authorized': False}


def seal_status(report):
    checks = report.get('checks', [])
    good = (report.get('flow_complete') is True and not report.get('failure')
            and len(checks) == 6 and {c.get('id') for c in checks} == {'WSI-B01', 'WSI-B02', 'WSI-B03', 'WSI-B04', 'WSI-B05', 'WSI-B06'}
            and all(c.get('status') == 'passed' for c in checks))
    good = good and not report['http_errors'] and not report['page_errors']
    good = good and report['cleanup'].get('complete') is True
    report['status'] = 'passed' if good else 'failed'
    return 0 if good else 1


def preflight(args):
    require(args.batond and args.candidate_manifest and args.frontend and args.frontend_build_record)
    expected = getattr(args, 'expected_source_head', BASE)
    require(type(expected) is str and bool(re.fullmatch(r'[0-9a-f]{40}', expected)),
            'Expected source must be an exact lowercase40 commit')
    binary = args.batond.resolve(strict=True)
    companion = binary.with_name('baton')
    manifest_path = args.candidate_manifest.resolve(strict=True)
    require(manifest_path.stat().st_size <= 2_097_152)
    manifest = decode(manifest_path.read_bytes())
    require(type(manifest.get('schema')) is int and manifest['schema'] == 16)
    heads = [manifest[k] for k in ('source_head', 'git_head', 'compiled_core_source_head') if k in manifest]
    require(bool(heads) and all(head == expected for head in heads), 'Missing or conflicting source aliases refused')
    compiled_head = heads[0]
    require(manifest.get('dirty', False) is False and manifest.get('tracked_clean', True) is True,
            'Dirty or untyped source declaration refused')
    tools = {name: file_digest(path) for name, path in [('batond', binary), ('baton', companion)]}
    require(all(os.access(path, os.X_OK) for path in (binary, companion)))
    for name in tools:
        require(manifest['binaries'][name] == tools[name]['sha256'])
    frontend = args.frontend.resolve(strict=True)
    build_raw = args.frontend_build_record.resolve(strict=True).read_bytes()
    require(len(build_raw) <= 1_048_576)
    build = decode(build_raw)
    require(build.get('source') == expected and type(build.get('exit')) is int and build['exit'] == 0
            and build.get('source_head_and_tracked_unchanged') is True
            and build.get('command') == ['npm', '--prefix', 'apps/desktop', 'run', 'build'])
    require(build.get('dirty', False) is False and build.get('tracked_clean', True) is True,
            'Dirty or untyped frontend declaration refused')
    return binary, frontend, {'expected_source_head': expected, 'manifest_sha256': sha(manifest_path.read_bytes()), 'tools': tools,
                              'frontend_assets': assets(frontend), 'compiled_core_source_head': compiled_head,
                              'frontend_build_head': build['source'], 'frontend_build_record_sha256': sha(build_raw)}


def run(args, binary, frontend, attribution, output):
    # Playwright import is deliberately below preflight/default/pure-control paths.
    from playwright.sync_api import sync_playwright, expect
    report = {'status': 'failed_or_interrupted', 'baseline': BASE, 'inputs': attribution,
              'harness_sha256': sha(Path(__file__).read_bytes()), 'checks': [], 'screenshots': [],
              'scenarios': {name: 'NOT_RUN' for name in ('WSI-B01', 'WSI-B02', 'WSI-B03', 'WSI-B04', 'WSI-B05', 'WSI-B06')},
              'http': [], 'http_errors': [], 'readonly_aborts': [], 'page_errors': [], 'cleanup': {},
              'not_run': ['macOS Native/WebView/OS Owner dialog', 'Provider/Agent/session',
                          'startup crash recovery', 'Grant/admission/rework'],
              'privacy': 'private generated fixture evidence; not an artifact upload projection'}
    daemon = server = thread = browser = engine = None
    server_started = False
    temp = tempfile.TemporaryDirectory(prefix='bw-', dir='/tmp')
    root = Path(temp.name).resolve()
    root.chmod(0o700)
    runtime, source = root / 'r', root / 's'
    runtime.mkdir(mode=0o700)
    source.mkdir(mode=0o700)
    log_path = output / 'daemon.log'
    log = log_path.open('xb')
    os.chmod(log_path, 0o600)
    stage = 'daemon_start'
    failure_kind = None
    event_pump = lambda: None

    def wait(read, accept, seconds=15):
        end = time.monotonic() + seconds
        while True:
            event_pump()
            value = read()
            require(time.monotonic() <= end, 'Bounded observation deadline')
            if accept(value):
                return value
            time.sleep(.02)

    def ipc(method, params, key=None, revision=None):
        request = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token,
                   'method': method, 'params': params}
        if key is not None:
            request['idempotency_key'] = key
        if revision is not None:
            request['expected_revision'] = revision
        raw = json.dumps(request).encode() + b'\n'
        require(len(raw) <= MAX_BODY)
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(5)
            sock.connect(str(runtime / 'batond.sock'))
            sock.sendall(raw)
            with sock.makefile('rb') as response:
                raw = response.readline(MAX_REPLY + 1)
        require(len(raw) <= MAX_REPLY and raw.endswith(b'\n'))
        return decode(raw)

    def result(method, params, key=None, revision=None):
        reply = ipc(method, params, key, revision)
        require('error' not in reply, 'Real Core fixture request refused')
        return reply['result']

    def passed(check_id, name, **data):
        report['checks'].append({'id': check_id, 'name': name, 'status': 'passed', **data})
        report['scenarios'][check_id] = 'passed'

    def cleanup_action(name, action):
        try:
            report['cleanup'][name] = action()
        except BaseException as error:
            report['cleanup'][name] = {'failed': type(error).__name__}
            report.setdefault('failure', {'stage': name, 'kind': type(error).__name__})

    try:
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
                                  stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        report['daemon_pid'] = daemon.pid
        report['runtime'] = {'canonical': True, 'mode': stat.S_IMODE(runtime.stat().st_mode),
                             'socket_utf8_bytes': len(str(runtime / 'batond.sock').encode())}
        require(report['runtime']['mode'] == 0o700)
        wait(lambda: (runtime / 'session.token').is_file() and (runtime / 'batond.sock').exists()
             and daemon.poll() is None, bool, 10)
        token = (runtime / 'session.token').read_text()
        require(stat.S_IMODE((runtime / 'session.token').stat().st_mode) == 0o600)
        socket_info = (runtime / 'batond.sock').lstat()
        require(stat.S_ISSOCK(socket_info.st_mode) and stat.S_IMODE(socket_info.st_mode) == 0o600)
        require(len(str(runtime / 'batond.sock').encode()) < 104)
        stage = 'private_owner_fixture'
        env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
        env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null', GIT_TERMINAL_PROMPT='0', GIT_NO_LAZY_FETCH='1')

        def git(*argv):
            proc = subprocess.run(['git', '-C', str(source), '-c', 'user.name=Fixture',
                                   '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', '-c', 'maintenance.auto=false', '-c', 'gc.auto=0', *argv],
                                  env=env, capture_output=True, text=True, timeout=5, check=True)
            return proc.stdout.strip()

        (source / 'input.txt').write_bytes(b'committed source\n')
        (source / 'empty').mkdir()
        git('init', '-q', '--initial-branch=main')
        git('add', 'input.txt')
        git('commit', '-qm', 'generated local fixture')
        (source / 'input.txt').write_bytes(b'dirty source must survive\n')
        (source / 'untracked.txt').write_bytes(b'untracked source must survive\n')
        project = result('projects.import', {'path': str(source), 'name': 'Generated isolated workspace fixture'}, 'fixture-project')
        tasks = [result('tasks.create', {'project_id': project['id'], 'goal': f'Workspace isolation fixture {i}'}, f'fixture-task-{i}') for i in (1, 2)]
        baseline_source = source_image(source, git)
        p = result('workspaces.resolve', {'task_id': tasks[0]['id'], 'expected_task_revision': tasks[0]['revision'],
                                         'expected_project_revision': project['revision'], 'host_id': 'local'})
        s = p['source']
        pin = {k: s[k] for k in ('canonical_root', 'device', 'inode')}
        pin.update(common_dir=s['kind']['common_dir'], head=s['kind']['head'])
        fixture_paths = ManagedFixturePaths(runtime, project['id'], pin)
        boundary = Boundary(project, tasks, pin)
        boundary.observe('workspaces.resolve', {'task_id': tasks[0]['id']}, {'result': p})
        require(pin['canonical_root'] == str(source)
                and pin['device'] == str(source.stat().st_dev)
                and pin['inode'] == str(source.stat().st_ino)
                and pin['head'] == git('rev-parse', 'HEAD'))
        no_execution(runtime)
        drop = {'task': tasks[1]['id'], 'done': False}

        class Server(http.server.ThreadingHTTPServer):
            daemon_threads = False
            block_on_close = True
            def handle_error(self, *_):
                report['http_errors'].append({'stage': 'unexpected_handler_exception'})

        class Handler(http.server.SimpleHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                # Serve only frozen assets, never a directory listing or source/runtime path.
                rel = self.path.split('?', 1)[0]
                if rel != '/' and rel.lstrip('/') not in attribution['frontend_assets']:
                    self.send_error(404)
                    return
                super().do_GET()

            def do_POST(self):
                method = None
                phase = 'decode'
                self.connection.settimeout(5)
                try:
                    require(self.path == ENDPOINT)
                    require(len(self.headers.get_all('Content-Length', [])) == 1
                            and not self.headers.get_all('Transfer-Encoding'))
                    n = self.headers.get('Content-Length', '')
                    require(bool(re.fullmatch(r'[1-9][0-9]{0,5}', n)) and int(n) <= MAX_BODY)
                    command, values = parse_http(self.rfile.read(int(n)), self.headers, origin)
                    method, params, key, revision = boundary.route(command, values)
                except (ValueError, PermissionError, KeyError, TypeError):
                    self.send_error(403)
                    return
                entry = {'method': method, 'phase': 'core_call', 'reply_complete': False,
                         'task_id': params.get('task_id'), 'project_id': params.get('project_id'),
                         'request_sha256': sha(json.dumps({'method': method, 'params': params, 'key': key, 'revision': revision}, sort_keys=True).encode())}
                report['http'].append(entry)
                try:
                    response = ipc(method, params, key, revision)
                    entry['reply_sha256'] = sha(json.dumps(response, sort_keys=True).encode())
                    boundary.observe(method, params, response)
                    if method == 'tasks.get' and 'error' not in response:
                        require(response['result']['id'] == params['task_id']
                                and response['result']['project_id'] == project['id'])
                        entry['task_projection'] = task_read_projection(response['result'])
                    if method == 'system.snapshot' and 'error' not in response:
                        records = response['result']['tasks']
                        require(type(records) is list and len(records) <= 32)
                        entry['snapshot_task_projections'] = [task_read_projection(task) for task in records]
                    if method in ('workspaces.list', 'grants.list') and 'error' not in response:
                        records = response['result']
                        require(type(records) is list and all(type(r) is dict and r.get('project_id') == project['id'] for r in records))
                        entry['scope_projection'] = ({'project_id': project['id'], 'workspace_ids': [r['id'] for r in records]}
                                                     if method == 'workspaces.list' else {'project_id': project['id'], 'grant_count': len(records)})
                    if method == 'workspaces.prepare_task_git' and 'error' not in response:
                        entry['receipt_sha256'] = sha(json.dumps(response['result']['original_receipt'], sort_keys=True).encode())
                    entry['result'] = 'error' if 'error' in response else 'result'
                    if 'error' in response:
                        entry['error_code'] = response['error']['code']
                        entry['side_effect_state'] = response['error']['side_effect_state']
                    if method == 'workspaces.prepare_task_git' and params['task_id'] == drop['task'] and not drop['done']:
                        require('error' not in response and response['result']['binding']['state'] == 'ready')
                        drop['done'] = True
                        entry['phase'] = 'intentional_post_commit_response_drop'
                        entry['receipt_sha256'] = sha(json.dumps(response['result']['original_receipt'], sort_keys=True).encode())
                        # Deliberate real transport loss, after actual Core commit; no fake reply.
                        self.close_connection = True
                        self.connection.shutdown(socket.SHUT_RDWR)
                        return
                    raw = json.dumps(response).encode()
                    require(len(raw) <= MAX_REPLY)
                    phase = 'headers'
                    self.send_response(200)
                    self.send_header('Content-Type', 'application/json')
                    self.send_header('Content-Length', str(len(raw)))
                    self.end_headers()
                    phase = 'response_body'
                    self.wfile.write(raw)
                    self.wfile.flush()
                    entry['reply_complete'] = True
                    entry['phase'] = 'complete'
                except (BrokenPipeError, ConnectionResetError) as error:
                    if method != 'workspaces.prepare_task_git' and phase == 'response_body':
                        report['readonly_aborts'].append({'method': method, 'phase': phase, 'kind': type(error).__name__})
                    else:
                        report['http_errors'].append({'method': method, 'phase': phase, 'kind': type(error).__name__})
                except BaseException as error:
                    report['http_errors'].append({'method': method, 'phase': phase, 'kind': type(error).__name__})
                    self.close_connection = True

        server = Server(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
        origin = f'http://127.0.0.1:{server.server_port}'
        thread = threading.Thread(target=server.serve_forever, name='private-workspace-http')
        thread.start()
        server_started = True
        stage = 'built_ui'
        engine = sync_playwright().start()
        browser = engine.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
        report['browser_version'] = browser.version
        context = browser.new_context(viewport={'width': 1440, 'height': 1100}, locale='en-US')
        context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_task_workspace',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw Error('Private fixed bridge refused');return r.json();}};""")
        context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(origin + '/') else route.abort())
        page = context.new_page()
        page.set_default_timeout(15_000)
        page.on('pageerror', lambda _: report['page_errors'].append({'kind': 'uncaught_browser_error'}))
        event_pump = lambda: page.evaluate('0')
        page.goto(origin)
        expect(page.get_by_text('Local daemon connected', exact=True)).to_be_visible()

        def open_task(task):
            page.get_by_test_id('task-card').filter(has_text=task['goal']).click()
            page.locator('#tab-details').click()
            panel = page.get_by_test_id('task-code-workspace')
            require(not panel.evaluate('(e)=>e.open'))
            panel.locator(':scope > summary').click()
            expect(panel.locator('section')).to_have_attribute('aria-busy', 'false')
            return panel

        def click(panel, label):
            button = panel.get_by_role('button', name=label, exact=True)
            expect(button).to_be_visible()
            expect(button).to_be_enabled()
            button.click()

        def shot(name, panel):
            panel.scroll_into_view_if_needed()
            path = output / (name + '.png')
            page.screenshot(path=str(path), full_page=True)
            os.chmod(path, 0o600)
            report['screenshots'].append({'name': path.name, 'sha256': file_digest(path, 32*1024*1024)['sha256'],
                                          'viewport': page.viewport_size, 'visual_review': 'NOT_RUN by author; Root must inspect original PNG'})

        stage = 'normal_first_task'
        panel = open_task(tasks[0])
        click(panel, 'Prepare isolated Git workspace for this task')
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_be_visible()
        t1 = result('tasks.get', {'task_id': tasks[0]['id']})
        b1 = t1['code_workspace_binding']
        require(b1['state'] == 'ready' and t1['state'] == 'draft'
                and not t1.get('acceptance_ref') and not t1.get('acceptance_sequence'))
        w1 = fixture_paths.workspace(b1, tasks[0]['id'])
        fixture_paths.committed_input(b1, tasks[0]['id'])
        require(source_image(source, git) == baseline_source)
        shot('01-first-task-ready-desktop', panel)
        passed('WSI-B01', 'normal real UI binds first Task without source dirty/index/content changes', task_id=tasks[0]['id'])
        page.get_by_role('button', name='Back to tasks', exact=True).click()
        stage = 'second_task_post_commit_transport_loss'
        panel = open_task(tasks[1])
        click(panel, 'Prepare isolated Git workspace for this task')
        expect(panel.get_by_text('Request outcome unknown. Check the current Task, then explicitly recover the same request. No automatic retry or new key is used.', exact=True)).to_be_visible()
        require(drop['done'])
        t2 = result('tasks.get', {'task_id': tasks[1]['id']})
        b2 = t2['code_workspace_binding']
        require(t2['state'] == 'draft' and not t2.get('acceptance_ref') and not t2.get('acceptance_sequence'))
        require(b2['state'] == 'ready' and b1['receipt']['workspace_id'] != b2['receipt']['workspace_id'])
        before_replay = db_image(runtime)
        click(panel, 'Check workspace preparation')
        expect(panel.get_by_text('Stored binding only; filesystem identity has not been revalidated by this read.', exact=True)).to_be_visible()
        expect(panel.get_by_role('button', name='Recover the exact preparation', exact=True)).to_be_enabled()
        click(panel, 'Recover the exact preparation')
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_be_visible()
        require(db_image(runtime) == before_replay, 'Exact replay created rows/events or changed existing receipt')
        t2_after = result('tasks.get', {'task_id': tasks[1]['id']})
        require(exact(t2, t2_after))
        w2 = fixture_paths.workspace(b2, tasks[1]['id'])
        require(w1 != w2)
        fixture_paths.committed_input(b1, tasks[0]['id'])
        fixture_paths.committed_input(b2, tasks[1]['id'])
        counts = db_image(runtime)
        require(len(counts['tasks']) == 2 and len(counts['managed_workspaces']) == 2)
        fixture_paths.write_output(b1, tasks[0]['id'])
        require(fixture_paths.output_absent(b2, tasks[1]['id']) and not (source / 'own-output.txt').exists())
        require(source_image(source, git) == baseline_source)
        writes = [e for e in report['http'] if e['method'] == 'workspaces.prepare_task_git']
        require(writes[1]['request_sha256'] == writes[2]['request_sha256']
                and writes[1]['task_id'] == tasks[1]['id'] == writes[2]['task_id'])
        require(len(writes) == 3 and sum(e['phase'] == 'intentional_post_commit_response_drop' for e in writes) == 1)
        require(all(e['reply_complete'] or e['phase'] == 'intentional_post_commit_response_drop' for e in writes))
        shot('02-second-task-exact-recovered-desktop', panel)
        passed('WSI-B02', 'actual Core success then HTTP response loss; TaskGet and original-key/payload replay preserve all rows',
               task_id=tasks[1]['id'], distinct_task_workspaces=True)
        stage = 'task_bound_execution_dialog_selection'
        dialog_before = db_image(runtime)
        dialog_start = len(report['http'])
        report['execution_dialog_reads'] = []

        def open_execution():
            start = len(report['http'])
            button = page.get_by_test_id('task-thread').locator('.thread-back').get_by_role('button', name='Execution & approval', exact=True)
            expect(button).to_be_visible()
            expect(button).to_be_enabled()
            button.click()
            dialog = page.get_by_role('dialog', name='Execution & approval', exact=True)
            expect(dialog).to_be_visible()
            return dialog, start

        def verify_dialog(dialog, start, current_task, other_workspace):
            proof = wait(lambda: confirmed_dialog_task_read(report['http'], start, current_task), lambda value: value is not None)
            workspace_id = current_task['code_workspace_binding']['receipt']['workspace_id']
            require(workspace_id != other_workspace and proof['workspace_id'] == workspace_id)
            expect(dialog.locator('#session-task')).to_have_value(current_task['id'])
            expect(dialog.locator('#session-project')).to_have_value(project['id'])
            expect(dialog.locator('#session-workspace')).to_have_value(workspace_id)
            expect(dialog.locator('#session-workspace')).to_be_disabled()
            expect(dialog.get_by_text('The task’s stored binding locates this workspace.', exact=False)).to_be_visible()
            expect(dialog.get_by_text('Stored binding is not fresh filesystem evidence or execution permission. Native Owner confirmation and the Core’s current admission checks still apply.', exact=False)).to_be_visible()
            expect(dialog.locator('#session-connection')).to_have_value('')
            expect(dialog.locator('#session-connection option')).to_have_count(1)
            expect(dialog.locator('#session-grant')).to_have_value('')
            expect(dialog.locator('#session-grant option')).to_have_count(1)
            expect(dialog.get_by_role('button', name='Create bounded execution grant', exact=True)).to_be_disabled()
            expect(dialog.get_by_role('button', name='Create managed session', exact=True)).to_be_disabled()
            expect(dialog.get_by_role('alert')).to_have_count(0)
            report['execution_dialog_reads'].append(proof)

        # Task2 opens from its actual thread, with no App refresh forced to
        # upgrade the parent snapshot. The dialog must fetch the latest Task.
        dialog, read_start = open_execution()
        verify_dialog(dialog, read_start, t2_after, b1['receipt']['workspace_id'])
        dialog.locator('#session-workspace').scroll_into_view_if_needed()
        shot('05-task2-bound-execution-desktop', dialog)
        # Switch the real Task selector in this same mounted dialog; do not
        # preserve Task2's workspace for Task1 or authorize either recipient.
        read_start = len(report['http'])
        dialog.locator('#session-task').select_option(tasks[0]['id'])
        verify_dialog(dialog, read_start, t1, b2['receipt']['workspace_id'])
        read_start = len(report['http'])
        dialog.locator('#session-task').select_option(tasks[1]['id'])
        verify_dialog(dialog, read_start, t2_after, b1['receipt']['workspace_id'])
        dialog.get_by_role('button', name='Close', exact=True).click()
        expect(dialog).to_have_count(0)
        # Also exercise initialTaskId from Task1's own thread on a new mount.
        page.get_by_role('button', name='Back to tasks', exact=True).click()
        panel = open_task(tasks[0])
        dialog, read_start = open_execution()
        verify_dialog(dialog, read_start, t1, b2['receipt']['workspace_id'])
        page.set_viewport_size({'width': 390, 'height': 844})
        expect(dialog.locator('#session-workspace')).to_have_value(b1['receipt']['workspace_id'])
        dialog.locator('#session-workspace').scroll_into_view_if_needed()
        shot('06-task1-bound-execution-mobile', dialog)
        dialog.get_by_role('button', name='Close', exact=True).click()
        expect(dialog).to_have_count(0)
        page.set_viewport_size({'width': 1440, 'height': 1100})
        require(db_image(runtime) == dialog_before, 'Dialog reads changed durable rows/events')
        require(source_image(source, git) == baseline_source)
        require(all(entry['method'] in {*READS.values(), 'events.subscribe', 'tasks.get', 'workspaces.resolve', 'workspaces.list', 'grants.list'}
                    for entry in report['http'][dialog_start:]), 'Execution dialog attempted a write')
        no_execution(runtime)
        require([proof['task_id'] for proof in report['execution_dialog_reads']]
                == [tasks[1]['id'], tasks[0]['id'], tasks[1]['id'], tasks[0]['id']])
        require([proof['workspace_id'] for proof in report['execution_dialog_reads']]
                == [b2['receipt']['workspace_id'], b1['receipt']['workspace_id'], b2['receipt']['workspace_id'], b1['receipt']['workspace_id']])
        passed('WSI-B05', 'real execution dialogs fetch each bound Task and select only its workspace without Grant/session',
               confirmed_selection_steps=len(report['execution_dialog_reads']),
               completed_task_get_responses=sum(entry['method'] == 'tasks.get' and entry.get('reply_complete') is True
                                                for entry in report['http'][dialog_start:]), execution_authorized=False)
        stage = 'unbound_git_task_execution_entry_parent_refresh_recovery'
        # A third private Owner fixture Task is created only after the old B02
        # two-Task/two-workspace assertions and readonly B05 are complete. Browser
        # HTTP remains unable to create Tasks, Grants, Sessions or approvals.
        page.get_by_role('button', name='Back to tasks', exact=True).click()
        third = result('tasks.create', {'project_id': project['id'], 'goal': 'Execution entry unbound Git Task fixture 3'}, 'fixture-entry-task-3')
        require(third['state'] == 'draft' and third.get('code_workspace_binding') is None)
        boundary.tasks[third['id']] = third
        entry_before = db_image(runtime)
        old_tasks = [result('tasks.get', {'task_id': task['id']}) for task in tasks]
        old_workspace_images = [fixture_paths.image(binding, task['id']) for binding, task in zip((b1, b2), tasks)]
        drop.update(task=third['id'], done=False)
        card = page.get_by_test_id('task-card').filter(has_text=third['goal'])
        # Private setup is not a UI action; the all-draft idle canvas does not
        # poll. Use its existing readonly Refresh before expecting the new card.
        canvas_start = len(report['http'])
        canvas_deadline = time.monotonic() + 15
        click(page.locator('.appbar'), 'Refresh')
        wait(lambda: confirmed_canvas_snapshot(report['http'], canvas_start, third), lambda value: value is not None,
             seconds=max(0, canvas_deadline - time.monotonic()))
        canvas_remaining = canvas_deadline - time.monotonic()
        require(canvas_remaining > 0, 'Canvas setup observation deadline')
        expect(card).to_have_count(1, timeout=max(1, int(canvas_remaining * 1000)))
        require(time.monotonic() <= canvas_deadline, 'Canvas setup observation deadline')
        card.click()
        # No Details expansion: this preparation must be entered from the real
        # execution dialog, using its automatic reuse of TaskCodeWorkspace.
        dialog, entry_start = open_execution()
        wait(lambda: confirmed_dialog_task_read(report['http'], entry_start, third), lambda v: v is not None)
        entry = dialog.get_by_test_id('session-task-workspace-entry')
        expect(entry).to_have_count(1)
        panel = entry.get_by_test_id('task-code-workspace')
        expect(panel).to_have_count(1)
        require(panel.evaluate('(e)=>e.open'), 'Execution entry did not open the reused preparation component')
        expect(panel.get_by_role('button', name='Prepare isolated Git workspace for this task', exact=True)).to_be_enabled()

        def entry_execution_blocked():
            expect(dialog.locator('#session-workspace')).to_be_disabled()
            expect(dialog.get_by_role('checkbox', name='I authorize this exact recipient, workspace and bounded execution scope.', exact=True)).to_be_disabled()
            expect(dialog.get_by_role('button', name='Create bounded execution grant', exact=True)).to_be_disabled()
            expect(dialog.get_by_role('button', name='Create managed session', exact=True)).to_be_disabled()
            no_execution(runtime)

        entry_execution_blocked()
        require(db_image(runtime) == entry_before, 'Opening an unbound execution entry changed durable rows')
        require(not [e for e in report['http'][entry_start:] if e['method'] == 'workspaces.prepare_task_git'])
        click(panel, 'Prepare isolated Git workspace for this task')
        expect(panel.get_by_text('Request outcome unknown. Check the current Task, then explicitly recover the same request. No automatic retry or new key is used.', exact=True)).to_be_visible()
        require(drop['done'], 'No actual Core success/transport drop occurred')
        third_ready = result('tasks.get', {'task_id': third['id']})
        b3 = third_ready['code_workspace_binding']
        require(b3['state'] == 'ready' and third_ready['state'] == 'draft'
                and not third_ready.get('acceptance_ref') and not third_ready.get('acceptance_sequence'))
        require(b3['receipt']['workspace_id'] not in {b1['receipt']['workspace_id'], b2['receipt']['workspace_id']})
        fixture_paths.workspace(b3, third['id'])
        fixture_paths.committed_input(b3, third['id'])
        third_before_recovery = db_image(runtime)
        original_panel = panel.element_handle()
        require(original_panel is not None)
        refresh_start = len(report['http'])
        click(dialog, 'Refresh session records')
        wait(lambda: confirmed_dialog_task_read(report['http'], refresh_start, third_ready), lambda v: v is not None)
        # Do not click the inner Check button: the actual parent refresh must
        # preserve the original pending intent and reconcile this same panel.
        expect(panel.get_by_role('button', name='Recover the exact preparation', exact=True)).to_be_enabled()
        require(original_panel.evaluate('(e)=>e.isConnected'), 'Parent refresh replaced the original recovery component')
        expect(panel.get_by_text('Stored binding only; filesystem identity has not been revalidated by this read.', exact=True)).to_be_visible()
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_have_count(0)
        entry_execution_blocked()
        require(db_image(runtime) == third_before_recovery, 'Parent readonly refresh changed committed preparation')
        require(len([e for e in report['http'][entry_start:] if e['method'] == 'workspaces.prepare_task_git']) == 1,
                'Parent refresh retried preparation without explicit intent')
        shot('07-unbound-entry-parent-refresh-recovery-desktop', dialog)
        recovery_start = len(report['http'])
        click(panel, 'Recover the exact preparation')
        fresh_proof = wait(lambda: confirmed_dialog_task_read(report['http'], recovery_start, third_ready), lambda v: v is not None)
        scope_proof = wait(lambda: confirmed_dialog_scope_reads(report['http'], recovery_start, project['id'], b3['receipt']['workspace_id']), lambda v: v is not None)
        expect(dialog.get_by_test_id('session-task-workspace-entry')).to_have_count(0)
        expect(dialog.locator('#session-workspace')).to_have_value(b3['receipt']['workspace_id'])
        expect(dialog.locator('#session-workspace')).to_be_disabled()
        expect(dialog.get_by_text('The task’s stored binding locates this workspace.', exact=False)).to_be_visible()
        expect(dialog.get_by_text('Stored binding is not fresh filesystem evidence or execution permission. Native Owner confirmation and the Core’s current admission checks still apply.', exact=False)).to_be_visible()
        entry_execution_blocked()  # No named connection, Grant or Session was created.
        replay_proof = confirmed_entry_replay(report['http'], entry_start, third_ready, b3)
        require(db_image(runtime) == third_before_recovery, 'Exact entry recovery changed durable rows/events')
        require(exact(result('tasks.get', {'task_id': third['id']}), third_ready))
        require(all(exact(result('tasks.get', {'task_id': old['id']}), old) for old in old_tasks), 'Entry changed either older Task binding')
        require([fixture_paths.image(binding, task['id']) for binding, task in zip((b1, b2), tasks)] == old_workspace_images,
                'Entry changed an older managed worktree file or .git backlink')
        require(source_image(source, git) == baseline_source, 'Entry changed registered Git source')
        require(all(e['method'] in {*READS.values(), 'events.subscribe', 'tasks.get', 'workspaces.resolve', 'workspaces.list', 'grants.list', 'workspaces.prepare_task_git'}
                    for e in report['http'][entry_start:]), 'Entry attempted an execution/authority write')
        shot('08-unbound-entry-exact-recovered-desktop', dialog)
        report['entry_read_proofs'] = {'task': fresh_proof, 'scope': scope_proof}
        passed('WSI-B06', 'unbound Git Task prepares through execution dialog; parent refresh preserves exact-key recovery without execution',
               **replay_proof, parent_refresh_recovery=True, same_component_retained=True, fresh_task_read=True,
               fresh_scope_lists=True, locked_own_workspace=True, old_tasks_unchanged=True, old_workspaces_unchanged=True, source_unchanged=True,
               exact_replay_no_rows=True, no_execution_or_grants=True)
        dialog.get_by_role('button', name='Close', exact=True).click()
        expect(dialog).to_have_count(0)

        # Restore exactly the selected Task2/panel/desktop state at the original
        # B02->B03 boundary. The original drift and negative checks stay intact.
        page.get_by_role('button', name='Back to tasks', exact=True).click()
        panel = open_task(tasks[1])
        page.get_by_role('button', name='Back to tasks', exact=True).click()
        stage = 'filesystem_drift_and_proof_invalidation'
        panel = open_task(tasks[0])
        # Cached prior proof may remain on remount; explicit fresh action verifies before fault injection.
        click(panel, 'Revalidate the original binding')
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_be_visible()
        retained = fixture_paths.replace_for_drift(b1, tasks[0]['id'])  # actual own FS, no SQL identity forgery
        drift_image = db_image(runtime)
        click(panel, 'Revalidate the original binding')
        expect(panel.get_by_role('alert')).to_contain_text('Workspace preparation failed.')
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_have_count(0)
        click(panel, 'Check workspace preparation')
        expect(panel.get_by_text('Stored binding only; filesystem identity has not been revalidated by this read.', exact=True)).to_be_visible()
        expect(panel.get_by_text('Filesystem identity revalidated', exact=True)).to_have_count(0)
        require(db_image(runtime) == drift_image)
        error = [e for e in report['http'] if e['method'] == 'workspaces.prepare_task_git'][-1]
        require(error['result'] == 'error' and error['error_code'] == 'INPUT_CHANGED'
                and error['side_effect_state'] == 'not_started' and error['reply_complete'])
        require(source_image(source, git) == baseline_source)
        shot('03-drift-invalidates-proof-desktop', panel)
        page.set_viewport_size({'width': 390, 'height': 844})
        expect(panel.get_by_text('Stored binding only; filesystem identity has not been revalidated by this read.', exact=True)).to_be_visible()
        shot('04-drift-history-mobile', panel)
        require(page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'), 'Mobile horizontal overflow')
        passed('WSI-B03', 'real replaced worktree refuses revalidation; subsequent Task-only refresh cannot recreate fresh proof')
        stage = 'http_permission_negatives'
        before = db_image(runtime)
        for command in ('approvals_finalize', 'projects_import', 'tasks_create', 'sessions_create', 'workspaces_create', 'dispatch', 'generic_rpc'):
            status = page.evaluate("""async command => (await fetch('/__test_task_workspace',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args:{}})})).status""", command)
            require(status == 403)
        require(db_image(runtime) == before)
        passed('WSI-B04', 'fixed browser bridge denies approval/create/session/generic writes without durable changes')
        report['no_execution'] = no_execution(runtime)
        report['binding_receipt_hashes'] = [sha(json.dumps(b['receipt'], sort_keys=True).encode()) for b in (b1, b2)]
        require(not report['http_errors'] and not report['page_errors'])
        report['flow_complete'] = True
    except BaseException as error:
        failure_kind = type(error).__name__
        report.setdefault('failure', {'stage': stage, 'kind': failure_kind})
        # No exception text/trace or full RPC/DB exported. Preserve actual protocol code only.
        if browser is not None:
            try:
                shot('failure-desktop', panel)
            except BaseException as capture_error:
                report['failure_capture'] = type(capture_error).__name__
        if daemon is not None and daemon.poll() is None:
            try:
                report['failure_table_counts'] = {k: len(v) for k, v in db_image(runtime).items()}
            except BaseException as diagnostic_error:
                report['failure_diagnostic'] = type(diagnostic_error).__name__
    finally:
        if browser is not None:
            cleanup_action('browser_close', lambda: browser.close())
        if engine is not None:
            cleanup_action('playwright_stop', lambda: engine.stop())
        if server is not None:
            if server_started:
                cleanup_action('http_shutdown', lambda: server.shutdown())
            cleanup_action('http_close_and_handlers_join', lambda: server.server_close())
        if thread is not None:
            def join_http():
                thread.join(5)
                require(not thread.is_alive(), 'HTTP serve thread failed to join')
                return {'thread_alive_after_join': thread.is_alive()}
            cleanup_action('http_thread', join_http)
        if daemon is not None:
            def stop_daemon():
                # Popen retains this child identity; no global scans/signals/adoption or Provider children.
                if daemon.poll() is not None:
                    daemon.wait(timeout=5)
                    raise AssertionError('Daemon exited before controlled stop; own child reaped')
                daemon.send_signal(signal.SIGTERM)
                try:
                    code = daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    daemon.kill()
                    daemon.wait(timeout=5)
                    raise AssertionError('SIGTERM join failed; own child killed/reaped, run remains failed')
                require(code == 0, 'Daemon graceful stop returned nonzero')
                return {'pid': daemon.pid, 'wait_exitcode': code, 'popen_reaped': daemon.returncode is not None}
            cleanup_action('daemon_stop', stop_daemon)
        cleanup_action('log_close', lambda: log.close())
        cleanup_action('tools_after', lambda: {
            name: file_digest(path) for name, path in [('batond', binary), ('baton', binary.with_name('baton'))]})
        cleanup_action('assets_after', lambda: assets(frontend))
        if report['cleanup'].get('tools_after') != attribution['tools'] or report['cleanup'].get('assets_after') != attribution['frontend_assets']:
            report.setdefault('failure', {'stage': 'input_hashes_changed', 'kind': 'AssertionError'})
        cleanup_action('temporary_cleanup', lambda: temp.cleanup())
        report['cleanup']['complete'] = not any(isinstance(v, dict) and 'failed' in v for v in report['cleanup'].values())
        # Final decision occurs after *all* handler joins, browser/daemon waits, tool checks and directory cleanup.
        code = seal_status(report)
        private_json(output / 'report.json', report)
    return code


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--batond', type=Path)
    p.add_argument('--candidate-manifest', type=Path)
    p.add_argument('--frontend', type=Path)
    p.add_argument('--frontend-build-record', type=Path)
    p.add_argument('--expected-source-head', default=BASE, help='Exact40 lowercase source shared by compiled Core and frontend record; default840 historical baseline')
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--chromium', default='/usr/bin/chromium')
    p.add_argument('--run', action='store_true')
    args = p.parse_args()
    output = args.output.absolute()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    require(stat.S_IMODE(output.stat().st_mode) == 0o700)
    binary, frontend, attribution = preflight(args)
    if not args.run:
        private_json(output / 'preflight.json', {'status': 'NOT_RUN', 'reason': '--run required', 'inputs': attribution})
        return 2
    return run(args, binary, frontend, attribution, output)


if __name__ == '__main__':
    raise SystemExit(main())
