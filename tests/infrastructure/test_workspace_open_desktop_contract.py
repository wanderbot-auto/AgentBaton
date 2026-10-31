"""Desktop boundary source controls only. Never opens IPC/filesystem targets or tools."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
NATIVE = (ROOT / 'apps/desktop/src-tauri/src/workspace_open.rs').read_text()
UI = (ROOT / 'apps/desktop/src/ExternalWorkspaceOpen.tsx').read_text()
API = (ROOT / 'apps/desktop/src/workspace-open-api.ts').read_text()
WIRE = (ROOT / 'apps/desktop/src/workspace-open-wire.ts').read_text()


class WorkspaceOpenDesktopBoundary(unittest.TestCase):
    def test_fixed_program_three_argv_and_child_only_environment(self):
        builder = NATIVE[NATIVE.index('fn opener_command('):NATIVE.index('fn completed(')]
        self.assertIn('const OPENER: &str = "/usr/bin/open";', NATIVE)
        self.assertIn('const TERMINAL: &str = "/System/Applications/Utilities/Terminal.app";', NATIVE)
        self.assertIn('.args(["-a", TERMINAL, path])', builder)
        self.assertIn('.env_clear()', builder)
        self.assertEqual(builder.count('.env('), 1)
        self.assertIn('"PATH", "/usr/bin:/bin:/usr/sbin:/sbin"', builder)
        for stream in ('stdin', 'stdout', 'stderr'):
            self.assertIn('.' + stream + '(Stdio::null())', builder)
        for forbidden in ('"sh"', '"-c"', 'osascript', 'xcrun', 'DEVELOPER_DIR', 'BATON_TOKEN'):
            self.assertNotIn(forbidden, builder)

    def test_original_owner_read_then_held_context_and_named_recheck_before_only_spawn(self):
        handoff = NATIVE[NATIVE.index('fn handoff_sync('):NATIVE.index('fn opener_command(')]
        self.assertIn('"method":"workspace_open.context","params":{"selection":selection}', NATIVE)
        self.assertLess(handoff.index('ipc::call('), handoff.index('pin_open_context('))
        self.assertLess(handoff.index('pin_open_context('), handoff.index('pin.recheck()'))
        self.assertLess(handoff.index('pin.recheck()'), handoff.index('open_directory('))
        self.assertEqual(NATIVE.count('.spawn()'), 1)
        self.assertNotIn('ensure_daemon', NATIVE)
        self.assertNotIn('confirm(', NATIVE)

    def test_unsupported_linux_and_untrusted_window_precede_context_or_spawn(self):
        handoff = NATIVE[NATIVE.index('fn handoff_sync('):NATIVE.index('fn opener_command(')]
        self.assertLess(handoff.index('!trusted_window('), handoff.index('ipc::call('))
        self.assertLess(handoff.index('!cfg!(target_os = "macos")'), handoff.index('ipc::call('))
        self.assertIn('label == "main"', NATIVE)
        self.assertIn('super::native_parent::with_caller', handoff)
        self.assertIn('parent.finalize(', handoff)
        self.assertIn('ACTIVE', handoff)

    def test_no_detached_or_unbounded_reaper_and_no_terminal_signal(self):
        self.assertEqual(NATIVE.count('Duration::from_secs(5)'), 1)
        self.assertNotIn('.wait()', NATIVE)
        self.assertNotIn('thread::spawn', NATIVE)
        self.assertNotIn('killpg', NATIVE)
        self.assertEqual(NATIVE.count('child.kill()'), 1)
        self.assertIn('owned_slot()', NATIVE)
        self.assertIn('if !clear_previous_owned_opener()', NATIVE)
        self.assertIn('Instant::now() < deadline', NATIVE)
        self.assertIn('unknown("opener_timeout")', NATIVE)
        self.assertIn('unknown("opener_failed")', NATIVE)

    def test_renderer_only_invokes_one_closed_native_request_and_never_passes_path(self):
        self.assertEqual(API.count("invoke<unknown>("), 1)
        self.assertIn("'open_workspace_terminal', { request: validateWorkspaceOpenRequest(request) }", API)
        request_type = WIRE[WIRE.index('export type WorkspaceOpenRequest'):WIRE.index('export type WorkspaceOpenCode')]
        self.assertNotIn('path', request_type)
        self.assertNotIn('env', request_type)
        self.assertIn("['selection', 'terminal']", WIRE)
        self.assertIn("['selection', 'canonical_path', 'directory_identity']", WIRE)
        self.assertIn("['status', 'code']", WIRE)

    def test_epoch_unknown_and_no_automatic_handoff_or_retry(self):
        self.assertNotIn('setTimeout', UI)
        self.assertNotIn('setInterval', UI)
        self.assertEqual(UI.count('workspaceOpenApi.open('), 1)
        self.assertIn('outstanding.size > 0', UI)
        self.assertIn('epoch.current === stamp', UI)
        self.assertIn('identity(current.current) === expected', UI)
        self.assertIn('uncertain.add(operationKey)', UI)
        self.assertIn('unknown && !retryChecked', UI)
        self.assertIn("value.status === 'refused' && !priorUnknown", UI)
        self.assertIn('onClick={() => void openDirectory()}', UI)

    def test_bilingual_keys_are_complete_and_claim_handoff_not_observed_directory(self):
        text = (ROOT / 'apps/desktop/src/workspace-open-i18n.ts').read_text()
        en, zh = text.split('  zh: {', 1)
        keys = lambda s: set(re.findall(r'\b(\w+):\s*\'', s))
        self.assertEqual(keys(en), keys(zh))
        self.assertIn('have not been observed', en)
        self.assertIn('尚未观察', zh)
        self.assertIn('does not start an Agent', en)
        self.assertIn('不启动 Agent', zh)


if __name__ == '__main__':
    unittest.main()
