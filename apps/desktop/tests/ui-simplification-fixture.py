#!/usr/bin/env python3
"""Exercise two built desktop renderers against a private copy of the UI fixture."""
import argparse
from base64 import b64encode
from collections import Counter
from contextlib import contextmanager
from functools import partial
import hashlib
from html import escape
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
from threading import Thread
import time
import uuid

from playwright.sync_api import expect, sync_playwright


ROOT = Path(__file__).resolve().parents[3]
VIEWPORT = {"width": 1440, "height": 920}
READS = {
    "system_snapshot": "system.snapshot",
    "background_status": "system.background",
    "events_subscribe": "events.subscribe",
    "connection_inspect": "connections.inspect",
    "connection_probe_get": "connection_probe.get",
    "goal_plan_advice_context": "goal_plan_advice.context",
    "factory_child_plans_context": "factory_child_plans.context",
    "integration_context_get": "integration_context.get",
    "integration_targets_list": "integration_targets.list",
    "workspaces_resolve": "workspaces.resolve",
}
for method in """
    projects.list connections.list sessions.list sessions.get tasks.get tasks.history
    workspaces.list grants.list conversations.list conversations.get conversations.history
    validation_policies.list validations.list data_validation_policies.list data_validations.list
    command_validation_policies.list command_validations.list command_validations.get
    goal_scopes.list goal_scopes.get goal_plans.list goal_plans.get goal_planning.list goal_planning.get
    goal_runs.list goal_runs.get goal_graph_runs.list goal_graph_runs.get
    factory_scopes.list factory_scopes.get factory_runs.get factory_child_plans.list factory_child_plans.get
    factory_child_goals.list factory_child_goals.get managed_skills.list managed_mcps.list
    managed_skill.get managed_mcp.get hosts.list hosts.get integration_policies.list
    integration_policies.get integrations.list integrations.get data_workspace_previews.list
    data_workspace_previews.get data_workspaces.list data_workspaces.get
""".split():
    READS[method.replace(".", "_")] = method

BRIDGE = """
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        const response = await fetch('/__fixture_read', {
          method: 'POST',
          headers: {'Content-Type': 'application/json'},
          body: JSON.stringify({command, args: args || {}}),
        });
        return response.json();
      },
    };
"""


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def audit_checklist(browser, path, output):
    page = browser.new_page(viewport=VIEWPORT)
    rows = []
    try:
        page.goto(path.resolve().as_uri())
        page.locator("#show-list").click()
        expect(page.get_by_role("heading", name="问题清单")).to_be_visible()
        entries = page.locator("#findings button[data-case]")
        expect(entries).to_have_count(41)
        ids = entries.evaluate_all("entries => entries.map(entry => entry.dataset.case)")
        page.screenshot(path=output / "audit-checklist.png", animations="disabled")
        for scene in ids:
            page.locator(f'#findings button[data-case="{scene}"]').click()
            expect(page.locator("#cut-copy")).to_be_visible()
            expect(page.locator("#default-copy")).to_be_visible()
            expect(page.locator("#advanced-copy")).to_be_hidden()
            page.locator("#advanced-layer summary").press("Enter")
            expect(page.locator("#advanced-copy")).to_be_visible()
            row = {
                "id": scene,
                "title": page.locator("#scene-title").inner_text(),
                "priority": page.locator("#severity").get_attribute("class").split()[-1],
                "删掉或合并": page.locator("#cut-copy").inner_text(),
                "默认首屏": page.locator("#default-copy").inner_text(),
                "高级与详细内容": page.locator("#advanced-copy").inner_text(),
            }
            assert all(row.values()), row
            rows.append(row)
            page.locator("#show-list").click()
        counts = Counter(row["priority"] for row in rows)
        assert counts == {"P1": 29, "P2": 11, "P3": 1}, counts
        write_json(output / "audit-checklist.json", {"counts": counts, "rows": rows})
    finally:
        page.close()


