"""The cold macOS build exception must not expand other CI resource budgets."""
import copy
from pathlib import Path
import sys
import unittest
import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
try:
    from validate_infrastructure import validate_job_timeout
finally:
    sys.path.pop(0)


class RuntimePolicyTests(unittest.TestCase):
    def setUp(self):
        workflow = yaml.load((ROOT / '.github/workflows/product.yml').read_text(), Loader=yaml.BaseLoader)
        self.desktop = workflow['jobs']['desktop']

    def test_actual_same_architecture_build_has_a_finite_45_minute_budget(self):
        self.assertEqual(self.desktop['timeout-minutes'], '45')
        validate_job_timeout('product.yml', 'desktop', self.desktop)

    def test_larger_zero_and_negative_budgets_are_refused(self):
        for value in ('46', '120', '0', '-1'):
            with self.subTest(value=value), self.assertRaises(AssertionError):
                validate_job_timeout('product.yml', 'desktop', {**self.desktop, 'timeout-minutes': value})

    def test_exception_cannot_be_borrowed_by_environment_other_jobs_or_workflows(self):
        for workflow, job in [('environment-preflight.yml', 'preflight'), ('environment-preflight.yml', 'desktop'), ('other.yml', 'desktop'), ('product.yml', 'core'), ('product.yml', 'frontend'), ('product.yml', 'desktop-copy')]:
            with self.subTest(workflow=workflow, job=job), self.assertRaises(AssertionError):
                validate_job_timeout(workflow, job, self.desktop)

    def test_ordinary_job_bound_is_preserved(self):
        validate_job_timeout('other.yml', 'job', {'timeout-minutes': '30'})
        for value in ('31', '45', '0', '-1'):
            with self.subTest(value=value), self.assertRaises(AssertionError):
                validate_job_timeout('other.yml', 'job', {'timeout-minutes': value})

    def test_changed_runner_matrix_or_concurrency_cannot_borrow_budget(self):
        variants = [
            {'runs-on': 'self-hosted'},
            {'runs-on': 'macos-15'},
            {'matrix': {'os': ['macos-15', 'ubuntu-24.04']}},
            {'matrix': {'os': ['macos-15', 'macos-15-intel', 'macos-15']}},
            {'matrix': {'os': ['macos-15', 'macos-15-intel'], 'include': [{'os': 'self-hosted'}]}},
            {'max-parallel': '3'}, {'max-parallel': None}, {'fail-fast': 'true'},
        ]
        for change in variants:
            job = copy.deepcopy(self.desktop)
            if 'runs-on' in change:
                job.update(change)
            else:
                job['strategy'].update(change)
            with self.subTest(change=change), self.assertRaises(AssertionError):
                validate_job_timeout('product.yml', 'desktop', job)

    def test_missing_dynamic_and_malformed_timeouts_are_refused(self):
        for value in (None, True, 45.9, '45.9', '${{ matrix.timeout }}', 'unlimited'):
            with self.subTest(value=value), self.assertRaises(AssertionError):
                validate_job_timeout('product.yml', 'desktop', {**self.desktop, 'timeout-minutes': value})


if __name__ == '__main__':
    unittest.main()
