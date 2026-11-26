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
from tests.infrastructure import host19_child_goal_source_fixture as H
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
            text=(ROOT/path).read_text()
            if path == H.PROJECTOR:
                text = H.restore_mcp_late_wait_layer(text)
            changes=[text+'\n# unknown\n',text.replace('schema_required=17','schema_required=16',1)if 'schema_required=17'in text else text.replace("'schema'", "'foreign_schema'",1)]
            for before,after in record['edits']:
                changes.extend([text.replace(after,before,1),text+after,text.replace(after,'',1)+after,text.replace(after,after.replace('\n','\r\n')if '\n'in after else after+' ',1)])
            for changed in changes:
                self.assertNotEqual(changed,text)
                # A completely restored original is explicitly valid; partial/moved layers are not.
                if hashlib.sha256(changed.encode()).hexdigest()in record['legacy']:continue
                with self.assertRaises(AssertionError):S.strip_capture_failure_layer(path,changed)


class McpLateWaitProjection(unittest.TestCase):
    def observation(self):
        return {'stage': 'bound_MCP_late_wait_still_present', 'before_state': 'S', 'after_state': 'Z',
                'after_parent_is_controller': True, 'birth_matches': True,
                'pgid_matches': True, 'pgid_error_type': None}

    def assertion(self, observation, *, filename=BASE, name='collect_started_records', line=556,
                  changed_binding=False, current=None, controller=12345, error=None):
        code = '\n' * (line - 2) + (
            f'def {name}(late_observation, old, fixed, current, reaper, error):\n'
            '    raise error\n'
        )
        namespace = {}
        exec(compile(code, str(filename), 'exec'), namespace)
        old = {'path': SECRET, 'pid': 12345, 'identity': {'birth': SECRET}}
        fixed = {**old, 'path': 'changed'} if changed_binding else copy.deepcopy(old)
        current = {'parent_pid': 54321, 'state': 'S'} if current is None else current
        if error is None:
            error = AssertionError(SECRET)
        try:
            namespace[name](observation, old, fixed, current, types.SimpleNamespace(pid=controller), error)
        except BaseException as observed:
            self.assertIs(observed, error)
            return observed
        self.fail('Synthetic assertion did not raise')

    def record(self, error, substage='started_records'):
        recorder = P.FiniteRuntimeDiagnosticsV5()
        recorder.completed('CG02')
        recorder.capture_failure(substage, error)
        recorder.failure(error, 'runtime', str(DRIVER), ValueError)
        return recorder.snapshot()

    def test_locator_names_the_unchanged_actual_assertion(self):
        raw = BASE.read_bytes()
        pinned = H.restore_factory_before_ui_simplification(BASE.relative_to(ROOT).as_posix(), raw.decode())
        self.assertEqual(hashlib.sha256(pinned.encode()).hexdigest(),
                         '6dfa764259b667f57fe0a579ecbbe242d6a4d2b253e1878f158e68cd161a1d93')
        collector = next(node for node in ast.parse(raw).body
                         if isinstance(node, ast.FunctionDef) and node.name == 'collect_started_records')
        exact = [node for node in ast.walk(collector) if isinstance(node, ast.Assert)
                 and node.lineno == P.MCP_LATE_WAIT_CAPTURE['helper_line']]
        self.assertEqual(len(exact), 1)
        self.assertIn('Bound MCP still exists after late adopted-child wait', ast.unparse(exact[0].msg))

    def test_exact_first_assertion_projects_only_closed_scalars_without_probes(self):
        error = self.assertion(self.observation())
        with (mock.patch.object(P.os, 'getpid', side_effect=AssertionError('Unexpected probe')) as pid,
              mock.patch.object(P.os, 'getpgid', side_effect=AssertionError('Unexpected probe')) as pgid,
              mock.patch.object(Path, 'open', side_effect=AssertionError('Unexpected read')) as read):
            got = P.finite_diagnostics(self.record(error))
        for probe in (pid, pgid, read):
            probe.assert_not_called()
        self.assertEqual(got['version'], 7)
        self.assertEqual(got['capture_failure'], P.MCP_LATE_WAIT_CAPTURE)
        self.assertEqual(got['mcp_late_wait'], {
            'before_state': 'sleeping', 'after_state': 'zombie', 'binding_matches': True, 'birth_matches': True,
            'before_parent_is_controller': False, 'after_parent_is_controller': True, 'process_group_match': 'match',
        })
        public = json.dumps(got)
        for private in (SECRET, '12345', '54321', str(BASE), 'pgid_error_type'):
            self.assertNotIn(private, public)

    def test_mismatch_and_unobserved_process_group_never_publish_raw_error(self):
        for observed, private, group in ((False, None, 'mismatch'), (None, SECRET, 'unobserved')):
            with self.subTest(group=group):
                raw = {**self.observation(), 'pgid_matches': observed, 'pgid_error_type': private}
                got = self.record(self.assertion(raw, current={'parent_pid': 12345, 'state': 'S'}))
                self.assertIs(got['mcp_late_wait']['before_parent_is_controller'], True)
                self.assertEqual(P.finite_diagnostics(got)['mcp_late_wait']['process_group_match'], group)
                self.assertNotIn(SECRET, json.dumps(got))

    def test_missing_invalid_or_foreign_capture_preserves_legacy_v5(self):
        raw = self.observation()
        cases = (
            (None, {}),
            ({**raw, 'before_state': SECRET}, {}),
            ({**raw, 'after_parent_is_controller': 1}, {}),
            ({**raw, 'pgid_matches': None}, {}),
            ({**raw, 'pid': 12345}, {}),
            (raw, {'controller': None}),
            (raw, {'changed_binding': True}),
            (raw, {'current': {'parent_pid': True, 'state': 'S'}}),
            (raw, {'filename': Path('/private/foreign/factory-real-ipc.py')}),
            (raw, {'name': 'unrelated_assertion'}),
            (raw, {'line': 557}),
            (raw, {'error': RuntimeError(SECRET)}),
        )
        for observation, options in cases:
            with self.subTest(options=options, observation=observation):
                got = self.record(self.assertion(observation, **options))
                self.assertEqual(P.finite_diagnostics(got)['version'], 5)
                self.assertNotIn('mcp_late_wait', got)
                self.assertNotIn(SECRET, json.dumps(got))
        self.assertEqual(self.record(self.assertion(raw), substage='graph_records')['version'], 5)
        with mock.patch.object(P, 'mcp_late_wait_observation', side_effect=RuntimeError(SECRET)):
            self.assertEqual(self.record(self.assertion(raw))['version'], 5)

    def test_later_assertion_cannot_extend_the_first_failure(self):
        recorder = P.FiniteRuntimeDiagnosticsV5()
        original = caught(AssertionError(SECRET), filename=BASE, line=557)
        recorder.capture_failure('started_records', original)
        recorder.failure(original, 'runtime', str(DRIVER), ValueError)
        baseline = recorder.snapshot()
        later = self.assertion(self.observation())
        recorder.capture_failure('started_records', later)
        recorder.failure(later, 'cleanup', str(DRIVER), ValueError)
        self.assertEqual(recorder.snapshot(), baseline)
        self.assertEqual(baseline['version'], 5)

    def test_invalid_v7_and_versions_without_exact_capture_fields_are_refused(self):
        good = self.record(self.assertion(self.observation()))
        checks = (
            lambda row: row.pop('mcp_late_wait'),
            lambda row: row.update(version=5),
            lambda row: row.update(version=7.0),
            lambda row: row.update(failure_class='runtime_error'),
            lambda row: row['capture_failure'].update(helper_line=557),
            lambda row: row['capture_failure'].update(substage='graph_records'),
            lambda row: row['mcp_late_wait'].update(after_state=SECRET),
            lambda row: row['mcp_late_wait'].update(process_group_match=SECRET),
            lambda row: row['mcp_late_wait'].update(estimated_pid=12345),
            lambda row: row['mcp_late_wait'].update(after_parent_is_controller=1),
            lambda row: row['mcp_late_wait'].update(binding_matches=False),
            lambda row: row['mcp_late_wait'].update(birth_matches=False),
            lambda row: row['mcp_late_wait'].update(before_state='zombie', after_parent_is_controller=False),
        )
        for change in checks:
            value = copy.deepcopy(good)
            change(value)
            with self.assertRaises(P.Refused):
                P.finite_diagnostics(value)
        _, manifest, summary, report = F.OLD.inputs()
        report.update(status='failed_or_interrupted', failure={'message': SECRET}, finite_diagnostics=good)
        result = P.project(report, manifest, summary, 'normal', 1, 'e' * 64)
        self.assertEqual(result['finite_diagnostics'], good)
        self.assertNotIn(SECRET, json.dumps(result))
        with self.assertRaises(P.Refused):
            P.project(report, manifest, summary, 'normal', 0, 'e' * 64)
        report['status'] = 'passed'
        with self.assertRaises(P.Refused):
            P.project(report, manifest, summary, 'normal', 1, 'e' * 64)

    def test_old_failure_and_success_projection_bytes_remain_identical(self):
        path = Path(P.__file__)
        module = types.ModuleType('prior_mcp_failure_projector')
        module.__file__ = str(path)
        exec(compile(H.restore_mcp_late_wait_layer(path.read_text()), str(path), 'exec'), module.__dict__)
        for status in ('passed', 'failed_or_interrupted'):
            _, manifest, summary, report = F.OLD.inputs()
            recorder = P.FiniteRuntimeDiagnosticsV5()
            if status != 'passed':
                error = caught(OSError(2, SECRET), filename=BASE, line=555)
                recorder.capture_failure('started_records', error)
                recorder.failure(error, 'runtime', str(DRIVER), ValueError)
            expected_version = 4 if status == 'passed' else 5
            report.update(status=status, finite_diagnostics=recorder.snapshot())
            code = int(status != 'passed')
            if code:
                report['failure'] = {'message': SECRET}
            old = module.project(report, manifest, summary, 'normal', code, 'e' * 64)
            new = P.project(report, manifest, summary, 'normal', code, 'e' * 64)
            self.assertEqual(new['finite_diagnostics']['version'], expected_version)
            self.assertEqual(json.dumps(new, indent=2), json.dumps(old, indent=2))

    def test_exact_outer_inverse_preserves_the_prior_and_historical_source(self):
        raw = Path(P.__file__).read_text()
        old = H.restore_mcp_late_wait_layer(raw)
        self.assertEqual(H.sha(raw), H.MCP_LATE_WAIT_LAYER[0])
        self.assertEqual(H.sha(old), '8d03881dd61db2c398f96a922190c779079d9cfcd9635483ffea185a6acbc741')
        self.assertEqual(H.restore_mcp_late_wait_layer(old), old)
        self.assertEqual(H.restore_child_goal_before_schema19(H.PROJECTOR, raw),
                         H.restore_child_goal_before_schema19(H.PROJECTOR, old))
        for altered in (raw + '# unknown\n', raw.replace('== 7:', '== 8:', 1),
                        raw.replace("'match', 'mismatch', 'unobserved'", "'match', 'mismatch', 'unobserved', 'other'", 1)):
            with self.assertRaises(AssertionError):
                H.restore_mcp_late_wait_layer(altered)
            with self.assertRaises(AssertionError):
                H.restore_child_goal_before_schema19(H.PROJECTOR, altered)


if __name__=='__main__':unittest.main()
