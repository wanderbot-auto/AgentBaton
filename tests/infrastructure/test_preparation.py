import importlib.util,json,tempfile,unittest,zipfile,hashlib,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('gate',R/'scripts/check_gate.py')
gate_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate_module)
class PreparationTests(unittest.TestCase):
    def test_gate_is_closed(self):
        gate=json.loads((R/'infra/execution-gate.json').read_text())
        with self.assertRaises(PermissionError):gate_module.assert_product_approval(gate)
    def test_boolean_without_provenance_does_not_unlock(self):
        with self.assertRaises(PermissionError):gate_module.assert_product_approval({'product_development_authorized':True})
    def test_account_alone_does_not_unlock(self):
        with self.assertRaises(PermissionError):gate_module.assert_product_approval({'product_development_authorized':True,'remote_repository':'example/AgentBaton','repository_owner_confirmed':True})
    def test_pure_function_accepts_explicit_complete_approval(self):
        # Test data only: does not change the live gate.
        gate_module.assert_product_approval({'product_development_authorized':True,'approval_reference':'test-only','remote_repository':'example/AgentBaton','repository_owner_confirmed':True})
    def test_core_design_not_modified(self):
        manifest=json.loads((R/'reference/design-v0.2-manifest.sha256.json').read_text())['files']
        paths=['specs/requirements.json','specs/decisions.json','specs/work-packages.json','docs/12-decisions.md']
        for p in paths:
            self.assertEqual(hashlib.sha256((R/p).read_bytes()).hexdigest(),manifest[p],p)
    def test_no_work_package_implicitly_authorized(self):
        ps=json.loads((R/'specs/work-packages.json').read_text())['packages']
        self.assertTrue(all(p['authorization']=='not_granted' for p in ps))
    def test_original_questions_preserved(self):
        q=json.loads((R/'specs/questionnaire.json').read_text())['questions']
        self.assertEqual(sorted(v['number'] for v in q),list(range(1,28)))
    def test_confirmed_target_does_not_unlock_product(self):
        plan=json.loads((R/'infra/environment-plan.json').read_text())
        gate=json.loads((R/'infra/execution-gate.json').read_text())
        self.assertEqual(plan['repository_owner'], 'wanderbot-auto')
        self.assertEqual(gate['target_repository'], 'wanderbot-auto/AgentBaton')
        self.assertTrue(gate['repository_owner_confirmed'])
        self.assertFalse(gate['product_development_authorized'])
        self.assertEqual(gate['remote_repository'], 'wanderbot-auto/AgentBaton')
    def test_bootstrap_rejects_previously_mistyped_owner(self):
        p=subprocess.run(['bash',str(R/'scripts/bootstrap-github.sh'),'--owner','wanderauto-bot'],text=True,capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2,p.stdout+p.stderr)
        self.assertIn('Approved owner is wanderbot-auto',p.stderr)
    def test_bootstrap_target_dry_run_is_non_mutating(self):
        p=subprocess.run(['bash',str(R/'scripts/bootstrap-github.sh'),'--owner','wanderbot-auto'],text=True,capture_output=True,timeout=10)
        self.assertEqual(p.returncode,0,p.stdout+p.stderr)
        self.assertIn('DRY RUN: create private wanderbot-auto/AgentBaton',p.stdout)
        self.assertIn('Product development remains on hold',p.stdout)
    def test_no_real_secret_files(self):
        for pattern in ['*.p12','*.p8','auth.json','.env']:
            self.assertEqual(list(R.rglob(pattern)),[])
if __name__=='__main__':unittest.main()
