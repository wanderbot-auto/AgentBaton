"""Synthetic stage/projection and complete source inverse; no real processes."""
import copy
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import types
import unittest
from unittest import mock

from tests.infrastructure import test_integration_browser_ci as F
from tests.infrastructure import test_schema17_migration_workflow as W

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_integration_setup_projection', ROOT / 'scripts/record-integration-browser-ci-evidence.py')
E = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(E)
HEAD, SECRET = F.HEAD, F.SECRET
UNAVAILABLE = {'status': 'unavailable', 'failure_kind': 'missing_or_invalid_setup_diagnostic'}


def seed(private, stage='schema_probe', exception='value_error', driver_exit=None):
    private.mkdir(mode=0o700)
    position = E.SETUP_STAGES.index(stage)
    data = {'source': HEAD, 'stage': stage, 'exception_kind': exception,
        'browser_discovered': position > E.SETUP_STAGES.index('chromium_discovery'),
        'browser_pinned': position > E.SETUP_STAGES.index('chromium_pin'),
        'traceback_bytes': 0 if exception is None else 100}
    status = {'source': HEAD, 'driver_exit': driver_exit,
        'probe_passed': position > E.SETUP_STAGES.index('schema_probe')}
    for name, value in [('setup-diagnostic.json', data), ('status.json', status)]:
        path = private / name; path.write_text(json.dumps(value)); path.chmod(0o600)
    (private / 'setup-error.traceback.txt').write_text(SECRET)
    return data, status


class WrapperStageControls(unittest.TestCase):
    def invoke(self, code, root, runner, chromium, stage, error=None):
        module = F.driver(); private = runner / 'agentbaton-integration-browser-ci'
        code['git'] = lambda *args: HEAD if args == ('rev-parse', 'HEAD') else 'dirty' if stage == 'tracked_source_guard' else ''
        def source_driver():
            if stage == 'driver_import': raise ImportError(SECRET)
            if stage == 'paired_record_decode': module.decode = mock.Mock(side_effect=ValueError(SECRET))
            if stage in ('private_records', 'private_tools'):
                target = private / ('candidate.json' if stage == 'private_records' else 'tools')
                if stage == 'private_tools': target.mkdir(mode=0o700)
                else: target.write_text(SECRET); target.chmod(0o600)
            if stage == 'chromium_pin': module.pinned_file = mock.Mock(side_effect=ValueError(SECRET))
            if stage == 'driver_log_open':
                def pin(*args):
                    (private / 'driver.log').write_text(SECRET)
                module.pinned_file = pin
            return module
        code['source_driver'] = source_driver
        original_copy = code['copy_tool']
        def copy_tool(source, destination, expected):
            if stage == 'copy_' + destination.name: raise OSError(SECRET)
            return original_copy(source, destination, expected)
        code['copy_tool'] = copy_tool
        def probe(*args):
            if stage == 'schema_probe': raise error if error is not None else ValueError(SECRET)
        code['actual_schema_probe'] = probe
        def browser():
            if stage == 'chromium_discovery': raise OSError(SECRET)
            return chromium
        code['chromium_path'] = browser
        run = mock.Mock(side_effect=OSError(SECRET)) if stage == 'driver_run' else mock.Mock(return_value=types.SimpleNamespace(returncode=7))
        code['subprocess'] = types.SimpleNamespace(run=run, STDOUT=subprocess.STDOUT)
        if stage == 'paired_record_read': (root / 'artifacts/ui-browser/frontend-build-record.json').unlink()
        if stage == 'paired_metadata_guard':
            path = root / 'artifacts/ui-browser/goal-execution-candidate.json'
            value = json.loads(path.read_text()); value['dirty'] = True; path.write_text(json.dumps(value))
        result = code['run_integration_browser']()
        return result, private, run

    def test_each_setup_stage_is_observed_without_washing_failure_or_changing_status(self):
        for stage in E.SETUP_STAGES:
            with self.subTest(stage=stage), F.fixture() as (root, runner, chromium):
                code, _ = F.program(F.RUN, 'INTEGRATION_BROWSER')
                result, private, run = self.invoke(code, root, runner, chromium, stage)
                self.assertEqual(result, 1)
                value = E.setup_diagnostic(private, HEAD)
                self.assertEqual((value['status'], value['stage']), ('observed', stage))
                self.assertNotIn(SECRET, json.dumps(value)); self.assertNotIn(str(private), json.dumps(value))
                self.assertEqual(set(json.loads((private / 'status.json').read_text())), {'source', 'driver_exit', 'probe_passed'})
                self.assertEqual(value['driver_exit'], 7 if stage == 'driver_returned' else None)
                self.assertEqual(run.call_count, 1 if stage in ('driver_run', 'driver_returned') else 0)
                self.assertEqual((private / 'setup-diagnostic.json').stat().st_mode & 0o777, 0o600)

    def test_exception_classification_is_closed_and_never_carries_error_text(self):
        for error, tag in [(ImportError(SECRET), 'import_error'), (OSError(SECRET), 'os_error'),
            (ValueError(SECRET), 'value_error'), (KeyError(SECRET), 'key_error'),
            (TypeError(SECRET), 'type_error'), (AttributeError(SECRET), 'attribute_error'),
            (RuntimeError(SECRET), 'other_exception')]:
            with self.subTest(tag=tag), F.fixture() as (root, runner, chromium):
                code, _ = F.program(F.RUN, 'INTEGRATION_BROWSER')
                _, private, _ = self.invoke(code, root, runner, chromium, 'schema_probe', error)
                value = E.setup_diagnostic(private, HEAD)
                self.assertEqual(value['exception_kind'], tag)
                self.assertNotIn(SECRET, json.dumps(value))