def ipc(runtime, method, params=None):
    request = {
        "api_version": 1,
        "request_id": uuid.uuid4().hex,
        "token": (runtime / "session.token").read_text().strip(),
        "method": method,
        "params": params or {},
    }
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(8)
        stream.connect(str(runtime / "batond.sock"))
        stream.sendall(json.dumps(request).encode() + b"\n")
        with stream.makefile("rb") as reader:
            data = reader.readline(8_388_609)
    if not data.endswith(b"\n") or len(data) > 8_388_608:
        raise RuntimeError(f"Incomplete or oversized IPC response for {method}")
    return json.loads(data)


def result(runtime, method, params=None):
    response = ipc(runtime, method, params)
    if "error" in response:
        raise RuntimeError(f"{method}: {response['error']}")
    return response["result"]


@contextmanager
def running_fixture(fixture, binary, manifest, output):
    baseline = fixture / "baseline.sqlite3"
    with baseline.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    assert digest == manifest["baseline_sha256"], "Fixture baseline checksum differs from the manifest"
    # Linux and macOS Unix sockets have short path limits.
    runtime_root = Path(os.environ["BATON_RUNTIME_DIR"]).resolve()
    with tempfile.TemporaryDirectory(prefix="ui-", dir=runtime_root) as scratch:
        runtime = Path(scratch)
        shutil.copyfile(baseline, runtime / "baton.sqlite3")
        (runtime / "baton.sqlite3").chmod(0o600)
        env = {**os.environ, "BATON_RUNTIME_DIR": str(runtime)}
        with (output / "daemon.log").open("wb") as log:
            proc = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
            try:
                until = time.monotonic() + 12
                while time.monotonic() < until:
                    if proc.poll() is not None:
                        raise RuntimeError("Fixture daemon exited; see daemon.log")
                    try:
                        snapshot = result(runtime, "system.snapshot")
                        break
                    except OSError:
                        time.sleep(0.04)
                else:
                    raise TimeoutError("Fixture daemon did not become ready")
                assert snapshot["database_schema_version"] == 19, snapshot["database_schema_version"]
                assert len(snapshot["tasks"]) == 41, len(snapshot["tasks"])
                yield runtime, snapshot
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=12)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                proc.wait(timeout=5)


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def list_directory(self, _path):
        self.send_error(404)


@contextmanager
def serve(directory):
    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(QuietHandler, directory=str(directory)))
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def install(context, url, runtime, reads, disconnected, language):
    context.add_init_script(f"localStorage.setItem('agentbaton.language',{json.dumps(language)});" + BRIDGE)

    def route(request):
        if request.request.url == url + "/__fixture_read":
            body = request.request.post_data_json
            command, args = body["command"], body["args"]
            if command == "client_recovery_list":
                # The browser has no native marker directory.
                response = {"result": {"entries": []}, "warnings": []}
            elif command not in READS:
                response = {"error": {"code": "CAPABILITY_UNSUPPORTED", "message": "需要原生桌面接口",
                                      "retryable": False, "side_effect_state": "none"}, "warnings": []}
            elif disconnected["value"] and command == "system_snapshot":
                response = {"error": {"code": "DAEMON_UNAVAILABLE", "message": "隔离验证连接已断开",
                                      "retryable": True, "side_effect_state": "none"}, "warnings": []}
            else:
                if list(args) in (["selection"], ["request"], ["query"]):
                    params = next(iter(args.values()))
                else:
                    params = {re.sub(r"[A-Z]", lambda char: "_" + char[0].lower(), key): value
                              for key, value in args.items()}
                response = ipc(runtime, READS[command], params)
            reads.add((command, response.get("error", {}).get("code", "")))
            request.fulfill(status=200, json=response)
        elif request.request.url.startswith(url + "/"):
            request.continue_()
        else:
            request.abort()

    context.route("**/*", route)


