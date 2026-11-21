"""Check the fixed Host invocations and their production dispatcher boundary."""

import json
from pathlib import Path
import re
import unittest


TAURI = Path(__file__).resolve().parents[1]
LIB = (TAURI / "src/lib.rs").read_text()
HOSTS = (TAURI / "src/hosts.rs").read_text()
COMMANDS = {
    "hosts_register",
    "hosts_list",
    "hosts_get",
    "hosts_registration_pending",
    "hosts_registration_reconcile",
    "hosts_registration_retry",
    "hosts_registration_acknowledge",
}


class HostsNativeSourceTests(unittest.TestCase):
    def test_exactly_seven_fixed_commands_enter_the_production_dispatcher(self):
        commands = re.search(r"tauri::generate_handler!\[(.*?)\n    \];", LIB, re.S)[1]
        self.assertEqual(set(re.findall(r"hosts::(\w+)", commands)), COMMANDS)
        self.assertEqual(len(re.findall(r"hosts::\w+", commands)), len(COMMANDS))
        self.assertEqual(
            set(re.findall(r"#\[tauri::command\]\s+pub\(super\) async fn (\w+)", HOSTS)),
            COMMANDS,
        )
        self.assertIn("mod hosts;", LIB)
        self.assertIn(".invoke_handler(dispatch_production)", LIB)
        for existing in ("background_service_status", "background_service_enable",
                         "background_service_disable", "connections_list",
                         "client_recovery_abandon", "task_message_acknowledge"):
            self.assertEqual(len(re.findall(rf"^ {{8}}{existing},$", commands, re.M)), 1)

    def test_fixed_methods_use_the_existing_private_runtime(self):
        self.assertEqual(
            set(re.findall(r'"method":"([^"]+)"', HOSTS)),
            {"hosts.register", "hosts.list", "hosts.get", "hosts.find_registration"},
        )
        self.assertEqual(HOSTS.count("ipc::call("), 1)
        self.assertIn("let directory = runtime.directory.clone();", HOSTS)
        self.assertIn("let InvokeBody::Json(args) = request.body()", HOSTS)
        self.assertIn("exact_fields(args, operation.fields())", HOSTS)
        for unrelated in ("ensure_daemon", "recover_task_messages", "UnixStream", "Command::new",
                          "expected_revision", "system.snapshot", "hosts.abandon", "native-smoke"):
            self.assertNotIn(unrelated, HOSTS)

    def test_webview_receipt_is_not_an_acknowledgement_argument(self):
        self.assertIn('Self::Acknowledge => &["entryId", "hostId", "createdSequence"]', HOSTS)
        self.assertIn("journal.acknowledge_host_registration(", HOSTS)
        self.assertEqual(json.loads((TAURI / "capabilities/main.json").read_text())["permissions"], [])
        self.assertRegex(HOSTS, r'#\[cfg\(test\)\]\s+#\[path = "hosts_tests.rs"\]')


if __name__ == "__main__":
    unittest.main()
