#!/usr/bin/env python3
"""Actual built UI + Unix daemon command execution, using private local fixtures.

The owner setup below approves only generated test files, outside HTTP. The
browser adapter cannot propose or finalize any approval. This is Linux/browser
evidence, not a native owner dialog, real Provider/model call or OS sandbox.
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
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from playwright.sync_api import sync_playwright


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--chromium')
    args = parser.parse_args()
    binary = args.batond.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    frontend = Path(__file__).resolve().parents[1] / 'dist'
    assert (frontend / 'index.html').is_file()
    report = {'dimension': 'actual built React + bounded test-only localhost adapter + real Unix IPC/daemon/owned commands',
              'native_owner_dialog': 'not_run; authenticated owner fixture setup is outside HTTP',
              'provider': 'generated local ACP handshake fixture; session/new intentionally fails; no vendor/account/model',
              'daemon_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'frontend_assets_sha256': {str(p.relative_to(frontend)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(frontend.rglob('*')) if p.is_file()},
              'status': 'failed_or_interrupted', 'checks': [], 'requests': [], 'screenshots': []}

    def passed(name, **details):
        report['checks'].append({'name': name, 'status': 'passed', **details})

    def wait(read, check, seconds=10):
        end = time.monotonic() + seconds
        while True:
            value = read()
            if check(value):
                return value
            assert time.monotonic() < end, f'Fixture observation timed out: {value}'
            time.sleep(.02)

    with tempfile.TemporaryDirectory(prefix='baton-command-browser-') as temporary:
        root = Path(temporary).resolve()
        root.chmod(0o700)
        runtime = root / 'runtime'
        runtime.mkdir(mode=0o700)
        daemon_log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)}, stdout=daemon_log, stderr=subprocess.STDOUT)
        server = thread = None
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()),
                 lambda v: v[0] and v[1] and v[2] is None)
            token = (runtime / 'session.token').read_text()

            def ipc(method, params, key=None, revision=None):
                request = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token, 'method': method, 'params': params}
                if key is not None:
                    request['idempotency_key'] = key
                if revision is not None:
                    request['expected_revision'] = revision
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(5)
                    connection.connect(str(runtime / 'batond.sock'))
                    connection.sendall(json.dumps(request).encode() + b'\n')
                    with connection.makefile('rb') as response:
                        raw = response.readline(1_048_577)
                assert len(raw) <= 1_048_576
                return json.loads(raw)

            def result(method, params, key=None, revision=None):
                envelope = ipc(method, params, key, revision)
                assert 'error' not in envelope, envelope
                return envelope['result']

            def owner_approve(method, params, key):
                proposal = result(method, params, key)
                # This setup is the fixture owner, never the browser/Agent.
                approved = result('approvals.finalize', {'proposal_id': proposal['id']}, f'proposal:{proposal["id"]}', proposal['revision'])
                return approved

            source = root / 'source'
            source.mkdir()
            script = """case "${1-}" in
normal) printf 'normal\\n' >> marker; printf 'verified output\\n' > result.txt; printf 'fixture stdout';;
cancel) printf 'cancel\\n' >> marker; /bin/sleep 8; printf 'late output\\n' > result.txt;;
*) exit 9;;
esac
"""
            (source / 'input.txt').write_text('fixed input\n')
            (source / '.validator.sh').write_text(script)
            (source / '.gitignore').write_text('result.txt\nmarker\n')
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}
            def git(*arguments):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *arguments], env=git_env, text=True).strip()
            git('init', '-q', '--initial-branch=main')
            git('add', 'input.txt', '.validator.sh', '.gitignore')
            git('commit', '-qm', 'generated command fixture')
            head = git('rev-parse', 'HEAD')
            project = result('projects.import', {'path': str(source), 'name': 'Actual command browser fixture'}, 'project')
            workspace = result('workspaces.create', {'project_id': project['id'], 'expected_head': head}, 'workspace', project['revision'])
            # A real owned process: successful initialize, invalid session/new.
            # This obtains typed reaping evidence; failed initialize alone is unknown.
            provider = root / 'generated-acp'
            provider.write_text(f'#!{sys.executable}\n' + """import json,sys
