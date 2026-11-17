"""Real local companion fault tests. Run via scripts/verify-remote-host.py.

No remote handshake or Provider calls. Root infrastructure discovery explicitly
skips process tests unless the verifier supplies its freshly built binary.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import select
import signal
import sqlite3
import subprocess
import tempfile
import time
import unittest

BINARY = os.environ.get("BATON_REMOTE_HOST_TEST_BINARY")


@unittest.skipUnless(BINARY, "independent workspace: run scripts/verify-remote-host.py explicitly")
class RealCompanionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="ab-rhost-", dir=os.environ.get("BATON_RUNTIME_DIR"))
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name).resolve()
        self.base.chmod(0o700)
        self.root, self.state = self.base / "mapping", self.base / "state"
        for path in (self.root, self.state):
            path.mkdir(mode=0o700)
        self.token = secrets.token_hex(32)
        # Generate only throwaway identities. Never inspect user's SSH directories.
        identity = self.base / "identity"
        subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(identity)], check=True, capture_output=True, timeout=5)
        host = "h" + hashlib.sha256(identity.with_suffix(".pub").read_bytes()).hexdigest()[:24]
        self.binding = dict(host=host, project="synthetic_project", connection="test_connection", attempt="attempt1", input_revision="rev1", configuration="cfg1", identity_ref="generated_only", mapping="approved_mapping", recipient=host, sensitivity="synthetic")
        now = int(time.time() * 1000)
        self.cfg = dict(schema=1, host_identity=host, root=str(self.root), state_dir=str(self.state), grant=dict(binding=self.binding, controller="ctrl1", fence=1, credential_hash=hashlib.sha256(self.token.encode()).hexdigest(), issued_ms=now - 100, expires_ms=now + 60000, max_operations=8, max_bytes=16384, allow_record=True))
        self.path = self.base / "trusted.json"
        self.save()
        self.processes = []

    def trace(self, direction, **metadata):
        path = os.environ.get("BATON_REMOTE_HOST_TEST_TRACE")
        if path:
            with Path(path).open("a") as output:
                output.write(json.dumps(dict(test=self.id(), direction=direction, **metadata), sort_keys=True) + "\n")

    def save(self):
        self.path.write_text(json.dumps(self.cfg))
        self.path.chmod(0o600)

    def start(self):
        p = subprocess.Popen([BINARY, "--serve", str(self.path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True, bufsize=0)
        self.processes.append(p)
        self.trace("supervisor_started", pid=p.pid)
        return p

    def req(self, name, action):
        return dict(version=1, request_id=name, host_identity=self.cfg["host_identity"], binding=self.binding, controller=self.cfg["grant"]["controller"], fence=self.cfg["grant"]["fence"], credential=self.token, action=action)

    def send(self, p, request):
        self.trace("controller_to_host", pid=p.pid, request_id=request["request_id"], action=request["action"]["kind"], fence=request["fence"])
        p.stdin.write(json.dumps(request, separators=(",", ":")).encode() + b"\n")

    def read(self, p):
        end = time.monotonic() + 3
        data = bytearray()
        while time.monotonic() < end:
            if select.select([p.stdout], [], [], max(0, end - time.monotonic()))[0]:
                b = os.read(p.stdout.fileno(), 1)
                self.assertTrue(b, "unexpected EOF")
                if b == b"\n":
                    response = json.loads(data)
                    self.trace("host_to_controller", pid=p.pid, response=response)
                    return response
                data.extend(b)
                self.assertLessEqual(len(data), 32768)
        self.fail("response timeout")

    def rpc(self, p, request):
        self.send(p, request)
        return self.read(p)["result"]

    def snapshot(self, p, cursor=0):
        r = self.rpc(p, self.req("sync", dict(kind="Reconcile", cursor=cursor)))
        self.assertIn("Ok", r, r)
        return r["Ok"]["snapshot"]

    def record(self, p, key="job", name="result", delay=0, content="approved synthetic bytes"):
        return self.rpc(p, self.req(key, dict(kind="Record", name=name, content=content, delay_ms=delay)))

    def finish(self, p):
        if p.stdin and not p.stdin.closed:
            p.stdin.close()
        p.wait(timeout=8)

    def joined(self, p):
        end = time.monotonic() + 7
        while time.monotonic() < end:
            s = self.snapshot(p)
            if s["jobs"] and all(j["stop_evidence"] is not None for j in s["jobs"]):
                return s
            time.sleep(.02)
        self.fail("worker not joined")

    def tearDown(self):
        for p in self.processes:
            try:
                self.finish(p)
            except (subprocess.TimeoutExpired, BrokenPipeError):
                if p.poll() is None:
                    os.killpg(p.pid, signal.SIGKILL)
                p.wait(timeout=3)
            for pipe in (p.stdin, p.stdout, p.stderr):
                if pipe and not pipe.closed:
                    pipe.close()
            self.assertIsNotNone(p.returncode)
            # A completed supervisor must have reaped all children in its own group.
            result = subprocess.run(["ps", "-axo", "pgid=,pid=,comm="], text=True, capture_output=True, check=True, timeout=3)
            survivors = [line for line in result.stdout.splitlines() if line.split() and line.split()[0] == str(p.pid)]
            self.trace("supervisor_joined", pid=p.pid, exit_code=p.returncode, surviving_group=survivors)
            self.assertEqual(survivors, [], survivors)
        artifacts = [{"name": path.name, "bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in sorted(self.root.iterdir()) if path.is_file() and not path.is_symlink()]
        self.trace("synthetic_artifacts", artifacts=artifacts)
        self.temp.cleanup()

    def test_lost_response_and_lost_ack_reopen_exactly_one_effect(self):
        p = self.start()
        self.snapshot(p)
        request = self.req("once", dict(kind="Record", name="once", content="one", delay_ms=50))
        # The real response pipe disappears after dispatch bytes; do not consume response.
        p.stdout.close()
        self.send(p, request)
        self.finish(p)
        self.assertEqual((self.root / "once").read_text(), "one")
        q = self.start()
        s = self.snapshot(q)
        self.assertTrue(s["paused"])
        self.assertEqual(s["used_operations"], 1)
        self.assertEqual(s["jobs"][0]["state"], "succeeded")
        self.assertIn("Ok", self.rpc(q, request))
        # Lost ACK means replay from cursor zero; sequence/payload remain identical.
        replay = self.snapshot(q)
        self.assertEqual(s["events"], replay["events"])
        self.assertEqual(self.snapshot(q, s["cursor"])["events"], [])
        self.assertEqual(len(list(self.root.iterdir())), 1)

    def test_disconnect_cancel_reconcile_and_join(self):
        p = self.start()
        self.snapshot(p)
        self.assertIn("Ok", self.record(p, delay=1200))
        p.stdin.close()  # disconnect; host is still supervising approved work
        q = self.start()  # same epoch reopening conservatively pauses uncertain execution
        s = self.snapshot(q)
        self.assertTrue(s["paused"])
        response = self.rpc(q, self.req("cancel", dict(kind="Cancel")))
        self.assertEqual(response["Ok"]["kind"], "CancelRequested")
        self.assertNotIn("confirmed_stopped", response["Ok"])
        p.wait(timeout=7)
        s = self.joined(q)
        self.assertTrue(s["confirmed_stopped"])
        self.assertFalse((self.root / "result").exists())

    def test_lease_expires_during_actual_worker(self):
        self.cfg["grant"]["expires_ms"] = int(time.time() * 1000) + 350
        self.save()
        p = self.start()
        self.snapshot(p)
        self.assertIn("Ok", self.record(p, delay=1000))
        s = self.joined(p)
        self.assertEqual(s["jobs"][0]["state"], "stopped_without_effect")
        self.assertFalse((self.root / "result").exists())
        self.assertEqual(self.record(p, key="late", name="late")["Err"]["code"], "paused")

    def test_real_idle_is_not_partial_frame_timeout(self):
        p = self.start()
        self.snapshot(p)
        time.sleep(2.15)
        self.assertIsNone(p.poll())
        self.assertFalse(self.snapshot(p)["paused"])
        self.assertIn("Ok", self.record(p))
        self.assertEqual(self.joined(p)["jobs"][0]["state"], "succeeded")

    def test_half_frame_unknown_fields_unknown_version_and_oversize(self):
        p = self.start()
        self.snapshot(p)
        r = self.req("v", dict(kind="Cancel"))
        r["version"] = 9
        self.assertEqual(self.rpc(p, r)["Err"]["code"], "version")
        r["version"] = 1
        r["action"] = dict(kind="Grant", role="root")
        self.assertEqual(self.rpc(p, r)["Err"]["code"], "protocol")
        p.stdin.write(b'{"version":')
        p.stdin.close()
        self.assertEqual(self.read(p)["result"]["Err"]["code"], "half_frame")
        p.wait(timeout=3)
        q = self.start()
        q.stdin.write(b"x" * 32769 + b"\n")
        self.assertEqual(self.read(q)["result"]["Err"]["code"], "oversize")
        q.wait(timeout=3)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_partial_frame_deadline(self):
        p = self.start()
        p.stdin.write(b"{")
        self.assertEqual(self.read(p)["result"]["Err"]["code"], "timeout")
        p.wait(timeout=3)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_journal_write_failure_before_any_spawn(self):
        p = self.start()
        self.snapshot(p)
        with sqlite3.connect(self.state / "journal.sqlite3") as db:
            db.execute("CREATE TRIGGER deny_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT, 'injected SQLite write rejection'); END")
        self.assertEqual(self.record(p)["Err"]["code"], "journal")
        with sqlite3.connect(self.state / "journal.sqlite3") as db:
            self.assertEqual(db.execute("SELECT count(*) FROM jobs").fetchone()[0], 0)
            self.assertEqual(db.execute("SELECT count(*) FROM receipts").fetchone()[0], 0)
            self.assertEqual(db.execute("SELECT used_ops FROM meta").fetchone()[0], 0)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_symlink_output_cannot_overwrite_source(self):
        source = self.base / "original"
        source.write_text("must remain")
        (self.root / "result").symlink_to(source)
        p = self.start()
        self.snapshot(p)
        self.record(p)
        self.assertEqual(self.joined(p)["jobs"][0]["state"], "failed_no_effect")
        self.assertEqual(source.read_text(), "must remain")

    def test_actual_worker_cannot_adopt_new_fence_lease(self):
        p = self.start()
        self.snapshot(p)
        self.record(p, delay=1200)
        self.cfg["grant"]["fence"] += 1
        self.cfg["grant"]["expires_ms"] += 10000
        self.save()
        q = self.start()
        self.snapshot(q)
        self.finish(p)
        s = self.joined(q)
        self.assertEqual(s["jobs"][0]["state"], "outcome_unknown")
        self.assertFalse((self.root / "result").exists())
        self.assertEqual(s["used_operations"], 1)

    def test_process_kill_after_dispatch_is_unknown_not_replayed(self):
        p = self.start()
        self.snapshot(p)
        self.record(p, delay=1200)
        # Crash the entire owned group to avoid intentionally orphaning a worker.
        os.killpg(p.pid, signal.SIGKILL)
        p.wait(timeout=3)
        q = self.start()
        s = self.snapshot(q)
        self.assertEqual(s["jobs"][0]["state"], "outcome_unknown")
        self.assertIsNone(s["jobs"][0]["stop_evidence"])
        self.assertIn("Ok", self.record(q, delay=1200))  # durable receipt, no new worker
        self.rpc(q, self.req("cancel", dict(kind="Cancel")))
        s = self.snapshot(q)
        self.assertTrue(s["cancel_requested"])
        self.assertFalse(s["confirmed_stopped"])
        self.assertFalse((self.root / "result").exists())

    def test_clock_rollback_stops_actual_worker_but_keeps_cancel_and_reconcile(self):
        p = self.start()
        self.snapshot(p)
        self.record(p, delay=1000)
        # Inject rollback during an actual running worker, not during its startup.
        deadline = time.monotonic() + 3
        with sqlite3.connect(self.state / "journal.sqlite3") as db:
            while True:
                state = db.execute("SELECT state FROM jobs WHERE id='job'").fetchone()[0]
                if state == "running":
                    break
                self.assertEqual(state, "reserved", "worker failed before clock fault injection")
                self.assertLess(time.monotonic(), deadline, "worker never entered running state")
                time.sleep(.01)
            db.execute("UPDATE meta SET highwater=highwater+20000")
        s = self.joined(p)
        self.assertTrue(s["paused"])
        self.assertEqual(s["jobs"][0]["state"], "stopped_without_effect")
        self.assertEqual(self.rpc(p, self.req("cancel", dict(kind="Cancel")))["Ok"]["kind"], "CancelRequested")
        self.assertTrue(self.snapshot(p)["confirmed_stopped"])
        self.assertFalse((self.root / "result").exists())

    def test_real_auth_budget_permission_and_binding_denials(self):
        self.cfg["grant"]["max_operations"] = 1
        self.save()
        p = self.start()
        self.snapshot(p)
        r = self.req("bad", dict(kind="Record", name="bad", content="x", delay_ms=0))
        r["credential"] = "not-authorized"
        self.assertEqual(self.rpc(p, r)["Err"]["code"], "unauthenticated")
        r["credential"] = self.token
        r["binding"] = dict(self.binding, input_revision="other")
        self.assertEqual(self.rpc(p, r)["Err"]["code"], "identity_binding")
        self.assertIn("Ok", self.record(p))
        self.joined(p)
        self.assertEqual(self.record(p, key="second", name="second")["Err"]["code"], "budget_exhausted")
        self.assertEqual(len(list(self.root.iterdir())), 1)
        self.assertEqual(self.snapshot(p)["used_operations"], 1)

    def test_journal_failure_after_file_effect_remains_unknown_without_retry(self):
        p = self.start()
        self.snapshot(p)
        with sqlite3.connect(self.state / "journal.sqlite3") as db:
            db.execute("CREATE TRIGGER deny_durable BEFORE INSERT ON events WHEN NEW.kind='record_durable' BEGIN SELECT RAISE(ABORT, 'post-effect journal write failure'); END")
        self.assertIn("Ok", self.record(p))
        s = self.joined(p)
        self.assertEqual(s["jobs"][0]["state"], "outcome_unknown")
        self.assertEqual(s["jobs"][0]["stop_evidence"], "owned_child_wait_nonzero")
        self.assertEqual((self.root / "result").read_text(), "approved synthetic bytes")
        self.finish(p)
        q = self.start()
        self.snapshot(q)
        self.assertIn("Ok", self.record(q))
        self.assertEqual(len(list(self.root.iterdir())), 1)
        self.assertEqual(self.snapshot(q)["used_operations"], 1)

    def test_parallel_owned_workers_do_not_false_pause_on_highwater(self):
        p = self.start()
        self.snapshot(p)
        for i in range(6):
            self.assertIn("Ok", self.record(p, key=f"j{i}", name=f"f{i}", delay=80))
        s = self.joined(p)
        self.assertEqual(len(s["jobs"]), 6)
        self.assertTrue(all(j["state"] == "succeeded" for j in s["jobs"]))
        self.assertFalse(s["paused"])
        self.assertEqual(len(list(self.root.iterdir())), 6)

    def test_reopen_fails_on_corrupt_or_future_schema_without_effects(self):
        p = self.start()
        self.snapshot(p)
        self.finish(p)
        with sqlite3.connect(self.state / "journal.sqlite3") as db:
            db.execute("PRAGMA user_version=99")
        q = self.start()
        self.finish(q)
        self.assertNotEqual(q.returncode, 0)
        self.assertIn(b"journal_version", q.stderr.read())
        (self.state / "journal.sqlite3").write_bytes(b"not a SQLite journal")
        r = self.start()
        self.finish(r)
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_old_companion_deadline_cannot_pause_new_fence(self):
        self.cfg["grant"]["expires_ms"] = int(time.time() * 1000) + 500
        self.save()
        old = self.start()
        self.snapshot(old)
        self.cfg["grant"]["fence"] += 1
        self.cfg["grant"]["expires_ms"] += 10000
        self.save()
        fresh = self.start()
        self.assertFalse(self.snapshot(fresh)["paused"])
        time.sleep(.65)
        self.assertIsNone(old.poll())
        self.assertFalse(self.snapshot(fresh)["paused"])
        self.assertIn("Ok", self.record(fresh))
        self.assertEqual(self.joined(fresh)["jobs"][0]["state"], "succeeded")


if __name__ == "__main__":
    unittest.main(verbosity=2)