def task_history(runtime, manifest, sequence):
    task_id = manifest["tasks"]["refund"]["id"]
    session_id = manifest["sessions"]["refund"]
    full = result(runtime, "tasks.history", {"task_id": task_id, "limit": 100})
    assert full["task_id"] == task_id and full["next_cursor"] is None, full
    events = full["events"]
    assert len(events) >= 2 and max(event["sequence"] for event in events) < sequence - 100
    assert {event["aggregate_id"] for event in events} == {session_id}
    assert {event["kind"] for event in events} == {"session.message_queued", "session.update"}
    before = []
    position = {}
    for _ in range(len(events)):
        page = result(runtime, "tasks.history", {"task_id": task_id, "limit": 1, **position})
        assert len(page["events"]) == 1
        before[:0] = page["events"]
        if page["next_cursor"] is None:
            break
        position = {"before": page["next_cursor"]}
    assert before == events, "Backward pagination skipped or duplicated task messages"
    after = result(runtime, "tasks.history", {"task_id": task_id, "after": events[0]["sequence"], "limit": 100})
    assert after["events"] == events[1:], "Forward cursor did not return the remaining task messages"
    global_window = result(runtime, "events.subscribe", {"cursor": sequence - 100, "limit": 100})
    assert not any(event["aggregate_id"] == session_id for event in global_window["events"])
    return {"task": "refund", "messages": len(events), "sequences": [event["sequence"] for event in events],
            "global_window_after": sequence - 100}


