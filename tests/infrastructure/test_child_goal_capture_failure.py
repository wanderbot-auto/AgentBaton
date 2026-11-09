"""Synthetic failure-only metadata and exact source controls; no browser/IPC."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import types
import unittest
from unittest import mock

from tests.infrastructure import child_goal_capture_source_fixture as S
from tests.infrastructure import test_child_goal_finite_diagnostics as F

ROOT=Path(__file__).resolve().parents[2]
P=F.P
SECRET='PRIVATE_PATH_TEXT_PID_TOKEN_CAPTURE_CANARY'
DRIVER=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
BASE=ROOT/'apps/desktop/tests/factory-real-ipc.py'


def caught(error,filename=None,line=31):
    try:
        if filename is None:raise error
        exec(compile('\n'*(line-1)+'raise original\n',str(filename),'exec'),{'original':error})
    except BaseException as observed:return observed


def snapshot(error,substage='started_records',filename=BASE,line=31):
    recorder=P.FiniteRuntimeDiagnosticsV5();error=caught(error,filename,line)
    recorder.capture_failure(substage,error)
    recorder.failure(error,'runtime',str(DRIVER),ValueError)
    return recorder,recorder.snapshot()


class CaptureFailureRuntimeObservations(unittest.TestCase):
    def capture(self,recorder,base_call,graph_call):
        tree=ast.parse(DRIVER.read_text())
        runtime=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='runtime_driver')
        node=next(n for n in runtime.body if isinstance(n,ast.FunctionDef)and n.name=='capture')
        namespace={name:object()for name in ['host','workspace','scope','runtime','verified','output','reaper','graph_verified']}
        namespace.update(record_context={'run_id':'generated'},base=types.SimpleNamespace(collect_started_records=base_call),collect_graph_records=graph_call,finite=recorder)
        exec(compile(ast.Module(body=[node],type_ignores=[]),str(DRIVER),'exec'),namespace)
        return namespace['capture']

    def test_success_runs_only_the_two_original_collectors_in_order_and_stays_exact_v4(self):
        calls=[];recorder=P.FiniteRuntimeDiagnosticsV5()
        capture=self.capture(recorder,lambda *args:calls.append('started'),lambda *args:calls.append('graph'))
        with mock.patch.object(P,'capture_failure_observation',side_effect=AssertionError('No success observation')):
            capture()
        self.assertEqual(calls,['started','graph'])
        self.assertEqual(recorder.snapshot(),P.FiniteRuntimeDiagnosticsV4().snapshot())
        self.assertEqual(recorder.snapshot()['version'],4)
        self.assertNotIn('capture_failure',recorder.snapshot())

    def test_each_collector_raises_the_same_original_exception_and_retains_its_stage(self):
        for stage in ['started_records','graph_records']:
            with self.subTest(stage=stage):
                recorder=P.FiniteRuntimeDiagnosticsV5();error=OSError(3,SECRET,SECRET);calls=[]
                def fail(*args):calls.append(stage);raise error
                def ok(*args):calls.append('started_records')
                capture=self.capture(recorder,fail if stage=='started_records'else ok,fail)
                try:capture()
                except OSError as observed:
                    self.assertIs(observed,error);recorder.failure(observed,'runtime',str(DRIVER),ValueError)
                else:self.fail('Original error was swallowed')
                value=P.finite_diagnostics(recorder.snapshot())
                self.assertEqual(value['version'],5);self.assertEqual(value['failure_class'],'os_error')
                self.assertEqual(value['capture_failure']['substage'],stage)
                self.assertEqual(value['capture_failure']['errno'],3)
                self.assertEqual(value['capture_failure']['helper_source'],'child_goal_driver')
                self.assertNotIn(SECRET,json.dumps(value))
                self.assertEqual(len(calls),1 if stage=='started_records' else 2)

    def test_exact_base_traceback_frame_is_retained_but_foreign_same_basename_is_unknown(self):
        _,value=snapshot(OSError(2,SECRET,SECRET),filename=BASE,line=71)
        self.assertEqual(value['capture_failure'],{'substage':'started_records','errno':2,'helper_source':'factory_base','helper_line':71})
        _,value=snapshot(OSError(2,SECRET),filename=Path('/foreign/private/factory-real-ipc.py'))
        self.assertIsNone(value['capture_failure']['helper_source']);self.assertIsNone(value['capture_failure']['helper_line'])
        self.assertNotIn('/foreign',json.dumps(value))

    def test_unknown_or_untyped_errno_is_null_without_coercion_and_known_bounds_survive(self):
        for number in [True,0,-1,4096,'PRIVATE',1.0,None]:
            error=OSError(2,SECRET);error.errno=number
            _,value=snapshot(error);self.assertIsNone(value['capture_failure']['errno'])
        for number in [1,4095]:
            _,value=snapshot(OSError(number,SECRET));self.assertEqual(value['capture_failure']['errno'],number)
        error=RuntimeError(SECRET);error.errno=3
        _,value=snapshot(error);self.assertIsNone(value['capture_failure']['errno'])

    def test_timeout_errno_remains_timeout_and_unknown_sources_are_null(self):
        _,value=snapshot(TimeoutError(110,SECRET),filename=None)
        self.assertEqual(P.finite_diagnostics(value)['failure_class'],'timeout')
        self.assertEqual(value['capture_failure']['errno'],110)
        self.assertIsNone(value['capture_failure']['helper_source'])

    def test_later_cleanup_capture_cannot_attach_to_the_original_non_capture_error(self):
        recorder=P.FiniteRuntimeDiagnosticsV5();first=caught(AssertionError(SECRET));recorder.failure(first,'runtime',str(DRIVER),ValueError)
        before=recorder.snapshot();later=caught(OSError(2,SECRET),BASE);recorder.capture_failure('started_records',later);recorder.failure(later,'cleanup',str(DRIVER),ValueError)
        self.assertEqual(recorder.snapshot(),before);self.assertEqual(before['version'],4)

    def test_first_capture_and_original_failure_are_not_replaced_by_cleanup(self):
        recorder,value=snapshot(OSError(3,SECRET),filename=BASE,line=51)
        later=caught(OSError(2,SECRET),DRIVER,line=65);recorder.capture_failure('graph_records',later);recorder.failure(later,'cleanup',str(DRIVER),ValueError)
        self.assertEqual(recorder.snapshot(),value)
        self.assertEqual(value['capture_failure']['helper_line'],51)

    def test_missing_or_failed_diagnostic_method_never_replaces_original_error(self):
        error=OSError(3,SECRET)
        def fail(*args):raise error
        for recorder in [object(),types.SimpleNamespace(capture_failure=mock.Mock(side_effect=RuntimeError(SECRET)))]:
            capture=self.capture(recorder,fail,lambda *args:None)
            try:capture()
            except OSError as observed:self.assertIs(observed,error)
            else:self.fail('Original exception was replaced')

    def test_diagnostic_failure_never_replaces_the_original_exception(self):
        error=OSError(2,SECRET);recorder=P.FiniteRuntimeDiagnosticsV5()
        def fail(*args):raise error
        capture=self.capture(recorder,fail,lambda *args:None)
        with mock.patch.object(P,'capture_failure_observation',side_effect=RuntimeError(SECRET)):
            try:capture()
            except OSError as observed:self.assertIs(observed,error);recorder.failure(observed,'runtime',str(DRIVER),ValueError)
            else:self.fail('Original exception was replaced')
        self.assertEqual(recorder.snapshot()['version'],4)
        self.assertNotIn('capture_failure',recorder.snapshot())


class ClosedCaptureProjection(unittest.TestCase):
    def test_failed_projection_keeps_original_failure_gate_and_never_publishes_private_data(self):
        _,manifest,summary,report=F.OLD.inputs();_,value=snapshot(OSError(3,SECRET))
        report.update(status='failed_or_interrupted',failure={'message':SECRET},finite_diagnostics=value)
        result=P.project(report,manifest,summary,'normal',1,'e'*64)
        self.assertEqual(result['status'],'failed');self.assertFalse(result['screenshots_published'])
        self.assertEqual(result['finite_diagnostics']['capture_failure']['errno'],3)
        self.assertNotIn(SECRET,json.dumps(result))
        report['status']='passed'
        with self.assertRaises(P.Refused):P.project(report,manifest,summary,'normal',1,'e'*64)
        report['status']='failed_or_interrupted'
        with self.assertRaises(P.Refused):P.project(report,manifest,summary,'normal',0,'e'*64)

    def test_unknown_extra_private_bool_or_inconsistent_closed_fields_are_rejected(self):
        _,good=snapshot(OSError(2,SECRET))
        changes=[lambda v:v['capture_failure'].update(substage=SECRET),lambda v:v['capture_failure'].update(errno=True),
            lambda v:v['capture_failure'].update(errno=0),lambda v:v['capture_failure'].update(errno=4096),
            lambda v:v['capture_failure'].update(errno=SECRET),lambda v:v['capture_failure'].update(helper_source=SECRET),
            lambda v:v['capture_failure'].update(helper_source=None),lambda v:v['capture_failure'].update(helper_line=None),
            lambda v:v['capture_failure'].update(helper_line=True),lambda v:v['capture_failure'].update(helper_line=0),
            lambda v:v['capture_failure'].update(helper_line=1000001),lambda v:v['capture_failure'].update(path=SECRET),
            lambda v:v.update(failure_class=None),lambda v:v.update(failure_class='assertion'),lambda v:v.update(extra=SECRET)]
        for change in changes:
            value=copy.deepcopy(good);change(value)
            with self.assertRaises(P.Refused):P.finite_diagnostics(value)
        legacy=copy.deepcopy(good);legacy.pop('capture_failure');legacy['version']=4
        self.assertEqual(P.finite_diagnostics(legacy),legacy)
        legacy['capture_failure']=good['capture_failure']
        with self.assertRaises(P.Refused):P.finite_diagnostics(legacy)


class CaptureSourceInverses(unittest.TestCase):
    def test_complete_original_driver_and_projector_bytes_restore_with_all_old_pins(self):
        for path,record in S.LAYERS.items():
            from tests.infrastructure.schema18_source_fixture import strip_schema18_source
            text=strip_schema18_source(path,(ROOT/path).read_text());self.assertEqual(hashlib.sha256(text.encode()).hexdigest(),record['after'])
            restored=S.strip_capture_failure_layer(path,text)
            self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),record['before'])
            self.assertEqual(S.strip_capture_failure_layer(path,restored),restored)

    def test_all_original_v4_test_bytes_pins_and_negative_vectors_restore_exactly(self):
        text=(ROOT/'tests/infrastructure/test_child_goal_diagnostics_v4.py').read_text()
        restored=S.strip_capture_test_guard_layer(text)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),S.TEST_GUARD_LAYER['before'])
        self.assertEqual(S.strip_capture_test_guard_layer(restored),restored)
        for before,after in S.TEST_GUARD_LAYER['edits']:
            with self.assertRaises(AssertionError):S.strip_capture_test_guard_layer(text.replace(after,before,1))
        with self.assertRaises(AssertionError):S.strip_capture_test_guard_layer(text+'# unknown guard change\n')

    def test_missing_duplicate_unknown_moved_layers_and_original_guards_are_not_normalized(self):
        for path,record in S.LAYERS.items():
            text=(ROOT/path).read_text();changes=[text+'\n# unknown\n',text.replace('schema_required=17','schema_required=16',1)if 'schema_required=17'in text else text.replace("'schema'", "'foreign_schema'",1)]
            for before,after in record['edits']:
                changes.extend([text.replace(after,before,1),text+after,text.replace(after,'',1)+after,text.replace(after,after.replace('\n','\r\n')if '\n'in after else after+' ',1)])
            for changed in changes:
                self.assertNotEqual(changed,text)
                # A completely restored original is explicitly valid; partial/moved layers are not.
                if hashlib.sha256(changed.encode()).hexdigest()in record['legacy']:continue
                with self.assertRaises(AssertionError):S.strip_capture_failure_layer(path,changed)


if __name__=='__main__':unittest.main()
