#!/usr/bin/env python3
"""Freeze same-checkout test tools and probe only their private empty runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import stat
import subprocess
import tempfile
import time

FACTORY_TABLES = (
    'factory_scopes', 'factory_runs', 'factory_task_lineage',
    'factory_delegations', 'factory_task_bindings', 'factory_attempts',
    'factory_scoped_handles', 'factory_handoffs', 'factory_budget_reservations',
    'factory_child_plan_heads', 'factory_child_plan_revisions', 'factory_child_goal_runs',
)


def verify_private_tool(path):
    metadata = path.lstat()
    if (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o500 or metadata.st_uid != os.geteuid()):
        raise ValueError('Frozen tool must be an owned regular single-link mode0500 file')


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def source_identity(root):
    def git(*args):
        return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('Tracked checkout changed during test-tool preparation')
    return {'git_head': git('rev-parse', 'HEAD'), 'git_tree': git('rev-parse', 'HEAD^{tree}'), 'tracked_clean': True}


def observe_database(runtime):
    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as conn:
        conn.execute('PRAGMA query_only=ON')
        conn.execute('BEGIN')
        schema = conn.execute('PRAGMA user_version').fetchone()[0]
        if type(schema) is not int or schema != 16:
            raise ValueError('Prepared daemon did not create exact schema16')
        counts = {name: conn.execute('SELECT count(*) FROM ' + name).fetchone()[0]
                  for name in FACTORY_TABLES}
        sequence = conn.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
        if sequence != 0 or any(type(count) is not int or count != 0 for count in counts.values()):
            raise ValueError('Private preparation probe unexpectedly created authority/runtime rows')
        conn.rollback()
    return {'schema': schema, 'query_only': True, 'sequence': sequence, 'zero_tables': counts}


def probe(tools, evidence):
    # /tmp resolves to /private/tmp on macOS. Keep AF_UNIX below its path bound.
    with tempfile.TemporaryDirectory(prefix='ab15-', dir='/tmp') as temporary:
        runtime = Path(temporary).resolve(strict=True)
        runtime.chmod(0o700)
        socket_bytes = len(str(runtime / 'batond.sock').encode('utf-8'))
        runtime_mode = runtime.stat().st_mode & 0o777
        if runtime_mode != 0o700 or socket_bytes > 103:
            raise ValueError('Private probe runtime is not mode0700 with a short Unix socket')
        env = {'PATH': '/usr/bin:/bin', 'BATON_RUNTIME_DIR': str(runtime)}
        with (evidence / 'probe-stdout.log').open('xb') as stdout, (evidence / 'probe-stderr.log').open('xb') as stderr:
            daemon = subprocess.Popen([str(tools / 'batond')], env=env, stdout=stdout, stderr=stderr)
            forced_kill = False
            sigterm_sent = False
            code = None
            try:
                deadline = time.monotonic() + 5
                while not (runtime / 'batond.sock').exists():
                    if daemon.poll() is not None:
                        raise RuntimeError('Prepared daemon exited before socket readiness')
                    if time.monotonic() >= deadline:
                        raise TimeoutError('Prepared daemon did not bind its private socket')
                    time.sleep(.01)
                cli = subprocess.run([str(tools / 'baton'), 'snapshot'], env=env,
                                     capture_output=True, timeout=5)
                (evidence / 'probe-snapshot.stderr').write_bytes(cli.stderr)
                (evidence / 'probe-snapshot.json').write_bytes(cli.stdout)
                cli.check_returncode()
                reply = json.loads(cli.stdout)
                if 'error' in reply or reply.get('result', {}).get('tasks') != []:
                    raise ValueError('Prepared CLI snapshot did not return an empty private runtime')
                observation = observe_database(runtime)
                sequence = reply['result'].get('sequence')
                if type(sequence) is not int or sequence != observation['sequence']:
                    raise ValueError('CLI snapshot and read-only database sequence differ')
            finally:
                try:
                    if daemon.poll() is None:
                        daemon.terminate()
                        sigterm_sent = True
                    try:
                        code = daemon.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        forced_kill = True
                        daemon.kill()
                        code = daemon.wait(timeout=5)
                finally:
                    (evidence / 'probe-cleanup.json').write_text(json.dumps({
                        'owned_pid': daemon.pid, 'sigterm_sent': sigterm_sent,
                        'forced_kill': forced_kill, 'wait_exit': code,
                    }, indent=2) + '\n')
            if not sigterm_sent or forced_kill or code != 0:
                raise RuntimeError('Prepared daemon did not stop normally after SIGTERM')
        return {**observation, 'runtime_mode': oct(runtime_mode), 'socket_utf8_bytes': socket_bytes,
                'snapshot_api_version': reply['result'].get('api_version'),
                'snapshot_sequence': sequence, 'cli_snapshot_exit': cli.returncode,
                'original_daemon_SIGTERM_wait_exit': code, 'sigterm_sent': sigterm_sent,
                'forced_kill': forced_kill}


def prepare(root, source, output, evidence):
    if not source.is_absolute() or not output.is_absolute():
        raise ValueError('Test-tool source and frozen output must be absolute')
    identity = source_identity(root)
    evidence.mkdir(parents=True, mode=0o700)
    source = source.resolve(strict=True)
    output.mkdir(mode=0o700)  # Never reuse a stale candidate directory.
    output = output.resolve(strict=True)
    hashes = {}
    for name in ('batond', 'baton'):
        original = source / name
        if not original.is_file() or original.is_symlink() or not os.access(original, os.X_OK):
            raise ValueError('Expected a regular executable from the current prebuild')
        hashes[name] = sha256(original)
        frozen = output / name
        shutil.copyfile(original, frozen)
        frozen.chmod(0o500)
        verify_private_tool(frozen)
        if sha256(frozen) != hashes[name]:
            raise ValueError('Frozen executable bytes differ from current prebuild')
    try:
        observation = probe(output, evidence)
    except Exception as error:
        (evidence / 'probe-failure.json').write_text(json.dumps({**identity, 'binaries': hashes, 'error_type': type(error).__name__, 'dimension': 'preparation probe failed; no product test pass claimed'}, indent=2) + '\n')
        raise
    if source_identity(root) != identity:
        raise ValueError('Checkout changed during test-tool probe')
    for name in hashes:
        verify_private_tool(output / name)
    if any(sha256(source / name) != value or sha256(output / name) != value for name, value in hashes.items()):
        raise ValueError('Test tools changed during preparation')
    record = {**identity, 'schema': 16, 'source_directory': str(source),
              'frozen_directory': str(output), 'binaries': hashes,
              'source_binaries_after': {name: sha256(source / name) for name in hashes},
              'frozen_binaries_after': {name: sha256(output / name) for name in hashes},
              'probe': observation,
              'dimension': 'Same-checkout normal product tools; empty private Unix IPC/read-only SQLite probe; no Provider or native GUI'}
    for directory in (output, evidence):
        (directory / 'manifest.json').write_text(json.dumps(record, indent=2) + '\n')
    return record


def verify(root, tools):
    if not tools.is_absolute():
        raise ValueError('Frozen tool directory must be absolute')
    record = json.loads((tools / 'manifest.json').read_text())
    if type(record.get('schema')) is not int or record['schema'] != 16:
        raise ValueError('Frozen tool manifest must be exact schema16')
    if record.get('frozen_directory') != str(tools.resolve(strict=True)):
        raise ValueError('Frozen tool directory changed')
    if any(record.get(key) != value for key, value in source_identity(root).items()):
        raise ValueError('Frozen tools are not from the current clean checkout')
    if set(record.get('binaries', {})) != {'batond', 'baton'}:
        raise ValueError('Frozen manifest must bind both normal product tools')
    for name in record['binaries']:
        verify_private_tool(tools / name)
    if any(sha256(tools / name) != value for name, value in record['binaries'].items()):
        raise ValueError('Frozen tool bytes changed while running tests')
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify', action='store_true')
    parser.add_argument('--evidence', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.verify:
        record = verify(root, args.output)
    else:
        if args.source is None or args.evidence is None:
            parser.error('--source and --evidence are required when preparing tools')
        record = prepare(root, args.source, args.output, args.evidence)
    print(json.dumps(record, sort_keys=True))


if __name__ == '__main__':
    main()
