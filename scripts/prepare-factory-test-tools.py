#!/usr/bin/env python3
"""Freeze same-checkout test tools and probe only their private empty runtime."""
import argparse
from contextlib import closing, contextmanager
import hashlib
import json
import os
import re
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

INTEGRATION_TABLES = ('integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences')


CURRENT_SCHEMA = 19
HISTORICAL_SCHEMA = 18
# Store schema 19 adds the inert Host registry. A fresh probe cannot create authority.
HOST_TABLES = ('host_registrations',)
TOOL_NAMES = ('batond', 'baton')
MANIFEST_LIMIT = 65536
MANIFEST_FIELDS = {
    'git_head', 'git_tree', 'tracked_clean', 'schema', 'source_directory',
    'frozen_directory', 'binaries', 'source_binaries_after', 'frozen_binaries_after',
    'frozen_file_identities', 'probe', 'dimension',
}


def file_identity(metadata):
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_uid,
            metadata.st_nlink, metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns)


def private_identity(metadata):
    # Bind physical frozen entries without disclosing their inode numbers.
    return hashlib.sha256(f'{metadata.st_dev}:{metadata.st_ino}'.encode()).hexdigest()


def verify_private_tool(path):
    metadata = path.lstat()
    if (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o500 or metadata.st_uid != os.geteuid()):
        raise ValueError('Frozen tool must be an owned regular single-link mode0500 file')
    return metadata


def verify_private_directory(path):
    metadata = path.lstat()
    if (not stat.S_ISDIR(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) != 0o700
            or metadata.st_uid != os.geteuid()):
        raise ValueError('Frozen tools must be in an owned mode0700 directory')
    return metadata.st_dev, metadata.st_ino


@contextmanager
def open_regular(path, expected=None):
    before = path.lstat()
    expected = file_identity(before) if expected is None else expected
    if not stat.S_ISREG(before.st_mode) or file_identity(before) != expected:
        raise ValueError('Expected regular file identity changed')
    def opener(name, flags):
        return os.open(name, flags | os.O_NOFOLLOW | os.O_NONBLOCK)
    with open(path, 'rb', opener=opener) as stream:
        if file_identity(os.fstat(stream.fileno())) != expected:
            raise ValueError('File changed while opening')
        yield stream
        if file_identity(os.fstat(stream.fileno())) != expected or file_identity(path.lstat()) != expected:
            raise ValueError('File changed while reading')


def sha256(path):
    digest = hashlib.sha256()
    with open_regular(path) as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def source_identity(root):
    def git(*args):
        return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('Tracked checkout changed during test-tool preparation')
    return {'git_head': git('rev-parse', 'HEAD'), 'git_tree': git('rev-parse', 'HEAD^{tree}'), 'tracked_clean': True}


def expected_tables(schema):
    if schema == HISTORICAL_SCHEMA:
        return (*FACTORY_TABLES, *INTEGRATION_TABLES)
    if schema == CURRENT_SCHEMA:
        return (*FACTORY_TABLES, *INTEGRATION_TABLES, *HOST_TABLES)
    raise ValueError('Unsupported tool probe contract')


def observe_database(runtime, *, expected_schema=CURRENT_SCHEMA):
    tables = expected_tables(expected_schema)
    database = runtime / 'baton.sqlite3'
    before = database.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_uid != os.geteuid():
        raise ValueError('Probe database must be an owned regular file')
    with closing(sqlite3.connect(database.as_uri() + '?mode=ro', uri=True)) as conn:
        conn.execute('PRAGMA query_only=ON')
        if conn.execute('PRAGMA query_only').fetchone() != (1,):
            raise ValueError('SQLite read-only observation could not enable query_only')
        conn.execute('BEGIN')
        schema = conn.execute('PRAGMA main.user_version').fetchone()[0]
        if type(schema) is not int or schema != expected_schema:
            raise ValueError(f'Prepared daemon did not create exact schema{expected_schema}')
        counts = {name: conn.execute('SELECT count(*) FROM main.' + name).fetchone()[0] for name in tables}
        event_count, sequence = conn.execute(
            'SELECT count(*),coalesce(max(sequence),0) FROM main.events'
        ).fetchone()
        if (type(sequence) is not int or sequence != 0 or event_count != 0
                or any(type(count) is not int or count != 0 for count in counts.values())
                or (schema == CURRENT_SCHEMA
                    and conn.execute('SELECT count(*) FROM main.tasks').fetchone() != (0,))):
            raise ValueError('Private preparation probe unexpectedly created authority/runtime rows')
        conn.rollback()
    if file_identity(database.lstat()) != file_identity(before):
        raise ValueError('Read-only database identity changed during probe')
    return {'schema': schema, 'query_only': True, 'sequence': sequence, 'zero_tables': counts}


