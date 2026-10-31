"""Pure source constraints; no Rust execution or macOS verification claim."""
from pathlib import Path
import hashlib
import copy
import json
from unittest import mock
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]
IPC = "crates/core/tests/integration_ipc.rs"
RUNTIME = "crates/core/tests/fixtures/integration_runtime.rs"
DIAGNOSTICS = "crates/core/tests/fixtures/integration_diagnostics.rs"


# Remove only the reviewed b8e/544 post-join observer hooks. The existing
# integration_diagnostics inverse and every historical 7ee digest remain below.
_POST_JOIN_LAYERS = json.loads(r'''
{
  "crates/core/tests/integration_ipc.rs": {
    "input_sha256": "bf89eb10ad51b55578d2455e877d47aab8151a73292639ed77bc558852f1f8b1",
    "restored_sha256": "90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb",
    "edits": [
      [
        "    join_outcome: Option<ActorJoinOutcome>,\n",
        ""
      ],
      [
        "// Test-host observation only; this does not certify worker/process stop.\n#[derive(Clone, Copy, PartialEq, Eq)]\nenum ActorJoinOutcome {\n    Ok,\n    Error,\n    Panic,\n}\n",
        ""
      ],
      [
        "            join_outcome: None,\n",
        ""
      ],
      [
        "            self.join_outcome = Some(match &outcome {\n                Ok(Ok(())) => ActorJoinOutcome::Ok,\n                Ok(Err(_)) => ActorJoinOutcome::Error,\n                Err(_) => ActorJoinOutcome::Panic,\n            });\n",
        ""
      ]
    ]
  },
  "crates/core/tests/fixtures/integration_runtime.rs": {
    "input_sha256": "f2dc88af2ede9507b01e118200319dd4760750745a1793d864188f8318e61761",
    "restored_sha256": "149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06",
    "edits": [
      [
        "#[path = \"integration_stage_evidence.rs\"]\nmod stage_evidence;\n",
        ""
      ],
      [
        "        stage_evidence::finish(\n            &self.server.runtime,\n            self.server.join_outcome,\n            self.task[\"id\"].as_str(),\n            std::thread::panicking(),\n        );\n",
        ""
      ],
      [
        "    let reply = server.request(frame);\n    stage_evidence::observe_reply(&server.runtime, method, &reply);\n    result(reply)\n",
        "    result(server.request(frame))\n"
      ]
    ]
  }
}
''')


def strip_post_join_layer(path,text):
    if type(path)is not str or path not in _POST_JOIN_LAYERS or type(text)is not str:
        raise AssertionError('Post-join inverse arguments refused')
    fixture=_POST_JOIN_LAYERS[path]
    if hashlib.sha256(text.encode()).hexdigest()!=fixture['input_sha256']:
        raise AssertionError('Unknown post-join fixture source refused')
    for expected,replacement in fixture['edits']:
        if text.count(expected)!=1:
            raise AssertionError('Exact post-join inverse anchor refused')
        text=text.replace(expected,replacement,1)
    if hashlib.sha256(text.encode()).hexdigest()!=fixture['restored_sha256']:
        raise AssertionError('Complete pre-stage fixture inverse refused')
    return text


class IntegrationDiagnosticsSourceConstraints(unittest.TestCase):
    def test_ipc_keeps_original_call_and_result_assertion(self):
        current=(ROOT / IPC).read_text()
        for changed in (current+'\n// unknown source region\n',
                current.replace('self.server.finish();','self.server.finish(); // changed original join',1)
                if IPC == RUNTIME else current.replace('let outcome = thread.join();','let outcome = thread.join(); // changed original join',1)):
            with self.assertRaises(AssertionError):strip_post_join_layer(IPC,changed)
        for slot in ('anchor','final'):
            bad=copy.deepcopy(_POST_JOIN_LAYERS)
            if slot=='anchor':bad[IPC]['edits'][0][0]+='changed anchor'
            else:bad[IPC]['restored_sha256']='0'*64
            with mock.patch.dict(_POST_JOIN_LAYERS,bad,clear=True),self.assertRaises(AssertionError):
                strip_post_join_layer(IPC,current)
        # Remove only the reviewed observer additions, then compare the source
        # to its 7ee2437 baseline digest. No Git history/process is required in
        # shallow CI. Intentional future fixture changes must review this guard.
        restored = strip_post_join_layer(IPC, (ROOT / IPC).read_text()).replace(
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
        current=(ROOT / RUNTIME).read_text()
        for changed in (current+'\n// unknown source region\n',
                current.replace('self.server.finish();','self.server.finish(); // changed original join',1)
                if RUNTIME == RUNTIME else current.replace('let outcome = thread.join();','let outcome = thread.join(); // changed original join',1)):
            with self.assertRaises(AssertionError):strip_post_join_layer(RUNTIME,changed)
        for slot in ('anchor','final'):
            bad=copy.deepcopy(_POST_JOIN_LAYERS)
            if slot=='anchor':bad[RUNTIME]['edits'][0][0]+='changed anchor'
            else:bad[RUNTIME]['restored_sha256']='0'*64
            with mock.patch.dict(_POST_JOIN_LAYERS,bad,clear=True),self.assertRaises(AssertionError):
                strip_post_join_layer(RUNTIME,current)
        restored = strip_post_join_layer(RUNTIME, (ROOT / RUNTIME).read_text()).replace(
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
