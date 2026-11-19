#!/usr/bin/env python3
"""Rehearse one verified Apple Silicon development package on its sealed fixture."""
import argparse
from contextlib import closing
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import signal
import sqlite3
import stat
import struct
import subprocess
import sys
import tempfile
import time


COMMIT = '14530c62a0b532e2ba43ca4be4fbc75ccc41d856'
TREE = '41b3e2fe076ee4ebcb6355dddc1505f6bb75e135'
PACKAGE = f'AgentBaton-arm64-{COMMIT}-development.dmg'
PACKAGE_SHA = '199090d5dbd25a596e18baf6e4e6cb6436e3b4ecf57c02081f4d49a9831c4498'
DAEMON_SHA = '0b18add53fc794e6684690a90b74542e487f18409216b2724c8d3d438785a1a1'
VERSION = '0.1.0'
LEGACY_COMMIT = 'a4e03303b54bd5a5544131e95ad39662f8310e29'
FIXTURE_SHA = '2ef73ba584bd368923a5834bb0e84189f414cd43f739bfe0a020fd4777b9a71d'
SOURCE_SCHEMA, TARGET_SCHEMA = 18, 19
MAX_DB = 4 * 1024 * 1024
INPUTS = (PACKAGE, 'batond', 'historical-store.sqlite3')
HISTORICAL_ROWS = {'projects': 1, 'tasks': 3, 'events': 5, 'idempotency': 4, 'sqlite_sequence': 1}
HOST_OBJECTS = {('table', 'host_registrations'), ('index', 'host_registrations_name_id'),
                ('index', 'sqlite_autoindex_host_registrations_1'),
                ('index', 'sqlite_autoindex_host_registrations_2')}


class Refused(ValueError):
    pass


def require(condition, code):
    if not condition:
        raise Refused(code)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fingerprint(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def private_directory(path):
    info = path.lstat()
    require(path.is_absolute() and path.resolve(strict=True) == path
            and stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
            and info.st_uid == os.geteuid(), 'UNSAFE_PRIVATE_DIRECTORY')
    return info.st_dev, info.st_ino


def read_file(path, limit, mode):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == mode
            and info.st_uid == os.geteuid() and info.st_nlink == 1
            and 0 < info.st_size <= limit, 'UNSAFE_INPUT_FILE')
    # Check the held descriptor as well as the name before and after the read.
    pin = fingerprint(info)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        require(fingerprint(os.fstat(fd)) == pin, 'INPUT_CHANGED')
        raw = source.read(limit + 1)
        require(len(raw) == info.st_size and fingerprint(os.fstat(fd)) == pin
                and fingerprint(path.lstat()) == pin, 'INPUT_CHANGED')
    return raw, pin


