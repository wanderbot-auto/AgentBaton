"""Test external native evidence and fatal/timeout cleanup; no GUI is simulated."""
import importlib.util
import copy
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('native_run', ROOT / 'scripts/native-run.py')
native_run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native_run)
GOOD = {
    'status': 'passed',
    'first_webview': dict(domMounted=True, realComposerSubmitted=True, realTaskRendered=True, taskId='fixture', projectId='fixture-project', idempotent=True, invalidInputRejected=True, journal=True, goalScopeLists=True, goalScopeEmptyControls=True, readonlyGoalNoMutation=True, goalReadonlySequence=dict(before=2, after=2)),
    'reopened_webview': dict(domMounted=True, taskId='fixture', projectId='fixture-project', restartPersisted=True, draftCancelled=True, stopEvidence=True),
    'fixture_project_id': 'fixture-project',
    'window_close_daemon_survived': True, 'second_writer_rejected': True,
    'owned_daemon_restarted': True, 'owned_daemon_cleanup': True,
    'goal_readonly_database': dict(read_only=True, unchanged=True, before=dict(scope_count=0, plan_count=0, attempt_count=0, sequence=2), after=dict(scope_count=0, plan_count=0, attempt_count=0, sequence=2)),
}

# New evidence is additive; the original GOOD fields and all old test bodies stay intact.
GOOD['first_webview'].update(goalExecutionLists=True, goalExecutionMissingRunDenied=True, readonlyGoalExecutionNoMutation=True, goalExecutionReadonlySequence=dict(before=2, after=2), goalExecutionMissingRun=dict(run_id='native-readonly-missing-goal-run', code='NOT_FOUND', side_effect_state='not_started'))
GOOD['goal_execution_readonly_database'] = dict(read_only=True, unchanged=True, before=dict(schema_version=12,run_count=0,node_count=0,reservation_count=0,sequence=2), after=dict(schema_version=12,run_count=0,node_count=0,reservation_count=0,sequence=2))


GOOD['first_webview'].update(goalGraphLists=True, goalGraphMissingRunDenied=True, readonlyGoalGraphNoMutation=True, goalGraphReadonlySequence=dict(before=2, after=2), goalGraphMissingRun=dict(run_id='native-readonly-missing-goal-graph-run', code='NOT_FOUND', side_effect_state='not_started'))
GOOD['goal_graph_readonly_database'] = dict(read_only=True, unchanged=True, before=dict(schema_version=12,capability_count=0,job_count=0,candidate_count=0,validation_count=0,owner_decision_count=0,acceptance_count=0,sequence=2), after=dict(schema_version=12,capability_count=0,job_count=0,candidate_count=0,validation_count=0,owner_decision_count=0,acceptance_count=0,sequence=2))


# Runtime crypto evidence is additive to the unchanged readonly/lifecycle fixture.
GOOD['webcrypto_checkpoints'] = {phase: native_run.webcrypto_expected(phase) for phase in ['before', 'after']}
GOOD['first_webview']['webCrypto'] = native_run.webcrypto_expected('first')
GOOD['reopened_webview']['webCrypto'] = native_run.webcrypto_expected('reopened')


