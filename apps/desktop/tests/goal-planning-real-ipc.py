#!/usr/bin/env python3
"""Built React + fixed localhost adapter + authenticated real Goal Unix IPC.

Private owner fixtures approve generated local resources outside HTTP. No SQL
writes, Provider session/model, native owner-dialog claim, or production hook.
The plan JSON remains an opaque original string across the browser adapter.
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
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import uuid

from playwright.sync_api import expect, sync_playwright


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Duplicate adapter envelope key')
        result[key] = value
    return result


def bounded_integer(value, minimum=0):
    return type(value) is int and minimum <= value <= 9_007_199_254_740_991


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    parser.add_argument('--structured-editor', action='store_true', help='Exercise real same-model step editing before the unchanged proposal/preview/history checks')
    args = parser.parse_args()
    binary = args.batond.resolve(strict=True)
    frontend = (args.frontend or Path(__file__).resolve().parents[1] / 'dist').resolve(strict=True)
    assert (frontend / 'index.html').is_file()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # Never replace earlier failed or successful evidence.
    assert not (output / 'report.json').exists(), 'Use a new output directory for each run'
    report = {
        'status': 'failed_or_interrupted',
        'mode': 'structured_editor' if args.structured_editor else 'advanced_raw_json',
        'dimension': 'actual built React / Chromium / fixed test-only localhost adapter / real Unix Goal IPC',
        'native_owner_dialog': 'not_run; fixture owner finalizes actual proposals outside HTTP',
        'provider': 'generated credential-free local --version probe only; no session or model',
        'not_claimed': ['native WebView', 'macOS', 'dispatch', 'reservation', 'task acceptance', 'generated planner'],
        'daemon_sha256': sha(binary.read_bytes()),
        'harness_sha256': sha(Path(__file__).read_bytes()),
        'frontend_assets_sha256': {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()},
        'checks': [], 'requests': [], 'observations': {}, 'screenshots': [],
    }

    def passed(name, **details):
        report['checks'].append({'name': name, 'status': 'passed', **details})

    event_pump = lambda: None

    def wait(read, predicate, seconds=10):
        end = time.monotonic() + seconds
        while True:
            event_pump()
            value = read()
            if predicate(value):
                return value
            assert time.monotonic() < end, f'Observation deadline exceeded: {value}'
            time.sleep(.02)

    with tempfile.TemporaryDirectory(prefix='baton-goal-browser-') as temporary:
        root = Path(temporary).resolve()
        root.chmod(0o700)
        runtime = root / 'runtime'
        runtime.mkdir(mode=0o700)
        daemon_log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)}, stdout=daemon_log, stderr=subprocess.STDOUT)
        server = thread = None
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()), lambda v: v[0] and v[1] and v[2] is None)
            token = (runtime / 'session.token').read_text()

            def ipc(method, params, key=None, revision=None):
                request = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token, 'method': method, 'params': params}
                if key is not None:
                    request['idempotency_key'] = key
                if revision is not None:
                    request['expected_revision'] = revision
                frame = json.dumps(request).encode() + b'\n'
                assert len(frame) <= 262_144, 'Whole fixture IPC frame exceeds Core bound'
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(5)
                    connection.connect(str(runtime / 'batond.sock'))
                    connection.sendall(frame)
                    with connection.makefile('rb') as response:
                        raw = response.readline(1_048_577)
                assert len(raw) <= 1_048_576, 'Daemon response exceeds fixture bound'
                return json.loads(raw)

            def result(method, params, key=None, revision=None):
                envelope = ipc(method, params, key, revision)
                assert not envelope.get('error'), {'method': method, 'response': envelope}
                return envelope['result']

            def owner_approve(method, params, key, kind=None):
                proposal = result(method, params, key)
                if kind is not None:
                    assert proposal['kind'] == kind
                pretty = json.dumps(proposal['snapshot'], ensure_ascii=False, indent=2)
                assert len(pretty.encode()) <= 16_384, 'Complete owner snapshot must fit; never truncate it'
                report['observations'][key] = {'kind': proposal['kind'], 'snapshot_utf8_bytes': len(pretty.encode()), 'snapshot_sha256': sha(pretty.encode())}
                # This authenticated fixture owner is deliberately outside HTTP.
                return result('approvals.finalize', {'proposal_id': proposal['id']}, 'finalize:' + proposal['id'], proposal['revision'])

            source = root / 'source'
            source.mkdir()
            fixed_input = b'fixed input\n'
            (source / 'input.txt').write_bytes(fixed_input)
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}

            def git(*arguments):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *arguments], env=git_env, text=True).strip()

            git('init', '-q', '--initial-branch=main')
            git('add', 'input.txt')
            git('commit', '-qm', 'generated Goal input fixture')
            project = result('projects.import', {'path': str(source), 'name': 'Real Goal planning fixture'}, 'project')
            workspace = result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'workspace', project['revision'])
            invocation_log = root / 'probe-invocations.jsonl'
            provider = root / 'generated-version-only-acp'
            provider.write_text(f'#!{sys.executable}\nimport json,sys\nfrom pathlib import Path\n' +
                                f'with Path({str(invocation_log)!r}).open("a") as log: log.write(json.dumps(sys.argv[1:])+"\\n")\n' +
                                'if sys.argv[1:]==["--version"]:\n print("AgentBaton generated Goal fixture 1.0");sys.exit(0)\n' +
                                'sys.exit("This fixture never starts an ACP session or model")\n')
            provider.chmod(0o700)
            connection = owner_approve('connections.propose', {'name': 'Generated --version only; no account', 'provider': 'custom_acp', 'executable': str(provider), 'arguments': []}, 'connection')
            wait(lambda: result('connections.list', {}), lambda rows: any(c['id'] == connection['id'] and c['probe_state'] == 'ready' for c in rows))
            grant = owner_approve('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 8, 'max_concurrency': 2, 'max_duration_ms': 3_600_000}, 'root-grant')
            goal = 'Plan an owner-reviewed report from one fixed input'
            task = result('tasks.create', {'project_id': project['id'], 'goal': goal}, 'task')
            budget = {'attempts': 8, 'parallel': 2, 'runtime_ms': 60_000, 'output_bytes': 8192, 'children': 3, 'reworks': 2, 'tokens': None, 'cost_microunits': None}
            selection = {'task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'], 'expected_grant_revision': grant['revision'],
                         'max_nodes': 4, 'new_max_children': 3, 'new_max_depth': 2, 'root_budget': budget, 'project_budget': budget,
                         'inputs': [{'artifact_id': 'source', 'relative_path': 'input.txt', 'max_bytes': 1024, 'expected_sha256': sha(fixed_input)}],
                         'outputs': [{'artifact': {'artifact_id': 'report', 'relative_path': 'outputs/report.txt', 'max_bytes': 1024, 'expected_sha256': None}, 'validators': []}],
                         'writable_prefixes': ['outputs'], 'owner_acceptance_criteria': 'Owner reviews the exact report; approval or plan validation alone never accepts this task.'}
            if args.structured_editor:
                # A second work node needs its own real, separately approved
                # output contract. Default/raw fixture remains unchanged.
                selection['outputs'].append({'artifact': {'artifact_id': 'details', 'relative_path': 'outputs/details.txt', 'max_bytes': 1024, 'expected_sha256': None}, 'validators': []})
            scope = owner_approve('goal_scopes.propose', selection, 'goal-scope', 'goal')
            assert scope['planning_only'] is True and scope['revoked'] is False
            current_grant = next(g for g in result('grants.list', {'project_id': project['id']}) if g['id'] == grant['id'])
            expected_grant = json.loads(json.dumps(grant))
            expected_grant['revision'] += 1
            expected_grant['scope']['max_children'] = 3
            expected_grant['scope']['max_depth'] = 2
            assert current_grant == expected_grant, 'Goal approval changed more than explicit delegation ceilings/revision'
            assert result('tasks.get', {'task_id': task['id']})['state'] == 'draft'
            passed('actual outside-HTTP Goal approval preserves execution bindings and does not accept task', scope_id=scope['id'], task_id=task['id'])

            def database_snapshot():
                # Read-only observation, never synthetic connection/ledger/evidence inserts.
                with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True) as database:
                    database.execute('PRAGMA query_only=ON')
                    database.execute('BEGIN')
                    tables = ['attempts', 'dispatch_intents', 'workspace_leases', 'sessions', 'command_validation_runs', 'command_workspace_fences', 'goal_node_evidence', 'goal_external_action_journal', 'goal_plan_heads', 'goal_plan_revisions', 'goal_plan_proposals']
                    value = {table: database.execute('SELECT count(*) FROM ' + table).fetchone()[0] for table in tables}
                    value['budgets'] = database.execute('SELECT * FROM budgets ORDER BY scope').fetchall()
                    value['goal_ledgers'] = database.execute('SELECT * FROM goal_ledgers ORDER BY scope').fetchall()
                    value['journal_sequence'] = database.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
                    return value

            def assert_no_execution():
                snapshot = database_snapshot()
                for table in ['attempts', 'dispatch_intents', 'workspace_leases', 'sessions', 'command_validation_runs', 'command_workspace_fences', 'goal_node_evidence', 'goal_external_action_journal']:
                    assert snapshot[table] == 0, (table, snapshot[table])
                assert not (Path(workspace['root']) / 'outputs/report.txt').exists()
                if args.structured_editor:
                    assert not (Path(workspace['root']) / 'outputs/details.txt').exists()
                invocations = [json.loads(line) for line in invocation_log.read_text().splitlines()]
                assert invocations and all(argv == ['--version'] for argv in invocations), invocations
                assert result('tasks.get', {'task_id': task['id']})['state'] == 'draft'

            allowed_read = {'system_snapshot': ('system.snapshot', {}), 'projects_list': ('projects.list', {}), 'connections_list': ('connections.list', {}), 'sessions_list': ('sessions.list', {})}
            allowed_scoped = {'workspaces_list': ('workspaces.list', 'projectId', 'project_id', project['id']), 'grants_list': ('grants.list', 'projectId', 'project_id', project['id']),
                              'goal_scopes_list': ('goal_scopes.list', 'taskId', 'task_id', task['id']), 'goal_scopes_get': ('goal_scopes.get', 'scopeId', 'scope_id', scope['id']),
                              'goal_plans_get': ('goal_plans.get', 'scopeId', 'scope_id', scope['id']), 'goal_plans_list': ('goal_plans.list', 'scopeId', 'scope_id', scope['id'])}
            allowed_write = {'goal_plans_propose': 'goal_plans.propose', 'goal_plans_revise': 'goal_plans.revise', 'goal_scopes_revoke': 'goal_scopes.revoke'}

            class Handler(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *_):
                    pass

                def do_POST(self):
                    if self.path != '/__test_goal_ipc':
                        self.send_error(404)
                        return
                    if self.headers.get('Origin') != url or self.headers.get('Host') != url.removeprefix('http://'):
                        self.send_error(403)
                        return
                    if self.headers.get_content_type() != 'application/json':
                        self.send_error(400)
                        return
                    try:
                        size = int(self.headers.get('Content-Length', '0'))
                        assert 0 < size <= 270_336
                        request = json.loads(self.rfile.read(size), object_pairs_hook=unique_object)
                        assert isinstance(request, dict) and set(request) <= {'command', 'args'} and 'command' in request
                        command, values = request['command'], request.get('args')
                        assert isinstance(command, str)
                        if values is None:
                            values = {}
                        assert isinstance(values, dict)
                        key = revision = None
                        if command in allowed_read:
                            assert not values
                            method, params = allowed_read[command]
                        elif command in allowed_scoped:
                            method, camel, snake, bound = allowed_scoped[command]
                            assert set(values) == {camel} and values[camel] == bound
                            params = {snake: bound}
                        elif command == 'events_subscribe':
                            assert set(values) <= {'cursor', 'limit'}
                            assert bounded_integer(values.get('cursor', 0)) and bounded_integer(values.get('limit', 100), 1) and values.get('limit', 100) <= 100
                            method, params = 'events.subscribe', {'cursor': values.get('cursor', 0), 'limit': values.get('limit', 100)}
                        elif command in allowed_write or command == 'goal_plans_ready_preview':
                            is_plan = command in {'goal_plans_propose', 'goal_plans_revise'}
                            required = {'scopeId', 'expectedRevision'} | ({'idempotencyKey'} if command in allowed_write else set()) | ({'planJson'} if is_plan else set())
                            assert set(values) == required and values['scopeId'] == scope['id']
                            revision = values['expectedRevision']
                            assert bounded_integer(revision, 0 if command == 'goal_plans_propose' else 1)
                            method = allowed_write.get(command, 'goal_plans.ready_preview')
                            params = {'scope_id': scope['id']}
                            if command in allowed_write:
                                key = values['idempotencyKey']
                                assert isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key)
                            if is_plan:
                                raw = values['planJson']
                                assert isinstance(raw, str) and 0 < len(raw.encode()) <= 262_144
                                # Deliberately opaque: Core, not a Value parser, rejects duplicate inner keys.
                                params['plan_json'] = raw
                        else:
                            self.send_error(403)
                            return
                    except (AssertionError, ValueError, TypeError, KeyError):
                        self.send_error(400)
                        return
                    try:
                        response = ipc(method, params, key, revision)
                        entry = {'method': method, 'args': values, 'error': response.get('error', {}).get('code')}
                        if 'plan_json' in params:
                            entry['plan_json_sha256'] = sha(params['plan_json'].encode())
                        report['requests'].append(entry)
                        raw = json.dumps(response).encode()
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.send_header('Content-Length', str(len(raw)))
                        self.send_header('Cache-Control', 'no-store')
                        self.end_headers()
                        self.wfile.write(raw)
                    except (BrokenPipeError, ConnectionResetError):
                        # Preserve client aborts without writing a second HTTP response.
                        report['observations']['adapter_client_aborts'] = report['observations'].get('adapter_client_aborts', 0) + 1
                    except (AssertionError, OSError, ValueError) as failure:
                        report['observations']['adapter_transport_error'] = str(failure)
                        self.send_error(502)

            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = f'http://127.0.0.1:{server.server_port}'
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
                context = browser.new_context(viewport={'width': 1488, 'height': 981}, locale='en-US')
                context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const response=await fetch('/__test_goal_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!response.ok)throw Error('Bounded Goal fixture adapter refused request');return response.json();}};""")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page()
                # Playwright's synchronous API must dispatch route callbacks while
                # polling the daemon/report; time.sleep alone starves browser IO.
                event_pump = lambda: page.evaluate('0')
                page_errors = []
                page.on('pageerror', lambda error: page_errors.append(str(error)))

                def screenshot(name):
                    path = output / (name + '.png')
                    page.screenshot(path=str(path), full_page=True)
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'viewport': page.viewport_size, 'dimension': 'Chromium, not native WebView/macOS'})

                def browser_request(command, values):
                    return page.evaluate("""async ({command,args})=>{const r=await fetch('/__test_goal_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});return {status:r.status,body:r.ok?await r.json():null};}""", {'command': command, 'args': values})

                def open_details(detail):
                    if not detail.evaluate('(element)=>element.open'):
                        detail.locator(':scope > summary').click()

                try:
                    page.goto(url)
                    page.get_by_test_id('task-card').filter(has_text=goal).click()
                    page.locator('#tab-details').click()
                    panel = page.get_by_test_id('goal-planning')
                    assert not panel.locator(':scope > details').evaluate('(element)=>element.open')
                    assert not any(r['method'].startswith('goal_') for r in report['requests'])
                    open_details(panel.locator(':scope > details'))
                    wait(lambda: report['requests'], lambda rows: any(r['method'] == 'goal_plans.list' for r in rows))
                    expect(panel.locator('[aria-busy]')).to_have_attribute('aria-busy', 'false')
                    report['observations']['empty_history_response'] = ipc('goal_plans.list', {'scope_id': scope['id']})
                    screenshot('01-fresh-goal-expanded')
                    assert report['observations']['empty_history_response'].get('result') == [], 'Fresh approved scope must return an empty plan history; no NOT_FOUND translation or preseed permitted'
                    expect(panel.get_by_role('combobox', name='Selected Goal scope', exact=True)).to_have_value(scope['id'])
                    plan_detail = panel.locator('details').filter(has=page.get_by_text('Versioned plan proposal', exact=True)).last
                    open_details(plan_detail)
                    passed('real UI lazily lists an independently approved Goal and empty immutable plan history')

                    contract = scope['outputs']['report']
                    node_budget = {'attempts': 2, 'parallel': 1, 'runtime_ms': 1000, 'output_bytes': 1024, 'children': 0, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
                    node = {'id': 'write-report', 'parent_id': None, 'kind': 'work', 'goal': 'Write report using fixed input', 'dependencies': [], 'workspace_id': scope['workspace_id'],
                            'executor': {'kind': 'connection', 'binding': scope['binding'], 'program_sha256': scope['program_sha256']}, 'recipients': [],
                            'required_inputs': [{'goal_task_id': task['id'], 'artifact_id': 'source', 'contract_id': scope['inputs']['source']['contract_id'], 'source': {'kind': 'approved_external', 'sha256': scope['inputs']['source']['sha256']}}],
                            'outputs': [{'artifact_id': 'report', 'contract_id': contract}], 'required_validators': scope['contracts'][contract], 'side_effect': 'workspace_write',
                            'write_set': [{'kind': 'workspace', 'path': 'outputs/report.txt'}], 'budget': node_budget,
                            'reservation': {'runtime_ms': 500, 'output_bytes': 1024, 'tokens': None, 'cost_microunits': None},
                            'retry': {'max_attempts': 2, 'max_reworks': 0, 'retry_classes': ['before_dispatch_transient']}, 'idempotency_key': 'report-action', 'external_action_id': None}
                    proposal = {'schema_version': 1, 'plan_id': 'browser-goal-plan', 'goal': scope['goal'], 'scope_revision': scope['authority_revision'], 'revision': 1, 'expected_revision': 0, 'acceptance_rules': scope['required_acceptance_rules'], 'nodes': [node]}
                    original_raw = '\n  ' + json.dumps(proposal, indent=2) + '\n '
                    textarea = panel.get_by_role('textbox', name='Plan proposal JSON', exact=True)
                    textarea.fill(original_raw)
                    if args.structured_editor:
                        editing = scope['edit_context']
                        assert editing is not None and editing['max_nodes'] == selection['max_nodes']
                        assert editing['writable_prefixes'] == selection['writable_prefixes']
                        assert editing['output_artifacts'] == {output['artifact']['artifact_id']: output['artifact'] for output in selection['outputs']}
                        assert set(editing['rule_executors']) == set(scope['rules'])
                        before_edit = database_snapshot()
                        editor = panel.get_by_test_id('goal-plan-editor')
                        assert not editor.evaluate('(element)=>element.open')
                        # Real Chromium native summary activation, not a jsdom
                        # substitute or product keydown shim.
                        editor.locator(':scope > summary').focus()
                        page.keyboard.press('Enter')
                        expect(editor).to_have_attribute('open', '')
                        editor.get_by_role('button', name='Prepare steps from approved scope', exact=True).click()
                        first_detail = editor.locator('.goal-plan-editor-node').first
                        open_details(first_detail)
                        first_id = first_detail.locator(':scope > summary').inner_text().rsplit(' · ', 1)[1]
                        first_detail.get_by_role('textbox', name=f'Step goal {first_id}', exact=True).fill('Write the approved report from the fixed input')
                        first_artifacts = first_detail.locator('details').filter(has=page.get_by_text('Inputs and outputs', exact=True)).last
                        open_details(first_artifacts)
                        first_detail.get_by_role('checkbox', name=f'Output {first_id}: details', exact=True).uncheck()
                        first_detail.get_by_role('textbox', name=f'Proposed write paths (one relative path per line) {first_id}', exact=True).fill(editing['output_artifacts']['report']['relative_path'])
                        first_budget = first_detail.locator('details').filter(has=page.get_by_text('Bounded budget and retry', exact=True)).last
                        open_details(first_budget)
                        first_detail.get_by_role('textbox', name='Runtime (ms)', exact=True).fill('2000')
                        editor.get_by_role('button', name='Add step', exact=True).click()
                        second_detail = editor.locator('.goal-plan-editor-node').last
                        open_details(second_detail)
                        second_id = second_detail.locator(':scope > summary').inner_text().rsplit(' · ', 1)[1]
                        assert second_id != first_id
                        second_detail.get_by_role('textbox', name=f'Step goal {second_id}', exact=True).fill('Write the separately approved details after the report')
                        dependencies = second_detail.locator('details').filter(has=page.get_by_text('Dependencies and lineage', exact=True)).last
                        open_details(dependencies)
                        second_detail.get_by_role('checkbox', name=f'Dependency {second_id} ← {first_id}', exact=True).check()
                        second_artifacts = second_detail.locator('details').filter(has=page.get_by_text('Inputs and outputs', exact=True)).last
                        open_details(second_artifacts)
                        second_detail.get_by_role('checkbox', name=f'Output {second_id}: details', exact=True).check()
                        second_detail.get_by_role('textbox', name=f'Proposed write paths (one relative path per line) {second_id}', exact=True).fill(editing['output_artifacts']['details']['relative_path'])
                        second_budget = second_detail.locator('details').filter(has=page.get_by_text('Bounded budget and retry', exact=True)).last
                        open_details(second_budget)
                        second_detail.get_by_role('textbox', name='Runtime (ms)', exact=True).fill('3000')
                        expect(editor.get_by_role('region', name='Dependency graph', exact=True)).to_contain_text(f'{first_id} → {second_id}')
                        assert textarea.input_value() == original_raw, 'Local step edits silently normalized or replaced the original advanced JSON'
                        assert database_snapshot() == before_edit
                        assert not any(row['method'] in {'goal_plans.propose', 'goal_plans.revise'} for row in report['requests'])
                        graph_heading = editor.get_by_role('heading', name='Dependency graph', exact=True)
                        graph_heading.scroll_into_view_if_needed()
                        expect(graph_heading).to_be_visible()
                        screenshot('editor-01-local-steps-no-write')
                        editor.get_by_role('button', name='Apply steps to JSON draft', exact=True).click()
                        original_raw = textarea.input_value()
                        proposal = json.loads(original_raw)
                        assert [node['id'] for node in proposal['nodes']] == [first_id, second_id]
                        assert [node['kind'] for node in proposal['nodes']] == ['work', 'work']
                        assert [node['budget']['runtime_ms'] for node in proposal['nodes']] == [2000, 3000]
                        assert proposal['nodes'][1]['dependencies'] == [first_id]
                        assert [node['outputs'] for node in proposal['nodes']] == [[{'artifact_id': 'report', 'contract_id': scope['outputs']['report']}], [{'artifact_id': 'details', 'contract_id': scope['outputs']['details']}]]
                        assert proposal['acceptance_rules'] == scope['required_acceptance_rules']
                        assert proposal['goal'] == scope['goal'] and proposal['scope_revision'] == scope['authority_revision']
                        for edited_node in proposal['nodes']:
                            assert edited_node['executor'] == {'kind': 'connection', 'binding': scope['binding'], 'program_sha256': scope['program_sha256']}
                            assert edited_node['recipients'] == [] and edited_node['external_action_id'] is None
                        assert database_snapshot() == before_edit, 'Applying local steps wrote the daemon journal or reserved execution resources'
                        assert_no_execution()
                        report['observations']['structured_editor'] = {'editing_context': editing, 'proposal': proposal, 'raw_sha256': sha(original_raw.encode()), 'local_edit_snapshot': before_edit, 'after_apply_snapshot': database_snapshot()}
                        graph_heading.scroll_into_view_if_needed()
                        expect(graph_heading).to_be_visible()
                        screenshot('editor-02-applied-local-draft')
                        page.set_viewport_size({'width': 390, 'height': 844})
                        graph_heading.scroll_into_view_if_needed()
                        expect(graph_heading).to_be_visible()
                        screenshot('editor-03-mobile-dependency-graph')
                        assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                        page.set_viewport_size({'width': 1488, 'height': 981})
                        passed('actual structured UI prepares and edits two approved work nodes and dependency without daemon writes; explicit Apply updates original model draft only')
                    node_ids = {node['id'] for node in proposal['nodes']}
                    panel.get_by_role('button', name='Store initial plan proposal', exact=True).click()
                    first = wait(lambda: ipc('goal_plans.get', {'scope_id': scope['id']}), lambda response: response.get('result', {}).get('proposal', {}).get('revision') == 1)['result']
                    expect(panel.get_by_role('button', name='Store revised plan proposal', exact=True)).to_be_enabled()
                    assert first['proposal'] == proposal and first['dispatched'] is False and first['state'] == 'validated_proposal'
                    request = next(r for r in report['requests'] if r['method'] == 'goal_plans.propose')
                    assert request['args']['planJson'] == original_raw and request['plan_json_sha256'] == sha(original_raw.encode())
                    with sqlite3.connect(f'file:{runtime / "baton.sqlite3"}?mode=ro', uri=True) as database:
                        persisted_raw = [json.loads(row[0])['plan_json'] for row in database.execute('SELECT document FROM goal_plan_proposals')]
                    assert persisted_raw == [original_raw], 'Original raw plan string was normalized in transit or persistence'
                    assert_no_execution()
                    passed('actual UI stores original raw plan bytes and server-derived scope references without dispatch', plan_digest=first['digest'], raw_sha256=sha(original_raw.encode()))

                    before = database_snapshot()
                    panel.get_by_role('button', name='Read readiness preview', exact=True).click()
                    expect(panel.get_by_role('region', name='Read-only readiness observation', exact=True)).to_be_visible()
                    preview = result('goal_plans.ready_preview', {'scope_id': scope['id']}, revision=1)
                    assert preview['read_only'] is True and preview['reservation_made'] is False and preview['dispatched'] is False
                    assert preview['journal_sequence'] == before['journal_sequence'] and database_snapshot() == before
                    for usage in preview['budget_usage'].values():
                        assert usage['attempts'] == usage['active'] == usage['children'] == 0
                        assert usage['runtime_ms'] == usage['output_bytes'] == 0, 'Fresh real zero history must be measured honestly'
                        assert all(value is None or bounded_integer(value) for value in usage.values())
                    if preview['decision'] is not None:
                        assert preview['decision']['requires_atomic_store_commit'] is True
                        assert all(item['node_id'] in node_ids for item in preview['decision']['admissions'])
                    if args.structured_editor:
                        assert preview['decision'] is not None
                        assert [item['node_id'] for item in preview['decision']['admissions']] == [proposal['nodes'][0]['id']]
                    report['observations']['ready_preview'] = preview
                    preview_heading = panel.get_by_role('heading', name='Read-only readiness observation', exact=True)
                    preview_heading.scroll_into_view_if_needed()
                    expect(preview_heading).to_be_visible()
                    screenshot('02-read-only-preview-desktop')
                    page.set_viewport_size({'width': 390, 'height': 844})
                    preview_heading.scroll_into_view_if_needed()
                    expect(preview_heading).to_be_visible()
                    screenshot('03-read-only-preview-mobile')
                    assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                    page.set_viewport_size({'width': 1488, 'height': 981})
                    assert_no_execution()
                    passed('real UI preview and direct repeat leave journal, ledgers, attempts, leases and execution unchanged')

                    first_args = request['args']
                    before = database_snapshot()
                    replay = result('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': original_raw}, first_args['idempotencyKey'], 0)
                    assert replay == first and database_snapshot() == before
                    conflict = ipc('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': ' ' + original_raw}, first_args['idempotencyKey'], 0)
                    assert conflict['error']['code'] == 'CONFIG_CONFLICT' and database_snapshot() == before
                    stale = ipc('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': original_raw}, 'stale-cas', 0)
                    assert stale['error']['code'] == 'STALE_REVISION' and database_snapshot() == before
                    node_marker = '"id": ' + json.dumps(proposal['nodes'][0]['id'])
                    duplicate = original_raw.replace(node_marker, node_marker + ', ' + node_marker, 1)
                    assert duplicate != original_raw, 'The actual node ID duplicate control was not constructed'
                    malformed = browser_request('goal_plans_propose', {**first_args, 'idempotencyKey': 'duplicate-plan', 'planJson': duplicate})
                    assert malformed['status'] == 200 and malformed['body']['error']['code'] == 'INVALID_REQUEST'
                    assert database_snapshot() == before
                    passed('exact-key replay is read-only; whitespace/key conflict, stale CAS and raw nested duplicates are refused by actual Core')

                    panel.get_by_role('button', name='Prepare next plan revision', exact=True).click()
                    next_proposal = json.loads(textarea.input_value())
                    assert next_proposal['revision'] == 2 and next_proposal['expected_revision'] == 1 and next_proposal['plan_id'] == proposal['plan_id']
                    next_proposal['nodes'][0]['goal'] = 'Refine the report within unchanged approved scope'
                    revised_raw = '\n' + json.dumps(next_proposal, indent=3) + '\n'
                    textarea.fill(revised_raw)
                    panel.get_by_role('button', name='Store revised plan proposal', exact=True).click()
                    second = wait(lambda: result('goal_plans.get', {'scope_id': scope['id']}), lambda row: row['proposal']['revision'] == 2)
                    assert second['proposal'] == next_proposal
                    history = result('goal_plans.list', {'scope_id': scope['id']})
                    assert [entry['proposal']['revision'] for entry in history] == [1, 2] and history[0] == first
                    before = database_snapshot()
                    old_preview = ipc('goal_plans.ready_preview', {'scope_id': scope['id']}, revision=1)
                    assert old_preview['error']['code'] == 'STALE_REVISION' and database_snapshot() == before
                    history_detail = panel.locator('details').filter(has=page.get_by_text('Plan revision history', exact=True)).last
                    open_details(history_detail)
                    screenshot('04-versioned-plan-history')
                    passed('actual UI revises with correct CAS and retains immutable first revision; stale preview is refused')

                    before = database_snapshot()
                    denied_commands = ['goal_scope_approve', 'approvals_finalize', 'grants_propose', 'connections_propose', 'tasks_create', 'sessions_create', 'goal_dispatch', 'goal_reserve', 'tasks_accept', 'generic_ipc']
                    assert all(browser_request(command, {})['status'] == 403 for command in denied_commands)
                    assert browser_request('goal_plans_list', {'scopeId': 'foreign-scope'})['status'] == 400
                    assert browser_request('goal_plans_list', {'scopeId': scope['id'], 'verified': True})['status'] == 400
                    assert browser_request('goal_plans_ready_preview', {'scopeId': scope['id'], 'expectedRevision': True})['status'] == 400
                    assert database_snapshot() == before
                    passed('test HTTP adapter refuses approval, generic IPC, execution, forged scope, extra fields and boolean revisions')

                    # An honest change to the selected fixture input, never a SQL
                    # mutation: strict history remains readable, live action fails.
                    input_path = Path(workspace['root']) / 'input.txt'
                    input_path.write_bytes(b'changed after immutable plan commit\n')
                    before = database_snapshot()
                    assert result('goal_plans.list', {'scope_id': scope['id']}) == history
                    panel.get_by_role('button', name='Read readiness preview', exact=True).click()
                    expect(panel.get_by_role('alert')).to_be_visible()
                    expect(panel.get_by_role('button', name='Read readiness preview', exact=True)).to_be_disabled()
                    expect(panel.get_by_role('button', name='Store revised plan proposal', exact=True)).to_be_disabled()
                    assert database_snapshot() == before
                    assert panel.get_by_role('region', name='Read-only readiness observation', exact=True).count() == 0
                    screenshot('05-input-drift-read-refused')
                    input_path.write_bytes(fixed_input)
                    panel.get_by_role('button', name='Refresh Goal records', exact=True).click()
                    expect(panel.get_by_role('button', name='Read readiness preview', exact=True)).to_be_enabled()
                    expect(panel.get_by_role('button', name='Store revised plan proposal', exact=True)).to_be_enabled()
                    assert database_snapshot() == before
                    passed('real input drift refuses live preview, clears old observation and disables new UI writes while retaining strict history')

                    panel.get_by_role('button', name='Revoke Goal scope', exact=True).click()
                    wait(lambda: result('goal_scopes.get', {'scope_id': scope['id']}), lambda row: row['revoked'] is True)
                    expect(panel.get_by_role('button', name='Read readiness preview', exact=True)).to_be_disabled()
                    expect(panel.get_by_role('button', name='Store revised plan proposal', exact=True)).to_be_disabled()
                    panel.get_by_role('button', name='Refresh Goal records', exact=True).click()
                    expect(panel.locator('[aria-busy]')).to_have_attribute('aria-busy', 'false')
                    report['observations']['revoked_history_response'] = ipc('goal_plans.list', {'scope_id': scope['id']})
                    screenshot('06-revoked-historical-scope')
                    assert report['observations']['revoked_history_response'].get('result') == history, 'Revocation must not erase strict immutable historical read access'
                    assert result('goal_plans.get', {'scope_id': scope['id']}) == second
                    before = database_snapshot()
                    assert result('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': original_raw}, first_args['idempotencyKey'], 0) == first
                    third = {**next_proposal, 'revision': 3, 'expected_revision': 2}
                    denied = ipc('goal_plans.revise', {'scope_id': scope['id'], 'plan_json': json.dumps(third)}, 'revoked-new-plan', 2)
                    assert denied['error']['code'] == 'PERMISSION_DENIED'
                    assert ipc('goal_plans.ready_preview', {'scope_id': scope['id']}, revision=2)['error']['code'] == 'PERMISSION_DENIED'
                    assert database_snapshot() == before
                    assert_no_execution()
                    assert not page_errors, page_errors
                    report['observations']['final_database'] = database_snapshot()
                    passed('revoked scope permits strict historical reads/exact-key replay while UI and Core deny new plans and previews')
                    report['status'] = 'passed'
                except BaseException:
                    screenshot('failure-current-ui')
                    raise
                finally:
                    report['observations']['browser_page_errors'] = page_errors
                    browser.close()
        except BaseException as failure:
            report['failure'] = {'type': type(failure).__name__, 'message': str(failure), 'traceback': traceback.format_exc()}
            raise
        finally:
            if server is not None:
                server.shutdown()
                server.server_close()
            if thread is not None:
                thread.join(timeout=5)
            daemon.terminate()
            try:
                daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait()
            daemon_log.close()
            (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'report': str(output / 'report.json')}), flush=True)


if __name__ == '__main__':
    main()
