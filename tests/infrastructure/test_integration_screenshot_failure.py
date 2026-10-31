"""Closed failed-shot observations with synthetic faults; no browser or IPC."""
import ast
import hashlib
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

from tests.infrastructure import integration_screenshot_failure_fixture as I
from tests.infrastructure import notification_screenshot_composition_fixture as C
from tests.infrastructure import test_integration_browser_ci_evidence_runtime as R

ROOT = Path(__file__).resolve().parents[2]
STEPS = ('scroll', 'viewport', 'bounding_box', 'capture', 'record')
E, F = R.E, R.F


class ActualShotOperations(unittest.TestCase):
    def shot(self, folder, report, failed):
        source = (ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_text()
        tree = ast.parse(source)
        node = next(n for n in ast.walk(tree) if isinstance(n, ast.FunctionDef) and n.name == 'shot')
        def operation(step, result=None):
            if failed == step:
                raise AssertionError(F.SECRET)
            return result
        focus = types.SimpleNamespace(scroll_into_view_if_needed=lambda: operation('scroll'),
            bounding_box=lambda: operation('bounding_box', {'x': 0, 'y': 0, 'width': 10, 'height': 10}))
        def screenshot(*, path):
            operation('capture'); Path(path).write_bytes(b'synthetic image bytes')
        namespace = {'report': report, 'output': folder,
            'expect': lambda _: types.SimpleNamespace(to_be_in_viewport=lambda **kw: operation('viewport')),
            'page': types.SimpleNamespace(screenshot=screenshot, viewport_size={'width': 1440, 'height': 1100}),
            'digest': lambda raw: operation('record', hashlib.sha256(raw).hexdigest())}
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-shot-function>', 'exec'), namespace)
        return namespace['shot'], focus

    def test_each_original_operation_retains_only_its_fixed_active_name_and_step(self):
        for name in E.NAMES:
            for step in STEPS:
                with self.subTest(name=name, step=step), tempfile.TemporaryDirectory() as folder:
                    report = {'screenshots': []}
                    shot, focus = self.shot(Path(folder), report, step)
                    with self.assertRaisesRegex(AssertionError, F.SECRET): shot(name, focus)
                    self.assertEqual(report, {'screenshots': [], 'active_screenshot': {'name': name, 'step': step}})
                    self.assertNotIn(F.SECRET, json.dumps(report))

    def test_success_deletes_observation_and_preserves_original_screenshot_shape(self):
        with tempfile.TemporaryDirectory() as folder:
            report = {'screenshots': []}
            shot, focus = self.shot(Path(folder), report, None)
            shot(E.NAMES[0], focus)
            self.assertNotIn('active_screenshot', report)
            row = report['screenshots'][0]
            self.assertEqual(set(row), {'name', 'sha256', 'bytes', 'viewport', 'focus_box_before_capture_observation',
                'focus_viewport_asserted_before_capture', 'pixel_synchronized', 'dimension'})
            self.assertEqual(row['sha256'], hashlib.sha256(b'synthetic image bytes').hexdigest())
            self.assertIs(row['focus_viewport_asserted_before_capture'], True)
            self.assertIs(row['pixel_synchronized'], False)


class FiniteScreenshotProjection(unittest.TestCase):
    def change(self, private, fn):
        path = private / 'driver/normal/report.json'
        report = json.loads(path.read_text()); fn(report); F.write(path, report)

    def seed(self, folder, index=0, step='viewport'):
        private = R.failed_seed(folder)
        # Preserve generated original rows, selecting only a completed prefix.
        (folder / 'original').mkdir(mode=0o700)
        original = F.seed(folder / 'original')
        rows = json.loads((original / 'driver/normal/report.json').read_text())['screenshots']
        def change(report):
            report['screenshots'] = rows[:index]
            report['failure'] = {'stage': 'private_independent_review' if index < 2 else 'real_owner_acceptance_and_watcher', 'kind': 'AssertionError'}
            report['active_screenshot'] = {'name': E.NAMES[index], 'step': step}
        self.change(private, change)
        return private

    def test_every_closed_name_step_projects_no_png_or_private_exception(self):
        for index, name in enumerate(E.NAMES):
            for step in STEPS:
                with self.subTest(name=name, step=step), tempfile.TemporaryDirectory() as folder:
                    private = self.seed(Path(folder).resolve(), index, step)
                    with mock.patch.object(E, 'screenshots', side_effect=AssertionError('No PNG open')):
                        value = E.runtime_failure_diagnostic(private, F.HEAD)
                    self.assertEqual(value['status'], 'observed')
                    self.assertEqual(value['cases'][0]['screenshot_observation'],
                        {'name': name, 'step': step, 'completed_screenshots': index})
                    self.assertNotIn(F.SECRET, json.dumps(value))
                    self.assertNotIn(str(private), json.dumps(value))

    def test_original_failed_report_without_active_shot_is_observation_with_null_detail(self):
        with tempfile.TemporaryDirectory() as folder:
            value = E.runtime_failure_diagnostic(R.failed_seed(Path(folder).resolve()), F.HEAD)
            self.assertEqual(value['status'], 'observed')
            self.assertIsNone(value['cases'][0]['screenshot_observation'])
            self.assertIsNone(value['cases'][1]['screenshot_observation'])

    def test_unknown_private_wrong_prefix_or_success_detail_is_unavailable(self):
        mutations = [lambda r: r['active_screenshot'].update(name=F.SECRET),
            lambda r: r['active_screenshot'].update(step=F.SECRET),
            lambda r: r['active_screenshot'].update(extra=F.SECRET),
            lambda r: r['active_screenshot'].update(name=E.NAMES[1]),
            lambda r: r.update(active_screenshot=None),
            lambda r: r.update(active_screenshot=[]),
            lambda r: r.update(active_screenshot={'name': True, 'step': 'scroll'}),
            lambda r: r.update(active_screenshot={'name': E.NAMES[0], 'step': True}),
            lambda r: r['failure'].update(stage='actual_frontend'),
            lambda r: r.update(screenshots=[{'name': E.NAMES[0]}]),
            lambda r: r.update(screenshots=[{'name': F.SECRET}]),
            lambda r: r.update(screenshots=[{'name': n} for n in E.NAMES])]
        for index, change in enumerate(mutations):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as folder:
                private = self.seed(Path(folder).resolve()); self.change(private, change)
                value = E.runtime_failure_diagnostic(private, F.HEAD)
                self.assertEqual(value, {'status': 'unavailable', 'failure_kind': 'missing_or_invalid_runtime_failure_diagnostic'})
                self.assertNotIn(F.SECRET, json.dumps(value))
        with tempfile.TemporaryDirectory() as folder:
            private = self.seed(Path(folder).resolve())
            path = private / 'driver/command-failure/report.json'; report = json.loads(path.read_text())
            report['active_screenshot'] = {'name': E.NAMES[0], 'step': 'capture'}; F.write(path, report)
            self.assertEqual(E.runtime_failure_diagnostic(private, F.HEAD)['status'], 'unavailable')

    def test_failure_publication_stays_nonzero_and_summary_only(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve(); private = self.seed(root)
            self.assertEqual(E.main(['--private', str(private), '--output', str(root / 'public'),
                '--expected-source-head', F.HEAD, '--driver-step-outcome', 'failure']), 1)
            self.assertEqual([p.name for p in (root / 'public').iterdir()], ['summary.json'])
            value = json.loads((root / 'public/summary.json').read_text())
            self.assertFalse(value['screenshots_published'])
            self.assertFalse(value['runtime_failure']['runtime_acceptance'])


def strip_driver(text):
    return I.strip_screenshot_driver_layer(C.strip_composed_notification_layer(text))


class CompleteOldSourceInverse(unittest.TestCase):
    def test_exact_complete_sources_restore_original_e509_bytes_and_old_pins(self):
        for path, strip, before in [
            ('apps/desktop/tests/integration-real-ipc.py', strip_driver, I.DRIVER_BEFORE_SHA256),
            ('scripts/record-integration-browser-ci-evidence.py', I.strip_screenshot_projector_layer, I.PROJECTOR_BEFORE_SHA256)]:
            source = (ROOT / path).read_text(); restored = strip(source)
            self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), before)
            self.assertEqual(strip(restored), restored)
        original = R.strip_runtime_failure_projection((ROOT / 'scripts/record-integration-browser-ci-evidence.py').read_text())
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(), R.RUNTIME_BASE_PROJECTOR_SHA256)

    def test_inserted_and_original_unknown_changes_are_not_normalized(self):
        pairs = [('apps/desktop/tests/integration-real-ipc.py', strip_driver, I.DRIVER_EDITS, AssertionError),
            ('scripts/record-integration-browser-ci-evidence.py', I.strip_screenshot_projector_layer, I.PROJECTOR_EDITS, ValueError)]
        for path, strip, edits, error in pairs:
            text = (ROOT / path).read_text()
            mutations = [text + '\n# unreviewed outside change\n', text.replace('= 536870912', '= 1073741824', 1) if path.startswith('apps/') else text.replace('MAX_REPORT, MAX_STATUS', 'CHANGED_REPORT, MAX_STATUS', 1)]
            for _, new in edits:
                mutations.extend([text.replace(new, '', 1), text.replace(new, new * 2, 1),
                    text.replace(new, new.replace('\n', '\r\n'), 1)])
            for changed in mutations:
                self.assertNotEqual(changed, text)
                with self.subTest(path=path), self.assertRaises(error): strip(changed)


if __name__ == '__main__':
    unittest.main()
