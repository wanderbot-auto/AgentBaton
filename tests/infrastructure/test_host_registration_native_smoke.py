"""Pure and private socket controls for the external Host acceptance driver."""

from copy import deepcopy
from hashlib import sha256
import importlib.util
import json
from pathlib import Path
import socket
from tempfile import TemporaryDirectory
import threading
import unittest


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("host_native_driver", ROOT / "scripts/host-registration-native-smoke.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


def observation():
    key = "989f39d7-251e-4676-8db7-221bd0385f90"
    host_id = "5c9ce3ba-0393-4604-a519-897bcf6cf573"
    receipt = {"receipt_version": 1, "revision": 1, "host_id": host_id,
               "created_sequence": 9, "created_at": "2026-11-18T00:00:00Z",
               "registration": driver.PAYLOAD}
    record = {"id": host_id, "created_sequence": 9, "created_at": receipt["created_at"],
              "creation_key": key, "registration": driver.PAYLOAD,
              "creation_facts": {"remote_connection": "not_contacted", "remote_probe": "not_probed",
                                 "execution_enabled": False}}
    digest = sha256(json.dumps(driver.PAYLOAD, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    event = {"aggregate_id": host_id, "source": "user_action",
             "payload": {"creation_key": key, "host_id": host_id, "registration_sha256": digest}}
    before = {"schema": 19, "other": {"projects": {"count": 1}}}
    after = {**deepcopy(before), "hosts": [[host_id, 1, 9, record]],
             "idempotency": [[key, driver.PAYLOAD, receipt]], "events": [[9, event]]}
    return before, after, key, receipt


class HostRegistrationNativeSmokeTest(unittest.TestCase):
    def test_proof_requires_one_matching_host_event_and_receipt_without_other_writes(self):
        before, after, key, receipt = observation()
        driver.confirmed(before, after, key, receipt, driver.PAYLOAD)
        for field in ("hosts", "idempotency", "events"):
            duplicate = deepcopy(after)
            duplicate[field] *= 2
            with self.subTest(duplicate=field), self.assertRaisesRegex(RuntimeError, "exactly one"):
                driver.confirmed(before, duplicate, key, receipt, driver.PAYLOAD)
        for field, change in (
            ("other", {"projects": {"count": 2}}),
            ("events", [[9, {"aggregate_id": "different"}]]),
            ("idempotency", [["different", driver.PAYLOAD, receipt]]),
        ):
            with self.subTest(drift=field), self.assertRaises(RuntimeError):
                driver.confirmed(before, {**after, field: change}, key, receipt, driver.PAYLOAD)

    def test_journal_checks_presence_shape_and_private_inode(self):
        with TemporaryDirectory() as temp:
            runtime = Path(temp).resolve()
            runtime.chmod(0o700)
            driver.private_directory(runtime)
            path = runtime / "host-registration-recovery.json"
            self.assertIsNone(driver.pending_entries(runtime))
            path.symlink_to(runtime / "missing")
            with self.assertRaisesRegex(RuntimeError, "Unsafe"):
                driver.pending_entries(runtime)
            path.unlink()
            path.write_text('{"version":1,"entries":[]}')
            path.chmod(0o600)
            self.assertEqual(driver.pending_entries(runtime), [])
            path.write_text('{"version":1,"entries":[],"unexpected":true}')
            with self.assertRaisesRegex(RuntimeError, "Invalid"):
                driver.pending_entries(runtime)

    def test_response_is_linked_to_the_exact_private_socket_request(self):
        with TemporaryDirectory() as directory:
            runtime = Path(directory).resolve()
            token = runtime / "session.token"
            token.write_text("synthetic-auth-token")
            token.chmod(0o600)
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                endpoint = runtime / "batond.sock"
                server.bind(str(endpoint))
                endpoint.chmod(0o600)
                server.listen(1)
                server.settimeout(3)
                requests = []

                def serve():
                    for response_id in (None, "other-request"):
                        conn, _ = server.accept()
                        with conn, conn.makefile("rb") as line:
                            request = json.loads(line.readline())
                            requests.append(request)
                            wire = {"request_id": response_id or request["request_id"], "result": {"ok": True}}
                            conn.sendall(json.dumps(wire).encode() + b"\n")

                thread = threading.Thread(target=serve)
                thread.start()
                try:
                    self.assertEqual(driver.exchange(runtime, "hosts.find_registration", {"key": "first"}), {"ok": True})
                    with self.assertRaisesRegex(RuntimeError, "request ID mismatch"):
                        driver.exchange(runtime, "hosts.find_registration", {"key": "second"})
                finally:
                    thread.join(timeout=5)
                self.assertFalse(thread.is_alive())
                self.assertEqual([request["params"] for request in requests], [{"key": "first"}, {"key": "second"}])
                self.assertTrue(all(request["token"] == "synthetic-auth-token" for request in requests))
                self.assertTrue(all("idempotency_key" not in request and "expected_revision" not in request
                                    for request in requests))


if __name__ == "__main__":
    unittest.main()
