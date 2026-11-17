"""Closed production export bridge; runtime behavior is exercised by Rust/UI/IPC tests."""
from pathlib import Path
import re
import unittest
from test_background_service_native_smoke_source import restored_parent_bridge_token_sha, PARENT_BRIDGE_TOKEN_SHA256

ROOT = Path(__file__).resolve().parents[2]
COMMANDS = ('history_export_preview', 'history_export_publish', 'history_export_get', 'history_export_reveal')


class HistoryExportNativeBoundary(unittest.TestCase):
    def test_original_parent_pin_survives_both_base_and_exact_current_composition(self):
        source=(ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        self.assertEqual(restored_parent_bridge_token_sha(source), PARENT_BRIDGE_TOKEN_SHA256)
        base=source.replace('mod history_export;\n','',1)
        for name in COMMANDS:
            base=re.sub(r'^\s*history_export::'+name+r',\n','',base,count=1,flags=re.M)
        self.assertNotEqual(base, source)
        self.assertEqual(restored_parent_bridge_token_sha(base), PARENT_BRIDGE_TOKEN_SHA256)

    def test_partial_duplicate_renamed_extra_and_old_bridge_tampering_cannot_pass_parent_pin(self):
        source=(ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        entry='history_export::history_export_preview,'
        malformed=(source.replace(entry,'',1),source.replace(entry,entry+' '+entry,1),
                   source.replace(entry,'history_export::history_export_preview_changed,',1),
                   source.replace('mod history_export;','',1))
        for value in malformed:
            with self.subTest(kind=value[:32]):
                with self.assertRaises(ValueError):
                    restored_parent_bridge_token_sha(value)
        with self.assertRaises(ValueError):
            restored_parent_bridge_token_sha(source.replace(entry,entry+' history_export::arbitrary_write,',1))
        self.assertNotEqual(restored_parent_bridge_token_sha(source.replace('"projects.list"','"projects.changed"',1)),
                            PARENT_BRIDGE_TOKEN_SHA256)

    def test_exact_typed_commands_are_registered_once_without_generic_native_io(self):
        bridge = (ROOT/'apps/desktop/src-tauri/src/history_export.rs').read_text()
        host = (ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        self.assertEqual(tuple(re.findall(r'pub\(super\) async fn (\w+)\(', bridge)), COMMANDS)
        self.assertEqual(bridge.count('#[tauri::command]'), 4)
        for name in COMMANDS:
            self.assertEqual(host.count('history_export::'+name+','), 1)
        self.assertIn('selection: Selection,', bridge)
        self.assertIn('idempotency_key: String,', bridge)
        self.assertEqual(bridge.count('reference: Reference,'), 3)
        for forbidden in ('path: String', 'program: String', 'arguments: Vec', 'method: String', 'native-smoke', 'TcpListener'):
            self.assertNotIn(forbidden, bridge)

    def test_only_verified_core_result_reaches_fixed_finder_operation_and_owned_join(self):
        bridge = (ROOT/'apps/desktop/src-tauri/src/history_export.rs').read_text()
        self.assertIn('native_parent::with_caller(window', bridge)
        self.assertIn('parent.finalize(', bridge)
        self.assertIn('history_export::published_path(', bridge)
        self.assertEqual(bridge.count('Command::new('), 1)
        self.assertIn('Command::new("/usr/bin/open")', bridge)
        self.assertIn('.arg("-R")', bridge)
        self.assertIn('.arg(path)', bridge)
        self.assertIn('child.try_wait()', bridge)
        self.assertIn('child.kill()', bridge)
        self.assertIn('child.wait()', bridge)
        self.assertNotIn('sh -c', bridge)

    def test_references_never_include_renderer_destination_or_program_authority(self):
        core = (ROOT/'crates/core/src/history_export.rs').read_text()
        reference = core.split('pub struct Reference {', 1)[1].split('}', 1)[0]
        self.assertEqual(re.findall(r'pub (\w+): String', reference),
                         ['preview_id', 'selection_digest', 'package_sha256'])
        self.assertIn('OFlags::NOFOLLOW', core)
        self.assertIn('OFlags::EXCL', core)
        self.assertIn('IDEMPOTENCY_CONFLICT', core)
        self.assertIn('PREVIEW_INCOMPLETE', core)
        self.assertIn('self.thread.take().unwrap().join()', core)


if __name__ == '__main__':
    unittest.main()
