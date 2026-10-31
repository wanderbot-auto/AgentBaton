"""Pure generated-source/metadata controls: no Core, process, SQL or browser."""
import ast
import io
import json
from pathlib import Path
import re
import types
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'crates/core/tests/fixtures'
PRIVATE = 'PRIVATE_ID_KEY_MESSAGE_ARGV_BODY'


class MemoryOS:
    def __init__(self):
        self.writes = []
        self.replacements = []
        self.O_WRONLY = 1
        self.O_CREAT = 2
        self.O_TRUNC = 4

    def open(self, path, flags, mode):
        self.mode = mode
        return 7

    def fdopen(self, fd, mode):
        owner = self
        class Writer(io.BytesIO):
            def fileno(self):
                return fd
            def __exit__(self, *args):
                owner.writes.append(self.getvalue())
                self.close()
        return Writer()

    def fchmod(self, fd, mode):
        assert fd == 7 and mode == 0o600

    def replace(self, source, destination):
        self.replacements.append((source, destination))


def helper():
    fake_os = MemoryOS()
    namespace = {'json': json, 'os': fake_os}
    path = FIXTURES / 'factory_child_goal_deadline_diagnostics.py'
    exec(compile(path.read_text(), str(path), 'exec'), namespace)
    return namespace, fake_os


def generated_source():
    """Read actual replacement operands, rather than copying probe operands."""
    base = (ROOT / 'crates/core/tests/factory_runtime_ipc.rs').read_text()
    script = re.search(r'const ACP_FACTORY: &str = r#"(.*?)"#;', base, re.S)[1]
    runtime = (FIXTURES / 'factory_child_goal_runtime.rs').read_text()
    inserted = re.search(r'script=script.replace\("  if stage==\'parent-initial\':",r#"(.*?)"#\);', runtime, re.S)[1]
    script = script.replace("  if stage=='parent-initial':", inserted)
    script = script.replace('mcp=None;mid=0', (FIXTURES / 'factory_child_goal_deadline_diagnostics.py').read_text() + '\nmcp=None;mid=0')
    diagnostics = runtime.split('    if deadline_probe {', 1)[1]
    literal = r'"(?:[^"\\]|\\.)*"'
    operands = re.findall(r'script = script.replace\(\s*(' + literal + r'),\s*(' + literal + r'),\s*\)', diagnostics)
    stage_pairs = diagnostics.split('for (original, observed) in [', 1)[1].split('] {', 1)[0]
    operands += re.findall(r'\(\s*(' + literal + r'),\s*(' + literal + r'),?\s*\)', stage_pairs)
    assert len(operands) == 8
    for original, observed in operands:
        original, observed = json.loads(original), json.loads(observed)
        assert script.count(original) == 1, original
        script = script.replace(original, observed)
    return script


