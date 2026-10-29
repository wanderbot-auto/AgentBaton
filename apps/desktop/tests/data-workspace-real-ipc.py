#!/usr/bin/env python3
"""Actual built React + fixed loopback adapter + real schema15 dataset-copy IPC.

Only generated sibling source/runtime directories are used. Owner confirmation
is an explicit host fixture OUTSIDE HTTP, never an OS dialog or Renderer grant.
The sole optional injected fault drops a real cancellation response after IPC;
it does not forge data, terminal states, approval or a copied directory.
"""
from __future__ import annotations

import argparse
from functools import partial
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import socket
import sqlite3
import stat
import subprocess
import tempfile
import threading
import time
import traceback
import uuid

from playwright.sync_api import expect, sync_playwright


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1_048_576), b''):
            digest.update(chunk)
    return digest.hexdigest()


def observation(path):
    value = path.stat(follow_symlinks=False)
    assert value.st_nlink == 1 and path.is_file()
    return {'sha256': file_sha(path), 'bytes': value.st_size, 'mtime_ns': value.st_mtime_ns}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('Duplicate test-adapter JSON field')
        value[key] = item
    return value


def integer(value, minimum=0, maximum=9_007_199_254_740_991):
    return type(value) is int and minimum <= value <= maximum


def copy_completion(record):
    """Observe a real terminal receipt; missing legacy metadata never implies join."""
    completion = record.get('io_completion')
    assert completion in {None, 'worker_joined', 'not_scheduled', 'interrupted_no_join_receipt'}, record
    assert record['state'] in {'ready', 'failed', 'cancelled', 'outcome_unknown'}, record
    joined = completion == 'worker_joined'
    if record['state'] == 'cancelled':
        error, observed = record.get('error') or {}, record.get('observation')
        assert joined and record['cancel_requested'] is True and record.get('contract') is None, record
        assert error.get('code') == 'CANCEL_REQUESTED' and error.get('side_effect_state') == 'confirmed', record
        assert observed is None or (observed['state'] == 'failed' and observed['error'] == 'cancel_requested'), record
    if completion == 'not_scheduled':
        assert record['state'] == 'failed' and record.get('contract') is None and record.get('observation') is None, record
    if completion == 'interrupted_no_join_receipt':
        assert record['state'] == 'outcome_unknown' and record.get('contract') is None, record
    return {'io_completion': completion, 'owned_copy_worker_join_observed': joined,
            'cancelled_owned_worker_join_observed': record['state'] == 'cancelled' and joined,
            'completion_unrecorded': completion is None,
            'general_filesystem_quiescence_claimed': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--frontend', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    parser.add_argument('--candidate-manifest', type=Path, help='Optional immutable candidate attribution; no build or authority is inferred')
    args = parser.parse_args()
    binary, frontend, output = args.batond.resolve(strict=True), args.frontend.resolve(strict=True), args.output.resolve()
    assert (frontend / 'index.html').is_file()
    output.mkdir(parents=True, exist_ok=False)  # interrupted evidence without report.json is still evidence
    report = {'status': 'failed_or_interrupted', 'dimension': 'actual built React / Chromium / fixed test-only HTTP / real Unix schema15 daemon / actual generated files',
              'owner_confirmation': 'host fixture outside HTTP, not a native OS OwnerDialog',
              'not_claimed': ['macOS', 'native WebView', 'Provider/session/model', 'OS sandbox', 'DLP', 'atomic directory freeze', 'execution grant', 'Task acceptance'],
              'daemon_sha256': sha(binary.read_bytes()), 'harness_sha256': sha(Path(__file__).read_bytes()),
              'frontend_assets_sha256': {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()},
              'checks': [], 'requests': [], 'faults': [], 'observations': {}, 'screenshots': [], 'unrun': [],
              'timing_semantics': {'clock': 'same_python_process_time.monotonic_ns',
                                   'http_received_ns': 'handler entry, not browser dispatch or Actor arrival',
                                   'pre_ipc_ns': 'immediately before synchronous authenticated Unix IPC call',
                                   'post_ipc_ns': 'IPC return or exception observed, not Actor execution time',
                                   'ui_click': 'Python click-call boundaries include Playwright actionability waiting; not a DOM event timestamp',
                                   'authority_or_stop_evidence': False}}
    if args.candidate_manifest:
        manifest_bytes = args.candidate_manifest.resolve(strict=True).read_bytes()
        manifest = json.loads(manifest_bytes)
        binary_entry = manifest['binaries']['batond']
        expected_binary = binary_entry['sha256'] if isinstance(binary_entry, dict) else binary_entry
        assert type(manifest.get('schema')) is int and manifest['schema'] == 15 and expected_binary == report['daemon_sha256']
        report['candidate'] = {'manifest_sha256': sha(manifest_bytes), 'status': manifest.get('status'),
                               'git_head': manifest['git_head'], 'dirty': manifest['dirty'],
                               'source_tree_sha256': manifest['source_tree_sha256'],
                               'not_final_review_or_ci_acceptance': True}
    pump = lambda: None
    token = ''

    def safe(text):
        return str(text).replace(token, '[redacted fixture authentication]') if token else str(text)

    def passed(name, **details):
        report['checks'].append({'name': name, 'status': 'passed', **details})

    # Observation includes the Core worker's fixed 30s I/O deadline; this does
    # not change that deadline or extend it to obtain a cancellation hit.
    def wait(read, predicate, seconds=35):
        end = time.monotonic() + seconds
        while True:
            pump()
            value = read()
            if predicate(value):
                return value
            assert time.monotonic() < end, safe(f'Fixture observation deadline exceeded: {value}')
            time.sleep(.02)

    with tempfile.TemporaryDirectory(prefix='baton-data-browser-') as temporary:
        root = Path(temporary).resolve()
        root.chmod(0o700)
        runtime, sources = root / 'runtime', root / 'sources'
        runtime.mkdir(mode=0o700)
        sources.mkdir(mode=0o700)
        assert not sources.is_relative_to(runtime) and not runtime.is_relative_to(sources)
        folders = {}
        original = {}
        for name in ['normal', 'preview-cancel', 'copy-cancel', 'drift']:
            folder = sources / name
            folder.mkdir(mode=0o700)
            if name in {'preview-cancel', 'copy-cancel'}:
                with (folder / 'large.bin').open('wb') as stream:
                    stream.truncate(32 * 1024 * 1024)  # real single-link regular sparse file, never FIFO/hardlink
            else:
                (folder / 'sample.csv').write_bytes(b'value\n1\n2\n')
                (folder / 'nested').mkdir()
                (folder / 'nested/note.txt').write_bytes(b'generated local fixture only\n')
            folders[name] = folder
            original[name] = {str(p.relative_to(folder)): observation(p) for p in folder.rglob('*') if p.is_file()}
        daemon_log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)}, stdout=daemon_log, stderr=subprocess.STDOUT)
        server = thread = None
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()), lambda value: value[0] and value[1] and value[2] is None)
            # Only this freshly generated fixture's auth token. Never a dataset
            # source, browser value, report/log field or another user's runtime.
            token = (runtime / 'session.token').read_text()

            def ipc(method, params, key=None, revision=None):
                frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token, 'method': method, 'params': params}
                if key is not None:
                    frame['idempotency_key'] = key
                if revision is not None:
                    frame['expected_revision'] = revision
                raw = json.dumps(frame).encode() + b'\n'
                assert len(raw) <= 262_144
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(5)
                    connection.connect(str(runtime / 'batond.sock'))
                    connection.sendall(raw)
                    with connection.makefile('rb') as stream:
                        response = stream.readline(8_388_609)
                assert len(response) <= 8_388_608, 'Bounded fixture response exceeded; not truncated'
                return json.loads(response)

            def result(method, params, key=None, revision=None):
                response = ipc(method, params, key, revision)
                assert not response.get('error'), safe({'method': method, 'response': response})
                return response['result']

            projects = {name: result('projects.import', {'path': str(folder), 'name': 'Generated data ' + name}, 'import:' + name) for name, folder in folders.items()}
            assert all(project['identity']['kind']['kind'] == 'directory' for project in projects.values())
            project_ids = {project['id'] for project in projects.values()}
            project_revisions = {project['id']: project['revision'] for project in projects.values()}
            allowed_files = {project['id']: set(original[name]) for name, project in projects.items()}
            preview_ids, creation_ids = {}, {}

            def database():
                with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True) as store:
                    store.execute('PRAGMA query_only=ON')
                    store.execute('BEGIN')
                    assert store.execute('PRAGMA user_version').fetchone()[0] == 15
                    assert store.execute('SELECT count(*) FROM factory_child_goal_runs').fetchone()[0] == 0
                    tables = ['data_workspace_previews', 'data_workspace_creations', 'managed_workspaces', 'approval_proposals', 'attempts', 'sessions', 'dispatch_intents']
                    snapshot = {table: store.execute('SELECT count(*) FROM ' + table).fetchone()[0] for table in tables}
                    snapshot['sequence'] = store.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
                    return snapshot

            def creation_records(project):
                return result('data_workspaces.list', {'project_id': project['id']})

            def preview_record(preview_id):
                return result('data_workspace_previews.get', {'preview_id': preview_id})

            def creation_record(creation_id):
                return result('data_workspaces.get', {'creation_id': creation_id})

            def owner_paths(proposal):
                snapshot, intent = proposal['snapshot'], proposal['snapshot']['intent']
                project_name = next(name for name, project in projects.items() if project['id'] == intent['project_id'])
                source, managed = snapshot['snapshot']['contract']['source'], intent['managed_root']
                for pin, expected in [(source, folders[project_name]), (managed, runtime / 'workspaces')]:
                    assert not expected.is_symlink() and expected.is_dir()
                    actual = expected.stat(follow_symlinks=False)
                    assert pin['canonical_root'] == str(expected.resolve())
                    assert pin['device'] == actual.st_dev and pin['inode'] == actual.st_ino
                creation_id = intent['creation_id']
                assert str(uuid.UUID(creation_id)) == creation_id
                assert intent['destination'] == str(runtime / 'workspaces' / ('data-' + creation_id))
                assert not Path(intent['destination']).is_relative_to(sources)

            def journal_commitments(creation, proposal):
                # Independently observe actual persisted receipts; never write
                # SQL or treat these public observations as a private witness.
                with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True) as store:
                    store.execute('PRAGMA query_only=ON'); store.execute('BEGIN')
                    receipt = store.execute('SELECT owner_sequence,prepared_sequence,finished_sequence FROM data_workspace_receipts WHERE creation_id=?', [creation['id']]).fetchone()
                    assert receipt and receipt[0] < receipt[1] < receipt[2]
                    proposal_row = store.execute('SELECT document,result FROM approval_proposals WHERE id=?', [proposal['id']]).fetchone()
                    stored_proposal, original_result = map(json.loads, proposal_row)
                    stored_creation = json.loads(store.execute('SELECT document FROM data_workspace_creations WHERE id=?', [creation['id']]).fetchone()[0])
                    assert stored_creation == creation and stored_proposal['consumed'] is True
                    assert stored_proposal['snapshot'] == proposal['snapshot'] and stored_proposal['revision'] == proposal['revision'] + 1
                    assert all(original_result[key] == creation[key] for key in ['id', 'approval_id', 'workspace_id', 'project_id', 'dataset_revision'])
                    events = [json.loads(store.execute('SELECT document FROM events WHERE sequence=?', [sequence]).fetchone()[0]) for sequence in receipt]
                owner, prepared, finished = events
                assert [(event['aggregate_id'], event['kind']) for event in events] == [(proposal['id'], 'approval.confirmed'), (creation['id'], 'data_workspace.copy_prepared'), (creation['id'], 'data_workspace.copy_finished')]
                assert owner['source'] == 'local_owner_confirmation' and owner['revision'] == stored_proposal['revision']
                compact_digest = lambda value: sha(json.dumps(value, ensure_ascii=False, separators=(',', ':')).encode())
                contract_commitment = {**stored_creation['contract'], 'contract_digest': ''}
                assert compact_digest(contract_commitment) == creation['contract']['contract_digest']
                assert owner['payload'] == {'kind': 'data_workspace', 'creation_id': creation['id'], 'consumed': True, 'snapshot_digest': compact_digest(stored_proposal['snapshot'])}
                assert prepared['revision'] == 1 and prepared['source'] == 'data_workspace_core'
                assert prepared['payload'] == {'workspace_id': creation['workspace_id'], 'dataset_revision': creation['dataset_revision'], 'copy_started': False, 'approval_id': creation['approval_id'], 'preview_id': creation['preview_id'], 'scope': creation['scope'], 'snapshot_digest': compact_digest(stored_creation['snapshot'])}
                assert finished['revision'] == creation['revision'] and finished['source'] == 'data_workspace_core'
                assert finished['payload'] == {'state': 'ready', 'workspace_id': creation['workspace_id'], 'cancel_requested': creation['cancel_requested'], 'automatic_retry': False, 'source_write': False, 'approval_id': creation['approval_id'], 'dataset_revision': creation['dataset_revision'], 'contract_digest': creation['contract']['contract_digest'], 'io_completion': creation['io_completion']}
                report['observations']['normal_journal_commitments'] = {'sequences': list(receipt), 'events': events, 'stored_snapshot_digest': compact_digest(stored_creation['snapshot']), 'owner_snapshot_digest': compact_digest(stored_proposal['snapshot']), 'contract_digest': creation['contract']['contract_digest']}

            def completion_receipt(creation):
                # Read the actual persisted terminal record and its correlated
                # event. This observes Core's join claim, never manufactures it.
                completion = copy_completion(creation)
                with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True) as store:
                    store.execute('PRAGMA query_only=ON'); store.execute('BEGIN')
                    stored = json.loads(store.execute('SELECT document FROM data_workspace_creations WHERE id=?', [creation['id']]).fetchone()[0])
                    assert stored == creation
                    sequence = store.execute('SELECT finished_sequence FROM data_workspace_receipts WHERE creation_id=?', [creation['id']]).fetchone()[0]
                    if sequence is None:
                        assert creation['state'] == 'outcome_unknown' and completion['io_completion'] in {None, 'interrupted_no_join_receipt'}, creation
                        return {**completion, 'finished_sequence': None, 'finished_event': None}
                    assert integer(sequence, 1)
                    event = json.loads(store.execute('SELECT document FROM events WHERE sequence=?', [sequence]).fetchone()[0])
                assert event['aggregate_id'] == creation['id'] and event['revision'] == creation['revision']
                assert event['kind'] == 'data_workspace.copy_finished' and event['source'] == 'data_workspace_core'
                expected = {'state': creation['state'], 'workspace_id': creation['workspace_id'], 'cancel_requested': creation['cancel_requested'], 'automatic_retry': False, 'source_write': False, 'approval_id': creation['approval_id'], 'dataset_revision': creation['dataset_revision'], 'contract_digest': (creation.get('contract') or {}).get('contract_digest')}
                if 'io_completion' in creation:
                    expected['io_completion'] = creation['io_completion']
                assert event['payload'] == expected
                return {**completion, 'finished_sequence': sequence, 'finished_event': event}

            def remember(method, response):
                value = response.get('result')
                rows = value if isinstance(value, list) else [value]
                for row in rows:
                    if isinstance(row, dict) and row.get('project_id') in project_ids and isinstance(row.get('id'), str):
                        if method.startswith('data_workspace_previews.'):
                            preview_ids[row['id']] = row['project_id']
                        elif method.startswith('data_workspaces.'):
                            creation_ids[row['id']] = row['project_id']

            def owner_proposal(preview, key):
                # Host fixture only. No HTTP method can reach this path.
                latest = preview_record(preview['id'])
                assert latest['revision'] == preview['revision'] and latest['state'] == 'verified'
                proposal = result('data_workspaces.propose', {'preview_id': latest['id']}, key, latest['revision'])
                assert proposal['kind'] == 'data_workspace'
                owner_paths(proposal)
                snapshot = proposal['snapshot']
                assert snapshot['intent']['preview_id'] == latest['id'] and snapshot['intent']['preview_revision'] == latest['revision']
                assert snapshot['intent']['project_id'] == latest['project_id'] and snapshot['intent']['project_revision'] == latest['project_revision']
                assert snapshot['snapshot'] == latest['snapshot'] and snapshot['scope'] == 'explicit_selected_dataset_copy_only'
                assert snapshot['guarantees']['filesystem_sandbox'] is False and snapshot['guarantees']['atomic_directory_freeze'] is False
                review = json.dumps(snapshot, ensure_ascii=False, indent=2).encode()
                assert len(review) <= 16_384, 'Review complete owner snapshot or refuse; never truncate'
                assert not Path(snapshot['intent']['destination']).exists()
                report['observations'][key] = {'kind': proposal['kind'], 'snapshot_sha256': sha(review), 'snapshot_bytes': len(review), 'preview_id': latest['id'], 'preview_revision': latest['revision']}
                return proposal

            def owner_finalize(proposal):
                owner_paths(proposal)
                response = ipc('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
                remember('data_workspaces.get', response)
                return response

            fault_lock = threading.Lock()
            drop_cancel = {'target': None}
            readonly = {'system_snapshot': ('system.snapshot', {}), 'projects_list': ('projects.list', {}), 'connections_list': ('connections.list', {}), 'sessions_list': ('sessions.list', {})}
            lists = {'workspaces_list': 'workspaces.list', 'data_workspace_previews_list': 'data_workspace_previews.list', 'data_workspaces_list': 'data_workspaces.list'}

            def selection_valid(selection):
                assert isinstance(selection, dict) and set(selection) == {'project_id', 'files', 'limits'}
                project_id = selection['project_id']
                assert project_id in project_ids and isinstance(selection['files'], list) and 1 <= len(selection['files']) <= 64
                limits = selection['limits']
                assert isinstance(limits, dict) and set(limits) == {'max_file_bytes', 'max_total_bytes', 'max_files'}
                assert integer(limits['max_file_bytes'], 1, 67_108_864) and integer(limits['max_total_bytes'], 1, 268_435_456) and integer(limits['max_files'], 1, 64)
                assert len(selection['files']) <= limits['max_files']
                ids, paths = set(), set()
                for file in selection['files']:
                    assert isinstance(file, dict) and set(file) == {'artifact_id', 'relative_path', 'max_bytes', 'expected_sha256'}
                    assert isinstance(file['artifact_id'], str) and re.fullmatch(r'[A-Za-z0-9._:-]{1,256}', file['artifact_id'])
                    assert file['relative_path'] in allowed_files[project_id] and integer(file['max_bytes'], 1, limits['max_file_bytes'])
                    assert file['expected_sha256'] is None or isinstance(file['expected_sha256'], str) and re.fullmatch(r'[a-f0-9]{64}', file['expected_sha256'])
                    assert file['artifact_id'] not in ids and file['relative_path'] not in paths
                    ids.add(file['artifact_id']); paths.add(file['relative_path'])

            class Handler(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *_):
                    pass

                def do_POST(self):
                    received_ns = time.monotonic_ns()
                    if self.path != '/__test_data_workspace_ipc':
                        self.send_error(404); return
                    if self.headers.get('Origin') != url or self.headers.get('Host') != url.removeprefix('http://'):
                        self.send_error(403); return
                    if self.headers.get_content_type() != 'application/json':
                        self.send_error(400); return
                    try:
                        size = int(self.headers.get('Content-Length', '0'))
                        assert 0 < size <= 32_768
                        request = json.loads(self.rfile.read(size), object_pairs_hook=unique_object)
                        assert isinstance(request, dict) and set(request) <= {'command', 'args'} and isinstance(request.get('command'), str)
                        command, values = request['command'], request.get('args')
                        values = {} if values is None else values
                        assert isinstance(values, dict)
                        key = revision = None
                        if command in readonly:
                            assert not values
                            method, params = readonly[command]
                        elif command in lists:
                            assert set(values) == {'projectId'} and values['projectId'] in project_ids
                            method, params = lists[command], {'project_id': values['projectId']}
                        elif command == 'events_subscribe':
                            assert set(values) <= {'cursor', 'limit'} and integer(values.get('cursor', 0)) and integer(values.get('limit', 100), 1, 100)
                            method, params = 'events.subscribe', {'cursor': values.get('cursor', 0), 'limit': values.get('limit', 100)}
                        elif command == 'data_workspace_previews_start':
                            assert set(values) == {'selection', 'expectedRevision', 'idempotencyKey'}
                            selection_valid(values['selection'])
                            assert values['expectedRevision'] == project_revisions[values['selection']['project_id']]
                            method, params = 'data_workspace_previews.start', values['selection']
                            key, revision = values['idempotencyKey'], values['expectedRevision']
                        elif command in {'data_workspace_previews_get', 'data_workspace_previews_cancel', 'data_workspaces_get', 'data_workspaces_cancel'}:
                            preview = command.startswith('data_workspace_previews_')
                            cancel = command.endswith('_cancel')
                            camel, snake, known = ('previewId', 'preview_id', preview_ids) if preview else ('creationId', 'creation_id', creation_ids)
                            assert set(values) == ({camel, 'expectedRevision', 'idempotencyKey'} if cancel else {camel}) and values[camel] in known
                            method = ('data_workspace_previews.' if preview else 'data_workspaces.') + ('cancel' if cancel else 'get')
                            params = {snake: values[camel]}
                            if cancel:
                                key, revision = values['idempotencyKey'], values['expectedRevision']
                                assert integer(revision, 1)
                        else:
                            self.send_error(403); return
                        if method in {'data_workspace_previews.start', 'data_workspace_previews.cancel', 'data_workspaces.cancel'}:
                            assert isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key) and integer(revision, 1)
                    except (AssertionError, ValueError, KeyError, TypeError):
                        self.send_error(400); return
                    timing = {'http_received_ns': received_ns}
                    try:
                        timing['pre_ipc_ns'] = time.monotonic_ns()
                        try:
                            response = ipc(method, params, key, revision)
                        finally:
                            timing['post_ipc_ns'] = time.monotonic_ns()
                        remember(method, response)
                        entry = {'method': method, 'args': values, 'response': response, 'timing': timing}
                        report['requests'].append(entry)
                        with fault_lock:
                            drop = method in {'data_workspace_previews.cancel', 'data_workspaces.cancel'} and drop_cancel['target'] in {params.get('preview_id'), params.get('creation_id')} and drop_cancel['target'] is not None and isinstance(response.get('result'), dict) and response['result'].get('id') == drop_cancel['target']
                            if drop:
                                drop_cancel['target'] = None
                        if drop:
                            report['faults'].append({'injection': 'after actual authenticated Unix cancellation response, before HTTP headers', 'method': method, 'params': params, 'key': key, 'actual_response': response, 'actual_database': database(), 'http_result': 'connection closed, no fabricated DTO'})
                            self.close_connection = True
                            self.connection.shutdown(socket.SHUT_RDWR)
                            self.connection.close()
                            return
                        raw = json.dumps(response).encode()
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.send_header('Content-Length', str(len(raw)))
                        self.send_header('Cache-Control', 'no-store')
                        self.end_headers(); self.wfile.write(raw)
                    except (BrokenPipeError, ConnectionResetError):
                        report['observations']['client_aborts'] = report['observations'].get('client_aborts', 0) + 1
                    except (AssertionError, OSError, ValueError) as failure:
                        report['observations'].setdefault('adapter_failure_timings', []).append({'method': method, 'timing': timing})
                        report['observations']['adapter_error'] = safe(failure)
                        self.send_error(502)

            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = f'http://127.0.0.1:{server.server_port}'
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
                context = browser.new_context(viewport={'width': 1488, 'height': 981}, locale='en-US')
                context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const response=await fetch('/__test_data_workspace_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!response.ok)throw Error('Bounded data workspace test adapter refused request');return response.json();}};""")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); page_errors = []
                page.on('pageerror', lambda failure: page_errors.append(str(failure)))
                pump = lambda: page.evaluate('0')

                def screenshot(name):
                    path = output / (name + '.png')
                    page.screenshot(path=str(path))
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'viewport': page.viewport_size, 'dimension': 'actual Chromium, not OS-native/macOS'})

                def open_details(details):
                    if not details.evaluate('(element)=>element.open'):
                        details.locator(':scope > summary').click()

                def open_project(name):
                    if page.get_by_role('dialog', name='Projects & workspaces', exact=True).count():
                        page.get_by_role('button', name='Close', exact=True).click()
                    page.get_by_role('button', name='Projects & workspaces', exact=True).click()
                    dialog = page.get_by_role('dialog', name='Projects & workspaces', exact=True)
                    dialog.get_by_role('button', name=re.compile('^Generated data ' + re.escape(name))).click()
                    panel = dialog.get_by_test_id('data-workspaces')
                    assert not panel.locator(':scope > details').evaluate('(element)=>element.open')
                    open_details(panel.locator(':scope > details'))
                    expect(panel.get_by_role('button', name='Refresh data workspace records', exact=True)).to_be_enabled()
                    return panel

                def refresh(panel):
                    panel.get_by_role('button', name='Refresh data workspace records', exact=True).click()
                    expect(panel.locator('[aria-busy]')).to_have_attribute('aria-busy', 'false')

                def detail(panel, record_id):
                    current = panel.locator('details').filter(has=page.locator('summary').filter(has_text=re.compile(r' · ' + re.escape(record_id) + '$'))).last
                    open_details(current)
                    return current

                def start_preview(panel, name, wrong_hash=False):
                    before = len(report['requests'])
                    lines = '\n'.join(path + ' | ' + ('0' * 64 if wrong_hash else value['sha256']) for path, value in original[name].items())
                    panel.get_by_role('textbox', name='Relative files (one path per line; optional | SHA-256)', exact=True).fill(lines)
                    limits = panel.locator('form details')
                    open_details(limits)
                    large = name in {'preview-cancel', 'copy-cancel'}
                    for label, value in [('Maximum bytes per file', 67_108_864 if large else 4096), ('Maximum total bytes', 67_108_864 if large else 8192), ('Maximum selected files', 4)]:
                        panel.get_by_role('textbox', name=label, exact=True).fill(str(value))
                    panel.get_by_role('button', name='Preview selected files', exact=True).click()
                    entry = wait(lambda: report['requests'][before:], lambda rows: any(row['method'] == 'data_workspace_previews.start' for row in rows))
                    request = next(row for row in entry if row['method'] == 'data_workspace_previews.start')
                    assert not request['response'].get('error'), request['response']
                    record = request['response']['result']
                    assert record['project_id'] == projects[name]['id'] and set(record['selection']) == {'project_id', 'files', 'limits'}
                    return record, request

                def verified(panel, name):
                    started, request = start_preview(panel, name)
                    record = wait(lambda: preview_record(started['id']), lambda value: value['state'] not in {'checking', 'cancel_requested'})
                    assert record['state'] == 'verified', record
                    refresh(panel)
                    current = detail(panel, record['id'])
                    for path, value in original[name].items():
                        expect(current.get_by_text(value['sha256'], exact=True)).to_be_visible()
                    assert record['snapshot']['total_bytes'] == sum(value['bytes'] for value in original[name].values())
                    assert {file['requirement']['relative_path'] for file in record['snapshot']['files']} == set(original[name])
                    expect(current.get_by_text(record['snapshot']['dataset_revision'], exact=True)).to_be_visible()
                    return record, request

                def browser_request(command, values):
                    return page.evaluate("""async ({command,args})=>{const response=await fetch('/__test_data_workspace_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});return {status:response.status,body:response.ok?await response.json():null};}""", {'command': command, 'args': values})

                try:
                    page.goto(url)
                    panel = open_project('normal')
                    before = database()
                    panel.get_by_role('textbox', name='Relative files (one path per line; optional | SHA-256)', exact=True).fill('../outside')
                    panel.get_by_role('button', name='Preview selected files', exact=True).click()
                    expect(panel.get_by_role('alert')).to_contain_text('relative regular-file paths')
                    assert database() == before
                    preview, request = verified(panel, 'normal')
                    assert database()['data_workspace_creations'] == 0 and database()['managed_workspaces'] == 0
                    detail(panel, preview['id']).locator(':scope > summary').evaluate("element=>element.scrollIntoView({block:'start'})")
                    screenshot('01-real-selected-file-preview')
                    passed('real UI relative selection and bounded preview show actual hashes/bytes with no copy or source writes', preview_id=preview['id'])

                    before = database()
                    denied = ['data_workspace_approve', 'data_workspaces_propose', 'approvals_finalize', 'generic_ipc', 'projects_import', 'workspaces_create', 'sessions_create', 'goal_plans_dispatch']
                    assert all(browser_request(command, {})['status'] == 403 for command in denied)
                    assert browser_request('data_workspace_previews_start', {'selection': {**preview['selection'], 'source': '/unapproved'}, 'expectedRevision': projects['normal']['revision'], 'idempotencyKey': 'forged-source'})['status'] == 400
                    assert browser_request('data_workspaces_get', {'creationId': 'foreign-copy'})['status'] == 400
                    assert database() == before
                    passed('HTTP refuses Renderer approval, finalize, generic IPC, execution and arbitrary source/foreign references')

                    proposal = owner_proposal(preview, 'normal-owner-proposal')
                    initial = owner_finalize(proposal)
                    assert not initial.get('error'), initial
                    creation_id = initial['result']['id']
                    ready = wait(lambda: creation_record(creation_id), lambda value: value['state'] not in {'creating', 'cancel_requested'})
                    assert ready['state'] == 'ready' and ready['contract'] is not None and ready['observation']['state'] == 'ready', ready
                    assert copy_completion(ready)['owned_copy_worker_join_observed'], ready
                    assert ready['observation']['copied_files'] == len(original['normal']) and ready['observation']['error'] is None
                    destination = Path(ready['destination'])
                    for path, original_file in original['normal'].items():
                        copied = destination / 'inputs' / path
                        assert copied.is_file() and file_sha(copied) == original_file['sha256']
                        assert observation(folders['normal'] / path) == original_file
                    assert not (destination / '.git').exists() and all(not (folder / '.git').exists() for folder in folders.values())
                    registry = result('workspaces.list', {'project_id': projects['normal']['id']})
                    registered = next(row for row in registry if row['id'] == ready['workspace_id'])
                    assert registered['state'] == 'ready' and registered['identity']['kind']['kind'] == 'directory'
                    assert registered['root'] == ready['destination'] and registered['base_revision'] == ready['dataset_revision']
                    journal_commitments(ready, proposal)
                    refresh(panel); current = detail(panel, creation_id)
                    expect(current.get_by_text(ready['contract']['contract_digest'], exact=True)).to_be_visible()
                    current.locator(':scope > summary').evaluate("element=>element.scrollIntoView({block:'start'})")
                    screenshot('02-real-ready-data-copy')
                    page.set_viewport_size({'width': 390, 'height': 844}); current.scroll_into_view_if_needed(); screenshot('03-real-ready-mobile')
                    assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                    page.set_viewport_size({'width': 1488, 'height': 981})
                    report['observations']['normal_ready'] = ready
                    report['observations']['normal_completion_receipt'] = completion_receipt(ready)
                    passed('outside-HTTP owner fixture produces actual ready copy, exact inputs bytes/registry identity and unchanged source mtime')

                    before = database()
                    replay = result('data_workspace_previews.start', preview['selection'], request['args']['idempotencyKey'], request['args']['expectedRevision'])
                    assert replay['id'] == preview['id']
                    finalized = owner_finalize(proposal)
                    assert finalized['result']['id'] == creation_id
                    assert database() == before and len(creation_records(projects['normal'])) == 1
                    refresh(panel)
                    before_read = database(); refresh(panel); assert database() == before_read
                    passed('exact-key preview/owner replay and UI readonly history refresh do not create another copy or journal entry')

                    # Actual worker cancellation, not a promised scheduling win.
                    panel = open_project('preview-cancel')
                    started, _ = start_preview(panel, 'preview-cancel')
                    # The start acknowledgement already refreshes the real UI.
                    # Go directly to this exact record; no second list roundtrip.
                    current = detail(panel, started['id'])
                    cancel_button = current.get_by_role('button', name='Request preview cancellation', exact=True)
                    requested = cancel_button.count() > 0
                    if requested:
                        with fault_lock:
                            drop_cancel['target'] = started['id']
                        # click() waits for this exact real button's actionability.
                        click_timing = {'before_click_call_ns': time.monotonic_ns()}
                        report['observations']['preview_cancel_ui_click'] = click_timing
                        try:
                            cancel_button.click()
                        finally:
                            click_timing['after_click_call_ns'] = time.monotonic_ns()
                        expect(panel.get_by_role('alert')).to_contain_text('Request outcome unknown')
                        assert any(fault['params'].get('preview_id') == started['id'] for fault in report['faults'])
                        screenshot('04-real-cancel-response-lost')
                        panel = open_project('preview-cancel')  # actual unmount/remount of the product component
                        refresh(panel)
                    terminal = wait(lambda: preview_record(started['id']), lambda value: value['state'] not in {'checking', 'cancel_requested'})
                    assert terminal['state'] in {'cancelled', 'verified'}, terminal
                    assert creation_records(projects['preview-cancel']) == []
                    requests = [row for row in report['requests'] if row['method'] == 'data_workspace_previews.cancel' and row['args'].get('previewId') == started['id']]
                    assert len(requests) == int(requested)
                    refresh(panel); assert panel.get_by_role('button', name='Recover the same request', exact=True).count() == 0
                    report['observations']['preview_cancel'] = {'requested_through_ui': requested, 'terminal': terminal, 'cooperative_cancel_observed': terminal['state'] == 'cancelled'}
                    if not requested:
                        report['unrun'].append('Preview completed before UI cancellation control was available; dropped-response/remount cancellation was not exercised')
                    elif terminal['state'] != 'cancelled':
                        report['unrun'].append('Preview request was exercised but its actual terminal was verified; no cooperative cancellation hit is claimed')
                    passed('preview terminal fact retained with no copy; actual dropped-response reconciliation is reported only if exercised', requested_through_ui=requested, terminal=terminal['state'])

                    panel = open_project('copy-cancel')
                    preview, _ = verified(panel, 'copy-cancel')
                    proposal = owner_proposal(preview, 'cancel-copy-owner-proposal')
                    initial = owner_finalize(proposal); assert not initial.get('error'), initial
                    copy_id = initial['result']['id']
                    refresh(panel); current = detail(panel, copy_id)
                    button = current.get_by_role('button', name='Request copy cancellation', exact=True)
                    requested = button.count() > 0
                    if requested:
                        button.click()
                    terminal = wait(lambda: creation_record(copy_id), lambda value: value['state'] not in {'creating', 'cancel_requested'})
                    completion = completion_receipt(terminal)
                    retained_destination = Path(terminal['destination']).exists()
                    refresh(panel); current = detail(panel, copy_id)
                    assert len(creation_records(projects['copy-cancel'])) == 1
                    if terminal['state'] == 'ready':
                        assert terminal['observation']['state'] == 'ready'
                        assert file_sha(Path(terminal['destination']) / 'inputs/large.bin') == original['copy-cancel']['large.bin']['sha256']
                        assert completion['owned_copy_worker_join_observed']
                        if terminal['cancel_requested']:
                            expect(current.get_by_text('Copy completed before cancellation took effect. The ready copy is retained; this is not cancellation success.', exact=True)).to_be_visible()
                    elif terminal['state'] == 'cancelled':
                        expect(current.get_by_text('The owned copy worker joined after cancellation. Files may have been written and partial output is retained; stopping does not mean deletion, a ready workspace or accepted artifacts.', exact=True)).to_be_visible()
                        expect(current.get_by_text('Owned copy worker joined; other filesystem state is not implied', exact=True)).to_be_visible()
                        expect(panel.get_by_role('button', name='Preview selected files', exact=True)).to_be_enabled()
                        expect(current.get_by_role('button', name='Request copy cancellation', exact=True)).to_have_count(0)
                    else:
                        expect(current.get_by_text('Partial or unknown output is retained. No automatic retry, new copy or deletion.', exact=True)).to_be_visible()
                        expect(panel.get_by_role('button', name='Preview selected files', exact=True)).to_be_disabled()
                    # A failed-before-reservation operation may have no target;
                    # zero observed files never means an existing target is empty.
                    if retained_destination:
                        assert Path(terminal['destination']).exists(), 'Readonly reconciliation deleted a retained target'
                    current.locator(':scope > summary').evaluate("element=>element.scrollIntoView({block:'start'})")
                    screenshot('05-real-copy-terminal-observation')
                    cancelled = requested and terminal['state'] != 'ready' and ((terminal.get('error') or {}).get('code') == 'CANCEL_REQUESTED' or (terminal.get('observation') or {}).get('error') == 'cancel_requested')
                    io_observation = terminal.get('observation')
                    in_copy_observation = cancelled and io_observation is not None and io_observation.get('error') == 'cancel_requested'
                    before_reservation = cancelled and io_observation is None and not retained_destination
                    retained_selected_files = []
                    for path, source in original['copy-cancel'].items():
                        target = Path(terminal['destination']) / 'inputs' / path
                        try:
                            current_stat = target.stat(follow_symlinks=False)
                        except FileNotFoundError:
                            continue
                        assert stat.S_ISREG(current_stat.st_mode) and current_stat.st_nlink == 1
                        retained_selected_files.append({'relative_path': 'inputs/' + path, 'bytes': current_stat.st_size, 'source_bytes': source['bytes']})
                    selected_progress = any(value['bytes'] > 0 for value in retained_selected_files)
                    partial_selected = any(0 < value['bytes'] < value['source_bytes'] for value in retained_selected_files)
                    writes_before = len([row for row in report['requests'] if row['method'] in {'data_workspace_previews.start', 'data_workspace_previews.cancel', 'data_workspaces.cancel'}])
                    before_read = database(); refresh(panel); assert database() == before_read
                    assert len([row for row in report['requests'] if row['method'] in {'data_workspace_previews.start', 'data_workspace_previews.cancel', 'data_workspaces.cancel'}]) == writes_before
                    cancel_requests = [row for row in report['requests'] if row['method'] == 'data_workspaces.cancel' and row['args'].get('creationId') == copy_id]
                    assert len(cancel_requests) == int(requested)
                    report['observations']['copy_cancel'] = {'requested_through_ui': requested, 'terminal': terminal, 'retained_destination_exists': retained_destination,
                        'completion_receipt': completion,
                        'cancellation_specific_receipt_observed': cancelled,
                        'cancellation_stage': 'before_destination_reservation' if before_reservation else 'copy_observation' if in_copy_observation else 'not_established',
                        'in_copy_cancellation_observation': in_copy_observation,
                        'retained_selected_files': retained_selected_files,
                        'selected_output_progress_observed': selected_progress,
                        'partial_selected_output_observed': partial_selected,
                        'partial_copy_stop_not_established': not (completion['cancelled_owned_worker_join_observed'] and partial_selected),
                        'no_join_unknown_case_exercised': terminal['state'] == 'outcome_unknown' and terminal.get('io_completion') == 'interrupted_no_join_receipt',
                        'ready_race_exercised': requested and terminal['state'] == 'ready',
                        'not_a_stop_claim': terminal['state'] == 'ready'}
                    if not requested:
                        report['unrun'].append('Copy completed before UI cancellation control was available; no successful cancellation is claimed')
                    elif not completion['cancelled_owned_worker_join_observed']:
                        report['unrun'].append('Copy request was exercised but no cancelled/worker_joined terminal receipt was observed; actual Ready/failed/unknown is preserved without promoting it to confirmed cancellation')
                    passed('actual copy request and persisted completion are reconciled without replay/deletion or inferred legacy join', requested_through_ui=requested, terminal=terminal['state'], owned_copy_worker_join_observed=completion['owned_copy_worker_join_observed'])

                    panel = open_project('drift')
                    failed, _ = start_preview(panel, 'drift', wrong_hash=True)
                    failed = wait(lambda: preview_record(failed['id']), lambda value: value['state'] not in {'checking', 'cancel_requested'})
                    assert failed['state'] == 'failed' and failed['error'] is not None
                    refresh(panel); current = detail(panel, failed['id'])
                    expect(current.get_by_text(failed['error']['code'] + ': ' + failed['error']['message'], exact=True)).to_be_visible()
                    expect(current.get_by_role('button', name='Review this data copy in native dialog', exact=True)).to_be_disabled()
                    assert creation_records(projects['drift']) == []
                    preview, _ = verified(panel, 'drift')
                    proposal = owner_proposal(preview, 'drift-owner-proposal')
                    changed = folders['drift'] / 'sample.csv'
                    changed.write_bytes(b'explicit fixture change after approved observation\n')
                    changed_observation = observation(changed)
                    response = owner_finalize(proposal)
                    if response.get('error'):
                        assert response['error']['code'] in {'INPUT_CHANGED', 'PERMISSION_DENIED'}, response
                        drift_result = {'finalize_error': response['error']}
                    else:
                        drift_result = wait(lambda: creation_record(response['result']['id']), lambda value: value['state'] not in {'creating', 'cancel_requested'})
                        assert drift_result['state'] in {'failed', 'outcome_unknown'} and drift_result['contract'] is None
                        refresh(panel); detail(panel, drift_result['id'])
                    assert observation(changed) == changed_observation
                    assert not any(record['state'] == 'ready' for record in creation_records(projects['drift']))
                    report['observations']['deliberate_fixture_source_change'] = {'before': original['drift']['sample.csv'], 'after_fixture_change': changed_observation, 'after_daemon_work': observation(changed), 'result': drift_result}
                    screenshot('06-real-historical-failure-and-drift')
                    passed('real invalid-hash preview and post-observation source drift cannot authorize a ready resnapshot or new copy')

                    # Runtime protection uses a generated benign file, never token bytes.
                    (runtime / 'benign-fixture.txt').write_bytes(b'generated private-boundary sentinel\n')
                    private = result('projects.import', {'path': str(runtime), 'name': 'Generated private boundary'}, 'private-runtime-project')
                    before = database()
                    refused = ipc('data_workspace_previews.start', {'project_id': private['id'], 'files': [{'artifact_id': 'sentinel', 'relative_path': 'benign-fixture.txt', 'max_bytes': 1024, 'expected_sha256': None}], 'limits': {'max_file_bytes': 1024, 'max_total_bytes': 1024, 'max_files': 1}}, 'private-runtime-preview', private['revision'])
                    assert refused['error']['code'] == 'PERMISSION_DENIED' and database() == before
                    assert result('data_workspace_previews.list', {'project_id': private['id']}) == []
                    final = database()
                    assert final['attempts'] == final['sessions'] == final['dispatch_intents'] == 0
                    for name in ['normal', 'preview-cancel', 'copy-cancel']:
                        for path, value in original[name].items():
                            assert observation(folders[name] / path) == value
                    assert not page_errors, page_errors
                    report['observations']['final_database'] = final
                    report['observations']['source_originals'] = original
                    passed('private-runtime benign source is rejected before preview persistence; generated sources unchanged and no session/execution occurred')
                    report['status'] = 'passed' if not report['unrun'] else 'incomplete_race_coverage'
                except BaseException:
                    screenshot('failure-current-ui')
                    raise
                finally:
                    report['observations']['browser_page_errors'] = page_errors
                    browser.close()
        except BaseException as failure:
            report['failure'] = {'type': type(failure).__name__, 'message': safe(failure), 'traceback': safe(traceback.format_exc())}
            raise AssertionError(safe(failure)) from None
        finally:
            if server is not None:
                server.shutdown(); server.server_close()
            if thread is not None:
                thread.join(timeout=5)
            daemon.terminate()
            try:
                daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                daemon.kill(); daemon.wait()
            daemon_log.close()
            # Defense in depth: authentication bytes never become evidence.
            (output / 'report.json').write_text(safe(json.dumps(report, indent=2)) + '\n')
            print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'unrun': len(report['unrun']), 'report': str(output / 'report.json')}), flush=True)
    return 0 if report['status'] == 'passed' else 2


if __name__ == '__main__':
    raise SystemExit(main())