class Capture:
    def __init__(self, browser, url, runtime, manifest, output, stage):
        self.browser = browser
        self.url = url
        self.runtime = runtime
        self.manifest = manifest
        self.output = output
        self.stage = stage
        self.reads = set()
        self.offline = {"value": False}
        self.failures = []
        self.page_errors = []
        self.cases = {}

    def init(self, viewport=VIEWPORT, language="zh", *, offline=False):
        self.offline["value"] = offline
        context = self.browser.new_context(viewport=viewport, locale="zh-CN" if language == "zh" else "en-US")
        install(context, self.url, self.runtime, self.reads, self.offline, language)
        self.context = context
        self.page = self.context.new_page()
        self.page.on("pageerror", lambda error: self.page_errors.append(str(error)))
        self.page.set_default_timeout(4500)
        self.page.goto(self.url)
        if not offline:
            name = "本地后台已连接" if language == "zh" else "Local daemon connected"
            expect(self.page.get_by_text(name, exact=True).first).to_be_visible()
            expect(self.page.get_by_test_id("task-card")).to_have_count(41)

    def button(self, name, root=None):
        return (root or self.page).get_by_role("button", name=name, exact=True)

    def shot(self, name):
        image = self.output / f"{name}.png"
        self.page.screenshot(path=image, animations="disabled")
        self.cases[name] = {"file": image.name, "viewport": self.page.viewport_size,
                            "sha256": hashlib.sha256(image.read_bytes()).hexdigest()}
        print(self.stage, name, flush=True)

    def task(self, name, tab=None):
        wanted = self.manifest["tasks"][name]
        self.page.locator(f'.task-card[data-task-id="{wanted["id"]}"]').click()
        thread = self.page.get_by_test_id("task-thread")
        expect(thread.get_by_role("heading", name=wanted["goal"], exact=True)).to_be_visible()
        if tab:
            self.page.locator(f"#tab-{tab}").click()
        self.page.locator(".page-scroll").evaluate("el => el.scrollTop = 0")
        return thread

    def grid(self, expected_columns):
        cards = self.page.get_by_test_id("task-card")
        grid = self.page.locator(".task-grid")
        columns = grid.evaluate("el => getComputedStyle(el).gridTemplateColumns.split(' ').length")
        assert columns == expected_columns, (self.page.viewport_size, columns)
        expect(cards.first).to_be_visible()
        assert self.page.evaluate("document.documentElement.scrollWidth <= innerWidth")
        assert self.page.locator(".page-scroll").evaluate("el => el.scrollWidth <= el.clientWidth + 1")

    def home(self):
        self.grid(2)
        if self.stage == "after":
            expect(self.page.locator(".task-card").get_by_text("已记录目标", exact=True)).to_have_count(0)
            expect(self.page.locator(".task-card").get_by_text("尚未派发", exact=True)).to_have_count(0)
        else:
            assert self.page.locator(".task-card").get_by_text("已记录目标", exact=True).count() > 0
        self.shot("home")

    def compact(self, language):
        self.grid(2)
        card = self.page.locator(f'.task-card[data-task-id="{self.manifest["tasks"]["long"]["id"]}"]')
        card.scroll_into_view_if_needed()
        assert "上海海棠" in card.inner_text() and "Shanghai Hawthorn" in card.inner_text()
        assert card.evaluate("el => el.scrollWidth <= el.clientWidth + 1")
        self.shot("compact-" + language)
        self.task("long")
        assert self.page.locator(".page-scroll").evaluate("el => el.scrollWidth <= el.clientWidth + 1")
        self.shot("long-title-" + language)

    def narrow(self):
        self.grid(1)
        self.shot("narrow")

    def empty(self):
        self.button("KiteOps · 运营支持交接").first.click()
        expect(self.page.get_by_test_id("task-card")).to_have_count(0)
        if self.stage == "after":
            expect(self.page.get_by_text("这个项目还没有任务。", exact=True)).to_be_visible()
            expect(self.page.get_by_test_id("goal-input")).to_be_visible()
        self.shot("empty")

    def draft(self):
        thread = self.task("slow")
        configure = self.button("配置执行", thread)
        if self.stage == "after":
            expect(configure).to_have_count(1)
            expect(self.page.get_by_test_id("goal-input")).to_have_count(0)
            goal = self.manifest["tasks"]["slow"]["goal"]
            expect(thread.get_by_text(goal, exact=True)).to_have_count(1)
        self.shot("draft")
        configure.first.click()
        box = self.page.get_by_role("dialog")
        expect(box).to_be_visible()
        if self.stage == "after":
            folded = box.locator('[data-testid="session-scope-review"]')
            expect(folded.locator("summary")).to_be_visible()
            assert folded.evaluate("el => !el.open")
            expect(box.get_by_text("可写工作区", exact=True)).to_be_hidden()
        self.shot("execution")
        if self.stage == "after":
            folded.locator("summary").press("Enter")
            assert folded.evaluate("el => el.open")
            expect(box.get_by_text("可写工作区", exact=True)).to_be_visible()
            expect(box.get_by_text("数据接收方", exact=True)).to_be_visible()
            self.shot("execution-scope")

    def task_thread(self):
        thread = self.task("refund")
        historic = thread.get_by_text("先从哪一段链路排查？需要避免用真实交易身份测试。", exact=True)
        if self.stage == "after":
            expect(historic).to_be_visible()
            expect(thread.get_by_text(re.compile("我先对照 src/reconcile.py"))).to_be_visible()
            expect(thread.get_by_text(re.compile("evidence-35342c77"))).to_be_hidden()
        else:
            expect(historic).to_be_hidden()
        self.shot("task-thread")

    def unknown(self):
        thread = self.task("restart")
        if self.stage == "after":
            expect(thread.locator(".thread-heading").get_by_text("结果未知", exact=True)).to_be_visible()
            expect(thread.get_by_text("我会模拟此次等待中的 daemon 重启。", exact=True)).to_be_visible()
            expect(self.page.get_by_test_id("goal-input")).to_have_count(0)
            recovery = self.button("核对会话结果", thread)
            expect(recovery).to_be_enabled()
        self.shot("unknown")
        if self.stage == "after":
            recovery.press("Enter")
            expect(thread.locator(".observable-events")).to_have_attribute("open", "")

    def review(self):
        thread = self.task("latency")
        if self.stage == "after":
            expect(thread.locator(".thread-heading").get_by_text("待验收", exact=True)).to_be_visible()
            expect(thread.get_by_text(re.compile("候选结果.*验收|执行授权已过期。仍可查看候选产物。"))).to_be_visible()
            expect(thread.get_by_text("reports/portal-latency.md", exact=True)).to_be_visible()
            button = self.button("审阅结果", thread)
        else:
            expect(thread.locator(".thread-heading").get_by_text("运行中", exact=True)).to_be_visible()
            button = self.button("结果与验收", thread)
        self.shot("goal-waiting")
        button.click()
        graph = self.page.get_by_test_id("goal-graph-panel").first
        expect(graph).to_be_visible()
        history = graph.get_by_test_id("goal-graph-history")
        if self.stage == "before" and not history.count():
            graph.get_by_test_id("goal-graph-inspect").first.click()
        expect(history).to_be_visible()
        history.scroll_into_view_if_needed()
        if self.stage == "after":
            expect(history.get_by_test_id("goal-graph-current-artifacts")).to_be_visible()
            expect(history.get_by_test_id("goal-graph-evidence")).not_to_have_attribute("open", "")
            expect(graph.get_by_test_id("goal-graph-owner-approve").first).to_be_visible()
        self.shot("goal-review")

    def blocked(self):
        thread = self.task("attribution")
        if self.stage == "after":
            expect(thread.locator(".thread-heading").get_by_text("已阻塞", exact=True)).to_be_visible()
            expect(self.button("查看证据", thread)).to_be_visible()
        self.shot("goal-blocked")

    def artifact(self, name):
        thread = self.task(name, "artifacts")
        failure = name == "month-end"
        state = "文件完整性检查失败" if failure else "文件完整性检查通过"
        path = "reports/month-end-reconciliation.md" if failure else "reports/refund-idempotency.md"
        expect(thread.get_by_text(state, exact=False).filter(visible=True).first).to_be_visible()
        if self.stage == "after":
            expect(thread.get_by_text(path, exact=True).filter(visible=True).first).to_be_visible()
            expect(thread.get_by_text("文件完整性检查，不代表代码正确、任务验收或合入授权。", exact=True)).to_be_hidden()
            disclosure = thread.locator("summary").get_by_text("高级验证", exact=True)
            expect(disclosure).to_be_visible()
            assert disclosure.evaluate("el => !el.closest('details').open")
            if failure:
                expect(thread.get_by_text(re.compile("哈希不匹配")).filter(visible=True).first).to_be_visible()
                expect(thread.get_by_text("281f227c6cc04a5a059b1afc973d5c99dcb8bf50c075d98af5c2c4b7079e53a2", exact=True).filter(visible=True)).to_have_count(0)
        self.shot("artifacts-fail" if failure else "artifacts-pass")
        if not failure:
            return
        evidence = thread.locator(".integrity-record > summary").filter(visible=True).first
        evidence.press("Enter")
        expect(thread.get_by_text(path, exact=True).filter(visible=True).first).to_be_visible()
        if self.stage == "after":
            digest = thread.get_by_text("30063a18a44b080cdb4496990599bcfcb002f5e7f7d9521d9360e1f5da7233d3", exact=True)
            expect(digest.filter(visible=True).first).to_be_visible()
        self.shot("validation-failure")
        if self.stage == "after":
            disclosure.press("Space")
            assert disclosure.evaluate("el => el.closest('details').open")
            disclosure.press("Enter")
            assert disclosure.evaluate("el => !el.closest('details').open")

    def standalone(self):
        self.button("快捷对话").click()
        self.shot("quick-chat")
        self.page.get_by_role("button", name=re.compile("^(新建)?独立( Agent )?对话$")).first.click()
        box = self.page.get_by_role("dialog")
        expect(box).to_be_visible()
        self.shot("standalone")
        choice = box.get_by_role("combobox", name="选定对话")
        if choice.count():
            choice.select_option(self.manifest["conversations"]["thread-7"])
        else:
            box.get_by_role("button", name=re.compile("KiteOps 值班交接｜发货 QA 完整复盘")).click()
        earlier = self.button("读取较早历史", box)
        expect(earlier).to_be_visible()
        first = box.get_by_text(re.compile("记录验证项 01｜SEA 本地截点前一分钟")).filter(visible=True)
        latest = box.get_by_text(re.compile("记录验证项 43")).filter(visible=True)
        expect(first).to_be_hidden()
        if self.stage == "after":
            expect(box.get_by_role("checkbox").filter(visible=True)).to_have_count(0)
        self.shot("long-history")
        earlier.press("Enter")
        expect(first).to_be_visible()
        assert latest.count() > 0, "Older page displaced the latest turn unexpectedly"
        self.shot("long-history-earlier")

    def disconnected(self):
        status = "后台未连接" if self.stage == "after" else "本地后台不可用"
        self.offline["value"] = True
        self.button("刷新").first.click()
        expect(self.page.locator(".appbar").get_by_text(status, exact=True)).to_be_visible()
        retry = self.button("重试连接")
        expect(retry).to_be_enabled()
        if self.stage == "after":
            expect(self.page.get_by_text("显示上次读取的任务。", exact=True)).to_be_visible()
            expect(self.page.locator(".disconnected-panel details")).not_to_have_attribute("open", "")
            expect(self.page.get_by_text("隔离验证连接已断开", exact=True)).to_be_hidden()
        self.shot("disconnected")
        self.offline["value"] = False
        retry.press("Enter")
        expect(self.page.get_by_text("本地后台已连接", exact=True).first).to_be_visible()
        expect(self.page.get_by_test_id("task-card")).to_have_count(41)

    def cold_offline(self):
        label = "后台未连接" if self.stage == "after" else "本地后台不可用"
        expect(self.page.locator(".appbar").get_by_text(label, exact=True)).to_be_visible()
        if self.stage == "after":
            expect(self.page.locator(".canvas-empty")).to_have_count(0)
            expect(self.page.get_by_test_id("goal-input")).to_have_count(0)
        self.shot("cold-disconnected")

    def capabilities(self):
        self.button("能力").click()
        expect(self.page.get_by_text(re.compile("Legacy ACP")).filter(visible=True).first).to_be_visible()
        self.shot("capabilities")
        if self.stage == "after":
            self.button("Skill").click()
            self.shot("skills")
            self.button("MCP").click()
            self.shot("mcp")

    def settings(self):
        self.button("设置").click()
        dialog = self.page.get_by_role("dialog")
        expect(dialog).to_be_visible()
        if self.stage == "after":
            expect(self.button("常规", dialog)).to_have_attribute("aria-pressed", "true")
            expect(dialog.get_by_text("语言", exact=True)).to_be_visible()
        self.shot("settings")
        if self.stage == "after":
            self.button("主机", dialog).click()
            self.shot("hosts")

    def projects(self):
        self.button("项目与工作区").first.click()
        dialog = self.page.get_by_role("dialog")
        expect(dialog).to_be_visible()
        if self.stage == "after":
            expect(dialog.get_by_text("项目目录", exact=True)).to_be_hidden()
        self.shot("projects")

    def scenario(self, name, action, **kwargs):
        try:
            self.init(**kwargs)
            action()
        except Exception as error:
            self.failures.append({"case": name, "error": str(error)})
            if hasattr(self, "page"):
                self.page.screenshot(path=self.output / f"failure-{name}.png", animations="disabled")
            print(f"{self.stage} {name}: {str(error).splitlines()[0]}", flush=True)
        finally:
            if hasattr(self, "context"):
                self.context.close()

    def run(self):
        for name, action in (
            ("home", self.home), ("empty", self.empty), ("draft", self.draft),
            ("task-thread", self.task_thread), ("unknown", self.unknown), ("review", self.review),
            ("blocked", self.blocked), ("artifacts-pass", lambda: self.artifact("refund")),
            ("artifacts-fail", lambda: self.artifact("month-end")), ("standalone", self.standalone),
            ("disconnect", self.disconnected), ("capabilities", self.capabilities),
            ("settings", self.settings), ("projects", self.projects),
        ):
            self.scenario(name, action)
        for lang in ("zh", "en"):
            self.scenario(f"compact-{lang}", lambda: self.compact(lang), viewport={"width": 900, "height": 700}, language=lang)
        self.scenario("narrow", self.narrow, viewport={"width": 390, "height": 780})
        self.scenario("cold-disconnected", self.cold_offline, offline=True)
        return {"shots": self.cases, "failures": self.failures, "page_errors": self.page_errors,
                "reads": [{"command": name, "code": code} for name, code in sorted(self.reads)]}


