#!/usr/bin/env python3
"""Genuine normal18 project/task history via production UDS; no SQL synthesis."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import time
import uuid

PATH = Path(__file__).with_name('prepare-schema18-migration-fixture.py')
SPEC = importlib.util.spec_from_file_location('schema18_provenance', PATH)
H = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(H)
CONTRACT = 'agentbaton.actual-project-task-schema18.v1'


def encoded(value):
    if isinstance(value, bytes):
        return {'sqlite_type': 'blob', 'hex': value.hex()}
    if value is None:
        return {'sqlite_type': 'null'}
    return {'sqlite_type': 'integer' if type(value) is int else 'real' if type(value) is float else 'text',
            'value': value}


def probe(path):
    H.read_regular(path, 16_777_216)
    with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True) as db:
        db.execute('PRAGMA query_only=ON')
        db.execute('BEGIN')
        version = db.execute('PRAGMA user_version').fetchone()[0]
        H.require(type(version) is int and version == 18, 'Expected actual predecessor18, never19 stamp')
        schema = db.execute('SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name').fetchall()
        tables = sorted(row[1] for row in schema if row[0] == 'table')
        H.require(tables == list(H.SCHEMA_TABLES), 'Expected complete63 predecessor tables; no Host19')
        H.require(db.execute('PRAGMA quick_check').fetchall() == [('ok',)]
                  and db.execute('PRAGMA foreign_key_check').fetchall() == [], 'Corrupt predecessor database')
        rows, counts = {}, {}
        for name in tables:
            result = db.execute('SELECT rowid,* FROM "' + name.replace('"', '""') + '" ORDER BY rowid').fetchall()
            H.require(len(result) <= 1024, 'Predecessor sample row bound exceeded')
            counts[name] = len(result)
            rows[name] = [[encoded(value) for value in row] for row in result]
        sequence = db.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
        image = dict(schema=schema, rows=rows, user_version=version,
                     application_id=db.execute('PRAGMA application_id').fetchone()[0])
        return dict(schema=version, query_only=True, foreign_key_errors=0, counts=counts, sequence=sequence,
                    logical_sha256=H.sha(H.canonical(image)),
                    schema_sha256=H.sha(H.canonical(schema)),
                    schema_cookie=db.execute('PRAGMA schema_version').fetchone()[0],
                    application_id=image['application_id'])


def inventory(runtime):
    result = {}
    for name in ('baton.sqlite3', 'baton.sqlite3-wal', 'baton.sqlite3-shm'):
        path = runtime / name
        if not path.exists():
            H.require(not path.is_symlink(), 'Dangling SQLite inventory link')
            result[name] = {'exists': False}
        else:
            raw = H.read_regular(path, 16_777_216)
            result[name] = dict(exists=True, bytes=len(raw), sha256=H.sha(raw), mode='0o600')
    return result


def wait_ready(runtime, child):
    deadline = time.monotonic() + 15
    while not ((runtime / 'session.token').exists() and (runtime / 'batond.sock').exists()):
        H.require(H.NATIVE.owned_child_exited(child.pid) is False and time.monotonic() < deadline,
                  'Owned normal18 daemon readiness failed')
        time.sleep(.02)
    info = (runtime / 'batond.sock').lstat()
    H.require(stat.S_ISSOCK(info.st_mode) and info.st_uid == os.geteuid()
              and stat.S_IMODE(info.st_mode) == 0o600, 'Unsafe owned socket')


def validate_report(m):
    H.require(type(m) is dict and m.get('contract') == CONTRACT and m.get('status') == 'passed'
              and type(m.get('schema')) is int and m['schema'] == 18
              and m['source_commit'] == H.SOURCE and m['source_tree'] == H.TREE
              and m['origin'] == 'normal_accepted18_production_rpc', 'Wrong genuine18 evidence contract')
    H.require(m['platform'] == sys.platform and m['architecture'] == platform.machine(), 'Wrong evidence platform')
    H.require(m['generator_sha256'] == H.sha(Path(__file__).read_bytes())
              and m['helper_sha256'] == H.sha(PATH.read_bytes())
              and H.strict_equal(m['helpers_sha256'], H.HELPERS), 'Changed producer/helpers')
    H.require(H.strict_equal(m['actual_cli_probe'], {'exit_code': 0, 'api_version': 1, 'database_schema_version': 18})
              and m['owner_rejection'] == 'AUTH_REQUIRED' and m['conflict_rejection'] == 'CONFIG_CONFLICT'
              and m['receipt_replay_exact'] is True, 'Missing actual tool/owner/replay evidence')
    d = m['daemon']
    H.require(d['sigterm_sent'] is True and d['forced_kill'] is False
              and type(d['daemon_wait_exit']) is int and d['daemon_wait_exit'] == 0
              and d['identity_gone_after_join'] is True and d['group_quiescent_before_reaping'] is True
              and d['leader_unreaped_until_signalling'] is True and d['cleanup_errors'] == [],
              'No confirmed normal owned daemon join')
    first, last = m['initial_probe'], m['final_probe']
    for item in (first, last):
        H.require(type(item['schema']) is int and item['schema'] == 18 and item['query_only'] is True
                  and type(item['foreign_key_errors']) is int and item['foreign_key_errors'] == 0
                  and set(item['counts']) == set(H.SCHEMA_TABLES)
                  and all(type(n) is int and 0 <= n <= 1024 for n in item['counts'].values())
                  and H.OLD.digest(item['logical_sha256']) and H.OLD.digest(item['schema_sha256']),
                  'Wrong complete typed18 snapshot')
    H.require(first['counts']['tasks'] == 0 and type(first['sequence']) is int and first['sequence'] == 0
              and last['counts']['tasks'] == 3 and last['counts']['projects'] == 1
              and last['counts']['events'] == 5 and last['counts']['idempotency'] == 4
              and type(last['sequence']) is int and type(m['final_api_sequence']) is int
              and last['sequence'] == m['final_api_sequence'] == 5
              and all(last['counts'][name] == 0 for name in H.EMPTY_AUTHORITY),
              'Missing genuine nonempty history or unexpected authority')
    H.require(m['backup_logical_exact'] is True and m['source_content_unchanged'] is True
              and m['linked_journal_verified'] is True
              and H.strict_equal(m['tool_metadata'], m['tool_metadata_after'])
              and set(m['binaries']) == {'batond', 'baton'}
              and all(m['binaries'][n] == m['tool_metadata'][n]['sha256'] for n in m['binaries']),
              'Unbound/changed source, links, backup or tools')
    H.require(H.OLD.digest(m['database_sha256']) and type(m['database_bytes']) is int
              and 0 < m['database_bytes'] <= 16_777_216 and H.OLD.digest(m['tool_provenance_sha256'])
              and H.OLD.digest(m['rpc_transcript_sha256']), 'Missing private artifact digests')


def verify_fixture(output, frozen, build_manifest):
    manifest = H.parse_json(H.read_regular(output / 'manifest.json', 4_194_304))
    validate_report(manifest)
    build, raw, tools = H.provenance(frozen, build_manifest)
    H.require(H.sha(raw) == manifest['tool_provenance_sha256']
              and H.strict_equal(tools, manifest['tool_metadata']), 'Wrong build/tool manifest binding')
    H.require(H.sha(H.read_regular(output / 'tool-provenance.json', 4_194_304)) == H.sha(raw)
              and H.sha(H.read_regular(output / 'rpc-private.jsonl', 1_048_576)) == manifest['rpc_transcript_sha256'],
              'Changed retained tool/RPC evidence')
    path = output / 'actual-schema18.sqlite3'
    data = H.read_regular(path, 16_777_216)
    H.require(H.sha(data) == manifest['database_sha256'] and len(data) == manifest['database_bytes'],
              'Changed predecessor bytes; reject before SQLite')
    actual = probe(path)
    H.require(actual['logical_sha256'] == manifest['final_probe']['logical_sha256']
              and actual['schema_sha256'] == manifest['final_probe']['schema_sha256']
              and actual['counts'] == manifest['final_probe']['counts'] and actual['sequence'] == 5,
              'Wrong bound actual18 database')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--frozen', type=Path, required=True)
    parser.add_argument('--build-manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    H.native_platform()
    output = args.output.absolute()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    output = output.resolve()
    report = dict(contract=CONTRACT, status='failed_or_interrupted', schema=18,
        origin='normal_accepted18_production_rpc', source_commit=H.SOURCE, source_tree=H.TREE,
        platform=sys.platform, architecture=platform.machine(), os_release=platform.release(),
        generator_sha256=H.sha(Path(__file__).read_bytes()), helper_sha256=H.sha(PATH.read_bytes()),
        helpers_sha256=H.HELPERS,
        coverage=['projects.import', 'tasks.create', 'tasks.cancel', 'events.subscribe', 'idempotent replay/conflict'],
        not_claimed=['nonempty Conversation/Factory/Goal/integration/Host history', 'Provider/SSH/approval',
                     'Store19 migration/backup/reopen/rollback', 'P1 Rust consumer', 'native GUI/CI'])
    child = None
    error = None
    completed = False
    try:
        frozen = args.frozen
        build, raw, tools = H.provenance(frozen, args.build_manifest)
        report.update(tool_provenance_sha256=H.sha(raw), tool_metadata=tools,
                      binaries={n: t['sha256'] for n, t in tools.items()})
        with (output / 'tool-provenance.json').open('xb') as stream:
            os.fchmod(stream.fileno(), 0o600)
            stream.write(raw)
        root = Path(tempfile.mkdtemp(prefix='ab18-history-', dir='/tmp')).resolve()
        root.chmod(0o700)
        runtime = root / 'r'
        source = root / 'source'
        runtime.mkdir(mode=0o700)
        source.mkdir(mode=0o700)
        source_bytes = 'Genuine accepted18 project sample 雪\n'.encode()
        (source / 'input.txt').write_bytes(source_bytes)
        report.update(retained_private_root=str(root), runtime=str(runtime))
        H.require(len(os.fsencode(runtime / 'batond.sock')) <= 100, 'Socket path bound')
        env = {k: v for k, v in os.environ.items() if k in ('PATH', 'LANG', 'LC_ALL', 'TMPDIR')}
        env['BATON_RUNTIME_DIR'] = str(runtime)
        with (output / 'daemon.log').open('xb') as log:
            os.fchmod(log.fileno(), 0o600)
            report['daemon'] = dict(owned_identity=None, sigterm_sent=False, forced_kill=False)
            child = subprocess.Popen([str(frozen / 'batond')], env=env, stdout=log,
                                     stderr=subprocess.STDOUT, start_new_session=True)
            report['daemon']['owned_identity'] = H.COMMON.darwin_owned_identity(child)
            wait_ready(runtime, child)
            report['initial_probe'] = probe(runtime / 'baton.sqlite3')
            token = H.read_regular(runtime / 'session.token', 1024).decode()
            with (output / 'rpc-private.jsonl').open('xb') as transcript:
                os.fchmod(transcript.fileno(), 0o600)
                def call(method, params, key=None, revision=None, expected_error=None, wrong_token=False):
                    request = dict(api_version=1, request_id=str(uuid.uuid4()), method=method, params=params)
                    if key is not None:
                        request['idempotency_key'] = key
                    if revision is not None:
                        request['expected_revision'] = revision
                    wire = json.dumps(dict(request, token='wrong-private-owner' if wrong_token else token),
                                      allow_nan=False, separators=(',', ':')).encode() + b'\n'
                    H.require(len(wire) <= 16_384, 'Bounded generated request required')
                    with socket.socket(socket.AF_UNIX) as connection:
                        connection.settimeout(10)
                        connection.connect(str(runtime / 'batond.sock'))
                        connection.sendall(wire)
                        with connection.makefile('rb') as reader:
                            response = reader.readline(1_048_577)
                    H.require(0 < len(response) <= 1_048_576 and response.endswith(b'\n'), 'Response frame bound')
                    reply = H.parse_json(response)
                    H.require(reply.get('request_id') == request['request_id'], 'Uncorrelated RPC response')
                    transcript.write(json.dumps(dict(request_without_token=request, reply=reply),
                                                ensure_ascii=False).encode() + b'\n')
                    transcript.flush()
                    if expected_error is not None:
                        H.require(reply.get('error', {}).get('code') == expected_error, 'Wrong actual rejection')
                        return reply
                    H.require('result' in reply and not reply.get('error'), 'Actual production RPC failed')
                    return reply['result']
                initial = call('system.snapshot', {})
                H.require(initial['api_version'] == 1 and type(initial['database_schema_version']) is int
                          and initial['database_schema_version'] == 18 and initial['tasks'] == []
                          and initial['sequence'] == 0, 'Wrong actual18 protocol/snapshot')
                call('system.snapshot', {}, wrong_token=True, expected_error='AUTH_REQUIRED')
                report['owner_rejection'] = 'AUTH_REQUIRED'
                cli = subprocess.run([str(frozen / 'baton'), 'snapshot'], env=env, capture_output=True, timeout=10)
                H.require(cli.returncode == 0 and len(cli.stdout) <= 1_048_576 and len(cli.stderr) <= 262_144,
                          'Actual companion snapshot failed')
                cli_value = H.parse_json(cli.stdout)['result']
                H.require(H.strict_equal(cli_value, initial), 'Companion/daemon snapshot mismatch')
                report['actual_cli_probe'] = dict(exit_code=0, api_version=1, database_schema_version=18)
                project = call('projects.import', dict(path=str(source), name='Genuine18 generated project'), 'p18-project')
                tasks = []
                for i, project_id in enumerate([project['id'], project['id'], 'local']):
                    params = dict(project_id=project_id, goal='Genuine18 task ' + str(i) + ' 雪')
                    key = 'p18-task-' + str(i)
                    task = call('tasks.create', params, key)
                    H.require(task['state'] == 'draft' and H.strict_equal(task, call('tasks.create', params, key)),
                              'Original creation receipt did not replay')
                    tasks.append(task)
                cancelled = call('tasks.cancel', dict(task_id=tasks[1]['id']), revision=tasks[1]['revision'])
                H.require(cancelled['state'] == 'cancelled', 'Cancellation not durable')
                original = call('tasks.create', dict(project_id=project['id'], goal='Genuine18 task 1 雪'), 'p18-task-1')
                H.require(H.strict_equal(original, tasks[1]), 'Later cancellation rewrote original receipt')
                call('tasks.create', dict(project_id=project['id'], goal='conflicting body'), 'p18-task-0',
                     expected_error='CONFIG_CONFLICT')
                report['conflict_rejection'] = 'CONFIG_CONFLICT'
                report['receipt_replay_exact'] = True
                final = call('system.snapshot', {})
                journal = call('events.subscribe', dict(cursor=0, limit=32))
                H.require(final['database_schema_version'] == 18 and len(final['tasks']) == 3
                          and final['sequence'] == journal['sequence'] == 5
                          and [x['sequence'] for x in journal['events']] == list(range(1, 6)),
                          'Missing actual linked18 journal')
                H.require({x['aggregate_id'] for x in journal['events']} == {project['id'], *(x['id'] for x in tasks)},
                          'Journal aggregate links mismatch')
                H.require(all(set(x) == {'sequence', 'event_id', 'aggregate_id', 'revision', 'kind',
                              'occurred_at', 'payload', 'source'} for x in journal['events']), 'Wrong typed Event envelope')
                report.update(linked_journal_verified=True, final_api_sequence=final['sequence'])
        completed = True
    except BaseException as caught:
        error = caught
        report['failure_kind'] = type(caught).__name__
    finally:
        try:
            if child is not None:
                H.COMMON.stop_darwin_owned(child, H.NATIVE, report['daemon'])
                H.require(report['daemon'].get('identity_gone_after_join') is True, 'Daemon stop not confirmed')
            if completed and error is None:
                report['tool_metadata_after'] = H.tool_records(frozen)
                H.require(H.strict_equal(report['tool_metadata_after'], tools), 'Frozen tools changed')
                H.require((source / 'input.txt').read_bytes() == source_bytes
                          and sorted(x.name for x in source.iterdir()) == ['input.txt'], 'Generated source changed')
                report['source_content_unchanged'] = True
                report['joined_sqlite_inventory_before_read'] = inventory(runtime)
                report['final_probe'] = probe(runtime / 'baton.sqlite3')
                destination = output / 'actual-schema18.sqlite3'
                fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                os.close(fd)
                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as original_db:
                    original_db.execute('PRAGMA query_only=ON')
                    with sqlite3.connect(destination) as backup:
                        original_db.backup(backup)
                copied = probe(destination)
                H.require(copied['logical_sha256'] == report['final_probe']['logical_sha256']
                          and copied['counts'] == report['final_probe']['counts'], 'SQLite backup logical mismatch')
                report.update(backup_logical_exact=True, backup_probe=copied,
                              database_sha256=H.sha(H.read_regular(destination, 16_777_216)),
                              database_bytes=destination.stat().st_size,
                              rpc_transcript_sha256=H.sha(H.read_regular(output / 'rpc-private.jsonl', 1_048_576)),
                              joined_sqlite_inventory_after_read=inventory(runtime), status='passed')
                validate_report(report)
                H.provenance(frozen, args.build_manifest)
        except BaseException as caught:
            report['status'] = 'failed_or_interrupted'
            report['cleanup_or_snapshot_failure_kind'] = type(caught).__name__
            if error is None:
                error = caught
        H.COMMON.publish_private_json(output / 'manifest.json', report)
    if error is not None:
        raise error
    verified = verify_fixture(output, frozen, args.build_manifest)
    print(json.dumps(dict(status=verified['status'], schema=18, sequence=verified['final_api_sequence'],
                          sha256=verified['database_sha256'], bytes=verified['database_bytes'])))


if __name__ == '__main__':
    main()
