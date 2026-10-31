"""Pure source constraints; no Rust execution or macOS verification claim."""
from pathlib import Path
import hashlib
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]
IPC = "crates/core/tests/integration_ipc.rs"
RUNTIME = "crates/core/tests/fixtures/integration_runtime.rs"
DIAGNOSTICS = "crates/core/tests/fixtures/integration_diagnostics.rs"


class IntegrationDiagnosticsSourceConstraints(unittest.TestCase):
    def test_ipc_keeps_original_call_and_result_assertion(self):
        # Remove only the reviewed observer additions, then compare the source
        # to its 7ee2437 baseline digest. No Git history/process is required in
        # shallow CI. Intentional future fixture changes must review this guard.
        restored = (ROOT / IPC).read_text().replace(
            '#[path = "fixtures/integration_diagnostics.rs"]\nmod diagnostics;\n\nstruct Server {',
            "struct Server {",
            1,
        ).replace(
            '        let method = diagnostics::method(&frame["method"]);\n'
            "        let reply = call(&self.runtime, frame).unwrap();\n"
            '        if let Some(error) = reply.get("error") {\n'
            "            diagnostics::ipc_error(method, error);\n"
            "        }\n        reply",
            "        call(&self.runtime, frame).unwrap()",
            1,
        ).replace(
            "        diagnostics::git_environment();\n"
            '        diagnostics::fixture("preview", &root, &source, None);\n'
            '        let server = Server::start(root.join("runtime"));',
            '        let server = Server::start(root.join("runtime"));',
            1,
        )
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),
                         "645e11fb6f1bfae4d52c9c844efce398e4fabdf6be85517b988ba5bf6b37e15c")

    def test_runtime_fixture_only_adds_readonly_observers(self):
        restored = (ROOT / RUNTIME).read_text().replace(
            "use super::{diagnostics, git, result, Server};",
            "use super::{git, result, Server};", 1,
        ).replace(
            "        diagnostics::git_environment();\n"
            '        diagnostics::fixture("runtime", &root, &project_root, None);\n'
            '        let server = Server::start(root.join("runtime"));',
            '        let server = Server::start(root.join("runtime"));', 1,
        ).replace(
            "        diagnostics::fixture(\n"
            '            "runtime-managed",\n            &root,\n            &project_root,\n'
            '            workspace["root"].as_str().map(Path::new),\n        );\n'
            '        let task = rpc(\n            &server,\n            "tasks.get",',
            '        let task = rpc(\n            &server,\n            "tasks.get",', 1,
        )
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),
                         "625aa04bc71f0d9c6d6154bab7a252c713b489904624ad1f5a3f5328047b78d0")

    def test_observer_has_no_process_launch_or_mutation(self):
        source = (ROOT / DIAGNOSTICS).read_text().split("#[cfg(test)]", 1)[0]
        for forbidden in (
            "Command::", "std::process", ".spawn(", ".output(", ".status(",
            "set_var(", "remove_var(", "set_permissions(", "fs::write(",
            "fs::copy(", "fs::rename(", "fs::remove", "fs::create", ".write(true)",
            ".create(true)", ".truncate(true)", "unsafe {",
        ):
            self.assertNotIn(forbidden, source)
        self.assertIn(".take(129)", source)
        self.assertIn("file.take(4097)", source)
        self.assertIn("OFlags::NOFOLLOW", source)
        self.assertIn("text.len() <= 96", source)
        self.assertIn("text.len() > 512", source)
        self.assertEqual(source.count("eprintln!("), 3)
        for tag in ("INTEGRATION_IPC_ERROR", "INTEGRATION_GIT_METADATA", "INTEGRATION_FIXTURE_METADATA"):
            self.assertEqual(source.count(tag), 1)

    def test_error_projection_only_reads_authorized_fields(self):
        source = (ROOT / DIAGNOSTICS).read_text().split("#[cfg(test)]", 1)[0]
        self.assertEqual(set(re.findall(r'error\["([a-z_]+)"\]', source)), {
            "code", "retryable", "side_effect_state", "details_ref",
        })
        details = source.split("fn details(", 1)[1].split("fn error_projection(", 1)[0]
        # Every returned phase is an exact static literal; arbitrary prefixes,
        # identities and DB diagnostics are deliberately projected as 'other'.
        phases = re.findall(r'"(integration-read-phase:[^"]+)"', details)
        self.assertTrue(phases)
        self.assertTrue(all(len(phase) <= 96 for phase in phases))
        self.assertIn('.find(|phase| *phase == text)', details)
        self.assertIn('.unwrap_or("other")', details)
        self.assertNotIn("to_owned()", details)
        self.assertNotIn("to_string()", details)
        ipc = (ROOT / IPC).read_text()
        request = ipc.split("    fn request(", 1)[1].split("    fn image(", 1)[0]
        self.assertLess(request.index("call("), request.index("ipc_error("))
        self.assertIn('if let Some(error) = reply.get("error")', request)


if __name__ == "__main__":
    unittest.main()
