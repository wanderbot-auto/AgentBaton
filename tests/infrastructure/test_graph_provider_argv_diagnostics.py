"""Finite argv diagnostic controls; no live process, Core or browser execution."""
import ast
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
P=types.ModuleType('graph_argv_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')
exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)
spec=importlib.util.spec_from_file_location('graph_argv_old_projection_controls',Path(__file__).with_name('test_child_goal_ci_evidence.py'))
OLD=importlib.util.module_from_spec(spec);spec.loader.exec_module(OLD)
SECRET='PRIVATE_ARGV_TOKEN_PATH_ID_MESSAGE'
EXPECTED=[SECRET,'--fixture-settings','/private/'+SECRET]


def failure(reason='empty',bound=True):
    return P.GraphProviderArgvFailure(reason,bound)


def observed(reason='empty',bound=True):
    recorder=P.FiniteRuntimeDiagnostics()
    recorder.failure(failure(reason,bound),'runtime',__file__,ValueError)
    return recorder


class GraphProviderArgvDiagnosticsTests(unittest.TestCase):
    def test_original_success_accepts_exact_last_three_with_interpreter_prefix(self):
        for argv in [EXPECTED,['python',*EXPECTED]]:
            raw=b'\0'.join(v.encode()for v in argv)+b'\0'
            for bound in [False,True]:
                self.assertIsNone(P.check_graph_provider_argv(raw,EXPECTED,bound))

    def test_original_8192_byte_boundary_is_preserved(self):
        expected=['p'*8180,'--s','x'*6]
        raw=b'\0'.join(v.encode()for v in expected)+b'\0'
        self.assertEqual(len(raw),8192)
        self.assertIsNone(P.check_graph_provider_argv(raw,expected,False))

    def test_each_original_refusal_is_still_assertion_and_only_fixed_reason_leaves(self):
        rows=[(b'','empty'),(b'x'*8193,'oversized'),(SECRET.encode(),'missing_nul'),
              (b'foreign\0--fixture-settings\0/private/other\0','argv_mismatch')]
        for raw,reason in rows:
            for bound in [False,True]:
                with self.subTest(reason=reason,bound=bound),self.assertRaises(P.GraphProviderArgvFailure)as caught:
                    P.check_graph_provider_argv(raw,EXPECTED,bound)
                self.assertIsInstance(caught.exception,AssertionError)
                self.assertEqual(caught.exception.diagnostic,{'reason':reason,'previously_bound':bound})
                self.assertNotIn(SECRET,str(caught.exception))
                recorder=P.FiniteRuntimeDiagnostics();recorder.failure(caught.exception,'runtime',__file__,ValueError)
                value=P.finite_diagnostics(recorder.snapshot())
                self.assertEqual(value['failure_class'],'assertion')
                self.assertEqual(value['graph_provider_argv'],{'reason':reason,'previously_bound':bound})
                self.assertNotIn(SECRET,json.dumps(value))

    def test_decode_failure_keeps_original_value_error_and_no_argv_assertion_claim(self):
        with self.assertRaises(UnicodeDecodeError)as caught:
            P.check_graph_provider_argv(b'\xff\0--fixture-settings\0file\0',EXPECTED,True)
        recorder=P.FiniteRuntimeDiagnostics();recorder.failure(caught.exception,'runtime',__file__,RuntimeError)
        value=P.finite_diagnostics(recorder.snapshot())
        self.assertEqual(value['failure_class'],'value_error');self.assertIsNone(value['graph_provider_argv'])

    def test_fixed_original_guard_acceptance_and_error_kind_are_unchanged(self):
        def original(raw):
            assert 0<len(raw)<=8192 and raw.endswith(b'\0')
            assert [v.decode()for v in raw[:-1].split(b'\0')][-3:]==EXPECTED
        def kind(call):
            try:call();return 'accepted'
            except AssertionError:return 'assertion'
            except UnicodeDecodeError:return 'unicode_decode_error'
        exact=b'\0'.join(v.encode()for v in EXPECTED)+b'\0'
        for raw in [b'',b'x'*8193,b'x'*8192,b'x',b'\0',b'foreign\0',
                    b'\xff\0x\0y\0',exact,b'python\0'+exact,exact+b'other\0']:
            with self.subTest(bytes=len(raw)):
                self.assertEqual(kind(lambda:original(raw)),kind(lambda:P.check_graph_provider_argv(raw,EXPECTED,True)))

    def test_version1_stays_exact_and_cannot_smuggle_new_field(self):
        value=P.unobserved_diagnostics();value['version']=1;value.pop('graph_provider_argv')
        self.assertEqual(P.finite_diagnostics(value),value)
        bad=copy.deepcopy(value);bad['graph_provider_argv']={'reason':'empty','previously_bound':True}
        with self.assertRaises(P.Refused):P.finite_diagnostics(bad)
        bad=copy.deepcopy(value);bad['extra']=SECRET
        with self.assertRaises(P.Refused):P.finite_diagnostics(bad)

    def test_version2_requires_its_exact_field_and_explicit_unobserved_null(self):
        value=P.unobserved_diagnostics();self.assertEqual(value['version'],2)
        self.assertIsNone(P.finite_diagnostics(value)['graph_provider_argv'])
        for version in [True,2.0,'2',3]:
            bad=copy.deepcopy(value);bad['version']=version
            with self.assertRaises(P.Refused):P.finite_diagnostics(bad)
        bad=copy.deepcopy(value);bad.pop('graph_provider_argv')
        with self.assertRaises(P.Refused):P.finite_diagnostics(bad)

    def test_exact_finite_fields_types_and_reason_vocabulary_refuse_private_data(self):
        original=observed().snapshot()
        mutations=[lambda v:v.update(graph_provider_argv=SECRET),
            lambda v:v['graph_provider_argv'].update(reason=SECRET),
            lambda v:v['graph_provider_argv'].update(previously_bound=1),
            lambda v:v['graph_provider_argv'].update(previously_bound=1.0),
            lambda v:v['graph_provider_argv'].update(previously_bound=None),
            lambda v:v['graph_provider_argv'].update(argv=SECRET),
            lambda v:v['graph_provider_argv'].update(identity=SECRET),
            lambda v:v['graph_provider_argv'].pop('reason'),
            lambda v:v.update(failure_class='timeout')]
        for mutate in mutations:
            bad=copy.deepcopy(original);mutate(bad)
            with self.assertRaises(P.Refused)as caught:P.finite_diagnostics(bad)
            self.assertNotIn(SECRET,str(caught.exception))

    def test_first_failure_wins_even_if_later_argv_or_cleanup_failure_differs(self):
        recorder=observed('empty',True);first=recorder.snapshot()
        recorder.failure(failure('oversized',False),'cleanup',__file__,ValueError)
        self.assertEqual(P.finite_diagnostics(recorder.snapshot()),first)
        recorder=P.FiniteRuntimeDiagnostics();recorder.failure(AssertionError(SECRET),'runtime',__file__,ValueError)
        recorder.failure(failure(),'runtime',__file__,ValueError)
        self.assertIsNone(P.finite_diagnostics(recorder.snapshot())['graph_provider_argv'])

    def test_real_projection_keeps_failed_status_and_all_original_acceptance_conditions(self):
        _,candidate,summary,report=OLD.inputs('cancel')
        report['finite_diagnostics']=observed().snapshot();report['status']='failed_or_interrupted'
        report['failure']={'message':SECRET};report['cleanup_complete']=False
        value=P.project(report,candidate,summary,'cancel',1,'e'*64)
        self.assertEqual(value['status'],'failed');self.assertFalse(value['cleanup_complete'])
        self.assertEqual(value['failure_kind'],'runtime_failed')
        self.assertEqual(value['finite_diagnostics']['graph_provider_argv'],{'reason':'empty','previously_bound':True})
        self.assertNotIn(SECRET,json.dumps(value))

    def test_argv_metadata_alone_cannot_make_a_failed_run_pass(self):
        _,candidate,summary,report=OLD.inputs('cancel')
        report['finite_diagnostics']=observed().snapshot()
        # An argv diagnostic claims a failure, so it is inconsistent with a
        # purported all-passed report even when its old checks/cleanup look green.
        self.assertEqual(P.project(report,candidate,summary,'cancel',0,'e'*64)['status'],'failed')

    def test_real_publisher_retains_reason_but_no_private_report_or_argv(self):
        raw,_,_,report=OLD.inputs('cancel');report['harness_sha256']=P.sha((ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_bytes())
        report.update(status='failed_or_interrupted',failure={'message':SECRET},cleanup_complete=False,finite_diagnostics=observed('argv_mismatch',False).snapshot())
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);private=root/'private';private.mkdir(mode=0o700)
            original=json.dumps(report).encode();(private/'report.json').write_bytes(original);(private/'report.json').chmod(0o600)
            manifest=root/'manifest.json';manifest.write_bytes(raw);out=root/'published'
            argv=['projection','--private-dir',str(private),'--manifest',str(manifest),'--case','cancel','--exit-code','1','--output',str(out)]
            with mock.patch('sys.argv',argv):self.assertEqual(P.main(),1)
            value=(out/'summary.json').read_text();self.assertNotIn(SECRET,value)
            self.assertEqual(json.loads(value)['finite_diagnostics']['graph_provider_argv'],{'reason':'argv_mismatch','previously_bound':False})
            self.assertEqual([p.name for p in out.iterdir()],['summary.json'])
            self.assertEqual((private/'report.json').read_bytes(),original)

    def test_driver_call_remains_within_existing_identity_and_pgid_guards(self):
        tree=ast.parse((ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_text())
        function=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='collect_graph_records')
        calls=[n for n in ast.walk(function)if isinstance(n,ast.Call)and isinstance(n.func,ast.Attribute)and n.func.attr=='check_graph_provider_argv']
        self.assertEqual(len(calls),1);call=calls[0]
        self.assertEqual(ast.unparse(call.args[2]),'old is not None')
        body=ast.unparse(function)
        self.assertIn('old is None or old == fixed',body)
        self.assertIn("current['start_ticks'] == identity['start_ticks']",body)
        self.assertIn("os.getpgid(identity['pid']) == identity['pgid']",body)
        self.assertNotIn('sleep',body);self.assertNotIn('cancel_requested',body)


if __name__=='__main__':unittest.main()