COMPARISONS = {
    "home": ("首页", "两列任务卡片保留标题和状态，删去每卡重复的目标记录与未派发文案。"),
    "compact-zh": ("900 像素窗口", "侧栏收起后仍为两列，快捷键提示移入对应按钮。"),
    "long-title-en": ("中英文混排长标题", "完整目标在详情可读，长标题不撑宽窗口。"),
    "narrow": ("390 像素窗口", "窄窗口切换单列，任务入口和状态可见。"),
    "empty": ("空项目", "首屏解释当前项目没有任务，创建入口保留在输入框。"),
    "draft": ("任务草稿", "目标只出现一次，使用配置执行进入下一步。"),
    "execution": ("执行配置", "先处理工作区与 Agent，权限审阅和高级参数可按需展开。"),
    "execution-scope": ("执行范围", "使用 Enter 展开审阅区可看到接收方和可写工作区。"),
    "task-thread": ("已存任务对话", "任务已恢复早于全局最近 100 条事件的消息。"),
    "unknown": ("结果未知", "保留最后一条消息，使用核对会话结果查看证据。"),
    "goal-waiting": ("待验收", "首屏显示待验收阶段和审阅结果操作。"),
    "goal-review": ("验收结果", "首屏展示候选产物及审阅入口，完整运行证据按需展开。"),
    "goal-blocked": ("阻塞", "查看证据可进入受阻的执行记录。"),
    "artifacts-pass": ("产物完整性通过", "先看最近检查状态与对应文件，检查策略和表单默认收起。"),
    "artifacts-fail": ("产物校验失败", "首屏展示失败文件、哈希不匹配和重查入口。"),
    "validation-failure": ("失败证据", "用键盘展开证据可读取完整的观测哈希。"),
    "long-history": ("独立对话", "阅读区隐藏摘录复选框，保留读取较早历史。"),
    "long-history-earlier": ("较早的对话页", "加载更早内容后，当前可见的消息仍然保留。"),
    "disconnected": ("已载入数据后断连", "标记上次读到的列表，重试连接后恢复显示。"),
    "cold-disconnected": ("启动时断连", "首屏显示重试，错误详情折叠，避免误报空项目。"),
    "projects": ("项目", "先列出现有项目和状态，导入时再显示表单。"),
    "capabilities": ("能力", "连接、Skill 和 MCP 各有入口，页首只看选择的类别。"),
    "settings": ("设置", "常规设置先显示语言、外观和后台状态。"),
}


