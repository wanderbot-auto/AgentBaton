#!/usr/bin/env python3
"""Browser product verification against a real temporary daemon.

Only this test harness serves a bounded localhost invoke adapter. It is never
imported by Vite/product code, included in the desktop bundle or a native GUI
proof. No vendor executable, account, model or simulated completion is used.
"""
from __future__ import annotations
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import threading
import time
import uuid
from functools import partial
from playwright.sync_api import sync_playwright


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    binary = args.batond.resolve(strict=True)
    frontend = Path(__file__).resolve().parents[1] / 'dist'
    assert (frontend / 'index.html').is_file(), 'Build the actual product frontend first'
    report = {'dimension': 'actual built React frontend + test-only localhost adapter + real Unix daemon/SQLite; not native GUI or real Provider', 'daemon_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'checks': [], 'screenshots': [], 'requests': []}
    def passed(name, **details):
        report['checks'].append({'name': name, 'status': 'passed', **details})
    with tempfile.TemporaryDirectory(prefix='baton-quiet-canvas-') as temporary:
        runtime = Path(temporary) / 'runtime'
        runtime.mkdir(mode=0o700)
        daemon_log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)}, stdout=daemon_log, stderr=subprocess.STDOUT)
        server = None
        thread = None
        try:
            deadline = time.monotonic() + 5
            while not (runtime / 'session.token').is_file() or not (runtime / 'batond.sock').exists():
                assert daemon.poll() is None, 'Real daemon exited during startup'
                assert time.monotonic() < deadline, 'Real daemon startup timed out'
                time.sleep(.01)
            token = (runtime / 'session.token').read_text()
            def ipc(method, params, key=None, revision=None):
                request = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token, 'method': method, 'params': params}
                if key is not None:
                    request['idempotency_key'] = key
                if revision is not None:
                    request['expected_revision'] = revision
                with socket.socket(socket.AF_UNIX) as stream:
                    stream.settimeout(3)
                    stream.connect(str(runtime / 'batond.sock'))
                    stream.sendall(json.dumps(request).encode() + b'\n')
                    with stream.makefile('rb') as reader:
                        line = reader.readline(1_048_577)
                assert len(line) <= 1_048_576 and line.endswith(b'\n')
                response = json.loads(line)
                report['requests'].append({'method': method, 'error': response.get('error', {}).get('code'), 'side_effect_state': response.get('error', {}).get('side_effect_state')})
                return response
            class Handler(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *_args):
                    pass
                def do_POST(self):
                    origin = f'http://127.0.0.1:{self.server.server_port}'
                    if self.path != '/__test_ipc' or self.headers.get('Origin') != origin or self.headers.get('Content-Type') != 'application/json':
                        self.send_error(403)
                        return
                    length = int(self.headers.get('Content-Length', '0'))
                    if not 0 < length <= 65536:
                        self.send_error(413)
                        return
                    try:
                        body = json.loads(self.rfile.read(length))
                        command, values = body['command'], body.get('args') or {}
                        allowed = {
                            'system_snapshot': ('system.snapshot', {}, set()),
                            'projects_list': ('projects.list', {}, set()),
                            'connections_list': ('connections.list', {}, set()),
                            'sessions_list': ('sessions.list', {}, set()),
                            'events_subscribe': ('events.subscribe', {'cursor': values.get('cursor', 0), 'limit': values.get('limit', 100)}, {'cursor', 'limit'}),
                            'tasks_create': ('tasks.create', {'goal': values.get('goal', ''), **({'project_id': values['projectId']} if values.get('projectId') else {})}, {'goal', 'projectId', 'idempotencyKey'}),
                            'tasks_cancel': ('tasks.cancel', {'task_id': values.get('taskId', '')}, {'taskId', 'expectedRevision', 'idempotencyKey'}),
                        }
                        if command not in allowed:
                            self.send_error(403)
                            return
                        method, params, keys = allowed[command]
                        if not isinstance(values, dict) or set(values) - keys:
                            self.send_error(400)
                            return
                        response = ipc(method, params, values.get('idempotencyKey'), values.get('expectedRevision'))
                        raw = json.dumps(response).encode()
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.send_header('Content-Length', str(len(raw)))
                        self.end_headers()
                        self.wfile.write(raw)
                    except (OSError, ValueError, KeyError, AssertionError):
                        self.send_error(502)
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            url = f'http://127.0.0.1:{server.server_port}'
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
                context = browser.new_context(viewport={'width': 1488, 'height': 981}, locale='zh-CN')
                context.add_init_script("""localStorage.setItem('agentbaton.language','zh');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const response=await fetch('/__test_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!response.ok)throw new Error('Test adapter transport unavailable');return response.json();}};""")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page()
                errors = []
                page.on('pageerror', lambda error: errors.append(str(error)))
                page.goto(url)
                page.get_by_text('本地后台已连接', exact=True).wait_for()
                assert page.get_by_test_id('task-card').count() == 0
                passed('empty state contains no demonstration task')
                goals = ['梳理工作区权限边界', '整理已记录的测试证据', '检查取消与恢复状态', '准备开发安装包验收清单', '检查项目配置的来源']
                task_ids = []
                for goal in goals:
                    page.get_by_test_id('goal-input').fill(goal)
                    page.get_by_role('button', name='创建任务', exact=True).click()
                    thread_view = page.get_by_test_id('task-thread')
                    thread_view.wait_for()
                    task_ids.append(thread_view.get_attribute('data-task-id'))
                    assert page.get_by_role('heading', name=goal, exact=True).is_visible()
                    assert page.get_by_text('尚无执行证据。本任务尚未派发给 Provider。', exact=True).is_visible()
                    page.get_by_role('button', name='返回任务', exact=True).click()
                assert len(set(task_ids)) == 5
                passed('five actual UI drafts persisted through Unix IPC', task_ids=task_ids)
                page.get_by_role('button', name='快捷对话', exact=True).click()
                assert page.get_by_role('button', name='发送消息 · 尚不可用').is_disabled()
                page.get_by_label('草稿上下文', exact=True).fill('检查只读配置输入')
                page.get_by_role('button', name='转为任务', exact=True).click()
                dialog = page.get_by_role('dialog', name='从此摘要创建任务')
                assert dialog.is_visible()
                page.get_by_label('选定摘要', exact=True).fill('仅将此审核摘要保存为任务')
                dialog.get_by_role('button', name='创建任务', exact=True).click()
                page.get_by_test_id('task-thread').wait_for()
                assert page.get_by_role('heading', name='仅将此审核摘要保存为任务').is_visible()
                page.get_by_role('button', name='返回任务', exact=True).click()
                passed('reviewed quick-context summary converts to actual draft, standalone model send unavailable')
                page.get_by_role('button', name='进行中', exact=True).click()
                assert page.get_by_test_id('task-card').count() == 0
                page.get_by_role('button', name='全部', exact=True).click()
                assert page.get_by_test_id('task-card').count() == 6
                passed('truthful recorded-state filtering')
                page.get_by_test_id('goal-input').fill('任' * 6000)
                page.get_by_role('button', name='创建任务', exact=True).click()
                page.get_by_role('alert').wait_for()
                assert any(item['error'] == 'INVALID_REQUEST' for item in report['requests'])
                assert len(ipc('system.snapshot', {})['result']['tasks']) == 6
                page.get_by_test_id('goal-input').fill('')
                page.get_by_role('button', name='刷新', exact=True).click()
                page.get_by_text('本地后台已连接', exact=True).wait_for()
                passed('real core rejects oversized UTF-8 goal without inserting a task')
                for width, height in [(1488, 981), (1440, 900), (1024, 768), (390, 844)]:
                    page.set_viewport_size({'width': width, 'height': height})
                    page.wait_for_timeout(50)
                    layout = page.evaluate("""()=>{const grid=document.querySelector('.task-grid');return{columns:getComputedStyle(grid).gridTemplateColumns.split(' ').length,overflow:document.documentElement.scrollWidth>innerWidth,sidebar:document.querySelector('.sidebar').getBoundingClientRect().width,fonts:[...new Set([...document.querySelectorAll('body *')].filter(e=>e.getClientRects().length&&e.textContent.trim()&&!e.children.length).map(e=>getComputedStyle(e).fontSize))],weights:[...new Set([...document.querySelectorAll('body *')].filter(e=>e.getClientRects().length&&e.textContent.trim()&&!e.children.length).map(e=>Number(getComputedStyle(e).fontWeight)))]}}""")
                    assert layout['columns'] == (1 if width < 760 else 2), layout
                    assert not layout['overflow'], layout
                    assert set(layout['fonts']) <= {'14px', '18px', '24px'}, layout
                    assert all(weight <= 600 for weight in layout['weights']), layout
                    if width == 1024:
                        assert layout['sidebar'] == 64, layout
                        assert page.get_by_role('button', name='新任务', exact=True).is_visible()
                        assert page.get_by_role('button', name='搜索', exact=True).is_visible()
                        assert page.get_by_role('combobox', name='语言', exact=True).is_visible()
                        passed('collapsed rail controls retain accessible localized names')
                    if width == 390:
                        toggle = page.get_by_role('button', name='切换导航', exact=True)
                        bounds = toggle.bounding_box()
                        assert bounds['width'] >= 44 and bounds['height'] >= 44, bounds
                        toggle.click()
                        assert page.get_by_role('button', name='新任务', exact=True).is_visible()
                        assert page.get_by_role('button', name='搜索', exact=True).is_visible()
                        assert page.get_by_role('combobox', name='语言', exact=True).is_visible()
                        toggle.click()
                        passed('mobile navigation exposes named controls and44px toggle target')
                    screenshot = output / f'home-{width}x{height}.png'
                    page.screenshot(path=str(screenshot))
                    report['screenshots'].append(str(screenshot))
                    passed(f'home layout {width}x{height}', **layout)
                    page.get_by_test_id('task-card').last.scroll_into_view_if_needed()
                    bounds = page.evaluate("""()=>({last:document.querySelector('.task-card:last-child').getBoundingClientRect().bottom,composer:document.querySelector('.composer-wrap').getBoundingClientRect().top})""")
                    assert bounds['last'] <= bounds['composer'], bounds
                    passed(f'last card remains accessible above composer {width}x{height}')
                    page.get_by_test_id('task-card').first.click()
                    page.get_by_test_id('task-thread').wait_for()
                    assert page.get_by_test_id('task-card').count() == 0
                    screenshot = output / f'task-{width}x{height}.png'
                    page.screenshot(path=str(screenshot))
                    report['screenshots'].append(str(screenshot))
                    page.get_by_role('tab', name='产物', exact=True).click()
                    assert page.get_by_text('此任务尚无已验证的产物。', exact=True).is_visible()
                    page.get_by_role('button', name='返回任务', exact=True).click()
                page.set_viewport_size({'width': 1488, 'height': 981})
                page.get_by_role('button', name='搜索', exact=True).click()
                search = page.get_by_role('dialog', name='搜索任务')
                page.get_by_role('textbox', name='搜索任务', exact=True).fill(goals[0])
                assert search.get_by_role('button', name=re.compile(goals[0])).count() == 1
                page.keyboard.press('Escape')
                assert page.get_by_role('dialog').count() == 0
                assert page.get_by_role('button', name='搜索', exact=True).evaluate('(e)=>e===document.activeElement')
                page.keyboard.press('Control+k')
                page.get_by_role('dialog', name='搜索任务').wait_for()
                page.keyboard.press('Escape')
                passed('keyboard command search, Escape and focus restore')
                page.get_by_role('button', name='设置', exact=True).click()
                settings = page.get_by_role('dialog', name='设置', exact=True)
                close = settings.get_by_role('button', name='关闭', exact=True)
                summary = settings.locator('summary')
                assert close.evaluate('(e)=>e===document.activeElement')
                page.keyboard.press('Shift+Tab')
                assert summary.evaluate('(e)=>e===document.activeElement')
                page.keyboard.press('Tab')
                assert close.evaluate('(e)=>e===document.activeElement')
                page.keyboard.press('Escape')
                assert page.get_by_role('button', name='设置', exact=True).evaluate('(e)=>e===document.activeElement')
                passed('actual closed-details summary is included in dialog focus trap')
                page.emulate_media(reduced_motion='reduce')
                assert page.get_by_test_id('task-card').first.evaluate('(e)=>getComputedStyle(e).transitionDuration') == '0s'
                passed('reduced motion disables product transitions')
                page.get_by_test_id('task-card').first.click()
                page.get_by_role('tab', name='详情', exact=True).click()
                page.get_by_role('button', name='请求取消', exact=True).click()
                page.get_by_text('已停止（已确认）', exact=True).wait_for()
                assert page.get_by_role('button', name='请求取消', exact=True).is_disabled()
                passed('actual never-dispatched draft cancellation is confirmed by core evidence')
                page.get_by_role('button', name='返回任务', exact=True).click()
                page.get_by_role('button', name='历史', exact=True).click()
                assert page.get_by_text('task.created', exact=True).count() == 6
                assert page.get_by_text('task.cancelled', exact=True).count() == 1
                passed('history uses actual journal events')
                assert not errors, errors
                passed('no browser runtime errors')
                context.close()
                browser.close()
            report['screenshot_sha256'] = {str(Path(name).name): hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in report['screenshots']}
            (output / 'evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
            print(json.dumps({'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'evidence': str(output / 'evidence.json')}, ensure_ascii=False))
        except BaseException:
            (output / 'failure-evidence.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
            raise
        finally:
            if server:
                server.shutdown()
                server.server_close()
            if thread:
                thread.join(timeout=2)
            if daemon.poll() is None:
                daemon.terminate()
                try:
                    daemon.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    daemon.kill()
                    daemon.wait(timeout=2)
            daemon_log.close()

if __name__ == '__main__':
    main()
