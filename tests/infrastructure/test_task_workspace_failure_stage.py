"""Synthetic closed failure-stage observations; no Core/browser or Provider run."""
import ast
import hashlib
import json
from pathlib import Path
import unittest
from unittest import mock

from tests.infrastructure import test_task_workspace_ci_evidence as F

ROOT = Path(__file__).resolve().parents[2]
P = F.P
SOURCE_BEFORE_SHA256 = 'd30279722e2967da93bb52d62c2db4c03a04e83c4287cebb517e15c629244f61'
SOURCE_AFTER_SHA256 = '98481eb1cc07fd08c2476a27a734d0ba66e28fdfa612f03edb57e76fcaf05b36'
LAYER_SHA256 = '882a21ca6c44d76e8241e41cd4dab1e55b83fbd40cdd4a08568905e20d1f421d'
LAYER_MARKER = '# Closed observations only; these do not establish a passing driver or cleanup.\n'
CALL_ADDITION = "    if not complete:\n        observation = failure_observation(report, len(passed))\n        if observation is not None:\n            result['runtime_failure'] = observation\n"


def strip_failure_stage_layer(text):
    digest = hashlib.sha256(text.encode()).hexdigest()
    if digest == SOURCE_BEFORE_SHA256:
        return text
    if digest != SOURCE_AFTER_SHA256 or text.count(LAYER_MARKER) != 1 or text.count(CALL_ADDITION) != 1:
        raise ValueError('Unrecognized complete failure observation source')
    start, end = text.index(LAYER_MARKER), text.index('def png_pixels(')
    block = text[start:end]
    if hashlib.sha256(block.encode()).hexdigest() != LAYER_SHA256:
        raise ValueError('Failure observation block changed')
    restored = (text[:start] + text[end:]).replace(CALL_ADDITION, '', 1)
    if hashlib.sha256(restored.encode()).hexdigest() != SOURCE_BEFORE_SHA256:
        raise ValueError('Failure observation inverse did not restore all original bytes')
    return restored


def failed_report(completed=0):
    value = F.report()
    value.update(status='failed', flow_complete=False,
        failure={'stage': 'normal_first_task', 'kind': 'ValueError'})
    value['checks'] = value['checks'][:completed]
    value['scenarios'] = {name: 'passed' if index < completed else 'NOT_RUN'
                          for index, name in enumerate(P.CASES)}
    return value