def write_comparison(output, report):
    datasets = {}
    for stage in ("before", "after"):
        if stage in report["verification"]:
            datasets[stage] = report["verification"][stage]["shots"]
        elif (output / f"verification-{stage}.json").is_file():
            data = json.loads((output / f"verification-{stage}.json").read_text())
            if data["fixture"] == report["fixture"]:
                datasets[stage] = data["verification"][stage]["shots"]
    if set(datasets) != {"before", "after"}:
        return
    sections = []
    for name, (title, note) in COMPARISONS.items():
        figures = []
        for stage, caption in (("before", "改造前"), ("after", "改造后")):
            if name in datasets[stage]:
                shot = datasets[stage][name]
                original = f"{stage}/{shot['file']}"
                embedded = b64encode((output / original).read_bytes()).decode("ascii")
                figures.append(f'<figure><figcaption>{caption}</figcaption><a href="{original}">'
                               f'<img src="data:image/png;base64,{embedded}" width="{shot["viewport"]["width"]}" '
                               f'height="{shot["viewport"]["height"]}" alt="{escape(title)} · {caption}" '
                               f'loading="lazy"></a></figure>')
        if figures:
            sections.append(f'<section id="{name}"><h2>{escape(title)}</h2><p>{escape(note)}</p>'
                            f'<div class="pair">{"".join(figures)}</div></section>')
    checks = "<li>".join(f'<a href="#{name}">{escape(title)}</a>' for name, (title, _) in COMPARISONS.items()
                          if any(name in stage for stage in datasets.values()))
    html = """<!doctype html>
<html lang="zh-CN">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>AgentBaton 前端精简 · 截图对照</title>
<style>
body{max-width:1180px;margin:0 auto;padding:24px;font:16px/1.6 system-ui;color:#23272e;background:#f7f8fa}
h1,h2,p,figure{margin:0 0 10px} nav ul{columns:3;margin-bottom:30px}
section{border-top:1px solid #d7dbe0;padding:22px 0}
.pair{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:18px}
figcaption{font-weight:600} img{display:block;width:100%;height:auto;border:1px solid #d7dbe0}
@media(max-width:760px){.pair{grid-template-columns:1fr}nav ul{columns:2}}
</style>
<h1>AgentBaton 前端精简 · 截图对照</h1>
<p>在 Apple Silicon 上使用 Chrome 渲染两次实际前端构建，通过只读 IPC 连接隔离的拟真 SQLite 19 数据库。截图已内嵌，点击可打开原图。</p>
"""
    (output / "comparison.html").write_text(html + f'<nav aria-label="场景"><ul><li>{checks}</ul></nav>' + "".join(sections))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--browser", type=Path)
    parser.add_argument("--audit", type=Path)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/batond")
    parser.add_argument("--before", type=Path, default=ROOT / "artifacts/ui-simplification/before-dist")
    parser.add_argument("--after", type=Path, default=ROOT / "apps/desktop/dist")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/ui-simplification/screenshots")
    parser.add_argument("--stage", choices=("before", "after", "both"), default="both")
    parser.add_argument("--audit-only", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path=args.browser)
        if args.audit:
            audit_checklist(browser, args.audit, args.output)
        if args.audit_only:
            return
        fixture = args.fixture.resolve(strict=True)
        manifest = json.loads((fixture / "manifest.json").read_text())
        report = {"browser": browser.version, "fixture": manifest["baseline_sha256"],
                  "daemon_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(), "verification": {}}
        with running_fixture(fixture, args.binary.resolve(strict=True), manifest, args.output) as (runtime, snapshot):
            report["schema"] = snapshot["database_schema_version"]
            report["sequence"] = snapshot["sequence"]
            report["task_history"] = task_history(runtime, manifest, snapshot["sequence"])
            stages = ("before", "after") if args.stage == "both" else (args.stage,)
            for stage in stages:
                dist = getattr(args, stage).resolve(strict=True)
                directory = args.output / stage
                directory.mkdir(exist_ok=True)
                frontend = {}
                for path in sorted(dist.rglob("*")):
                    if path.is_file():
                        frontend[str(path.relative_to(dist))] = hashlib.sha256(path.read_bytes()).hexdigest()
                with serve(dist) as url:
                    captured = Capture(browser, url, runtime, manifest, directory, stage).run()
                report["verification"][stage] = {**captured, "frontend": frontend}
        write_json(args.output / f"verification-{args.stage}.json", report)
        write_comparison(args.output, report)
        failed = [case for stage in report["verification"].values() for case in stage["failures"]]
        errors = [error for stage in report["verification"].values() for error in stage["page_errors"]]
        if failed or errors:
            raise SystemExit(f"Failed browser checks: {len(failed)}; uncaught page errors: {len(errors)}")


if __name__ == "__main__":
    main()