class NativeRunnerTests(unittest.TestCase):
    def run_fixture(self, code, timeout=3, stale=False):
        with tempfile.TemporaryDirectory(prefix='native-runner-test-') as temporary:
            root = Path(temporary)
            path = root / 'native-smoke.json'
            write = f'from pathlib import Path;Path({str(path)!r}).write_text({json.dumps(GOOD)!r});'
            code = code.replace('WRITE_GOOD;', write)
            if stale:
                path.write_text(json.dumps(GOOD))
            status = native_run.run([sys.executable, '-c', code], root, timeout)
            report = json.loads((root / 'native-runner.json').read_text())
            if stale:
                self.assertTrue(list((root / 'previous').glob('*/native-smoke.json')))
            return status, report, json.loads(path.read_text()) if path.exists() else None

    def test_success_requires_current_native_evidence_after_group_cleanup(self):
        status, report, _ = self.run_fixture('WRITE_GOOD;pass')
        self.assertEqual(status, 0)
        self.assertEqual(report['status'], 'passed')
        self.assertTrue(report['owned_group_cleanup'])

    def test_goal_readonly_flags_require_actual_booleans_and_all_fields(self):
        for field in ['goalScopeLists', 'goalScopeEmptyControls', 'readonlyGoalNoMutation']:
            missing = copy.deepcopy(GOOD)
            del missing['first_webview'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
            for value in [False, None, 'true', 'false', 1, 0, [], {}]:
                with self.subTest(field=field, value=value):
                    changed = copy.deepcopy(GOOD)
                    changed['first_webview'][field] = value
                    self.assertIsNotNone(native_run.native_report_error(changed))

    def test_goal_readonly_database_requires_complete_unchanged_integer_counts(self):
        for location in ['goal_readonly_database', 'before', 'after']:
            missing = copy.deepcopy(GOOD)
            if location == 'goal_readonly_database':
                del missing[location]
            else:
                del missing['goal_readonly_database'][location]
            self.assertIsNotNone(native_run.native_report_error(missing))
        for field in ['scope_count', 'plan_count', 'attempt_count', 'sequence']:
            for value in [True, False, '0', None, -1, 1.0]:
                with self.subTest(field=field, value=value):
                    changed = copy.deepcopy(GOOD)
                    changed['goal_readonly_database']['after'][field] = value
                    self.assertIsNotNone(native_run.native_report_error(changed))
            changed = copy.deepcopy(GOOD)
            changed['goal_readonly_database']['after'][field] += 1
            self.assertIsNotNone(native_run.native_report_error(changed))
        for value in [False, 'true', 1, None]:
            for field in ['read_only', 'unchanged']:
                changed = copy.deepcopy(GOOD)
                changed['goal_readonly_database'][field] = value
                self.assertIsNotNone(native_run.native_report_error(changed))
        changed = copy.deepcopy(GOOD)
        changed['first_webview']['goalReadonlySequence']['after'] = 3
        self.assertIsNotNone(native_run.native_report_error(changed))
        for section in ['first_webview', 'reopened_webview']:
            changed = copy.deepcopy(GOOD)
            changed[section]['projectId'] = 'another-project'
            self.assertIsNotNone(native_run.native_report_error(changed))

    def test_exit_zero_without_native_report_is_failure(self):
        status, report, _ = self.run_fixture('pass')
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], 0)
        self.assertEqual(report['status'], 'failed')
        self.assertIn('not produced', report['evidence_error'])

    def test_stale_passing_report_is_archived_and_cannot_pass(self):
        status, report, _ = self.run_fixture('pass', stale=True)
        self.assertEqual(status, 1)
        self.assertEqual(report['status'], 'failed')

    def test_malformed_report_section_has_structured_failure(self):
        malformed = dict(GOOD, first_webview=[])
        self.assertIn('lacks required', native_run.native_report_error(malformed))

    def test_nonzero_downgrades_even_a_passing_native_report(self):
        status, report, native = self.run_fixture('WRITE_GOOD;raise SystemExit(7)')
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], 7)
        self.assertEqual(native['status'], 'failed')

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux subreaper evidence; macOS lane remains separate')
    def test_fatal_signal_reaps_adopted_owned_descendant(self):
        status, report, _ = self.run_fixture("import os,signal,subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(20)']);os.kill(os.getpid(),signal.SIGKILL)")
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], -9)
        self.assertGreaterEqual(report['adopted_owned_group_children_reaped'], 1)

    def test_timeout_remains_bounded_when_child_ignores_term(self):
        status, report, _ = self.run_fixture('import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);time.sleep(20)', timeout=1)
        self.assertEqual(status, 1)
        self.assertTrue(report['timeout'])
        self.assertEqual(report['exit_code'], -9)

    def test_permission_denied_on_only_owned_zombies_preserves_valid_evidence(self):
        # Synthetic syscall outcome on a real Linux child/group. This exercises
        # the Darwin failure contract, not actual macOS/native GUI acceptance.
        with mock.patch.object(native_run.os, 'killpg', side_effect=PermissionError(1, 'controlled denial')):
            status, report, _ = self.run_fixture('WRITE_GOOD;pass')
        self.assertEqual(status, 0)
        self.assertTrue(report['owned_group_cleanup'])
        self.assertEqual([item['result'] for item in report['group_signals']],
                         ['observed_quiescent', 'observed_quiescent'])

    def test_permission_denied_without_quiescence_proof_cannot_pass(self):
        with mock.patch.object(native_run.os, 'killpg', side_effect=PermissionError(1, 'controlled denial')):
            with mock.patch.object(native_run, 'owned_group_is_quiescent', return_value=False):
                status, report, native = self.run_fixture('WRITE_GOOD;pass')
        self.assertEqual(status, 1)
        self.assertFalse(report['owned_group_cleanup'])
        self.assertEqual(report['exit_code'], 0)
        self.assertEqual(len(report['cleanup_errors']), 2)
        self.assertEqual(native['status'], 'failed')

    def test_unexpected_permission_error_cannot_use_the_eperm_exception(self):
        with mock.patch.object(native_run.os, 'killpg', side_effect=PermissionError(13, 'controlled denial')):
            with mock.patch.object(native_run, 'owned_group_is_quiescent', return_value=True) as proof:
                status, report, _ = self.run_fixture('WRITE_GOOD;pass')
        self.assertEqual(status, 1)
        self.assertFalse(report['owned_group_cleanup'])
        proof.assert_not_called()

    def test_observation_failure_still_reaps_child_and_writes_failed_report(self):
        with mock.patch.object(native_run, 'owned_child_exited', side_effect=OSError(22, 'controlled observation failure')):
            status, report, _ = self.run_fixture('import time;time.sleep(20)')
        self.assertEqual(status, 1)
        self.assertEqual(report['status'], 'failed')
        self.assertIn('OSError:22', report['observation_error'])
        self.assertTrue(report['owned_group_cleanup'])

    def test_group_query_requires_the_original_zombie_leader_and_no_live_members(self):
        for output in ['', '99 99 Z\n', '42 42 S\n', '42 42 Z\n43 42 S\n', 'malformed\n']:
            with self.subTest(output=output):
                result = subprocess.CompletedProcess([], 0, stdout=output, stderr='')
                with mock.patch.object(native_run.subprocess, 'run', return_value=result):
                    self.assertFalse(native_run.owned_group_is_quiescent(42))
        result = subprocess.CompletedProcess([], 0, stdout='42 42 Zs\n43 42 Z\n', stderr='')
        with mock.patch.object(native_run.subprocess, 'run', return_value=result):
            self.assertTrue(native_run.owned_group_is_quiescent(42))


