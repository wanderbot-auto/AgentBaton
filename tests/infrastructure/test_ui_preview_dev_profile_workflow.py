"""Source-only UI preview build-profile controls; never run Cargo or the driver."""
import ast
import hashlib
from pathlib import Path
import re
import shlex
import textwrap
import unittest

import yaml
from tests.infrastructure import test_schema17_migration_workflow as H

ROOT = Path(__file__).resolve().parents[2]


class UiPreviewDevProfileWorkflow(unittest.TestCase):
    def setUp(self):
        self.text = (ROOT / '.github/workflows/product.yml').read_text()
        self.headers = list(re.finditer(r'^      - .*\n', self.text, re.MULTILINE))
        self.build_start, self.build_end = self.headers[25].start(), self.headers[26].start()
        self.build = self.text[self.build_start:self.build_end]
        self.candidate_start = self.text.index('          candidate = {\n')
        self.candidate_end = self.text.index('          async def chromium_executable_path():\n')
        self.candidate = self.text[self.candidate_start:self.candidate_end]

    def changed_build(self, old, new):
        self.assertIn(old, self.build)
        return self.text[:self.build_start] + self.build.replace(old, new, 1) + self.text[self.build_end:]

    def reject(self, text):
        with self.assertRaises(ValueError):
            H.verify_only_six_historical_target_changes(text)

    def test_only_ui_preview_build_gets_inline_debuginfo_override(self):
        document = yaml.load(self.text, Loader=yaml.BaseLoader)
        step = next(s for s in document['jobs']['frontend']['steps'] if s.get('name') == 'Real daemon and browser test dependencies')
        self.assertNotIn('env', step)
        self.assertNotIn('CARGO_PROFILE_DEV_DEBUG', step['run'].splitlines()[2])
        self.assertEqual(step['run'].splitlines()[2], 'cargo test --locked -p agentbaton-core --features preview-evidence --lib evidence_gate -- --nocapture --test-threads=1')
        self.assertEqual(shlex.split(step['run'].splitlines()[3]), [
            'CARGO_PROFILE_DEV_DEBUG=0', 'cargo', 'build', '--locked', '--bin', 'batond', '--bin', 'baton',
            '--features', 'agentbaton-core/preview-evidence'])
        self.assertEqual(self.text.count(H.UI_PREVIEW_BUILD_AFTER), 1)
        self.assertNotIn('export CARGO_PROFILE_DEV_DEBUG', self.text)
        for lane in ('core', 'desktop'):
            self.assertNotIn('CARGO_PROFILE_DEV_DEBUG', str(document['jobs'][lane]))
        # Preserve the original dependency-specific optimization override; the
        # scope does not claim that every dependency is unoptimized.
        self.assertIn('[profile.dev.package.sha2]\nopt-level = 3', (ROOT / 'Cargo.toml').read_text())

    def test_existing_closed_candidate_keys_preserve_scope_profile_provenance(self):
        tree = ast.parse(textwrap.dedent(self.candidate))
        candidate = next(n.value for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'candidate' for t in n.targets))
        keys = [k.value for k in candidate.keys]
        self.assertEqual(keys, ['schema', 'git_head', 'git_tree', 'dirty', 'cargo_features', 'binaries', 'scope'])
        scope = candidate.values[keys.index('scope')]
        self.assertEqual(scope.value, H.UI_PREVIEW_SCOPE_AFTER)
        self.assertIn('CARGO_PROFILE_DEV_DEBUG=0 (debuginfo=0)', scope.value)
        self.assertIn('dev opt-level/debug_assertions/panic defaults and existing package overrides are unchanged', scope.value)
        self.assertIn('not a release artifact', scope.value)
        self.assertNotIn('full DWARF', scope.value)
        self.assertEqual(ast.literal_eval(candidate.values[keys.index('cargo_features')]), ['agentbaton-core/preview-evidence'])

    def test_exact_two_line_inverse_keeps_all_older_complete_source_pins(self):
        self.assertEqual(H.APPROVED_UI_PREVIEW_PROFILE_BASE, 'fbd5b3103794cc163ae91d9f3407fe2dfb90ea49')
        restored = H.strip_approved_ui_preview_profile(self.text)
        self.assertEqual(restored, H.strip_private_browser_python(self.text).replace(H.UI_PREVIEW_BUILD_AFTER, H.UI_PREVIEW_BUILD_BEFORE, 1).replace(H.UI_PREVIEW_SCOPE_AFTER, H.UI_PREVIEW_SCOPE_BEFORE, 1))
        self.assertEqual(H.BASELINE_PRODUCT_SHA256, '4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d')
        self.assertEqual(H.BROWSER_CI_BASE_SHA256, '599074c95afb9324470bc859f11added4b5232a877ba6e0486001e1bebf015fc')
        self.assertEqual(H.APPROVED_TASK_WORKSPACE_DIAGNOSTIC_FUNCTION_SHA256, '4cd9ae4b6ec58defa7ddfd693c9e567bfa972596ceb0500fbabea35a2b852bf9')
        H.verify_only_six_historical_target_changes(self.text)

    def test_unreviewed_debug_optimization_assertions_panic_or_rustflags_refused(self):
        for replacement in [
            'CARGO_PROFILE_DEV_DEBUG=1 cargo build',
            'CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_OPT_LEVEL=3 cargo build',
            'CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false cargo build',
            'CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_PANIC=abort cargo build',
            'CARGO_PROFILE_DEV_DEBUG=0 RUSTFLAGS=-Cdebug-assertions=off cargo build',
            'cargo build',
            'export CARGO_PROFILE_DEV_DEBUG=0\n          cargo build',
        ]:
            with self.subTest(replacement=replacement):
                self.reject(self.changed_build('CARGO_PROFILE_DEV_DEBUG=0 cargo build', replacement))

    def test_same_step_gate_features_locked_flags_and_later_dependencies_remain_exact(self):
        for old, new in [
            ('cargo test --locked -p agentbaton-core', 'CARGO_PROFILE_DEV_DEBUG=0 cargo test --locked -p agentbaton-core'),
            ('--features agentbaton-core/preview-evidence', '--features agentbaton-core/preview-evidence,other'),
            ('cargo build --locked', 'cargo build'),
            ('rustup toolchain install stable --profile minimal', 'rustup toolchain install nightly --profile minimal'),
            ('--require-hashes -r requirements-browser.txt', '-r requirements-browser.txt'),
        ]:
            with self.subTest(old=old):
                self.reject(self.changed_build(old, new))

    def test_candidate_unknown_keys_wrong_features_or_misleading_scope_are_refused(self):
        for old, new in [
            ("'dirty': False,", "'dirty': False, 'profile': 'unreviewed',"),
            ("'cargo_features': ['agentbaton-core/preview-evidence']", "'cargo_features': []"),
            ('(debuginfo=0)', '(full DWARF)'),
            ('existing package overrides are unchanged', 'all dependencies use opt-level=0'),
        ]:
            self.assertIn(old, self.candidate)
            with self.subTest(old=old):
                self.reject(self.text[:self.candidate_start] + self.candidate.replace(old, new, 1) + self.text[self.candidate_end:])

    def test_duplicate_missing_moved_or_raw_edited_build_step_is_refused(self):
        without = self.text[:self.build_start] + self.text[self.build_end:]
        move = without.index('      - name: Preview transport timing and failure contracts\n')
        variants = [
            without,
            self.text[:self.build_start] + self.build + self.text[self.build_start:],
            without[:move] + self.build + without[move:],
            self.text[:self.build_start] + self.build.replace('\n', '\r\n') + self.text[self.build_end:],
            self.changed_build(H.UI_PREVIEW_BUILD_AFTER.strip(), H.UI_PREVIEW_BUILD_AFTER.strip() + '\n' + H.UI_PREVIEW_BUILD_AFTER.rstrip()),
        ]
        for index, text in enumerate(variants):
            with self.subTest(index=index):
                self.reject(text)

    def test_candidate_missing_duplicate_moved_or_raw_bytes_changed_is_refused(self):
        # The pinned candidate range ends before the following async definition.
        without = self.text[:self.candidate_start] + self.text[self.candidate_end:]
        move = without.index('          def run_task_workspace_evidence():\n')
        for index, text in enumerate([
            without,
            self.text[:self.candidate_start] + self.candidate + self.text[self.candidate_start:],
            without[:move] + self.candidate + without[move:],
            self.text[:self.candidate_start] + self.candidate.replace('\n', '\r\n') + self.text[self.candidate_end:],
        ]):
            with self.subTest(index=index):
                self.reject(text)

    def test_original_taskworkspace_size_owner_identity_hash_and_exit_guards_stay_pinned(self):
        start = self.text.index('          def run_task_workspace_evidence():\n')
        end = self.text.index('          run_task_workspace_evidence()\n')
        diagnostic = self.text[start:end]
        for old, new in [
            ('before.st_size <= 134217728', 'before.st_size <= 268435456'),
            ('before.st_uid == os.getuid()', 'True'),
            ('not before.st_mode & 0o022', 'True'),
            ('stamp(before) != stamp(after)', 'False'),
            ("digest.hexdigest() != candidate['binaries'][name]", 'False'),
            ('type(driver_exit) is not int or driver_exit != 0', 'driver_exit != 0'),
        ]:
            self.assertIn(old, diagnostic)
            with self.subTest(old=old):
                self.reject(self.text[:start] + diagnostic.replace(old, new, 1) + self.text[end:])

    def test_other_lanes_release_checks_actions_limits_and_permissions_unchanged(self):
        for old, new in [
            ('cargo test --workspace --locked', 'CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace --locked'),
            ('cargo build --release --locked --bin batond --bin baton', 'CARGO_PROFILE_DEV_DEBUG=0 cargo build --release --locked --bin batond --bin baton'),
            ('contents: read', 'contents: write'),
            ('timeout-minutes: 20', 'timeout-minutes: 21'),
            ('timeout-minutes: 60', 'timeout-minutes: 61'),
            ('cancel-in-progress: true', 'cancel-in-progress: false'),
            ('actions/checkout@11d5960a326750d5838078e36cf38b85af677262', 'actions/checkout@01d5960a326750d5838078e36cf38b85af677262'),
        ]:
            self.assertIn(old, self.text)
            with self.subTest(old=old):
                self.reject(self.text.replace(old, new, 1))


if __name__ == '__main__':
    unittest.main()