class FailureStageProjection(unittest.TestCase):
    write = F.TaskWorkspaceProjectionTests.write
    run_projection = F.TaskWorkspaceProjectionTests.run_projection
    assert_no_images = F.TaskWorkspaceProjectionTests.assert_no_images

    def setUp(self):
        F.TaskWorkspaceProjectionTests.setUp(self)

    def test_each_existing_driver_stage_and_safe_kind_is_a_closed_observation(self):
        for stage in P.FAILURE_STAGES:
            for kind in P.FAILURE_KINDS:
                with self.subTest(stage=stage, kind=kind):
                    value = failed_report(); value['failure'] = {'stage': stage, 'kind': kind}
                    result = P.project(value, 'f' * 64, F.HEAD, F.HARNESS)
                    self.assertEqual(result['runtime_failure'], {'stage': stage,
                        'exception_kind': kind, 'completed_cases': 0, 'observation_only': True})
                    self.assertTrue(result['metadata_only'])
                    self.assertEqual(result['status'], 'failed')
                    self.assertFalse(result['screenshots_published'])
                    self.assertNotIn(F.SECRET, json.dumps(result))

    def test_zero_through_six_completed_cases_never_become_test_pass_or_pngs(self):
        for completed in range(7):
            with self.subTest(completed=completed):
                self.write(failed_report(completed))
                with mock.patch.object(P, 'screenshots', side_effect=AssertionError('Do not open PNG')):
                    code, result, out = self.run_projection()
                self.assertEqual(code, 0)  # Original projector semantics, not the driver exit.
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(result['failure_kind'], 'driver_failed_or_incomplete')
                self.assertEqual(result['runtime_failure']['completed_cases'], completed)
                self.assert_no_images(out)
                self.assertEqual({p.name for p in out.iterdir()}, {'summary.json'})

    def test_missing_unknown_or_private_failure_is_omitted_without_changing_exit(self):
        variants = [None, {}, [], F.SECRET, {'stage': F.SECRET, 'kind': 'ValueError'},
            {'stage': 'normal_first_task', 'kind': F.SECRET},
            {'stage': 'normal_first_task', 'kind': 'ValueError', 'message': F.SECRET},
            {'stage': True, 'kind': 'ValueError'}, {'stage': 'normal_first_task', 'kind': False},
            {'stage': ['normal_first_task'], 'kind': 'ValueError'},
            {'stage': 'normal_first_task', 'kind': {'traceback': F.SECRET}},
            {'stage': 'normal_first_task'}, {'kind': 'ValueError'}]
        for failure in variants:
            with self.subTest(failure=failure):
                value = failed_report(); value['failure'] = failure; self.write(value)
                code, result, out = self.run_projection()
                self.assertEqual(code, 0)
                self.assertEqual(result['status'], 'failed')
                self.assertNotIn('runtime_failure', result)
                self.assertNotIn(F.SECRET, json.dumps(result)); self.assert_no_images(out)
        value = failed_report(); value.pop('failure'); self.write(value)
        self.assertNotIn('runtime_failure', self.run_projection()[1])

    def test_original_success_summary_and_all_original_images_remain_identical(self):
        value = self.write(); code, result, out = self.run_projection()
        old_text = strip_failure_stage_layer((ROOT / 'scripts/record-task-workspace-ci-evidence.py').read_text())
        old_project = next(n for n in ast.parse(old_text).body if isinstance(n, ast.FunctionDef) and n.name == 'project')
        namespace = dict(vars(P)); exec(compile(ast.Module(body=[old_project], type_ignores=[]), '<old-project>', 'exec'), namespace)
        expected = namespace['project'](value, result['report_sha256'], F.HEAD, F.HARNESS)
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'passed')
        self.assertNotIn('runtime_failure', result)
        result_without_images = {k: v for k, v in result.items() if k != 'screenshots'}
        result_without_images['screenshots_published'] = False
        self.assertEqual(result_without_images, expected)
        self.assertEqual({p.name for p in out.iterdir()}, {'summary.json', *P.PNG_NAMES})
        for name in P.PNG_NAMES: self.assertEqual((out / name).read_bytes(), (self.private / name).read_bytes())

    def test_failed_or_interrupted_status_is_supported_but_incomplete_passed_gets_no_claim(self):
        for status in ['failed_or_interrupted', 'passed']:
            value = failed_report(); value['status'] = status; self.write(value)
            code, result, out = self.run_projection()
            self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)
            self.assertEqual('runtime_failure' in result, status == 'failed_or_interrupted')

    def test_original_source_harness_and_case_guards_admit_no_failure_detail(self):
        mutations = [lambda r: r['inputs'].update(expected_source_head='0' * 40),
            lambda r: r['inputs'].update(compiled_core_source_head='0' * 40),
            lambda r: r['inputs'].update(frontend_build_head='0' * 40),
            lambda r: r.update(harness_sha256='0' * 64),
            lambda r: r['inputs']['tools']['baton'].update(mode=0o777),
            lambda r: r['inputs'].update(manifest_sha256=F.SECRET),
            lambda r: r['inputs'].update(frontend_build_record_sha256=F.SECRET),
            lambda r: r['checks'].append({'id': F.SECRET, 'status': 'passed'}),
            lambda r: r['scenarios'].update({'WSI-B01': 'passed'})]
        for change in mutations:
            value = failed_report(); change(value); self.write(value)
            code, result, out = self.run_projection()
            self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid')
            self.assertNotIn('runtime_failure', result); self.assertNotIn(F.SECRET, json.dumps(result)); self.assert_no_images(out)

    def test_report_replacement_and_permission_guards_still_refuse_before_publication(self):
        self.write(failed_report()); target = self.private / 'report.json'; before = target.read_bytes()
        original = P.regular_bytes; seen = False
        def replace(descriptor, name, *args, **kwargs):
            nonlocal seen
            raw = original(descriptor, name, *args, **kwargs)
            if name == 'report.json' and not seen:
                seen = True; target.write_bytes(before + b' ')
            return raw
        with mock.patch.object(P, 'regular_bytes', side_effect=replace): code, result, out = self.run_projection()
        self.assertEqual(code, 1); self.assertNotIn('runtime_failure', result); self.assert_no_images(out)
        self.write(failed_report()); target.chmod(0o644)
        code, result, out = self.run_projection(); self.assertEqual(code, 1)
        self.assertNotIn('runtime_failure', result); self.assert_no_images(out)