def create_file(path, raw, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as result:
        os.fchmod(fd, mode)
        result.write(raw)
        result.flush()
        os.fsync(fd)


def standalone(path):
    for suffix in ('-wal', '-shm', '-journal'):
        # A dangling link is also unexpected.
        require(not os.path.lexists(path.with_name(path.name + suffix)), 'SQLITE_SIDECAR_PRESENT')


@dataclass(frozen=True)
class Snapshot:
    schema: int
    sequence: int
    objects: dict
    rows: dict
    task_states: frozenset
    event_kinds: frozenset

    def summary(self):
        return {'schema': self.schema, 'sequence': self.sequence,
                'nonempty_tables': {table: len(rows) for table, rows in self.rows.items() if rows}}


def quote_identifier(name):
    return '"' + name.replace('"', '""') + '"'


def inspect_db(path):
    """Read a closed, standalone private database for internal comparison."""
    private_directory(path.parent)
    standalone(path)
    raw, pin = read_file(path, MAX_DB, 0o600)
    require(raw.startswith(b'SQLite format 3\x00'), 'DATABASE_FORMAT_UNSUPPORTED')
    uri = path.as_uri() + '?mode=ro&immutable=1'
    with closing(sqlite3.connect(uri, uri=True)) as db:
        db.execute('PRAGMA query_only=ON')
        db.execute('PRAGMA trusted_schema=OFF')
        require(db.execute('PRAGMA quick_check(1)').fetchall() == [('ok',)]
                and db.execute('PRAGMA foreign_key_check').fetchone() is None, 'DATABASE_INTEGRITY_UNPROVED')
        schema, = db.execute('PRAGMA user_version').fetchone()
        sequence, = db.execute('SELECT COALESCE(max(sequence),0) FROM events').fetchone()
        require(type(sequence) is int and sequence >= 0, 'DATABASE_SEQUENCE_UNKNOWN')
        catalog = db.execute('SELECT type,name,tbl_name,sql FROM sqlite_schema LIMIT 512').fetchall()
        require(len(catalog) < 512, 'DATABASE_CATALOG_UNSUPPORTED')
        objects = {(kind, name): (table, sql) for kind, name, table, sql in catalog}
        rows = {}
        for kind, table in objects:
            if kind == 'table':
                data = db.execute(f'SELECT * FROM {quote_identifier(table)} LIMIT 32').fetchall()
                require(len(data) < 32, 'DATABASE_CONTENT_UNSUPPORTED')
                rows[table] = tuple(sorted(map(repr, data)))
        task_states = frozenset(state for state, in
                                db.execute("SELECT DISTINCT json_extract(document,'$.state') FROM tasks"))
        event_kinds = frozenset(kind for kind, in
                                db.execute("SELECT DISTINCT json_extract(document,'$.kind') FROM events"))
    standalone(path)
    require(fingerprint(path.lstat()) == pin, 'INPUT_CHANGED')
    return Snapshot(schema, sequence, objects, rows, task_states, event_kinds), sha(raw), pin[:2]


def prove_idle_fixture(snapshot):
    """Require the work-free state established for the generated fixture."""
    require(snapshot.schema == SOURCE_SCHEMA and snapshot.sequence == 5
            and {'connections', 'attempts', 'dispatch_intents', 'sessions',
                 'conversation_dispatches', 'goal_runs', 'factory_runs'} <= snapshot.rows.keys(),
            'FIXTURE_STATE_UNPROVED')
    require(snapshot.summary()['nonempty_tables'] == HISTORICAL_ROWS, 'FIXTURE_WORK_OR_CONTENT_UNPROVED')
    require(snapshot.task_states <= {'draft', 'cancelled'}
            and snapshot.event_kinds <= {'project.imported', 'task.created', 'task.cancelled'},
            'FIXTURE_WORK_OR_CONTENT_UNPROVED')


def verify_migration(before, after):
    require(after.schema == TARGET_SCHEMA and before.sequence == after.sequence,
            'MIGRATION_SCHEMA_OR_SEQUENCE_MISMATCH')
    require(after.objects.keys() - before.objects.keys() == HOST_OBJECTS
            and all(after.objects.get(key) == value for key, value in before.objects.items())
            and after.rows == {**before.rows, 'host_registrations': ()}, 'MIGRATION_CONTENT_MISMATCH')


def bounded_start(child, runtime, timeout=15):
    marker = f'batond API v1 ready at {runtime}\n'.encode()
    deadline = time.monotonic() + timeout
    observed = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(child.stderr, selectors.EVENT_READ)
        while time.monotonic() < deadline:
            if not selector.select(max(0, deadline - time.monotonic())):
                break
            data = os.read(child.stderr.fileno(), 1024)
            if not data:
                break
            observed.extend(data)
            require(len(observed) <= 4096, 'DAEMON_READINESS_UNPROVED')
            if marker in observed.splitlines(keepends=True):
                socket = (runtime / 'batond.sock').lstat()
                require(stat.S_ISSOCK(socket.st_mode) and stat.S_IMODE(socket.st_mode) == 0o600
                        and socket.st_uid == os.geteuid(), 'DAEMON_READINESS_UNPROVED')
                return
    raise Refused('DAEMON_READINESS_UNPROVED')


def join_candidate(child, timeout=8):
    # Hold the original group leader unreaped until the entire owned group is
    # proven quiescent. Never send a group signal after wait() consumes that PID.
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT):
            break
        time.sleep(0.02)
    else:
        raise Refused('DAEMON_JOIN_UNPROVED')
    group = subprocess.run(
        ['/bin/ps', '-o', 'pid=,pgid=,stat=', '-g', str(child.pid)],
        env={'LC_ALL': 'C'}, capture_output=True, text=True, timeout=1, check=False,
    )
    rows = [line.split() for line in group.stdout.splitlines()]
    require(group.returncode == 0 and len(rows) == 1 and len(rows[0]) == 3
            and rows[0][:2] == [str(child.pid)] * 2 and rows[0][2].startswith('Z'),
            'DAEMON_GROUP_UNPROVED')
    require(child.wait(timeout=1) == 0, 'DAEMON_SHUTDOWN_FAILED')
    return {'pid': child.pid, 'pgid': child.pid, 'sid': child.pid, 'ready': True,
            'stop_signal': 'SIGTERM', 'only_unreaped_leader_before_join': True,
            'joined': True, 'exit': 0}


