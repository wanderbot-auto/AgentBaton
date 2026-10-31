"""Pure finite-evidence controls only: these never launch Native/OS services."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('background_native_runner', ROOT / 'scripts/native-background-service-smoke.py')
R = importlib.util.module_from_spec(spec)
spec.loader.exec_module(R)


def report(case):
    count = {'normal': 9, 'race': 3, 'conflict': 2}[case]
    value = {name: True for name in R.REQUIRED_FLAGS}
    value.update(status='passed', case=case, dimension='real_macos_webview_ipc_native_sheet_generated_paths',
                 completed_operations=count, renderer_observations_untrusted=count,
                 receipts=[R.expected_receipt(case, i) for i in range(count)],
                 screenshots={'normal': 6, 'race': 2, 'conflict': 0}[case], os='macos', architecture='aarch64',
                 actual_os_login='not_run', installed_layout='generated_not_supported_installation_proof',
                 receipt_before_plist='source_order_only_not_terminal_chronology', effective_login_start='not_observed',
                 current_daemon_action='none', fixture_retention='retained_private_no_unknown_data_deleted',
                 contract='abg-registration-fixture-v1', provider_model_calls='not_run', failed=None,
                 generated_fixture_root='/private/tmp/abg-12345-0',
                 conflict_mutation_path='explicit_bridge_only_ui_remains_disabled' if case=='conflict' else 'production_dom_buttons')
    return value


class FiniteNativeBackgroundReport(unittest.TestCase):
    def test_three_closed_case_sequences_keep_unknown_and_cleanup_distinctions(self):
        for case in R.CASES:
            for arch in ('aarch64', 'x86_64'):
                value=report(case);value['architecture']=arch
                R.validate_report(value, case, arch)
        self.assertEqual(report('race')['receipts'][1]['side_effect_state'],'not_started')
        self.assertEqual(report('normal')['receipts'][5]['login_registration'],'unknown')
        self.assertEqual(report('normal')['receipts'][7]['current_daemon_action'],'none')

    def test_each_missing_unknown_wrong_type_or_unobserved_field_refuses(self):
        base=report('normal')
        for key in R.REPORT_KEYS:
            value=copy.deepcopy(base);value.pop(key)
            with self.subTest(missing=key),self.assertRaises(ValueError):R.validate_report(value,'normal','aarch64')
        for key in R.REQUIRED_FLAGS:
            for bad in (None,False,1,'true'):
                value=copy.deepcopy(base);value[key]=bad
                with self.subTest(flag=key,bad=bad),self.assertRaises(ValueError):R.validate_report(value,'normal','aarch64')
        for key,bad in [('private_path','PRIVATE'),('failed','unknown'),('actual_os_login','passed'),
                        ('receipt_before_plist','observed_from_final_files'),('os','linux'),
                        ('current_daemon_action','stopped'),('effective_login_start','running'),
                        ('screenshots',True),('completed_operations',True),('renderer_observations_untrusted',True),
                        ('generated_fixture_root','/Users/user/Library/LaunchAgents')]:
            value=copy.deepcopy(base);value[key]=bad
            with self.subTest(key=key),self.assertRaises(ValueError):R.validate_report(value,'normal','aarch64')

    def test_receipt_widening_success_on_cancel_and_any_foreign_phase_refuse(self):
        for case in R.CASES:
            base=report(case)
            for index in range(len(base['receipts'])):
                for key,bad in [('private_path','PRIVATE'),('api_version',True),('current_daemon_action','started'),
                                ('effective_login_start','running'),('side_effect_state','unknown')]:
                    value=copy.deepcopy(base);value['receipts'][index][key]=bad
                    with self.subTest(case=case,index=index,key=key),self.assertRaises(ValueError):R.validate_report(value,case,'aarch64')
            for receipts in ([],base['receipts']*2,list(reversed(base['receipts']))):
                if receipts==base['receipts']:continue
                value=copy.deepcopy(base);value['receipts']=receipts
                with self.assertRaises(ValueError):R.validate_report(value,case,'aarch64')
        value=report('normal');value['receipts'][1]=R.expected_receipt('normal',3)
        with self.assertRaises(ValueError):R.validate_report(value,'normal','aarch64')

    def test_conflict_cannot_claim_ui_enable_or_os_sheet(self):
        for field,bad in [('screenshots',1),('conflict_mutation_path','production_dom_buttons')]:
            value=report('conflict');value[field]=bad
            with self.assertRaises(ValueError):R.validate_report(value,'conflict','aarch64')

    def test_duplicate_json_keys_at_every_level_remain_rejected(self):
        for raw in ['{"case":"normal","case":"normal"}','{"outer":{"seen":true,"seen":false}}']:
            with self.assertRaises(ValueError):json.loads(raw,object_pairs_hook=R.unique)

    def test_fixture_roots_cannot_be_renderer_account_or_overlong_paths(self):
        for root in ('/tmp/abg-1-0','/private/tmp/abg-../h','/private/tmp/abg-1-0/Library','/private/tmp/abg-'+('1'*30)+'-0',None):
            value=report('normal');value['generated_fixture_root']=root
            with self.assertRaises(ValueError):R.validate_report(value,'normal','aarch64')
