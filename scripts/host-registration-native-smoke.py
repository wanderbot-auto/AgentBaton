#!/usr/bin/env python3
"""Verify Host registration using an isolated .app or a private production socket."""

import argparse
from contextlib import closing, contextmanager
from hashlib import sha256
import json
import os
from pathlib import Path
import plistlib
import re
import signal
import socket
import sqlite3
import stat
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXED_SOURCE = "cde501db847d1bbfc27a298e16cd772afc1a41e9"
TASK = "host-registration-native-e2e"
MAX_FRAME = 512 * 1024
HOST_NAME = "Synthetic Host E2E"
PAYLOAD = {
    "schema_version": 1, "name": HOST_NAME, "declared_platform": "linux",
    "endpoint": {
        "expected_host_identity": "synthetic-protocol-host",
        "address": "host.example.invalid", "user": "fixture", "port": 2222,
        "host_key_alias": "synthetic-alias",
    },
    "known_hosts": {"reference": "fixture:known-hosts", "file_sha256": "a" * 64},
    "transport_identity_ref": "fixture:transport",
    "configuration": {"reference": "fixture:configuration", "declared_ownership": "external_read_only"},
    "directory_mappings": [],
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def git(*args):
    return subprocess.check_output(("git", "-C", ROOT, *args), text=True).strip()


def private_directory(path):
    metadata = path.lstat()
    require(path == path.resolve(strict=True) and stat.S_ISDIR(metadata.st_mode) and
            stat.S_IMODE(metadata.st_mode) == 0o700 and metadata.st_uid == os.geteuid(),
            "Expected an owned 0700 canonical directory")


def task_runtime():
    base = Path(os.environ["BATON_RUNTIME_DIR"])
    state = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir")) / "agentbaton-collab"
    require((state / "active" / TASK).is_file(), "Expected the active native test claim")
    require((state / f"runtime-{TASK}").read_text().strip() == str(base),
            "Expected this task's collab runtime")
    private_directory(base)
    require(re.fullmatch(r"/private/tmp/ab-collab-[A-Za-z0-9_-]+", str(base)) is not None,
            "Expected the short private collab runtime")
    return base


def file_evidence(path):
    with path.open("rb") as stream:
        return {"path": str(path), "bytes": path.stat().st_size,
                "sha256": sha256_stream(stream)}


def sha256_stream(stream):
    digest = sha256()
    for data in iter(lambda: stream.read(64 * 1024), b""):
        digest.update(data)
    return digest.hexdigest()


def bundle_artifacts(bundle, for_app):
    expected = ROOT / "target/release/bundle/macos"
    require(bundle == bundle.resolve(strict=True) and bundle.parent == expected and bundle.suffix == ".app",
            "Expected an absolute .app from this worktree's release bundle directory")
    metadata = bundle / "Contents/Info.plist"
    with metadata.open("rb") as stream:
        info = plistlib.load(stream)
    if for_app:
        require(re.fullmatch(r"dev\.agentbaton\.hoste2e-[a-f0-9]+", info["CFBundleIdentifier"]) is not None,
                "Refusing to launch a non-isolated bundle identifier")
    name = info["CFBundleExecutable"]
    require(Path(name).name == name and name == "agentbaton-desktop", "Unexpected main executable name")
    paths = {key: bundle / "Contents/MacOS" / value
             for key, value in (("desktop", name), ("daemon", "batond"), ("cli", "baton"))}
    all_evidence = {"info_plist": file_evidence(metadata), "identifier": info["CFBundleIdentifier"]}
    for key, path in paths.items():
        require(path == path.resolve(strict=True) and path.is_file(), f"Invalid bundled {key}")
        arch = subprocess.check_output(("/usr/bin/lipo", "-archs", path), text=True).strip()
        require(arch == "arm64", f"Unexpected bundled {key} architecture")
        all_evidence[key] = {**file_evidence(path), "architecture": arch}
        if key in ("daemon", "cli"):
            expected_binary = ROOT / "target/release" / path.name
            require(all_evidence[key]["sha256"] == file_evidence(expected_binary)["sha256"],
                    f"Bundled {key} differs from the pinned release build")
    return paths, all_evidence


def eventually(description, probe, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = probe()
        if result is not None:
            return result
        time.sleep(0.1)
    raise RuntimeError(f"Timed out waiting for {description}")


def authenticate(runtime):
    file = runtime / "session.token"
    metadata = file.lstat()
    require(stat.S_ISREG(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) == 0o600 and
            metadata.st_uid == os.geteuid() and metadata.st_nlink == 1, "Unsafe private test token")
    token = file.read_text()
    require(1 <= len(token) <= 256, "Invalid private test token")
    return token


@contextmanager
def request_stream(runtime, method, params, *, key=None):
    request_id = str(uuid.uuid4())
    request = {"api_version": 1, "request_id": request_id, "token": authenticate(runtime),
               "method": method, "params": params}
    if key is not None:
        request["idempotency_key"] = key
    wire = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    endpoint = runtime / "batond.sock"
    metadata = endpoint.lstat()
    require(stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == os.geteuid() and
            stat.S_IMODE(metadata.st_mode) == 0o600, "Unsafe private test socket")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(5)
        stream.connect(str(endpoint))
        stream.sendall(wire)
        yield stream, request_id


def exchange(runtime, method, params, *, key=None):
    with request_stream(runtime, method, params, key=key) as (stream, request_id):
        with stream.makefile("rb") as reply_stream:
            line = reply_stream.readline(MAX_FRAME + 1)
    require(line.endswith(b"\n") and len(line) <= MAX_FRAME, "Invalid daemon frame")
    reply = json.loads(line)
    require(reply.get("request_id") == request_id, "Daemon response request ID mismatch")
    if "error" in reply:
        raise RuntimeError(f"{method} refused: {reply['error'].get('code')}")
    require("result" in reply, f"{method} has no result")
    return reply["result"]


@contextmanager
def process(binary, runtime, output, name, cleanup):
    with (output / f"{name}.log").open("xb") as log:
        environment = {**os.environ, "BATON_RUNTIME_DIR": str(runtime)}
        owned = subprocess.Popen([binary], env=environment, stdin=subprocess.DEVNULL,
                                 stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        action = "already_exited"
        try:
            yield owned
        finally:
            if owned.poll() is None:
                action = "SIGTERM"
                owned.terminate()
            try:
                exit_code = owned.wait(timeout=10)
            except subprocess.TimeoutExpired:
                action = "SIGKILL_after_timeout"
                owned.kill()
                exit_code = owned.wait(timeout=5)
            cleanup.append({"name": name, "pid": owned.pid, "exit": exit_code, "signal": action,
                            "joined": owned.poll() is not None})


def ready(runtime, daemon):
    def probe():
        require(daemon.poll() is None, "Owned daemon exited before readiness")
        try:
            if (runtime / "session.token").lstat().st_size == 0:
                return None
            return exchange(runtime, "system.background", {})
        except (FileNotFoundError, ConnectionRefusedError, socket.timeout):
            return None
    background = eventually("authenticated owned daemon", probe)
    require(background["database_schema_version"] == 19 and background["api_version"] == 1,
            "Daemon is not the current Host registry authority")
    require(background["owned_provider_workers"] is False, "Unexpected Provider workers")
    return background


def audit(runtime):
    path = runtime / "baton.sqlite3"
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.geteuid() and
            stat.S_IMODE(metadata.st_mode) == 0o600, "Unsafe private SQLite path")
    with closing(sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=3)) as db:
        db.execute("PRAGMA query_only=ON")
        db.execute("BEGIN")
        version, = db.execute("PRAGMA user_version").fetchone()
        rows = db.execute("SELECT id,revision,created_sequence,document FROM host_registrations ORDER BY created_sequence").fetchall()
        idempotency = db.execute("SELECT key,payload,result FROM idempotency WHERE scope='hosts.register:v1' ORDER BY key").fetchall()
        events = db.execute("SELECT sequence,document FROM events WHERE json_extract(document,'$.kind')='host.registered' ORDER BY sequence").fetchall()
        unchanged = {}
        for table in ("projects", "tasks", "sessions", "session_turns", "connections", "execution_grants"):
            values = db.execute(f"SELECT * FROM {table} ORDER BY id").fetchall()
            unchanged[table] = {"count": len(values), "sha256": sha256(repr(values).encode()).hexdigest()}
    return {"schema": version, "hosts": [[a, b, c, json.loads(d)] for a, b, c, d in rows],
            "idempotency": [[key, json.loads(payload), json.loads(result)] for key, payload, result in idempotency],
            "events": [[sequence, json.loads(document)] for sequence, document in events], "other": unchanged}


def empty(audited):
    require(audited["schema"] == 19 and all(audited[k] == [] for k in ("hosts", "idempotency", "events")),
            "Expected an empty Host registry in a fresh schema19 runtime")


def confirmed(before, after, key, receipt, payload):
    require(after["schema"] == 19 and after["other"] == before["other"], "Other fixture records changed")
    require(all(len(after[field]) == 1 for field in ("hosts", "idempotency", "events")),
            "Expected exactly one Host row, receipt and creation event")
    host_id, revision, sequence, record = after["hosts"][0]
    require(revision == receipt["receipt_version"] == receipt["revision"] == 1 and
            receipt["host_id"] == record["id"] == host_id and
            sequence == receipt["created_sequence"] == record["created_sequence"] and
            receipt["created_at"] == record["created_at"] and record["creation_key"] == key and
            receipt["registration"] == record["registration"] == payload, "Host receipt/row mismatch")
    facts = record["creation_facts"]
    require(facts["remote_connection"] == "not_contacted" and facts["remote_probe"] == "not_probed" and
            facts["execution_enabled"] is False, "Unexpected Host contact or authority")
    event_sequence, event = after["events"][0]
    registration_sha = sha256(json.dumps(payload, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    require(event_sequence == sequence and event["aggregate_id"] == host_id and
            event["payload"]["creation_key"] == key and event["payload"]["host_id"] == host_id and
            event["payload"]["registration_sha256"] == registration_sha and
            event["source"] == "user_action", "Host event does not bind the original key, metadata and receipt")
    require(after["idempotency"][0] == [key, payload, receipt], "Stored original key or immutable receipt differs")


def pending_entries(runtime):
    marker = runtime / "host-registration-recovery.json"
    try:
        meta = marker.lstat()
    except FileNotFoundError:
        return None
    require(stat.S_ISREG(meta.st_mode) and meta.st_uid == os.geteuid() and
            stat.S_IMODE(meta.st_mode) == 0o600 and meta.st_nlink == 1, "Unsafe Host recovery journal")
    journal = json.loads(marker.read_bytes())
    require(set(journal) == {"version", "entries"} and journal["version"] == 1 and
            isinstance(journal["entries"], list) and len(journal["entries"]) <= 1, "Invalid Host journal")
    return journal["entries"]


def socket_case(binary, base, output, evidence):
    evidence["socket"] = item = {"status": "in_progress", "boundary": "real_unix_socket_reply_not_read"}
    with tempfile.TemporaryDirectory(prefix="abhs-", dir=base) as isolated:
        runtime = Path(isolated)
        private_directory(runtime)
        key = str(uuid.uuid4())
        with process(binary, runtime, output, "daemon-socket", evidence["cleanup"]) as daemon:
            instance = ready(runtime, daemon)
            before = audit(runtime)
            empty(before)
            def committed_row():
                observed = audit(runtime)
                return observed if observed["idempotency"] else None
            # Preserve admission with a connected peer, but never consume the reply.
            # An independent read-only transaction proves commit before client close.
            with request_stream(runtime, "hosts.register", PAYLOAD, key=key):
                committed = eventually("committed response-less registration", committed_row)
                require(len(committed["idempotency"]) == 1, "Unexpected registration count at commit")
                committed_key, _, committed_receipt = committed["idempotency"][0]
                require(committed_key == key, "Unexpected original key at commit")
                confirmed(before, committed, key, committed_receipt, PAYLOAD)
            item["committed_sqlite_observed_before_client_close"] = True
            item["complete_request_sent_and_client_closed_without_reading"] = True

            def lookup():
                found = exchange(runtime, "hosts.find_registration", {"key": key})
                require(found["lookup_version"] == 1 and found["key"] == key, "Wrong recovered operation")
                return found["receipt"]
            receipt = lookup()
            require(receipt == committed_receipt, "Original receipt was unavailable after disconnect")
            require(exchange(runtime, "hosts.register", PAYLOAD, key=key) == receipt,
                    "Same-key replay returned a different receipt")
            require(audit(runtime) == committed, "Replay changed persisted state")
        require(not (runtime / "batond.sock").exists(), "First owned daemon socket still exists")
        with process(binary, runtime, output, "daemon-socket-reopened", evidence["cleanup"]) as daemon:
            reopened = ready(runtime, daemon)
            require(reopened["instance_id"] != instance["instance_id"], "Expected a new owned daemon instance")
            require(lookup() == receipt and audit(runtime) == committed, "Committed state changed after daemon reopen")
        require(not (runtime / "batond.sock").exists(), "Reopened daemon socket still exists")
        item.update(status="passed", original_key=key, receipt=receipt, before=before, after=committed,
                    discarded_bytes_read=0, replay_returned_same_receipt=True, daemon_reopened=True)


# External Accessibility driver. The only target is the exact executable/PID
# passed by process(); no production WebView hooks or global keyboard actions.
AX_SOURCE = r'''
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

func attribute(_ node: AXUIElement, _ name: String) -> AnyObject? {
    var result: AnyObject?
    guard AXUIElementCopyAttributeValue(node, name as CFString, &result) == .success else { return nil }
    return result
}

func string(_ node: AXUIElement, _ key: String) -> String {
    return attribute(node, key) as? String ?? ""
}

func actions(_ node: AXUIElement) -> [String] {
    var names: CFArray?
    AXUIElementCopyActionNames(node, &names)
    return names as? [String] ?? []
}

struct Element {
    let node: AXUIElement
    let role: String
    let title: String
    let label: String
    let value: String

    func matches(_ subject: String) -> Bool {
        return title == subject || label == subject || value == subject
    }
}

func elements(_ root: AXUIElement) -> [Element] {
    var found: [Element] = []
    func visit(_ node: AXUIElement, _ level: Int) {
        if level > 36 || found.count >= 1600 { return }
        found.append(Element(node: node, role: string(node, "AXRole"),
                             title: string(node, "AXTitle"), label: string(node, "AXDescription"),
                             value: string(node, "AXValue")))
        if let children = attribute(node, "AXChildren") as? [AXUIElement] {
            for child in children { visit(child, level + 1) }
        }
    }
    if let windows = attribute(root, "AXWindows") as? [AXUIElement] {
        for window in windows { visit(window, 0) }
    }
    return found
}

func key(_ code: CGKeyCode, pid: pid_t) {
    for down in [true, false] {
        guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
            fail("Cannot create an owned-app key event")
        }
        event.postToPid(pid)
    }
    usleep(55_000)
}

func textKey(_ text: String, pid: pid_t) {
    let units = Array(text.utf16)
    for down in [true, false] {
        guard let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down) else {
            fail("Cannot create owned text entry")
        }
        units.withUnsafeBufferPointer {
            event.keyboardSetUnicodeString(stringLength: units.count, unicodeString: $0.baseAddress)
        }
        event.postToPid(pid)
    }
    usleep(100_000)
}

let args = CommandLine.arguments
guard AXIsProcessTrusted() else { fail("Accessibility denied by macOS; no actions taken") }
guard args.count >= 4, let pid = Int32(args[1]),
      let app = NSRunningApplication(processIdentifier: pid),
      app.executableURL?.resolvingSymlinksInPath().path == args[2] else {
    fail("Expected the exact owned app PID and executable")
}
let root = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(root, 1.5)
let operation = args[3]
if operation == "activate" {
    _ = app.activate(options: [])
    for _ in 0..<15 where !app.isActive { usleep(50_000) }
    print("active=\(app.isActive) finishedLaunching=\(app.isFinishedLaunching)")
    exit(0)
}
let nodes = elements(root)

if operation == "dump" {
    let output = nodes.map { item -> [String: Any] in
        return ["role": item.role, "title": item.title, "label": item.label, "value": item.value,
                "actions": actions(item.node), "enabled": attribute(item.node, "AXEnabled") as? Bool ?? true]
    }
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: output))
    exit(0)
}

if operation == "window" {
    guard CGPreflightScreenCaptureAccess() else { fail("Window capture denied by macOS") }
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    let owned = (windows as? [[String: Any]] ?? []).filter {
        $0[kCGWindowOwnerPID as String] as? Int32 == pid && $0[kCGWindowLayer as String] as? Int == 0
    }
    guard owned.count == 1, let id = owned[0][kCGWindowNumber as String] as? Int else {
        fail("Expected one visible window belonging to the exact owned App")
    }
    print(id)
    exit(0)
}

guard args.count >= 5 else { fail("Missing control label") }
let label = args[4]
if operation == "disclose" {
    let targets = nodes.filter { $0.role == "AXDisclosureTriangle" && $0.matches(label) }
    guard targets.count == 1, let target = targets.first else { fail("Missing owned disclosure: " + label) }
    guard CGPreflightPostEventAccess() else { fail("Owned input denied by macOS") }
    guard AXUIElementPerformAction(target.node, "AXScrollToVisible" as CFString) == .success else {
        fail("Cannot scroll owned disclosure into view: " + label)
    }
    if !app.isActive {
        _ = app.activate(options: [])
        for _ in 0..<15 where !app.isActive { usleep(50_000) }
    }
    guard app.isActive else { fail("Owned App could not activate for input") }
    guard AXUIElementSetAttributeValue(target.node, "AXFocused" as CFString, kCFBooleanTrue) == .success else {
        fail("Cannot focus owned disclosure: " + label)
    }
    key(49, pid: pid)
    exit(0)
}
if operation == "press" {
    let buttons = nodes.filter {
        $0.role == "AXButton" && $0.matches(label) && attribute($0.node, "AXEnabled") as? Bool != false
    }
    guard buttons.count == 1, let button = buttons.first else { fail("Missing or ambiguous enabled button: " + label) }
    guard AXUIElementPerformAction(button.node, "AXPress" as CFString) == .success else {
        fail("AXPress refused: " + label)
    }
    exit(0)
}

guard args.count == 6 else { fail("Missing control value") }
let targetRoles = operation == "select" ? ["AXPopUpButton", "AXComboBox"] :
    ["AXTextField", "AXTextArea", "AXSpinButton", "AXIncrementor"]
let matches = nodes.filter { item in
    guard targetRoles.contains(item.role) else { return false }
    if item.matches(label) || string(item.node, "AXPlaceholderValue") == label { return true }
    if let labelNode = attribute(item.node, "AXTitleUIElement"), CFGetTypeID(labelNode) == AXUIElementGetTypeID() {
        let linked = labelNode as! AXUIElement
        return string(linked, "AXTitle") == label || string(linked, "AXValue") == label
    }
    return false
}
guard matches.count == 1, let item = matches.first else { fail("Missing or ambiguous editable control: " + label) }
guard attribute(item.node, "AXEnabled") as? Bool != false else { fail("Control disabled: " + label) }
if operation == "fill" {
    guard CGPreflightPostEventAccess() else { fail("Owned text input denied by macOS") }
    guard AXUIElementSetAttributeValue(item.node, "AXFocused" as CFString, kCFBooleanTrue) == .success else {
        fail("Cannot focus owned field: " + label)
    }
    usleep(100_000)
    guard attribute(item.node, "AXFocused") as? Bool == true else { fail("Owned field is not focused: " + label) }
    let previous = string(item.node, "AXValue")
    guard previous.count <= 8 else { fail("Unexpected prefilled value in owned field: " + label) }
    for _ in previous { key(124, pid: pid) } // Move right through the default value.
    for _ in previous { key(51, pid: pid) } // Backspace, including in numeric inputs.
    var remaining = args[5][...]
    while !remaining.isEmpty {
        textKey(String(remaining.prefix(8)), pid: pid)
        remaining = remaining.dropFirst(8)
    }
    exit(0)
}
guard operation == "select", CGPreflightPostEventAccess(), !args[5].isEmpty else {
    fail("Unexpected selection or input access denied")
}
guard AXUIElementSetAttributeValue(item.node, "AXFocused" as CFString, kCFBooleanTrue) == .success else {
    fail("Cannot focus owned selector: " + label)
}
// HTML select supports type-ahead while the native popup remains closed.
textKey(String(args[5].prefix(1)), pid: pid)
'''


class AX:
    def __init__(self, directory, output):
        self.last_dump = output / "last-owned-window.ax.json"
        source = directory / "owned-ax.swift"
        source.write_text(AX_SOURCE)
        self.binary = directory / "owned-ax"
        with (output / "ax-build.log").open("xb") as log:
            subprocess.run(("/usr/bin/swiftc", source, "-o", self.binary), stdout=log, stderr=subprocess.STDOUT,
                           timeout=45, check=True)

    def call(self, process, executable, operation, *arguments):
        require(process.poll() is None, "Owned App has exited")
        command = subprocess.run((self.binary, str(process.pid), executable, operation, *arguments),
                                 capture_output=True, text=True, timeout=8)
        require(command.returncode == 0, command.stderr.strip() or "Owned App AX action failed")
        return command.stdout

    def dump(self, process, executable):
        nodes = json.loads(self.call(process, executable, "dump"))
        if nodes or not self.last_dump.exists():
            self.last_dump.write_text(json.dumps(nodes, ensure_ascii=False, indent=2) + "\n")
        return nodes


def accessible_text(nodes):
    return "\n".join(str(node[key]) for node in nodes for key in ("title", "label", "value") if node[key])


def observe(ax, app, executable, text):
    def observed():
        nodes = ax.dump(app, executable)
        return nodes if all(value in accessible_text(nodes) for value in text) else None
    return eventually(f"owned WebView text {text!r}", observed, timeout=35)


def check_value(ax, app, executable, title, value):
    def matches():
        nodes = ax.dump(app, executable)
        return True if any(node["title"] == title and node["value"] == value for node in nodes) else None
    eventually(f"{title!r} to display the entered value", matches, timeout=4)


def frame(ax, app, executable, output, name):
    nodes = ax.dump(app, executable)
    (output / f"{name}.ax.json").write_text(json.dumps(nodes, ensure_ascii=False, indent=2) + "\n")
    window = ax.call(app, executable, "window").strip()
    require(window.isascii() and window.isdigit(), "Invalid owned window identifier")
    image = output / f"{name}.png"
    subprocess.run(("/usr/sbin/screencapture", "-x", "-o", "-l", window, image),
                   timeout=10, check=True)
    require(image.stat().st_size > 8 and image.read_bytes()[:8] == b"\x89PNG\r\n\x1a\n", "No owned window PNG")
    return {"window_id": int(window), **file_evidence(image)}


CHINESE = {
    "Settings": "设置", "Remote Hosts": "远程主机", "Close": "关闭",
    "No Hosts are registered.": "尚无已登记的主机。", "Register Host": "登记主机",
    "Declared platform": "声明的平台", "Declared configuration ownership": "声明的配置归属",
    "External read-only": "外部只读",
    "Host name": "主机名称", "Expected protocol Host ID": "预期协议主机 ID",
    "SSH endpoint address": "SSH 端点地址", "SSH user": "SSH 用户", "SSH port": "SSH 端口",
    "SSH Host key alias": "SSH 主机密钥别名", "known_hosts reference": "known_hosts 引用",
    "known_hosts file SHA-256": "known_hosts 文件 SHA-256", "Transport identity reference": "传输身份引用",
    "Configuration reference": "配置引用", "Acknowledge registration": "确认登记回执",
    "Registry Host UUID": "登记主机 UUID", "Registration acknowledged.": "登记回执已确认。",
    "View registration": "查看登记详情", "Remote Host has not been contacted or probed": "远程主机尚未联系或探测",
}


def activate(ax, app, executable):
    time.sleep(1)  # Allow LaunchServices to register the direct child before PID verification.
    return eventually("owned App to finish launching",
                      lambda: activation_state(ax, app, executable), timeout=20)


def activation_state(ax, app, executable):
    state = ax.call(app, executable, "activate").strip()
    return state if "finishedLaunching=true" in state else None


def open_settings(ax, app, executable):
    def language():
        contents = accessible_text(ax.dump(app, executable)).splitlines()
        return next((copy for copy in (CHINESE, {}) if copy.get("Settings", "Settings") in contents), None)

    copy = eventually("localized Settings button in the owned WebView", language, timeout=35)
    settings = copy.get("Settings", "Settings")
    hosts = copy.get("Remote Hosts", "Remote Hosts")
    ax.call(app, executable, "press", settings)
    observe(ax, app, executable, (hosts,))
    ax.call(app, executable, "disclose", hosts)
    return lambda label: copy.get(label, label)


def app_case(paths, base, output, evidence):
    evidence["app"] = item = {"status": "in_progress", "boundary": "isolated_release_app_ax_settings"}
    with tempfile.TemporaryDirectory(prefix="abha-", dir=base) as isolated:
        runtime = Path(isolated)
        private_directory(runtime)
        ax = AX(runtime, output)
        executable = paths["desktop"]
        with process(paths["daemon"], runtime, output, "daemon-app", evidence["cleanup"]) as daemon:
            background = ready(runtime, daemon)
            before = audit(runtime)
            empty(before)
            require(pending_entries(runtime) in (None, []), "Expected empty private Host journal initially")
            with process(executable, runtime, output, "desktop-first", evidence["cleanup"]) as app:
                first_pid = app.pid
                item["initial_activation"] = activate(ax, app, executable)
                label_for = open_settings(ax, app, executable)
                observe(ax, app, executable, (label_for("No Hosts are registered."), label_for("Register Host")))
                item["empty_registry_observed"] = True
                for control, value in (("Declared platform", "Linux"),
                                       ("Declared configuration ownership", "External read-only")):
                    title, expected = label_for(control), label_for(value)
                    ax.call(app, executable, "select", title, expected)
                    check_value(ax, app, executable, title, expected)
                for label, value in (
                    ("Host name", HOST_NAME), ("Expected protocol Host ID", PAYLOAD["endpoint"]["expected_host_identity"]),
                    ("SSH endpoint address", PAYLOAD["endpoint"]["address"]), ("SSH user", PAYLOAD["endpoint"]["user"]),
                    ("SSH port", str(PAYLOAD["endpoint"]["port"])), ("SSH Host key alias", PAYLOAD["endpoint"]["host_key_alias"]),
                    ("known_hosts reference", PAYLOAD["known_hosts"]["reference"]),
                    ("known_hosts file SHA-256", PAYLOAD["known_hosts"]["file_sha256"]),
                    ("Transport identity reference", PAYLOAD["transport_identity_ref"]),
                    ("Configuration reference", PAYLOAD["configuration"]["reference"]),
                ):
                    ax.call(app, executable, "fill", label_for(label), value)
                    check_value(ax, app, executable, label_for(label), value)
                ax.call(app, executable, "press", label_for("Register Host"))
                observe(ax, app, executable, (label_for("Acknowledge registration"), label_for("Registry Host UUID")))
                with_pending = audit(runtime)
                require(len(with_pending["idempotency"]) == 1, "No occupied receipt after displayed registration")
                key, _, receipt = with_pending["idempotency"][0]
                confirmed(before, with_pending, key, receipt, PAYLOAD)
                observe(ax, app, executable, (receipt["host_id"], label_for("Acknowledge registration")))
                journal = pending_entries(runtime)
                require(journal and journal[0]["request_key"] == key and journal[0]["registration"] == PAYLOAD,
                        "Journal must retain the exact displayed creation before acknowledgement")
                item["journal_before_ack"] = journal[0]
                item["receipt"] = receipt
                item["captured_receipt"] = frame(ax, app, executable, output, "pending-receipt")
                ax.call(app, executable, "press", label_for("Acknowledge registration"))
                observe(ax, app, executable, (label_for("Registration acknowledged."), receipt["host_id"]))
                require(pending_entries(runtime) == [], "Successful acknowledgement did not empty journal")
                require(audit(runtime) == with_pending, "Acknowledgement modified SQLite")
                item["journal_empty_after_explicit_ack"] = True

                ax.call(app, executable, "press", label_for("Close"))
                label_for = open_settings(ax, app, executable)
                observe(ax, app, executable, (HOST_NAME, label_for("View registration")))
                item["settings_reopened"] = True
            with process(executable, runtime, output, "desktop-reopened", evidence["cleanup"]) as app:
                require(app.pid != first_pid, "Expected a newly launched owned App")
                item["reopen_activation"] = activate(ax, app, executable)
                label_for = open_settings(ax, app, executable)
                observe(ax, app, executable, (HOST_NAME, label_for("View registration")))
                ax.call(app, executable, "press", f"{label_for('View registration')} · {HOST_NAME}")
                observe(ax, app, executable, (receipt["host_id"], label_for("Remote Host has not been contacted or probed")))
                item["captured_reopened"] = frame(ax, app, executable, output, "app-reopened")
                item["app_reopened"] = True
            require(ready(runtime, daemon)["instance_id"] == background["instance_id"],
                    "An unexpected daemon instance replaced the owned daemon")
            require(pending_entries(runtime) == [] and audit(runtime) == with_pending,
                    "App reopen changed the registry or recovery state")
        require(not (runtime / "batond.sock").exists(), "Owned App daemon socket still exists")
        item.update(status="passed", before=before, after=with_pending, same_daemon_across_app_reopen=True)


def interrupted(signum, _frame):
    raise KeyboardInterrupt(f"Test stopped by signal {signum}")


def main():
    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("app", "socket"), required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--overlay", type=Path)
    args = parser.parse_args()
    require(os.uname().sysname == "Darwin" and os.uname().machine == "arm64", "Expected Apple Silicon macOS")
    require(git("merge-base", FIXED_SOURCE, "HEAD") == FIXED_SOURCE, "Unexpected source base")
    require(not git("diff", "--name-only", FIXED_SOURCE, "--", "apps", "crates", "Cargo.lock"),
            "App or daemon sources differ from the fixed build")
    base = task_runtime()
    output = args.output.resolve()
    require(output.is_relative_to(ROOT / "docs/execution" / TASK / "logs"), "Expected a task-owned output path")
    output.mkdir(mode=0o700, parents=True)
    evidence = {"phase": args.phase, "source": FIXED_SOURCE, "driver_commit": git("rev-parse", "HEAD"),
                "driver_source": file_evidence(Path(__file__).resolve()),
                "status": "error", "cleanup": [], "platform": "arm64-apple-darwin"}
    try:
        paths, evidence["bundle"] = bundle_artifacts(args.bundle, args.phase == "app")
        if args.phase == "app":
            require(args.overlay is not None, "Expected the isolated bundle identifier overlay")
            require(json.loads(args.overlay.read_text()) == {"identifier": evidence["bundle"]["identifier"]},
                    "Overlay must change only the isolated bundle identifier")
            evidence["overlay"] = file_evidence(args.overlay)
            evidence["desktop_configuration"] = {
                str(path.relative_to(ROOT)): file_evidence(path)
                for path in [
                    ROOT / "apps/desktop/src-tauri/tauri.conf.json",
                    ROOT / "apps/desktop/src-tauri/capabilities/main.json",
                    *sorted(path for path in (ROOT / "apps/desktop/dist").rglob("*") if path.is_file()),
                ]
            }
            app_case(paths, base, output, evidence)
        else:
            socket_case(paths["daemon"], base, output, evidence)
        require(all(item["joined"] and item["signal"] != "SIGKILL_after_timeout" for item in evidence["cleanup"]),
                "An owned process required forced shutdown")
        evidence["status"] = "passed"
    except (Exception, KeyboardInterrupt) as error:
        evidence["failure"] = str(error)
        raise
    finally:
        (output / "result.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