def migrate_with_candidate(binary, private, runtime):
    env = {'BATON_RUNTIME_DIR': str(runtime), 'LC_ALL': 'C', 'PATH': ''}
    for variable, name in [('HOME', 'home'), ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'),
                           ('XDG_CACHE_HOME', 'cache'), ('XDG_STATE_HOME', 'state'), ('TMPDIR', 'tmp')]:
        directory = private / name
        directory.mkdir(mode=0o700)
        env[variable] = str(directory)
    require(len(os.fsencode(runtime / 'batond.sock')) < 104, 'RUNTIME_SOCKET_PATH_TOO_LONG')
    child = subprocess.Popen([str(binary)], cwd=private, env=env, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, start_new_session=True)
    try:
        require(os.getpgid(child.pid) == child.pid and os.getsid(child.pid) == child.pid,
                'DAEMON_IDENTITY_UNPROVED')
        bounded_start(child, runtime)
        os.killpg(child.pid, signal.SIGTERM)
        return join_candidate(child)
    finally:
        child.stderr.close()
        if child.returncode is None:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            finally:
                child.wait(timeout=5)


def read_inputs(source):
    identity = private_directory(source)
    standalone(source / INPUTS[2])
    files = {}
    for name, limit, mode, expected, error in (
        (PACKAGE, 32 * 1024 * 1024, 0o600, PACKAGE_SHA, 'CANDIDATE_PACKAGE_MISMATCH'),
        ('batond', 32 * 1024 * 1024, 0o500, DAEMON_SHA, 'CANDIDATE_DAEMON_MISMATCH'),
        ('historical-store.sqlite3', MAX_DB, 0o600, FIXTURE_SHA, 'HISTORICAL_FIXTURE_MISMATCH'),
    ):
        raw, pin = read_file(source / name, limit, mode)
        digest = sha(raw)
        require(digest == expected, error)
        files[name] = raw, digest, pin
    standalone(source / INPUTS[2])
    require(private_directory(source) == identity, 'INPUT_CHANGED')
    daemon = files['batond'][0]
    magic, architecture, _, file_type = struct.unpack_from('<IIII', daemon)
    require((magic, architecture, file_type) == (0xfeedfacf, 0x0100000c, 2),
            'CANDIDATE_ARCHITECTURE_MISMATCH')
    return files


def proof(snapshot, digest, name):
    return {'name': name, 'sha256': digest, **snapshot.summary()}