if '--version' in sys.argv:
 print('AgentBaton local browser fixture 1.0');sys.exit(0)
for line in sys.stdin:
 r=json.loads(line)
 if r.get('method')=='initialize':
  print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'protocolVersion':1,'agentCapabilities':{}}}),flush=True)
 elif r.get('method')=='session/new':
  print('generated-invalid-session-response',flush=True);sys.exit(1)
""")
            provider.chmod(0o700)
            connection = owner_approve('connections.propose', {'name': 'Generated ACP only; no account', 'provider': 'custom_acp', 'executable': str(provider), 'arguments': []}, 'connection')
            wait(lambda: result('connections.list', {}), lambda rows: any(c['id'] == connection['id'] and c['probe_state'] == 'ready' for c in rows))
            grant = owner_approve('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 2, 'max_concurrency': 1, 'max_duration_ms': 10000}, 'grant')
            goal = 'Verify actual approved command execution and cancellation'
            task = result('tasks.create', {'project_id': project['id'], 'goal': goal}, 'task')
            session = result('sessions.create', {'task_id': task['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'grant_id': grant['id']}, 'session', task['revision'])
            session = wait(lambda: result('sessions.get', {'session_id': session['id']}), lambda s: s['state'] == 'stopped')
            task = result('tasks.get', {'task_id': task['id']})
            assert task['state'] == 'outcome_unknown'
            journal = result('events.subscribe', {'cursor': 0, 'limit': 100})
            assert any(e['kind'] == 'attempt.stopped' and e['payload']['stop_evidence']['kind'] == 'executor_stop' for e in journal['events'])
            passed('actual local fixture handshake failure yields confirmed owned stop and distinct task-result uncertainty', task_id=task['id'], session_id=session['id'])
            command_root = Path(workspace['root'])
            enforcement = dict.fromkeys(['filesystem_isolation', 'network_isolation', 'process_tree_containment', 'execution_time_input_immutability', 'auxiliary_source_immutability', 'sealed_executable_image'], False)
            selection = {'session_id': session['id'], 'executable': str(Path('/bin/sh').resolve()), 'mode': 'posix_sh_stdin',
                         'script': {'artifact_id': 'script', 'relative_path': '.validator.sh', 'expected_sha256': hashlib.sha256(script.encode()).hexdigest(), 'max_bytes': 65536},
                         'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'expected_sha256': hashlib.sha256(b'fixed input\n').hexdigest(), 'max_bytes': 4096}],
                         'outputs': [{'artifact_id': 'output', 'relative_path': 'result.txt', 'expected_sha256': None, 'max_bytes': 4096}],
                         'arguments': ['normal'], 'environment': {}, 'expected_exit_codes': [0], 'wall_time_ms': 10000, 'cleanup_time_ms': 1000, 'stdout_limit': 4096, 'stderr_limit': 4096, 'require': enforcement}
            normal = owner_approve('command_validation_policies.propose', selection, 'normal-policy')
            assert normal['max_runs'] == 1 and normal['scope'] == 'approved_deterministic_command_only'

            class Handler(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *_):
                    pass
                def do_POST(self):
                    if self.path != '/__test_command_ipc':
                        self.send_error(404); return
                    try:
                        size = int(self.headers.get('Content-Length', '0'))
                        assert 0 < size <= 65536
                        request = json.loads(self.rfile.read(size))
                        command, values = request['command'], request.get('args') or {}
                        assert isinstance(values, dict)
                        read = {'system_snapshot': ('system.snapshot', {}), 'projects_list': ('projects.list', {}), 'connections_list': ('connections.list', {}), 'sessions_list': ('sessions.list', {})}
                        scoped = {'workspaces_list': ('workspaces.list', 'projectId', 'project_id'), 'grants_list': ('grants.list', 'projectId', 'project_id'),
                                  'validation_policies_list': ('validation_policies.list', 'taskId', 'task_id'), 'validations_list': ('validations.list', 'taskId', 'task_id'),
                                  'data_validation_policies_list': ('data_validation_policies.list', 'taskId', 'task_id'), 'data_validations_list': ('data_validations.list', 'taskId', 'task_id'),
                                  'command_validation_policies_list': ('command_validation_policies.list', 'taskId', 'task_id'), 'command_validations_list': ('command_validations.list', 'taskId', 'task_id'),
                                  'command_validations_get': ('command_validations.get', 'runId', 'run_id')}
                        if command in read:
                            method, params = read[command]; assert not values
                        elif command in scoped:
                            method, source_key, target_key = scoped[command]; assert set(values) == {source_key}; params = {target_key: values[source_key]}
                        elif command == 'events_subscribe':
                            assert set(values) <= {'cursor', 'limit'}; method, params = 'events.subscribe', {'cursor': values.get('cursor', 0), 'limit': values.get('limit', 100)}
                        elif command in {'command_validations_run', 'command_validations_cancel', 'command_validation_policies_revoke'}:
                            method, camel, snake = {'command_validations_run': ('command_validations.run', 'policyId', 'policy_id'), 'command_validations_cancel': ('command_validations.cancel', 'runId', 'run_id'), 'command_validation_policies_revoke': ('command_validation_policies.revoke', 'policyId', 'policy_id')}[command]
                            assert set(values) == {camel, 'idempotencyKey', 'expectedRevision'}; params = {snake: values[camel]}
                        else:
                            self.send_error(403); return
                        response = ipc(method, params, values.get('idempotencyKey'), values.get('expectedRevision'))
                        report['requests'].append({'method': method, 'args': values, 'error': response.get('error', {}).get('code')})
                        raw = json.dumps(response).encode()
                        self.send_response(200); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(raw))); self.end_headers(); self.wfile.write(raw)
                    except (AssertionError, OSError, ValueError, KeyError):
                        self.send_error(502)
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            url = f'http://127.0.0.1:{server.server_port}'
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
                context = browser.new_context(viewport={'width': 1488, 'height': 981}, locale='en-US')
                context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_command_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw Error('Bounded test adapter refused request');return r.json();}};""")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); errors = []; page.on('pageerror', lambda e: errors.append(str(e)))
                page.goto(url)
                page.get_by_test_id('task-card').filter(has_text=goal).click()
                page.locator('#tab-artifacts').click()
                panel = page.get_by_test_id('command-validation')
                assert not panel.locator(':scope > details').evaluate('(e)=>e.open')
                panel.locator(':scope > details > summary').click()
                def policy_detail():
                    return panel.locator(':scope > details > div > details').filter(has=page.get_by_role('button', name='Run approved command once', exact=True, include_hidden=True)).last
                def open_detail(detail):
                    if not detail.evaluate('(e)=>e.open'):
                        detail.locator(':scope > summary').click()
                def records():
                    return result('command_validations.list', {'task_id': task['id']})
                def terminal(policy):
                    return wait(records, lambda rows: any(r['policy_id'] == policy['id'] and r['state'] not in {'prepared', 'running', 'cancel_requested'} for r in rows))
                detail = policy_detail(); open_detail(detail)
                detail.get_by_role('button', name='Run approved command once', exact=True).click()
                rows = terminal(normal); record = next(r for r in rows if r['policy_id'] == normal['id'])
                assert record['state'] == 'command_passed' and record['evidence']['passed'] is True
                assert record['task_acceptance'] == 'not_established' and record['integration_authorized'] is False
                assert (command_root / 'result.txt').read_text() == 'verified output\n'
                rendered_pass = panel.locator('summary').filter(has_text=re.compile('^Command contract passed'))
                rendered_pass.wait_for(); open_detail(rendered_pass.locator('..')); rendered_pass.locator('..').scroll_into_view_if_needed()
                assert rendered_pass.locator('..').evaluate('(e)=>e.open')
                assert panel.get_by_text(record['evidence']['reason'], exact=True).is_visible()
                assert detail.get_by_role('button', name='Run approved command once', exact=True).is_disabled()
                screenshot = output / 'actual-command-passed.png'; page.screenshot(path=str(screenshot)); report['screenshots'].append(str(screenshot))
                passed('actual UI invokes one approved fixed script and renders real command-only success', run_id=record['id'])
                request = next(r for r in report['requests'] if r['method'] == 'command_validations.run')
                replay = result('command_validations.run', {'policy_id': normal['id']}, request['args']['idempotencyKey'], normal['revision'])
                assert replay['id'] == record['id']
                denied = ipc('command_validations.run', {'policy_id': normal['id']}, 'different-key', normal['revision'])
                assert denied['error']['code'] == 'PERMISSION_DENIED' and len(records()) == 1
                assert (command_root / 'marker').read_text() == 'normal\n'
                passed('same-key recovery returns original run; new-key policy replay cannot duplicate effects')
                cancel_policy = owner_approve('command_validation_policies.propose', {**selection, 'arguments': ['cancel']}, 'cancel-policy')
                panel.get_by_role('button', name='Refresh command records', exact=True).click()
                wait(lambda: panel.get_by_role('button', name='Run approved command once', exact=True, include_hidden=True).count(), lambda n: n == 2)
                detail = policy_detail(); open_detail(detail); detail.get_by_role('button', name='Run approved command once', exact=True).click()
                wait(lambda: (command_root / 'marker').read_text(), lambda text: text == 'normal\ncancel\n')
                cancel_detail = panel.locator(':scope > details > div > details').filter(has=page.get_by_role('button', name='Request owned command cancellation', exact=True, include_hidden=True)).first
                open_detail(cancel_detail); cancel_detail.get_by_role('button', name='Request owned command cancellation', exact=True).click()
                rows = terminal(cancel_policy); stopped = next(r for r in rows if r['policy_id'] == cancel_policy['id'])
                assert stopped['state'] == 'command_failed' and stopped['evidence']['cancellation_observed'] is True
                assert stopped['evidence']['cleanup']['owned_leader_reaped'] is True
                assert stopped['evidence']['cleanup']['descendant_containment'] == 'not_enforced_unknown'
                rendered_failure = panel.locator('summary').filter(has_text=re.compile('^Command contract failed'))
                rendered_failure.wait_for(); open_detail(rendered_failure.locator('..')); rendered_failure.locator('..').scroll_into_view_if_needed()
                assert rendered_failure.locator('..').evaluate('(e)=>e.open')
                assert panel.get_by_text(stopped['evidence']['reason'], exact=True).is_visible()
                screenshot = output / 'actual-command-cancelled.png'; page.screenshot(path=str(screenshot)); report['screenshots'].append(str(screenshot))
                passed('actual UI cancellation reaches the owned command and records reaping without claiming whole-tree containment', run_id=stopped['id'])
                assert result('tasks.get', {'task_id': task['id']})['state'] == 'outcome_unknown'
                sequence = result('system.snapshot', {})['sequence']
                for command in ['approvals_finalize', 'command_validation_policy_approve']:
                    status = page.evaluate("""async command=>(await fetch('/__test_command_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args:{}})})).status""", command)
                    assert status == 403
                assert result('system.snapshot', {})['sequence'] == sequence
                assert not any(r['method'] in {'approvals.finalize', 'command_validation_policies.propose'} for r in report['requests'])
                passed('HTTP test adapter cannot approve/finalize; real command results do not promote the task or authorize integration')
                assert not errors, errors
                report['browser_version'] = browser.version
                report['status'] = 'passed'
                browser.close()
        finally:
            if server:
                server.shutdown(); server.server_close(); thread.join(timeout=3)
            if daemon.poll() is None:
                daemon.terminate()
                try:
                    daemon.wait(timeout=12)
                except subprocess.TimeoutExpired:
                    report['forced_daemon_cleanup'] = True; daemon.kill(); daemon.wait(timeout=5)
            report['daemon_exit_code'] = daemon.returncode
            daemon_log.close()
            (output / 'evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(f'PASS actual approved-command UI/IPC fixture: {len(report["checks"])} checks')


if __name__ == '__main__':
    main()