class GoalExecutionReadonlyEvidenceTests(unittest.TestCase):
    def test_execution_flags_are_required_actual_booleans(self):
        self.assertIsNone(native_run.native_report_error(copy.deepcopy(GOOD)))
        for field in ['goalExecutionLists','goalExecutionMissingRunDenied','readonlyGoalExecutionNoMutation']:
            missing=copy.deepcopy(GOOD);del missing['first_webview'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
            for value in [False,None,'true','false',1,0,[],{}]:
                with self.subTest(field=field,value=value):
                    changed=copy.deepcopy(GOOD);changed['first_webview'][field]=value
                    self.assertIsNotNone(native_run.native_report_error(changed))

    def test_execution_database_is_independent_complete_schema11_and_all_zero(self):
        for location in ['goal_execution_readonly_database','before','after']:
            missing=copy.deepcopy(GOOD)
            if location=='goal_execution_readonly_database':del missing[location]
            else:del missing['goal_execution_readonly_database'][location]
            self.assertIsNotNone(native_run.native_report_error(missing))
        for field in ['schema_version','run_count','node_count','reservation_count','sequence']:
            missing=copy.deepcopy(GOOD);del missing['goal_execution_readonly_database']['after'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
            for value in [True,False,'0',None,-1,0.0]:
                with self.subTest(field=field,value=value):
                    changed=copy.deepcopy(GOOD);changed['goal_execution_readonly_database']['after'][field]=value
                    self.assertIsNotNone(native_run.native_report_error(changed))
        for field in ['schema_version','run_count','node_count','reservation_count']:
            changed=copy.deepcopy(GOOD)
            for phase in ['before','after']:changed['goal_execution_readonly_database'][phase][field]+=1
            self.assertIsNotNone(native_run.native_report_error(changed))
        extra=copy.deepcopy(GOOD);extra['goal_execution_readonly_database']['before']['claimed_by_js']=True
        self.assertIsNotNone(native_run.native_report_error(extra))

    def test_execution_interval_cannot_replace_or_drift_from_original_goal_interval(self):
        for value in [False,'true',1,None]:
            for field in ['read_only','unchanged']:
                changed=copy.deepcopy(GOOD);changed['goal_execution_readonly_database'][field]=value
                self.assertIsNotNone(native_run.native_report_error(changed))
        for value in [True,2.0,'2',None,-1,3]:
            changed=copy.deepcopy(GOOD);changed['first_webview']['goalExecutionReadonlySequence']['after']=value
            self.assertIsNotNone(native_run.native_report_error(changed))
        missing=copy.deepcopy(GOOD);del missing['first_webview']['goalExecutionReadonlySequence']
        self.assertIsNotNone(native_run.native_report_error(missing))
        replayed=copy.deepcopy(GOOD)
        for phase in ['before','after']:
            replayed['goal_execution_readonly_database'][phase]['sequence']=3
            replayed['first_webview']['goalExecutionReadonlySequence'][phase]=3
        self.assertIsNotNone(native_run.native_report_error(replayed))
        old_missing=copy.deepcopy(GOOD);del old_missing['goal_readonly_database']
        self.assertIsNotNone(native_run.native_report_error(old_missing))

    def test_missing_run_requires_exact_absent_reference_and_not_started_core_error(self):
        for field,value in [('run_id','another-run'),('code','PERMISSION_DENIED'),('code',True),('side_effect_state','unknown')]:
            changed=copy.deepcopy(GOOD);changed['first_webview']['goalExecutionMissingRun'][field]=value
            self.assertIsNotNone(native_run.native_report_error(changed))
        for field in ['run_id','code','side_effect_state']:
            missing=copy.deepcopy(GOOD);del missing['first_webview']['goalExecutionMissingRun'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
        fabricated=copy.deepcopy(GOOD);fabricated['first_webview']['goalExecutionMissingRun']['result']={}
        self.assertIsNotNone(native_run.native_report_error(fabricated))



class GoalGraphReadonlyEvidenceTests(unittest.TestCase):
    def test_graph_flags_require_actual_booleans_and_all_fields(self):
        self.assertIsNone(native_run.native_report_error(copy.deepcopy(GOOD)))
        for field in ['goalGraphLists','goalGraphMissingRunDenied','readonlyGoalGraphNoMutation']:
            missing=copy.deepcopy(GOOD);del missing['first_webview'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
            for value in [False,None,'true','false',1,0,[],{}]:
                changed=copy.deepcopy(GOOD);changed['first_webview'][field]=value
                self.assertIsNotNone(native_run.native_report_error(changed))

    def test_graph_database_requires_schema12_and_all_six_independent_zero_counts(self):
        fields=['schema_version','capability_count','job_count','candidate_count','validation_count','owner_decision_count','acceptance_count','sequence']
        for location in ['goal_graph_readonly_database','before','after']:
            missing=copy.deepcopy(GOOD)
            if location=='goal_graph_readonly_database':del missing[location]
            else:del missing['goal_graph_readonly_database'][location]
            self.assertIsNotNone(native_run.native_report_error(missing))
        for phase in ['before','after']:
            for field in fields:
                missing=copy.deepcopy(GOOD);del missing['goal_graph_readonly_database'][phase][field]
                self.assertIsNotNone(native_run.native_report_error(missing))
                for value in [True,False,'0',None,-1,0.0]:
                    changed=copy.deepcopy(GOOD);changed['goal_graph_readonly_database'][phase][field]=value
                    self.assertIsNotNone(native_run.native_report_error(changed))
        for field in fields[1:-1]:
            changed=copy.deepcopy(GOOD)
            for phase in ['before','after']:changed['goal_graph_readonly_database'][phase][field]=1
            self.assertIsNotNone(native_run.native_report_error(changed))
        for schema in [10,11,13]:
            for group in ['goal_graph_readonly_database','goal_execution_readonly_database']:
                changed=copy.deepcopy(GOOD)
                for phase in ['before','after']:changed[group][phase]['schema_version']=schema
                self.assertIsNotNone(native_run.native_report_error(changed))
        extra=copy.deepcopy(GOOD);extra['goal_graph_readonly_database']['before']['claimed_by_js']=True
        self.assertIsNotNone(native_run.native_report_error(extra))

    def test_graph_interval_cannot_replace_goal_or_execution_database_evidence(self):
        for value in [False,'true',1,None]:
            for field in ['read_only','unchanged']:
                changed=copy.deepcopy(GOOD);changed['goal_graph_readonly_database'][field]=value
                self.assertIsNotNone(native_run.native_report_error(changed))
        for value in [True,2.0,'2',None,-1,3]:
            changed=copy.deepcopy(GOOD);changed['first_webview']['goalGraphReadonlySequence']['after']=value
            self.assertIsNotNone(native_run.native_report_error(changed))
        replayed=copy.deepcopy(GOOD)
        for phase in ['before','after']:
            replayed['goal_graph_readonly_database'][phase]['sequence']=3
            replayed['first_webview']['goalGraphReadonlySequence'][phase]=3
        self.assertIsNotNone(native_run.native_report_error(replayed))
        for group in ['goal_readonly_database','goal_execution_readonly_database']:
            missing=copy.deepcopy(GOOD);del missing[group]
            self.assertIsNotNone(native_run.native_report_error(missing))
        missing=copy.deepcopy(GOOD);del missing['first_webview']['goalGraphReadonlySequence']
        self.assertIsNotNone(native_run.native_report_error(missing))

    def test_graph_missing_requires_fixed_not_started_core_denial_without_result(self):
        for field,value in [('run_id','foreign'),('code','PERMISSION_DENIED'),('code',True),('side_effect_state','unknown')]:
            changed=copy.deepcopy(GOOD);changed['first_webview']['goalGraphMissingRun'][field]=value
            self.assertIsNotNone(native_run.native_report_error(changed))
        for field in ['run_id','code','side_effect_state']:
            missing=copy.deepcopy(GOOD);del missing['first_webview']['goalGraphMissingRun'][field]
            self.assertIsNotNone(native_run.native_report_error(missing))
        fabricated=copy.deepcopy(GOOD);fabricated['first_webview']['goalGraphMissingRun']['result']={}
        self.assertIsNotNone(native_run.native_report_error(fabricated))


class NativeArtifactAuditTests(unittest.TestCase):
    def inspect(self, content, failed_tool=False):
        with tempfile.TemporaryDirectory(prefix='native-audit-test-') as temporary:
            root = Path(temporary)
            candidate = root / 'controlled-audit-fixture'
            candidate.write_text(content)
            environment = dict(os.environ)
            if failed_tool:
                tools = root / 'tools'
                tools.mkdir()
                strings = tools / 'strings'
                strings.write_text('#!/bin/sh\nexit 23\n')
                strings.chmod(0o700)
                environment['PATH'] = str(tools) + os.pathsep + environment['PATH']
            return subprocess.run(['/bin/bash', str(ROOT / 'scripts/native-audit-release.sh'), str(candidate)], env=environment, capture_output=True, text=True)

    def test_inspection_tool_failure_cannot_report_success(self):
        result = self.inspect('controlled fixture without test markers', failed_tool=True)
        self.assertEqual(result.returncode, 23)
        self.assertNotIn('PASS', result.stdout)

    def test_native_test_marker_is_rejected(self):
        result = self.inspect('controlled native-smoke-create fixture')
        self.assertEqual(result.returncode, 1)
        self.assertNotIn('PASS', result.stdout)

    def test_absent_markers_passes_only_the_marker_inspection(self):
        result = self.inspect('controlled fixture without test markers')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('PASS', result.stdout)


class WebCryptoEvidenceTests(unittest.TestCase):
    def test_known_original_utf8_vector_retains_big_integer_literal_bytes(self):
        import hashlib
        self.assertEqual(len(native_run.WEBCRYPTO_INPUT.encode('utf-8')), 109)
        self.assertEqual(hashlib.sha256(native_run.WEBCRYPTO_INPUT.encode('utf-8')).hexdigest(), native_run.WEBCRYPTO_SHA256)
        self.assertIn('18446744073709551615', native_run.WEBCRYPTO_INPUT)
        self.assertIn('原文证据🔒', native_run.WEBCRYPTO_INPUT)
        self.assertIsNone(native_run.native_report_error(copy.deepcopy(GOOD)))

    def test_absent_crypto_bad_digest_algorithm_bytes_and_type_forgery_fail(self):
        for section in ['first_webview', 'reopened_webview']:
            missing = copy.deepcopy(GOOD)
            del missing[section]['webCrypto']
            self.assertIsNotNone(native_run.native_report_error(missing))
            for field, wrong in [('available',False),('available',1),('available','true'),('version',True),('algorithm','SHA-1'),('sha256','0'*64),('input_bytes','109'),('input_bytes',109.0),('input','rounded or reserialized JSON'),('encoding','UTF-16'),('source','node.crypto')]:
                changed = copy.deepcopy(GOOD)
                changed[section]['webCrypto'][field] = wrong
                self.assertIsNotNone(native_run.native_report_error(changed), (section,field,wrong))
            for key in GOOD[section]['webCrypto']:
                missing = copy.deepcopy(GOOD)
                del missing[section]['webCrypto'][key]
                self.assertIsNotNone(native_run.native_report_error(missing), key)
            changed = copy.deepcopy(GOOD)
            changed[section]['webCrypto']['mocked'] = True
            self.assertIsNotNone(native_run.native_report_error(changed))

    def test_checkpoint_missing_mixed_replayed_and_reopened_witness_are_rejected(self):
        missing = copy.deepcopy(GOOD)
        del missing['webcrypto_checkpoints']
        self.assertIsNotNone(native_run.native_report_error(missing))
        for checkpoint in ['before', 'after']:
            missing = copy.deepcopy(GOOD)
            del missing['webcrypto_checkpoints'][checkpoint]
            self.assertIsNotNone(native_run.native_report_error(missing))
            changed = copy.deepcopy(GOOD)
            changed['webcrypto_checkpoints'][checkpoint] = native_run.webcrypto_expected('first')
            self.assertIsNotNone(native_run.native_report_error(changed))
        changed = copy.deepcopy(GOOD)
        changed['webcrypto_checkpoints']['before'], changed['webcrypto_checkpoints']['after'] = changed['webcrypto_checkpoints']['after'], changed['webcrypto_checkpoints']['before']
        self.assertIsNotNone(native_run.native_report_error(changed))
        changed = copy.deepcopy(GOOD)
        changed['reopened_webview']['webCrypto'] = changed['first_webview']['webCrypto']
        self.assertIsNotNone(native_run.native_report_error(changed))

    def test_known_digest_cannot_substitute_original_database_or_sequence_evidence(self):
        for field in ['goal_readonly_database', 'goal_execution_readonly_database', 'goal_graph_readonly_database']:
            changed = copy.deepcopy(GOOD)
            changed[field]['after']['sequence'] += 1
            self.assertIsNotNone(native_run.native_report_error(changed))
        changed = copy.deepcopy(GOOD)
        changed['goal_graph_readonly_database']['after']['candidate_count'] = 1
        self.assertIsNotNone(native_run.native_report_error(changed))


if __name__ == '__main__':
    unittest.main()
