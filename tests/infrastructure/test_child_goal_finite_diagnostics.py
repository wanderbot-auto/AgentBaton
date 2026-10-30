"""Finite metadata controls only: no Core, browser, listener or product run."""
import ast
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import threading
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
P=types.ModuleType('finite_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')
exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)
old_spec=importlib.util.spec_from_file_location('old_finite_projection_controls',Path(__file__).with_name('test_child_goal_ci_evidence.py'))
OLD=importlib.util.module_from_spec(old_spec);old_spec.loader.exec_module(OLD)
SECRET='PRIVATE_CANARY_TOKEN_PATH_PROPOSAL_ARGV_SNAPSHOT'

def reply(code='INPUT_CHANGED',retryable=False,effect='not_started'):
    return {'request_id':SECRET,'error':{'code':code,'message':SECRET,'retryable':retryable,'side_effect_state':effect,'details_ref':SECRET},'revision':19}

def actual_failure(recorder,filename=__file__):
    try:raise AssertionError(SECRET)
    except AssertionError as error:recorder.failure(error,'runtime',filename,ValueError)

class ChildGoalFiniteDiagnosticsTests(unittest.TestCase):
    def test_absent_and_no_reply_are_explicitly_unobserved(self):
        self.assertEqual(P.finite_diagnostics(None),P.unobserved_diagnostics())
        self.assertEqual(P.FiniteRuntimeDiagnostics().snapshot(),P.unobserved_diagnostics())
        _,m,s,r=OLD.inputs();self.assertIs(type(m['schema']),int);self.assertEqual(m['schema'],17)
        for rejected_schema in (15,16,18):
            bad=copy.deepcopy(m);bad['schema']=rejected_schema
            with self.assertRaises(P.Refused):P.candidate_summary(json.dumps(bad).encode())
        value=P.project(r,m,s,'normal',0,'e'*64)
        self.assertEqual(value['status'],'passed')
        self.assertIsNone(value['finite_diagnostics']['failure_class'])
        self.assertIsNone(value['finite_diagnostics']['core_replies']['errors'])

    def test_direct_reply_metadata_omits_all_raw_private_fields(self):
        recorder=P.FiniteRuntimeDiagnostics();recorder.completed('CG02');recorder.reply('factory_child_goals.get',reply())
        actual_failure(recorder)
        safe=P.finite_diagnostics(recorder.snapshot())
        self.assertEqual(safe['failure_class'],'assertion');self.assertEqual(safe['phase'],'runtime')
        self.assertEqual(safe['last_completed_check'],'CG02')
        self.assertTrue(all(type(n)is int and n>0 for n in safe['driver_lines']))
        self.assertEqual(safe['core_replies']['errors'],[{'method':'factory_child_goals.get','code':'INPUT_CHANGED','retryable':False,'side_effect_state':'not_started','count':1}])
        self.assertNotIn(SECRET,json.dumps(safe))

    def test_success_reply_is_observed_zero_errors_not_missing(self):
        recorder=P.FiniteRuntimeDiagnostics();recorder.reply('system.snapshot',{'result':{'secret':SECRET}})
        value=P.finite_diagnostics(recorder.snapshot())['core_replies']
        self.assertEqual(value['captured_count'],1);self.assertEqual(value['errors'],[])
        self.assertEqual(value['unmapped_error_count'],0);self.assertFalse(value['truncated'])

    def test_unknown_code_method_or_effect_is_counted_without_leaking_string(self):
        for method,code,effect in [(SECRET,'INPUT_CHANGED','not_started'),('system.snapshot',SECRET,'unknown'),('system.snapshot','INPUT_CHANGED',SECRET)]:
            recorder=P.FiniteRuntimeDiagnostics();recorder.reply(method,reply(code,effect=effect));safe=P.finite_diagnostics(recorder.snapshot())
            self.assertEqual(safe['core_replies']['captured_count'],1)
            self.assertEqual(safe['core_replies']['unmapped_error_count'],1)
            self.assertEqual(safe['core_replies']['errors'],[]);self.assertNotIn(SECRET,json.dumps(safe))

    def test_malformed_observed_error_invalidates_projection_not_silently_no_error(self):
        for mutate in [lambda e:e.update(retryable=1),lambda e:e.update(code=17),lambda e:e.update(side_effect_state=False),lambda e:e.update(message=None),lambda e:e.update(details_ref=[]),lambda e:e.update(extra=SECRET),lambda e:e.pop('code')]:
            item=reply();mutate(item['error']);recorder=P.FiniteRuntimeDiagnostics();recorder.reply('system.snapshot',item)
            with self.assertRaises(P.Refused):P.finite_diagnostics(recorder.snapshot())

    def test_caps_are_explicit_and_never_claim_all_errors_observed(self):
        recorder=P.FiniteRuntimeDiagnostics()
        for _ in range(P.MAX_DIAGNOSTIC_REPLIES+4):recorder.reply('system.snapshot',reply())
        rows=P.finite_diagnostics(recorder.snapshot())['core_replies']
        self.assertTrue(rows['truncated']);self.assertEqual(rows['captured_count'],2048)
        self.assertEqual(rows['errors'][0]['count'],2048)
        recorder=P.FiniteRuntimeDiagnostics()
        for method in sorted(P.DIAGNOSTIC_METHODS)[:70]:recorder.reply(method,reply())
        rows=P.finite_diagnostics(recorder.snapshot())['core_replies']
        self.assertTrue(rows['truncated']);self.assertEqual(len(rows['errors']),64)
        self.assertEqual(rows['unmapped_error_count'],6)

    def test_parallel_reply_counting_is_exact(self):
        recorder=P.FiniteRuntimeDiagnostics();threads=[threading.Thread(target=lambda:[recorder.reply('system.snapshot',reply())for _ in range(100)])for _ in range(8)]
        for t in threads:t.start()
        for t in threads:t.join(timeout=2);self.assertFalse(t.is_alive())
        rows=P.finite_diagnostics(recorder.snapshot())['core_replies']
        self.assertEqual(rows['captured_count'],800);self.assertEqual(rows['errors'][0]['count'],800)

    def test_classification_uses_exception_objects_not_messages_and_preserves_first(self):
        for error,kind in [(AssertionError(SECRET),'assertion'),(TimeoutError(SECRET),'timeout'),(OSError(SECRET),'os_error'),(TypeError(SECRET),'type_error'),(ValueError(SECRET),'refusal'),(RuntimeError(SECRET),'runtime_error'),(Exception('INPUT_CHANGED '+SECRET),'other')]:
            recorder=P.FiniteRuntimeDiagnostics();recorder.failure(error,'cleanup',__file__,ValueError)
            recorder.failure(AssertionError('later'),'runtime',__file__,ValueError)
            safe=P.finite_diagnostics(recorder.snapshot());self.assertEqual(safe['failure_class'],kind)
            self.assertEqual(safe['phase'],'cleanup');self.assertIsNone(safe['driver_lines']);self.assertNotIn(SECRET,json.dumps(safe))

    def test_only_exact_driver_filename_lines_leave_traceback(self):
        recorder=P.FiniteRuntimeDiagnostics();actual_failure(recorder,'/different/private/'+SECRET)
        safe=P.finite_diagnostics(recorder.snapshot());self.assertIsNone(safe['driver_lines']);self.assertNotIn(SECRET,json.dumps(safe))

    def test_public_whitelist_rejects_secret_fields_strings_types_and_duplicates(self):
        recorder=P.FiniteRuntimeDiagnostics();recorder.reply('system.snapshot',reply());actual_failure(recorder);original=recorder.snapshot()
        for mutate in [lambda v:v.update(extra=SECRET),lambda v:v.update(failure_class=SECRET),lambda v:v.update(driver_lines=[True]),lambda v:v.update(driver_lines=[1.0]),lambda v:v.update(driver_lines=[1000001]),lambda v:v.update(phase=SECRET),lambda v:v.update(last_completed_check=SECRET),lambda v:v['core_replies'].update(captured_count=True),lambda v:v['core_replies'].update(captured_count=1.0),lambda v:v['core_replies'].update(truncated=1),lambda v:v['core_replies']['errors'][0].update(retryable=0),lambda v:v['core_replies']['errors'][0].update(count=True),lambda v:v['core_replies']['errors'][0].update(method=SECRET),lambda v:v['core_replies']['errors'][0].update(code=SECRET),lambda v:v['core_replies']['errors'].append(v['core_replies']['errors'][0])]:
            bad=copy.deepcopy(original);mutate(bad)
            with self.assertRaises(P.Refused)as error:P.finite_diagnostics(bad)
            self.assertNotIn(SECRET,str(error.exception))
        for raw in [b'{"version":1,"version":1}',b'{"driver_lines":[NaN]}',b'{"count":Infinity}']:
            with self.assertRaises(P.Refused):P.decode(raw)

    def test_failed_summary_retains_finite_error_without_changing_pass_predicate(self):
        _,m,s,r=OLD.inputs();recorder=P.FiniteRuntimeDiagnostics();actual_failure(recorder);r['finite_diagnostics']=recorder.snapshot();r['status']='failed_or_interrupted';r['failure']={'raw':SECRET}
        safe=P.project(r,m,s,'normal',1,'e'*64)
        self.assertEqual(safe['status'],'failed');self.assertEqual(safe['failure_kind'],'runtime_failed')
        self.assertEqual(safe['finite_diagnostics']['failure_class'],'assertion');self.assertNotIn(SECRET,json.dumps(safe))

    def test_invalid_diagnostics_make_real_projector_cli_nonzero_and_never_publish_raw(self):
        raw,_,_,report=OLD.inputs();report['finite_diagnostics']={'version':1,'message':SECRET}
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);private=root/'private';private.mkdir(mode=0o700);(private/'report.json').write_text(json.dumps(report));(private/'report.json').chmod(0o600);manifest=root/'manifest';manifest.write_bytes(raw)
            argv=['projection','--private-dir',str(private),'--manifest',str(manifest),'--case','normal','--exit-code','0','--output',str(root/'out')]
            # Bind actual harness SHA, not the static unit placeholder.
            report['harness_sha256']=P.sha((ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_bytes());(private/'report.json').write_text(json.dumps(report))
            with mock.patch('sys.argv',argv):self.assertEqual(P.main(),1)
            published=(root/'out/summary.json').read_text();self.assertNotIn(SECRET,published);self.assertEqual(json.loads(published)['failure_kind'],'report_invalid')

    def test_driver_hook_preserves_single_backend_call_and_original_reply(self):
        source=(ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_text();tree=ast.parse(source)
        driver=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
        host=next(n for n in ast.walk(driver)if isinstance(n,ast.ClassDef)and n.name=='ObservedHost')
        response=reply();raw=b'original unit reply bytes';calls=[]
        class Base:
            def wire(self,call):calls.append(call);return response,raw
        class Recorder:
            def begin(self,call):return {'method':call.method,'params':call.params,'key':call.key,'revision':call.revision}
            def finish(self,slot,received):self.received=received
            def failed(self,*args):raise AssertionError('unexpected observer failure')
        recorder=Recorder();finite=P.FiniteRuntimeDiagnostics();env={'base':types.SimpleNamespace(PrivateHost=Base),'wire_recorder':recorder,'finite':finite,'time':types.SimpleNamespace(monotonic_ns=lambda:1),'FixedCall':lambda *a:types.SimpleNamespace(method=a[0],params=a[1],key=a[2],revision=a[3])}
        exec(compile(ast.fix_missing_locations(ast.Module(body=[host],type_ignores=[])),'<inert exact wire hook>','exec'),env)
        call=types.SimpleNamespace(method='system.snapshot',params={'private':SECRET},key=SECRET,revision=1)
        value,original=env['ObservedHost']().wire(call)
        self.assertIs(value,response);self.assertIs(original,raw);self.assertIs(recorder.received,raw);self.assertEqual(len(calls),1)
        self.assertNotIn(SECRET,json.dumps(P.finite_diagnostics(finite.snapshot())))

if __name__=='__main__':unittest.main()