def snapshot_result(reply, observation):
    result = reply.get('result') if type(reply) is dict and 'error' not in reply else None
    if type(result) is not dict or result.get('tasks') != []:
        raise ValueError('Prepared CLI snapshot did not return an empty private runtime')
    api = result.get('api_version')
    if type(api) is not int or api != 1:
        raise ValueError('Prepared CLI snapshot did not return exact API 1')
    schema, sequence = result.get('database_schema_version'), result.get('sequence')
    if type(schema) is not int or schema != observation['schema']:
        raise ValueError('CLI snapshot and read-only database schema differ')
    if type(sequence) is not int or sequence != observation['sequence']:
        raise ValueError('CLI snapshot and read-only database sequence differ')
    return result


def probe(tools, evidence):
    # /tmp resolves to /private/tmp on macOS. Keep AF_UNIX below its path bound.
    with tempfile.TemporaryDirectory(prefix='ab19-', dir='/tmp') as temporary:
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
                observation = observe_database(runtime)
                snapshot = snapshot_result(json.loads(cli.stdout), observation)
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
                'snapshot_api_version': snapshot['api_version'],
                'snapshot_database_schema_version': snapshot['database_schema_version'],
                'snapshot_sequence': snapshot['sequence'], 'cli_snapshot_exit': cli.returncode,
                'original_daemon_SIGTERM_wait_exit': code, 'sigterm_sent': sigterm_sent,
                'forced_kill': forced_kill}


def validate_current_probe(observation):
    required = {
        'schema': CURRENT_SCHEMA, 'query_only': True, 'sequence': 0,
        'runtime_mode': '0o700', 'snapshot_api_version': 1,
        'snapshot_database_schema_version': CURRENT_SCHEMA, 'snapshot_sequence': 0,
        'cli_snapshot_exit': 0, 'original_daemon_SIGTERM_wait_exit': 0,
        'sigterm_sent': True, 'forced_kill': False,
    }
    if (type(observation) is not dict
            or set(observation) != {*required, 'socket_utf8_bytes', 'zero_tables'}):
        raise ValueError('Frozen tool manifest lacks a complete current probe')
    for name, value in required.items():
        if type(observation[name]) is not type(value) or observation[name] != value:
            raise ValueError(f'Frozen tool probe has invalid {name}')
    socket_bytes = observation['socket_utf8_bytes']
    if type(socket_bytes) is not int or not 0 < socket_bytes <= 103:
        raise ValueError('Frozen tool probe has an invalid Unix socket size')
    counts = observation['zero_tables']
    if (type(counts) is not dict or set(counts) != set(expected_tables(CURRENT_SCHEMA))
            or any(type(value) is not int or value != 0 for value in counts.values())):
        raise ValueError('Frozen tool probe has incomplete or nonempty table observations')


def publish_manifest(directory, content):
    with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', dir=directory, delete=False) as staged:
        try:
            staged.write(content)
            staged.close()
            os.replace(staged.name, directory / 'manifest.json')
        finally:
            Path(staged.name).unlink(missing_ok=True)


def prebuild_identity(path):
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        raise ValueError('Expected a regular executable from the current prebuild') from None
    if not stat.S_ISREG(metadata.st_mode) or not os.access(path, os.X_OK):
        raise ValueError('Expected a regular executable from the current prebuild')
    return file_identity(metadata)


def prepare(root, source, output, evidence):
    if not source.is_absolute() or not output.is_absolute():
        raise ValueError('Test-tool source and frozen output must be absolute')
    identity = source_identity(root)
    evidence.mkdir(parents=True, mode=0o700)
    source = source.resolve(strict=True)
    output.mkdir(mode=0o700)  # Never reuse a stale candidate directory.
    output = output.resolve(strict=True)
    output_identity = verify_private_directory(output)
    origins = {name: prebuild_identity(source / name) for name in TOOL_NAMES}
    hashes, frozen_entries = {}, {}
    for name, origin in origins.items():
        original = source / name
        frozen = output / name
        hashes[name] = sha256(original)
        with open_regular(original, origin) as reader, frozen.open('xb') as writer:
            shutil.copyfileobj(reader, writer, length=1024 * 1024)
        frozen.chmod(0o500)
        frozen_entries[name] = private_identity(verify_private_tool(frozen))
        if sha256(frozen) != hashes[name]:
            raise ValueError('Frozen executable bytes differ from current prebuild')
    try:
        observation = probe(output, evidence)
    except Exception as error:
        (evidence / 'probe-failure.json').write_text(json.dumps({**identity, 'binaries': hashes, 'error_type': type(error).__name__, 'dimension': 'preparation probe failed; no product test pass claimed'}, indent=2) + '\n')
        raise
    if source_identity(root) != identity:
        raise ValueError('Checkout changed during test-tool probe')
    if verify_private_directory(output) != output_identity:
        raise ValueError('Frozen output directory changed during preparation')
    if any(prebuild_identity(source / name) != origin for name, origin in origins.items()):
        raise ValueError('Prebuild executable identity changed during preparation')
    if any(private_identity(verify_private_tool(output / name)) != frozen_entries[name] for name in hashes):
        raise ValueError('Frozen executable identity changed during preparation')
    source_after = {name: sha256(source / name) for name in hashes}
    frozen_after = {name: sha256(output / name) for name in hashes}
    if source_after != hashes or frozen_after != hashes:
        raise ValueError('Test tools changed during preparation')
    validate_current_probe(observation)
    record = {**identity, 'schema': CURRENT_SCHEMA, 'source_directory': str(source),
              'frozen_directory': str(output), 'binaries': hashes,
              'source_binaries_after': source_after,
              'frozen_binaries_after': frozen_after,
              'frozen_file_identities': frozen_entries,
              'probe': observation,
              'dimension': 'Same-checkout normal product tools; empty private Unix IPC/read-only SQLite probe; no Provider or native GUI'}
    content = json.dumps(record, indent=2) + '\n'
    publish_manifest(evidence, content)
    publish_manifest(output, content)
    return record


