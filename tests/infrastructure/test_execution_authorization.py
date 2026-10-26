"""Owner-approved phase migration; preserve separately gated high-risk actions."""
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('gate', ROOT / 'scripts/check_gate.py')
gate_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate_module)

class ExecutionAuthorizationTests(unittest.TestCase):
    def setUp(self):
        self.gate = json.loads((ROOT / 'infra/execution-gate.json').read_text())

    def test_live_product_approval_has_provenance(self):
        gate_module.assert_product_approval(self.gate)
        self.assertEqual(self.gate['phase'], 'product_development')
        self.assertEqual(self.gate['approval_id'], 'AB-START-001')
        path = ROOT / self.gate['approval_reference']
        self.assertTrue(path.resolve().is_relative_to(ROOT))
        self.assertIn('确认，开始调用codex cloud进行研发', path.read_text())

    def test_exact_repository_and_branch(self):
        self.assertEqual(self.gate['remote_repository'], 'wanderbot-auto/AgentBaton')
        self.assertEqual(self.gate['integration_branch'], 'main')

    def test_package_authorization_is_explicit_overlay(self):
        packages = json.loads((ROOT / 'specs/work-packages.json').read_text())['packages']
        self.assertEqual(set(self.gate['authorized_work_packages']), {p['id'] for p in packages})
        self.assertTrue(all(p['authorization'] == 'not_granted' for p in packages))

    def test_high_risk_authorizations_remain_closed(self):
        for key in ('real_provider_calls_authorized', 'production_release_authorized', 'paid_resource_creation_authorized'):
            self.assertIs(self.gate[key], False, key)
        self.assertFalse(self.gate['resource_policy']['automatic_topups'])

    def test_revoke_blocks_even_with_record(self):
        self.gate['product_development_authorized'] = False
        with self.assertRaises(PermissionError):
            gate_module.assert_product_approval(self.gate)

    def test_remove_provenance_blocks(self):
        self.gate['approval_reference'] = None
        with self.assertRaises(PermissionError):
            gate_module.assert_product_approval(self.gate)

    def test_required_execution_documents_exist(self):
        for name in ('codex/MASTER_PROMPT.md', 'codex/GOAL.md', 'codex/RUNBOOK.md', 'docs/execution/STATUS.md', 'docs/execution/OWNER_INBOX.md'):
            self.assertGreater((ROOT / name).stat().st_size, 100)

    def test_goal_keeps_unverified_work_incomplete(self):
        text = (ROOT / 'codex/GOAL.md').read_text()
        self.assertIn('不能标记整个产品完成', text)
        self.assertIn('Mock', text)

if __name__ == '__main__':
    unittest.main()