class DeadlineDiagnosticsTests(unittest.TestCase):
    def test_rust_collector_keeps_closed_projection_and_nonblocking_pinned_read_boundary(self):
        source = (FIXTURES / 'factory_child_goal_deadline_evidence.rs').read_text()
        for required in ('O_NOFOLLOW', 'O_NONBLOCK', 'O_DIRECTORY', 'libc::openat',
                         'info.uid() == uid', 'info.nlink() == 1', '0o600',
                         'take(MAX_BYTES + 1)', 'file.metadata()', 'fs::symlink_metadata',
                         'serde_json::from_slice(raw)', 'serde_json::to_vec(&marker)',
                         'output.write_all(&safe)', 'marker.records.len() > 32'):
            self.assertIn(required, source)
        self.assertNotIn('fs::copy', source)
        self.assertNotIn('output.write_all(&raw)', source)
        for name in ('Marker', 'Record'):
            self.assertRegex(source, r'#\[serde\(deny_unknown_fields\)\]\s*struct ' + name)
        stages = re.search(r'enum Stage \{(.*?)\}', source, re.S)[1]
        stages = {re.sub(r'(?<!^)(?=[A-Z])', '_', value).lower()
                  for value in re.findall(r'^\s*(\w+),', stages, re.M)}
        self.assertEqual(stages, helper()[0]['DEADLINE_STAGES'])
        runtime = (FIXTURES / 'factory_child_goal_runtime.rs').read_text()
        self.assertLess(runtime.index('thread.join()'), runtime.index('retain_deadline_parent_stages(&self.workspace'))

    def test_actual_source_hooks_are_unique_and_generated_python_compiles(self):
        source = generated_source()
        tree = ast.parse(source)
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Name) and node.func.id == 'deadline_tool']
        self.assertEqual(len(calls), 6)
        self.assertEqual({call.args[0].value for call in calls}, helper()[0]['DEADLINE_STAGES'])

    def test_success_preserves_exact_result_and_calls_tool_once(self):
        namespace, os = helper()
        result = {PRIVATE: [PRIVATE]}
        calls = []
        namespace['tool'] = lambda name, args: (calls.append((name, args)), result)[1]
        self.assertIs(namespace['deadline_tool']('delegate_child', 'delegate_task', {PRIVATE: PRIVATE}), result)
        self.assertEqual(calls, [('delegate_task', {PRIVATE: PRIVATE})])
        self.assertEqual([r['event'] for r in namespace['_deadline_records']], ['before', 'after'])
        self.assertIsNone(namespace['_deadline_active'])
        self.assertNotIn(PRIVATE, os.writes[-1].decode())
        self.assertEqual(os.mode, 0o600)

    def test_original_exception_is_rethrown_once_without_retry_or_raw_body(self):
        for error in (AssertionError(PRIVATE), json.JSONDecodeError(PRIVATE, PRIVATE, 0), BrokenPipeError(PRIVATE)):
            namespace, os = helper()
            calls = []
            def fail(name, args):
                calls.append(name)
                raise error
            namespace['tool'] = fail
            with self.assertRaises(type(error)) as caught:
                namespace['deadline_tool']('execute_child_goal', 'execute_child_goal', {})
            self.assertIs(caught.exception, error)
            self.assertEqual(calls, ['execute_child_goal'])
            self.assertIsNone(namespace['_deadline_active'])
            self.assertNotIn(PRIVATE, os.writes[-1].decode())

    def test_actual_instrumented_tool_keeps_wire_once_and_original_error_assertions(self):
        definitions = [node for node in ast.parse(generated_source()).body
                       if isinstance(node, ast.FunctionDef) and node.name in {'mcp_call', 'tool'}]
        for layer in ('success', 'jsonrpc_error', 'actual_error'):
            namespace, os = helper()
            actual = {'result': {PRIVATE: PRIVATE}}
            if layer == 'actual_error':
                actual = {'error': {'code': 'INPUT_CHANGED', 'retryable': False,
                                    'side_effect_state': 'not_started', 'message': PRIVATE}}
            reply = {'result': {'content': [{'text': json.dumps(actual)}]}}
            if layer == 'jsonrpc_error':
                reply = {'error': {'code': -32601, 'message': PRIVATE}}
            stdin = io.StringIO()
            namespace.update(mcp=types.SimpleNamespace(stdin=stdin, stdout=io.StringIO(json.dumps(reply) + '\n')), mid=0)
            exec(compile(ast.Module(body=definitions, type_ignores=[]), '<actual-generated-tool-definitions>', 'exec'), namespace)
            if layer == 'success':
                self.assertEqual(namespace['deadline_tool']('submit_child_plan', 'submit_child_plan', {}), actual['result'])
            else:
                with self.assertRaises(AssertionError):
                    namespace['deadline_tool']('submit_child_plan', 'submit_child_plan', {})
            self.assertEqual(len(stdin.getvalue().splitlines()), 1)
            self.assertEqual(namespace['mid'], 1)
            self.assertNotIn(PRIVATE, os.writes[-1].decode())

    def test_reply_layers_stay_distinct_and_codes_effects_flags_are_closed(self):
        namespace, os = helper()
        namespace['_deadline_active'] = 'submit_child_plan'
        namespace['deadline_envelope_reply']({'error': {'code': -32601, 'message': PRIVATE}}, rpc=True)
        namespace['deadline_tools_reply']({'isError': True, 'content': [{'text': PRIVATE}]})
        namespace['deadline_envelope_reply']({'error': {'code': 'WORKSPACE_BUSY', 'retryable': True, 'side_effect_state': 'not_started', 'message': PRIVATE}}, rpc=False)
        rows = namespace['_deadline_records']
        self.assertEqual([r['layer'] for r in rows], ['jsonrpc', 'tools_call', 'actual_tool'])
        self.assertEqual(rows[0]['code'], 'method_not_found')
        self.assertEqual(rows[1]['reply_class'], 'error')
        self.assertEqual((rows[2]['code'], rows[2]['retryable'], rows[2]['side_effect_state']), ('WORKSPACE_BUSY', 'true', 'not_started'))
        namespace['deadline_envelope_reply']({'error': {'code': PRIVATE, 'retryable': 1, 'side_effect_state': PRIVATE}}, rpc=False)
        self.assertEqual(namespace['_deadline_records'][-1]['code'], 'other')
        self.assertEqual(namespace['_deadline_records'][-1]['retryable'], 'invalid')
        self.assertEqual(namespace['_deadline_records'][-1]['side_effect_state'], 'invalid')
        self.assertNotIn(PRIVATE, os.writes[-1].decode())

    def test_malformed_and_mixed_replies_cannot_be_recorded_as_result(self):
        namespace, _ = helper()
        namespace['_deadline_active'] = 'child_execution_status'
        for reply in (None, [], {}, {'result': {}, 'error': None}):
            namespace['deadline_envelope_reply'](reply, rpc=False)
            self.assertEqual(namespace['_deadline_records'][-1]['reply_class'], 'malformed')
        namespace['deadline_tools_reply']({'isError': 'true', 'content': []})
        self.assertEqual(namespace['_deadline_records'][-1]['reply_class'], 'malformed')

    def test_capture_is_bounded_and_io_failure_does_not_replace_original_result(self):
        namespace, os = helper()
        namespace['_deadline_active'] = 'parent_task_status'
        for _ in range(1000):
            namespace['deadline_envelope_reply']({'error': {'code': PRIVATE}}, rpc=False)
        self.assertEqual(len(namespace['_deadline_records']), 32)
        self.assertLessEqual(len(os.writes[-1]), 16384)
        self.assertEqual(len(os.writes), 32)
        namespace, os = helper()
        def unavailable(*args):
            raise OSError(PRIVATE)
        os.open = unavailable
        expected = object()
        namespace['tool'] = lambda name, args: expected
        self.assertIs(namespace['deadline_tool']('delegate_child', 'delegate_task', {}), expected)


if __name__ == '__main__':
    unittest.main()