def read_manifest(tools):
    path = tools / 'manifest.json'
    meta = path.lstat()
    if (not stat.S_ISREG(meta.st_mode) or meta.st_nlink != 1
            or meta.st_uid != os.geteuid() or meta.st_mode & 0o022):
        raise ValueError('Frozen tool manifest must be an owned regular single-link file')
    with open_regular(path, file_identity(meta)) as stream:
        data = stream.read(MANIFEST_LIMIT + 1)
    if len(data) > MANIFEST_LIMIT:
        raise ValueError('Frozen tool manifest exceeds bound')
    def unique_object(pairs):
        obj = dict(pairs)
        if len(obj) != len(pairs):
            raise ValueError('Frozen tool manifest has duplicate keys')
        return obj
    return json.loads(data, object_pairs_hook=unique_object)


def _verify_manifest(root, tools, expected_schema):
    # The historical contract is exercised explicitly by the legacy fixture.
    # The workflow CLI exclusively calls verify(), which fixes schema 19.
    expected_tables(expected_schema)
    if not tools.is_absolute():
        raise ValueError('Frozen tool directory must be absolute')
    directory_identity = verify_private_directory(tools)
    record = read_manifest(tools)
    if type(record) is not dict or type(record.get('schema')) is not int or record['schema'] != expected_schema:
        raise ValueError(f'Frozen tool manifest must be exact schema{expected_schema}')
    if record.get('frozen_directory') != str(tools.resolve(strict=True)):
        raise ValueError('Frozen tool directory changed')
    if any(record.get(key) != value for key, value in source_identity(root).items()):
        raise ValueError('Frozen tools are not from the current clean checkout')
    hashes = record.get('binaries')
    if (type(hashes) is not dict or set(hashes) != set(TOOL_NAMES)
            or any(type(value) is not str or re.fullmatch(r'[a-f0-9]{64}', value) is None
                   for value in hashes.values())):
        raise ValueError('Frozen manifest must bind both normal product tools')
    entries = {name: verify_private_tool(tools / name) for name in hashes}
    if expected_schema == CURRENT_SCHEMA:
        if set(record) != MANIFEST_FIELDS or record['tracked_clean'] is not True:
            raise ValueError('Frozen tool manifest lacks the current receipt fields')
        source = record.get('source_directory')
        if (type(source) is not str or '\0' in source or not Path(source).is_absolute()
                or record.get('source_binaries_after') != hashes or record.get('frozen_binaries_after') != hashes):
            raise ValueError('Frozen test tool source or post-probe digests are unbound')
        if record.get('frozen_file_identities') != {name: private_identity(meta) for name, meta in entries.items()}:
            raise ValueError('Frozen test tool entry identity changed')
        validate_current_probe(record.get('probe'))
    if any(sha256(tools / name) != value for name, value in hashes.items()):
        raise ValueError('Frozen tool bytes changed while running tests')
    if (verify_private_directory(tools) != directory_identity
            or any(file_identity(verify_private_tool(tools / name)) != file_identity(meta)
                   for name, meta in entries.items())):
        raise ValueError('Frozen tools changed while verifying')
    return record


def verify(root, tools):
    return _verify_manifest(root, tools, CURRENT_SCHEMA)


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
    # Diagnostic observation only; tool identity remains bound by the existing
    # checkout, private-file and complete SHA-256 checks, not these byte counts.
    record['observed_binary_bytes'] = {
        name: (args.output / name).stat().st_size for name in ('baton', 'batond')
    }
    print(json.dumps(record, sort_keys=True))


if __name__ == '__main__':
    main()
