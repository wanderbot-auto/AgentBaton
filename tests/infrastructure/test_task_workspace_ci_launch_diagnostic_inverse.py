"""Exact reviewed-source inverse controls; no workflow, tools or driver execution."""
from pathlib import Path
import hashlib
import unittest
from unittest import mock

from tests.infrastructure import test_schema17_migration_workflow as H

ROOT = Path(__file__).resolve().parents[2]
DEFINITION = '          def run_task_workspace_evidence():\n'
CALL = '          run_task_workspace_evidence()\n'


class TaskWorkspaceDiagnosticInverse(unittest.TestCase):
    def setUp(self):
        self.text = (ROOT / '.github/workflows/product.yml').read_text()
        self.start = self.text.index(DEFINITION)
        self.end = self.text.index(CALL)
        self.reviewed = self.text[self.start:self.end]
        self.fixture = ROOT / 'tests/infrastructure/fixtures/task_workspace_launch_diagnostics/pre_diagnostics_function.py.txt'

    def changed_function(self, old, new):
        self.assertIn(old, self.reviewed)
        return self.text[:self.start] + self.reviewed.replace(old, new, 1) + self.text[self.end:]

    def test_complete_reviewed_function_restores_exact_original_and_old_whole_pins(self):
        self.assertEqual(H.APPROVED_TASK_WORKSPACE_DIAGNOSTIC_SOURCE, '3dc7f0f231fd63dcf829cc7b1e6d0488b0743313')
        self.assertEqual(hashlib.sha256(self.reviewed.encode()).hexdigest(), '4cd9ae4b6ec58defa7ddfd693c9e567bfa972596ceb0500fbabea35a2b852bf9')
        raw = self.fixture.read_bytes()
        self.assertEqual(len(raw), 4189)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), '28da00e8807fd758282b8e9beebe005fd669daf8618b0ac41e77dda0d8e72b20')
        restored = H.strip_approved_task_workspace_launch_diagnostics(self.text)
        self.assertEqual(restored, self.text[:self.start] + raw.decode() + self.text[self.end:])
        self.assertEqual(H.BASELINE_PRODUCT_SHA256, '4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d')
        self.assertEqual(H.BROWSER_CI_BASE_SHA256, '599074c95afb9324470bc859f11added4b5232a877ba6e0486001e1bebf015fc')
        H.verify_only_six_historical_target_changes(self.text)

    def test_unreviewed_diagnostic_fields_types_and_publication_are_refused(self):
        for old, new in [
            ("'tool_role', 'tools', 'driver_exit', 'projection_exit'", "'tool_role', 'tools', 'driver_exit', 'projection_exit', 'path'"),
            ("'tool_role': None, 'tools'", "'tool_role': 'unreviewed', 'tools'"),
            ("'os_error', 'subprocess_error'", "'os_error', 'subprocess_error', 'exception_text'"),
            ("type(value[key]) is not int", "not isinstance(value[key], int)"),
            ("len(raw.encode()) > 4096", "len(raw.encode()) > 8192"),
            ("print(diagnostic_json(value), flush=True)", "print(value, flush=True)"),
            ("return False", "return True"),
        ]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(self.changed_function(old, new))

    def test_unreviewed_copy_permissions_identity_and_hash_changes_are_refused(self):
        for old, new in [
            ('before.st_size <= 134217728', 'before.st_size <= 268435456'),
            ('before.st_uid == os.getuid()', 'True'),
            ('not before.st_mode & 0o022', 'True'),
            ('os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK', 'os.O_RDONLY | os.O_NONBLOCK'),
            ('private.mkdir(mode=0o700, exist_ok=False)', 'private.mkdir(mode=0o755, exist_ok=True)'),
            ('copied != before.st_size', 'False'),
            ('stamp(before) != stamp(after)', 'False'),
            ("digest.hexdigest() != candidate['binaries'][name]", 'False'),
            ('os.fchmod(target.fileno(), 0o500)', 'os.fchmod(target.fileno(), 0o755)'),
        ]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(self.changed_function(old, new))

    def test_unreviewed_exit_guards_real_argv_or_failure_raise_are_refused(self):
        for old, new in [
            ('type(driver_exit) is not int or driver_exit != 0', 'driver_exit != 0'),
            ('type(projection_exit) is not int or projection_exit != 0', 'projection_exit != 0'),
            (' or not diagnostic_ok:', ':'),
            ("'--expected-source-head', candidate['git_head']", "'--expected-source-head', 'unreviewed'"),
            ("sys.executable, 'apps/desktop/tests/task-workspace-binding-real-ipc.py', '--run'", "sys.executable, 'apps/desktop/tests/task-workspace-binding-real-ipc.py', '--skip'"),
            ("raise RuntimeError('Task workspace driver or finite evidence projection failed')", 'return'),
        ]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(self.changed_function(old, new))

    def test_missing_duplicate_reordered_definition_or_call_is_refused(self):
        without = self.text[:self.start] + self.text[self.end:]
        for label, text in [
            ('missing definition', without),
            ('duplicate definition', self.text[:self.start] + self.reviewed + self.text[self.start:]),
            ('missing call', self.text.replace(CALL, '', 1)),
            ('duplicate call', self.text.replace(CALL, CALL + CALL, 1)),
            ('call before definition', CALL + self.text.replace(CALL, '', 1)),
        ]:
            with self.subTest(label=label), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(text)

    def test_complete_unchanged_function_and_call_cannot_move_or_hide_outside_edits(self):
        addition = self.reviewed + CALL
        self.assertEqual(self.text.count(addition), 1)
        without = self.text.replace(addition, '', 1)
        at = without.index('          candidate = {\n')
        moved = without[:at] + addition + without[at:]
        # Exact function bytes alone can be restored; the ORIGINAL full-workflow
        # pin must still reject the changed position and surrounding source.
        H.strip_approved_task_workspace_launch_diagnostics(moved)
        with self.assertRaises(ValueError):
            H.verify_only_six_historical_target_changes(moved)
        changed = self.text[:self.start] + '          unreviewed_outside_action = True\n' + self.text[self.start:]
        with self.assertRaises(ValueError):
            H.verify_only_six_historical_target_changes(changed)

    def test_unreviewed_raw_function_edits_and_pre_diagnostic_function_are_refused(self):
        for changed in [
            self.reviewed.replace('\n', '\r\n'),
            self.reviewed.replace('              import os\n', '              import os  # unreviewed byte\n', 1),
            self.reviewed.replace('run_task_workspace_evidence', 'unreviewed_task_workspace_evidence', 1),
            self.fixture.read_text(),
        ]:
            with self.subTest(digest=hashlib.sha256(changed.encode()).hexdigest()), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(self.text[:self.start] + changed + self.text[self.end:])

    def test_original_function_fixture_missing_changed_or_invalid_is_refused(self):
        for content in [b'', self.fixture.read_bytes() + b'\n', b'\xff' * 4189]:
            with self.subTest(size=len(content)), mock.patch.object(Path, 'read_bytes', return_value=content):
                with self.assertRaises(ValueError):
                    H.strip_approved_task_workspace_launch_diagnostics(self.text)
        with mock.patch.object(Path, 'read_bytes', side_effect=FileNotFoundError('private canary')):
            with self.assertRaisesRegex(ValueError, '^Pinned original TaskWorkspace function unavailable$'):
                H.strip_approved_task_workspace_launch_diagnostics(self.text)

    def test_original_permissions_actions_timeouts_checks_and_retention_stay_pinned(self):
        for old, new in [
            ('contents: read', 'contents: write'),
            ('timeout-minutes: 25', 'timeout-minutes: 26'),
            ('timeout-minutes: 20', 'timeout-minutes: 21'),
            ('timeout-minutes: 60', 'timeout-minutes: 61'),
            ('cancel-in-progress: true', 'cancel-in-progress: false'),
            ('actions/checkout@11d5960a326750d5838078e36cf38b85af677262', 'actions/checkout@01d5960a326750d5838078e36cf38b85af677262'),
            ('cargo test --workspace --locked', 'cargo test --workspace --locked -- --ignored'),
            ('if: always()', 'if: success()'),
            ('retention-days: 14', 'retention-days: 15'),
        ]:
            self.assertIn(old, self.text)
            with self.subTest(old=old), self.assertRaises(ValueError):
                H.verify_only_six_historical_target_changes(self.text.replace(old, new, 1))


if __name__ == '__main__':
    unittest.main()
