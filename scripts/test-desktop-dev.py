#!/usr/bin/env python3
"""Helper tests use fake build tools; they do not prove product/native behavior."""
import importlib.util
import fcntl
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("desktop_dev", Path(__file__).with_name("desktop-dev.py"))
DEV = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEV)


class Helpers(unittest.TestCase):
    def test_copy_does_not_invalidate_watch_on_unchanged_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            source, target = Path(directory) / "source", Path(directory) / "target"
            source.write_bytes(b"one")
            DEV.copy_changed(source, target)
            before = target.stat().st_mtime_ns
            DEV.copy_changed(source, target)
            self.assertEqual(before, target.stat().st_mtime_ns)
            source.write_bytes(b"two")
            DEV.copy_changed(source, target)
            self.assertEqual(target.read_bytes(), b"two")

    def test_private_directory_refuses_symlink_and_permissive_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "target").mkdir(mode=0o700)
            (root / "link").symlink_to(root / "target")
            with self.assertRaises(RuntimeError):
                DEV.private_directory(root / "link")
            (root / "target").chmod(0o755)
            with self.assertRaises(RuntimeError):
                DEV.private_directory(root / "target")

    def test_env_preserves_explicit_developer_overrides(self):
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": "mine", "CARGO_INCREMENTAL": "0",
                "CARGO_PROFILE_DEV_DEBUG": "2", "BATON_RUNTIME_DIR": "/explicit"}, clear=True):
            env = DEV.environment()
            self.assertEqual(env["CARGO_TARGET_DIR"], str(DEV.ROOT / "mine"))
            self.assertEqual(env["CARGO_INCREMENTAL"], "0")
            self.assertEqual(env["CARGO_PROFILE_DEV_DEBUG"], "2")
            self.assertEqual(env["BATON_RUNTIME_DIR"], "/explicit")

    def test_ports_and_cli_are_bounded_and_parameterized(self):
        for port in (-1, 0, 80, 65536):
            with self.assertRaises(RuntimeError):
                DEV.available_port(port)
        argv = DEV.tauri_command(12345)
        config = json.loads(argv[argv.index("--config") + 1])
        self.assertEqual(config["build"]["devUrl"], "http://127.0.0.1:12345")
        self.assertEqual(argv[-2:], ["--", "--locked"])
        self.assertLess(len(os.fsencode(DEV.CONTROL / "runtime/batond.sock")), 104)

    def test_explicit_target_rejected_before_build(self):
        with patch.object(DEV, "command") as command:
            with self.assertRaisesRegex(RuntimeError, "CARGO_BUILD_TARGET"):
                DEV.prepare({"CARGO_BUILD_TARGET": "aarch64-apple-darwin"})
            command.assert_not_called()

    def test_lock_distinguishes_unresponsive_session_from_stopped(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(DEV, "CONTROL", Path(directory)):
            with (Path(directory) / "lock").open("a") as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                self.assertTrue(DEV.busy())
            self.assertFalse(DEV.busy())

    def test_completed_group_stays_pinned_until_owned_cleanup(self):
        child = DEV.OwnedProcess([sys.executable, "-c", "pass"], DEV.ROOT, os.environ.copy())
        try:
            deadline = time.monotonic() + 5
            while child.poll() is None and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertEqual(child.returncode, 0)
            self.assertEqual(os.getpgid(child.pid), child.pid)
            self.assertIsNone(child.process.returncode)  # group guardian remains unreaped
        finally:
            DEV.terminate([child])
        with self.assertRaises(ProcessLookupError):
            os.getpgid(child.pid)


class Lifecycle(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="baton helper spaces ")
        self.root = Path(self.temporary.name).resolve()
        (self.root / "scripts").mkdir()
        (self.root / "apps/desktop").mkdir(parents=True)
        (self.root / "apps/desktop/package-lock.json").write_text("{}")
        shutil.copy2(Path(__file__).with_name("desktop-dev.py"), self.root / "scripts/desktop-dev.py")
        self.bin = self.root / "fakebin"
        self.bin.mkdir()
        fake = self.bin / "tool"
        fake.write_text('''#!''' + sys.executable + '''
import json, os, pathlib, signal, subprocess, sys, time
root = pathlib.Path.cwd()
while not (root / 'scripts').is_dir(): root = root.parent
tool = pathlib.Path(sys.argv[0]).name
target = pathlib.Path(os.environ['CARGO_TARGET_DIR'])
if tool == 'rustc': print('host: aarch64-apple-darwin')
elif tool == 'cargo' and sys.argv[1] == 'metadata': print(json.dumps({'target_directory':str(target)}))
elif tool == 'cargo':
    if os.environ.get('FAKE_BUILD_FAIL'): sys.exit(31)
    if os.environ.get('FAKE_BUILD_SLOW'): time.sleep(60)
    (target/'debug').mkdir(parents=True, exist_ok=True)
    for name in ['batond','baton']:
        path=target/'debug'/name
        if name=='batond': body='import time; time.sleep(600)'
        else: body='print("{}")'
        path.write_text('#!'+sys.executable+'\\n'+body+'\\n'); path.chmod(0o755)
elif tool == 'npm' and sys.argv[1] == 'ci':
    folder=root/'apps/desktop/node_modules/.bin'; folder.mkdir(parents=True,exist_ok=True)
    for name in ['tauri','vite']: (folder/name).touch()
elif tool == 'npm':
    child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(600)'])
    (root/'ui-child.pid').write_text(str(child.pid))
    (root/'ui.pid').write_text(str(os.getpid()))
    time.sleep(600)
''')
        fake.chmod(0o755)
        for name in ("cargo", "rustc", "npm"):
            (self.bin / name).symlink_to(fake)
        self.env = os.environ.copy()
        for key in ("CARGO_TARGET_DIR", "BATON_RUNTIME_DIR", "CARGO_BUILD_TARGET"):
            self.env.pop(key, None)
        self.env["PATH"] = str(self.bin) + os.pathsep + self.env["PATH"]
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            self.env["BATON_DEV_PORT"] = str(sock.getsockname()[1])

    def cli(self, action, check=True):
        return subprocess.run([sys.executable, str(self.root / "scripts/desktop-dev.py"), action],
            cwd="/", env=self.env, capture_output=True, text=True, timeout=30, check=check)

    def wait_for(self, predicate):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.1)
        self.fail("condition timed out: " + (self.root / "artifacts/desktop-dev/dev.log").read_text())

    def tearDown(self):
        self.cli("stop", check=False)
        self.temporary.cleanup()

    def test_start_idempotent_restart_and_stop_preserve_unrelated_process(self):
        unrelated = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"])
        try:
            self.cli("start")
            self.wait_for(lambda: (self.root / "ui.pid").exists())
            first = json.loads(self.cli("status").stdout)["pid"]
            self.cli("start")
            self.assertEqual(first, json.loads(self.cli("status").stdout)["pid"])
            ui = int((self.root / "ui.pid").read_text())
            descendant = int((self.root / "ui-child.pid").read_text())
            self.cli("restart")
            self.wait_for(lambda: int((self.root / "ui.pid").read_text()) != ui)
            self.assertNotEqual(first, json.loads(self.cli("status").stdout)["pid"])
            self.cli("stop")
            self.assertIn("stopped", self.cli("status").stdout)
            self.assertIsNone(unrelated.poll())
            with self.assertRaises(ProcessLookupError):
                os.kill(ui, 0)
            with self.assertRaises(ProcessLookupError):
                os.kill(descendant, 0)
        finally:
            unrelated.terminate()
            unrelated.wait()

    def test_stop_during_build(self):
        self.env["FAKE_BUILD_SLOW"] = "1"
        self.cli("start")
        self.wait_for(lambda: "cargo build" in (self.root / "artifacts/desktop-dev/dev.log").read_text())
        self.cli("stop")
        self.assertIn("stopped", self.cli("status").stdout)
        self.assertFalse((self.root / "ui.pid").exists())

    def test_concurrent_starts_converge_on_one_supervisor(self):
        argv = [sys.executable, str(self.root / "scripts/desktop-dev.py"), "start"]
        workers = [subprocess.Popen(argv, env=self.env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True) for _ in range(2)]
        for worker in workers:
            out, err = worker.communicate(timeout=15)
            self.assertEqual(worker.returncode, 0, out + err)
        self.wait_for(lambda: (self.root / "ui.pid").exists())
        self.assertIn("pid", json.loads(self.cli("status").stdout))

    def test_build_failure_cleans_session_and_preserves_log(self):
        self.env["FAKE_BUILD_FAIL"] = "1"
        self.cli("start", check=False)
        self.wait_for(lambda: "stopped" in self.cli("status").stdout)
        self.assertIn("exit status 31", (self.root / "artifacts/desktop-dev/dev.log").read_text())
        self.assertFalse((self.root / "ui.pid").exists())

    def test_occupied_port_is_refused_without_killing_listener(self):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", int(self.env["BATON_DEV_PORT"])))
            listener.listen()
            self.cli("start", check=False)
            self.wait_for(lambda: "stopped" in self.cli("status").stdout)
            self.assertGreater(listener.fileno(), 0)
            self.assertFalse((self.root / "ui.pid").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
