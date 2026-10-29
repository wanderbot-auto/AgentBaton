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
SCHEMA = 15
MAX_DB = 16 * 1024 * 1024
MAX_CASES = 16
MAX_ENTRIES = 64
MAX_ROWS = 512
MAX_PUBLIC = 128 * 1024
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


def project_db(raw):
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
            return {'schema': SCHEMA, 'child_graph_count': 1, 'runs': runs,
                    'node_counts': dict(sorted(nodes.items())),
                    'validation_counts': dict(sorted(validations.items()))}
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
        result['post_shutdown'] = project_db(raw)
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
