"""Preparation-only regressions for public PR CI and the importable ruleset."""
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import yaml

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('doctor', ROOT / 'scripts/doctor.py')
doctor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(doctor)

class PublicRepositoryTests(unittest.TestCase):
    def setUp(self):
        self.ruleset = json.loads((ROOT / 'infra/github-main-ruleset.json').read_text())
        self.workflow = yaml.load((ROOT / '.github/workflows/environment-preflight.yml').read_text(), Loader=yaml.BaseLoader)

    def test_every_main_pr_is_checked(self):
        self.assertEqual(self.workflow['on']['pull_request'], {'branches': ['main'], 'types': ['opened', 'synchronize', 'reopened', 'ready_for_review']})
        self.assertNotIn('pull_request_target', self.workflow['on'])

    def test_read_only_ephemeral_runner_without_credentials(self):
        self.assertEqual(self.workflow['permissions'], {'contents': 'read'})
        job = self.workflow['jobs']['preflight']
        self.assertEqual(job['runs-on'], '${{ matrix.os }}')
        self.assertEqual(job['steps'][0]['with']['persist-credentials'], 'false')
        self.assertNotIn('secrets.', (ROOT / '.github/workflows/environment-preflight.yml').read_text())

    def test_ruleset_is_main_only_without_bypass(self):
        self.assertEqual(self.ruleset['enforcement'], 'active')
        self.assertEqual(self.ruleset['conditions']['ref_name'], {'include': ['refs/heads/main'], 'exclude': []})
        self.assertEqual(self.ruleset['bypass_actors'], [])

    def test_force_push_and_deletion_are_blocked(self):
        kinds = {rule['type'] for rule in self.ruleset['rules']}
        self.assertTrue({'deletion', 'non_fast_forward', 'pull_request'}.issubset(kinds))

    def test_exact_ci_names_and_github_actions_source(self):
        rule = next(x for x in self.ruleset['rules'] if x['type'] == 'required_status_checks')['parameters']
        expected = [{'context': 'Environment / ' + name, 'integration_id': 15368} for name in self.workflow['jobs']['preflight']['strategy']['matrix']['os']]
        self.assertEqual(rule['required_status_checks'], expected)
        self.assertTrue(rule['strict_required_status_checks_policy'])

    def test_single_maintainer_not_blocked_by_self_review(self):
        rule = next(x for x in self.ruleset['rules'] if x['type'] == 'pull_request')['parameters']
        self.assertEqual(rule['required_approving_review_count'], 0)
        self.assertFalse(rule['require_last_push_approval'])
        self.assertTrue(rule['required_review_thread_resolution'])

    def test_codesign_presence_is_not_signing_success(self):
        with patch.object(doctor.shutil, 'which', return_value='/usr/bin/codesign'), patch.object(doctor.subprocess, 'run') as run:
            result = doctor.probe_executable('codesign')
        run.assert_not_called()
        self.assertEqual(result['status'], 'present')
        self.assertEqual(result['functional_test'], 'not_run')
        self.assertNotIn('version', result)

    def test_missing_codesign_is_explicit(self):
        with patch.object(doctor.shutil, 'which', return_value=None):
            self.assertEqual(doctor.probe_executable('codesign')['status'], 'missing')

if __name__ == '__main__':
    unittest.main()