class FiniteSetupProjection(unittest.TestCase):
    def test_failed_step_exports_only_closed_setup_fields_and_stays_failed(self):
        for stage in E.SETUP_STAGES:
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(strict=True); private = root / 'private'
                seed(private, stage, None if stage == 'driver_returned' else 'os_error', 1 if stage == 'driver_returned' else None)
                output = root / 'public'
                self.assertEqual(E.main(['--private', str(private), '--output', str(output),
                    '--expected-source-head', HEAD, '--driver-step-outcome', 'failure']), 1)
                self.assertEqual({p.name for p in output.iterdir()}, {'summary.json'})
                raw = (output / 'summary.json').read_text(); value = json.loads(raw)
                self.assertEqual(value['failure_kind'], 'driver_step_not_successful')
                self.assertEqual(value['setup_diagnostic']['stage'], stage)
                self.assertIs(value['screenshots_published'], False)
                self.assertNotIn(SECRET, raw); self.assertNotIn(str(private), raw)

    def test_alias_output_parent_refuses_without_publishing_even_with_valid_private_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            private = root / 'private'; seed(private)
            self.assertEqual(E.setup_diagnostic(private, HEAD)['status'], 'observed')
            parent = root / 'canonical-parent'; parent.mkdir(mode=0o700)
            alias = root / 'alias-parent'; alias.symlink_to(parent, target_is_directory=True)
            summary = E.failure(HEAD, 'driver_step_not_successful')
            # The same real parent publishes successfully through its canonical
            # name; an alias must remain refused before creating any output.
            E.publish(parent / 'control', summary, {})
            self.assertEqual({p.name for p in (parent / 'control').iterdir()}, {'summary.json'})
            with self.assertRaises(OSError) as refused:
                E.publish(alias / 'refused', summary, {})
            self.assertIn(refused.exception.errno, (errno.ENOTDIR, errno.ELOOP))
            with mock.patch.object(E.P, 'write_new', wraps=E.P.write_new) as written:
                self.assertEqual(E.main(['--private', str(private), '--output', str(alias / 'via-main'),
                    '--expected-source-head', HEAD, '--driver-step-outcome', 'failure']), 1)
                written.assert_not_called()
            self.assertFalse((parent / 'refused').exists())
            self.assertFalse((parent / 'via-main').exists())
            self.assertEqual({p.name for p in parent.iterdir()}, {'control'})

    def test_unknown_extra_nonfinite_wrong_source_bool_int_or_inconsistent_facts_are_unavailable(self):
        changes = [('setup-diagnostic.json', lambda d: d.update(stage=SECRET)),
            ('setup-diagnostic.json', lambda d: d.update(exception_kind=SECRET)),
            ('setup-diagnostic.json', lambda d: d.update(raw_traceback=SECRET)),
            ('setup-diagnostic.json', lambda d: d.update(source='f' * 40)),
            ('setup-diagnostic.json', lambda d: d.update(browser_pinned=True)),
            ('setup-diagnostic.json', lambda d: d.update(browser_discovered=0)),
            ('setup-diagnostic.json', lambda d: d.update(traceback_bytes=True)),
            ('setup-diagnostic.json', lambda d: d.update(traceback_bytes=16385)),
            ('setup-diagnostic.json', lambda d: d.update(traceback_bytes=float('nan'))),
            ('setup-diagnostic.json', lambda d: d.update(exception_kind=None)),
            ('status.json', lambda d: d.update(source='f' * 40)),
            ('status.json', lambda d: d.update(probe_passed=True)),
            ('status.json', lambda d: d.update(probe_passed=0)),
            ('status.json', lambda d: d.update(driver_exit=False)),
            ('status.json', lambda d: d.update(driver_exit=0)),
            ('status.json', lambda d: d.update(database=SECRET))]
        for name, change in changes:
            with self.subTest(name=name, change=change), tempfile.TemporaryDirectory() as temporary:
                private = Path(temporary).resolve(strict=True) / 'private'; seed(private)
                self.assertEqual(E.setup_diagnostic(private, HEAD)['status'], 'observed')
                self.assertEqual(E.setup_diagnostic(private, HEAD)['stage'], 'schema_probe')
                path = private / name; data = json.loads(path.read_text()); change(data); path.write_text(json.dumps(data))
                value = E.setup_diagnostic(private, HEAD)
                self.assertEqual(value, UNAVAILABLE); self.assertNotIn(SECRET, json.dumps(value))

    def test_missing_duplicate_symlink_hardlink_permissions_and_oversize_refuse(self):
        for kind in ('missing', 'duplicate', 'symlink', 'hardlink', 'permissions', 'oversize', 'root-mode'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(strict=True); private = root / 'private'; seed(private); path = private / 'setup-diagnostic.json'
                self.assertEqual(E.setup_diagnostic(private, HEAD)['status'], 'observed')
                self.assertEqual(E.setup_diagnostic(private, HEAD)['stage'], 'schema_probe')
                if kind == 'missing': path.unlink()
                elif kind == 'duplicate': path.write_text('{"source":"' + HEAD + '","source":"' + HEAD + '"}')
                elif kind == 'symlink':
                    outside = root / 'outside'; path.rename(outside); path.symlink_to(outside)
                elif kind == 'hardlink': os.link(path, root / 'alias')
                elif kind == 'permissions': path.chmod(0o644)
                elif kind == 'oversize': path.write_text(' ' * 4097)
                else: private.chmod(0o755)
                self.assertEqual(E.setup_diagnostic(private, HEAD), UNAVAILABLE)

    def test_bool_or_out_of_range_return_code_is_never_valid_observation(self):
        for exit_code in (False, True, 256, -256, None, '0'):
            with self.subTest(exit_code=exit_code), tempfile.TemporaryDirectory() as temporary:
                private = Path(temporary).resolve(strict=True) / 'private'; seed(private, 'driver_returned', None, 0)
                self.assertEqual(E.setup_diagnostic(private, HEAD)['status'], 'observed')
                self.assertEqual(E.setup_diagnostic(private, HEAD)['stage'], 'driver_returned')
                status_path = private / 'status.json'; status = json.loads(status_path.read_text())
                status['driver_exit'] = exit_code; status_path.write_text(json.dumps(status))
                self.assertEqual(E.setup_diagnostic(private, HEAD), UNAVAILABLE)


class ExactDiagnosticSourceInverse(unittest.TestCase):
    def test_complete_step_restores_exact7ee_and_all_older_whole_workflow_pins(self):
        text = F.WORKFLOW.read_text()
        self.assertEqual(hashlib.sha256(W.strip_integration_setup_diagnostics(text).encode()).hexdigest(),
            '2571ca962ee7eca8ef0b679b86bd939187d69427d83fce3d5ca18c885fc18d23')
        W.verify_only_six_historical_target_changes(text)
        original = (ROOT / 'tests/infrastructure/fixtures/integration_setup_diagnostics/pre_diagnostics_step.yml.txt').read_text()
        changed = text[text.index('      - name: ' + F.RUN):text.index('      - name: ' + F.PROJECT)]
        # All six original helpers, original arguments/guards and old status
        # statement retain exact original bytes; only runner diagnostics change.
        import ast
        def program(block):
            source = block.split("          artifacts/browser-venv/bin/python - <<'INTEGRATION_BROWSER'\n", 1)[1].split('          INTEGRATION_BROWSER\n', 1)[0]
            import textwrap
            source = textwrap.dedent(source)
            return {n.name: ast.get_source_segment(source, n) for n in ast.parse(source).body if isinstance(n, ast.FunctionDef)}
        before, after = program(original), program(changed)
        for name in ('git', 'chromium_path', 'private_json', 'source_driver', 'copy_tool', 'actual_schema_probe'):
            self.assertEqual(before[name], after[name])

    def test_exact_complete_flagged_step_restores765_driver_before_its_original_inverse(self):
        text = F.WORKFLOW.read_text()
        flagged = text.replace(W.INTEGRATION_SETUP_NOTIFICATION_ANCHOR,
            W.INTEGRATION_SETUP_NOTIFICATION_FLAG + W.INTEGRATION_SETUP_NOTIFICATION_ANCHOR, 1)
        restored = W.strip_integration_setup_diagnostics(flagged)
        original = W.strip_integration_setup_diagnostics(text)
        self.assertEqual(restored, original.replace(W.INTEGRATION_SETUP_NOTIFICATION_ANCHOR,
            W.INTEGRATION_SETUP_NOTIFICATION_FLAG + W.INTEGRATION_SETUP_NOTIFICATION_ANCHOR, 1))
        headers = list(re.finditer(r'^      - .*\n', restored, re.M))
        block = restored[headers[27].start():headers[28].start()]
        self.assertEqual(hashlib.sha256(block.encode()).hexdigest(),
            'df3ddde5681355b95ae69e79bdaae674a6531f68c438102007dde0ca82d5727d')
        self.assertEqual(W.BROWSER_CI_STEPS[0][2], 'a1db5e7447f25ac6dd8f7a3fff8005e071971cb75c0aee109ace816b23ece680')
        for old, new in [("'--notification-probe',", "'--notification-probe', '--unknown',"),
            ("'--notification-probe',", "'--notification-probe', '--notification-probe',"),
            ("diagnostic['stage'] = 'schema_probe'", "diagnostic['stage'] = 'driver_run'")]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                W.strip_integration_setup_diagnostics(flagged.replace(old, new, 1))

    def test_any_diagnostic_guard_outside_edit_move_missing_duplicate_or_fixture_corruption_refuses(self):
        text = F.WORKFLOW.read_text()
        changes = [("'exception_kind': None", "'exception_kind': 'unknown'"),
            ("diagnostic['stage'] = 'schema_probe'", "diagnostic['stage'] = 'driver_returned'"),
            ("'traceback_bytes': 0", "'traceback_bytes': True"),
            ('0 < before.st_size <= 268435456', '0 < before.st_size <= 536870912'),
            ('contents: read', 'contents: write'), ('timeout-minutes: 20', 'timeout-minutes: 21'),
            ('134217728', '268435456'), ("'--case', 'all'", "'--case', 'normal'"),
            ("private_json(private / 'setup-diagnostic.json', diagnostic)", 'print(diagnostic)')]
        for old, new in changes:
            self.assertIn(old, text)
            with self.subTest(old=old), self.assertRaises(ValueError):
                W.verify_only_six_historical_target_changes(text.replace(old, new, 1))
        headers = list(re.finditer(r'^      - .*\n', text, re.M)); start, end = headers[27].start(), headers[28].start(); block = text[start:end]
        without = text[:start] + text[end:]
        for changed in (without, text[:start] + block + text[start:],
            text[:start] + block.replace('\n', '\r\n') + text[end:],
            without.replace('      - name: Real daemon and browser test dependencies\n', block + '      - name: Real daemon and browser test dependencies\n')):
            with self.assertRaises(ValueError): W.verify_only_six_historical_target_changes(changed)
        with mock.patch.object(Path, 'read_bytes', return_value=b'corrupted'):
            with self.assertRaises(ValueError): W.strip_integration_setup_diagnostics(text)


if __name__ == '__main__':
    unittest.main()