def rehearse(task):
    private_directory(task)
    source = task / 'update-inputs'
    inputs = read_inputs(source)
    frozen_daemon = inputs['batond'][0]
    source_raw, input_sha, input_pin = inputs[INPUTS[2]]
    source_db = source / INPUTS[2]
    initial, inspected_sha, source_identity = inspect_db(source_db)
    require(inspected_sha == input_sha and source_identity == input_pin[:2], 'INPUT_CHANGED')
    prove_idle_fixture(initial)

    # Keep only verified evidence files on success. Remove candidate runtime
    # (including its generated IPC state) and partial evidence on failure.
    with tempfile.TemporaryDirectory(prefix='update-', dir=task) as stage_name:
        stage = Path(stage_name)
        report_dir = stage / 'evidence'
        report_dir.mkdir(mode=0o700)
        backup = report_dir / 'backup.sqlite3'
        create_file(backup, source_raw)
        backed_up, backup_sha, backup_identity = inspect_db(backup)
        require(backed_up == initial and backup_sha == input_sha, 'BACKUP_MISMATCH')

        with tempfile.TemporaryDirectory(prefix='core-', dir=task) as private_name:
            private = Path(private_name)
            binary = private / 'batond'
            create_file(binary, frozen_daemon, mode=0o500)
            runtime = private / 'r'
            runtime.mkdir(mode=0o700)
            live = runtime / 'baton.sqlite3'
            # Migration receives a separate copy of the checked backup.
            backup_raw, _ = read_file(backup, MAX_DB, 0o600)
            require(sha(backup_raw) == backup_sha, 'BACKUP_MISMATCH')
            create_file(live, backup_raw)
            before, live_sha, live_identity = inspect_db(live)
            require(before == initial and live_sha == backup_sha
                    and len({source_identity, backup_identity, live_identity}) == 3, 'MIGRATION_INPUT_MISMATCH')
            prove_idle_fixture(before)
            child = migrate_with_candidate(binary, private, runtime)
            migrated, migrated_sha, migrated_identity = inspect_db(live)
            require(migrated_identity == live_identity, 'MIGRATION_OUTPUT_CHANGED_IDENTITY')
            verify_migration(initial, migrated)
            core_backups = list(runtime.glob('baton.pre-v19-*.sqlite3'))
            require(len(core_backups) == 1, 'CORE_BACKUP_UNPROVED')
            core_backup, core_backup_sha, core_identity = inspect_db(core_backups[0])
            require(core_backup == initial and core_identity not in
                    {source_identity, backup_identity, live_identity}, 'CORE_BACKUP_MISMATCH')
            migrated_raw, _ = read_file(live, MAX_DB, 0o600)
            require(sha(migrated_raw) == migrated_sha, 'MIGRATION_OUTPUT_CHANGED_IDENTITY')
            archive = report_dir / 'migrated.sqlite3'
            create_file(archive, migrated_raw)
            require(inspect_db(archive)[:2] == (migrated, migrated_sha), 'MIGRATION_OUTPUT_MISMATCH')

            # Rehearse replacement on the same private runtime path. Preserve
            # the migrated bytes in the evidence dir before restoring.
            replacement = runtime / 'restore-part.sqlite3'
            create_file(replacement, backup_raw)
            os.replace(replacement, live)
            restored, restored_sha, restored_identity = inspect_db(live)
            require(restored == initial and restored_sha == backup_sha and restored_identity not in
                    {source_identity, backup_identity, live_identity, core_identity}, 'RESTORE_MISMATCH')
            live.rename(report_dir / 'restored.sqlite3')

        post = read_inputs(source)
        require(all(post[name][1:] == inputs[name][1:] for name in INPUTS), 'INPUT_CHANGED')
        require(inspect_db(backup) == (initial, backup_sha, backup_identity), 'BACKUP_MISMATCH')
        report = {
            'contract': 'agentbaton.development-update-rehearsal.v1',
            'status': 'rehearsed',
            'installation_authorized': False,
            'scope': 'isolated_generated_fixture_only',
            'candidate': {'version': VERSION, 'commit': COMMIT, 'tree': TREE, 'target_schema': TARGET_SCHEMA,
                          'os': 'macos', 'arch': 'aarch64', 'channel': 'development',
                          'developer_id_signed': False, 'artifact_sha256': PACKAGE_SHA,
                          'bundled_daemon_sha256': DAEMON_SHA},
            'source': {'historical_commit': LEGACY_COMMIT, **initial.summary()},
            'inputs': {name: {'before_sha256': inputs[name][1], 'after_sha256': post[name][1]}
                       for name in INPUTS},
            'process': child,
            'backup': proof(backed_up, backup_sha, 'backup.sqlite3'),
            'core_migration_backup': {'sha256': core_backup_sha, **core_backup.summary()},
            'migration': {'input_backup_sha256': backup_sha,
                          **proof(migrated, migrated_sha, 'migrated.sqlite3')},
            'recovery': {'migration_output_sha256': migrated_sha,
                         **proof(restored, restored_sha, 'restored.sqlite3')},
        }
        receipt = json.dumps(report, indent=2, sort_keys=True) + '\n'
        create_file(report_dir / 'receipt.json', receipt.encode())
        # Publish the complete private result as one new directory.
        evidence = stage.with_name(stage.name + '-evidence')
        require(not os.path.lexists(evidence), 'EVIDENCE_DESTINATION_EXISTS')
        report_dir.rename(evidence)
    return evidence


def task_runtime():
    require(sys.platform == 'darwin' and platform.machine() == 'arm64', 'CANDIDATE_PLATFORM_UNSUPPORTED')
    value = os.environ.get('BATON_RUNTIME_DIR')
    require(value, 'COLLAB_RUNTIME_REQUIRED')
    path = Path(value)
    require(path.parent == Path('/private/tmp') and path.name.startswith('ab-collab-'),
            'COLLAB_RUNTIME_REQUIRED')
    return path


def interrupted(_number, _frame):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        evidence = rehearse(task_runtime())
        print(evidence / 'receipt.json')
        return 0
    except Refused as error:
        print(str(error), file=sys.stderr)
    except (OSError, sqlite3.Error, ValueError, subprocess.SubprocessError, struct.error):
        print('REHEARSAL_EVIDENCE_UNPROVED', file=sys.stderr)
    except KeyboardInterrupt:
        print('REHEARSAL_INTERRUPTED', file=sys.stderr)
        return 130
    finally:
        signal.signal(signal.SIGTERM, previous)
    return 1


if __name__ == '__main__':
    sys.exit(main())