class ExactStageSource(unittest.TestCase):
    def test_stage_allowlist_matches_actual_driver_fixed_assignments_and_cleanup_names(self):
        tree = ast.parse((ROOT / 'apps/desktop/tests/task-workspace-binding-real-ipc.py').read_text())
        stages = {n.value.value for n in ast.walk(tree) if isinstance(n, ast.Assign)
                  and any(isinstance(t, ast.Name) and t.id == 'stage' for t in n.targets)
                  and isinstance(n.value, ast.Constant) and isinstance(n.value.value, str)}
        cleanup = {n.args[0].value for n in ast.walk(tree) if isinstance(n, ast.Call)
                   and isinstance(n.func, ast.Name) and n.func.id == 'cleanup_action'
                   and n.args and isinstance(n.args[0], ast.Constant) and isinstance(n.args[0].value, str)}
        self.assertEqual(set(P.FAILURE_STAGES), stages | cleanup | {'input_hashes_changed'})
        self.assertEqual(len(P.FAILURE_STAGES), len(set(P.FAILURE_STAGES)))
        self.assertEqual(len(P.FAILURE_KINDS), len(set(P.FAILURE_KINDS)))

    def test_complete_inverse_retains_original_source_and_every_old_function(self):
        source = (ROOT / 'scripts/record-task-workspace-ci-evidence.py').read_text()
        restored = strip_failure_stage_layer(source)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), SOURCE_BEFORE_SHA256)
        self.assertEqual(strip_failure_stage_layer(restored), restored)
        def functions(text): return {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body if isinstance(n, ast.FunctionDef)}
        old, new = functions(restored), functions(source)
        self.assertEqual([name for name in old if old[name] != new[name]], ['project'])
        self.assertEqual(new['project'].replace(CALL_ADDITION, '', 1), old['project'])
        self.assertEqual(set(new) - set(old), {'failure_observation'})

    def test_unknown_layer_original_guard_and_outside_bytes_cannot_be_normalized(self):
        text = (ROOT / 'scripts/record-task-workspace-ci-evidence.py').read_text()
        start, end = text.index(LAYER_MARKER), text.index('def png_pixels('); block = text[start:end]
        variants = [text + '\n# unknown byte\n', text.replace(CALL_ADDITION, '', 1),
            text.replace(CALL_ADDITION, CALL_ADDITION * 2, 1),
            text[:start] + text[end:], text[:start] + block + block + text[end:],
            text[:start] + block.replace('\n', '\r\n') + text[end:],
            text.replace("'failed_or_interrupted')", "'failed_or_interrupted', 'passed')", 1),
            text.replace('MAX_REPORT = 4 * 1024 * 1024', 'MAX_REPORT = 8 * 1024 * 1024', 1),
            text.replace('before.st_nlink == 1', 'before.st_nlink >= 1', 1)]
        without = text[:start] + text[end:]
        variants.append(without.replace('def source_summary(', block + 'def source_summary(', 1))
        for changed in variants:
            self.assertNotEqual(changed, text)
            with self.assertRaises(ValueError): strip_failure_stage_layer(changed)


if __name__ == '__main__': unittest.main()
