"""Exact collector fault injection with own files/SQLite; no Core/Actor/process.

AST loading avoids importing Playwright or executing either browser harness.
The three SQL tables are generated relation surrogates, not real Core authority.
No fake result from these controls is used as runtime/stop/Owner acceptance.
"""
import ast
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import sqlite3
import stat
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SUBJECT = ROOT / 'apps/desktop/tests/factory-real-ipc.py'
SUPPORT = ROOT / 'apps/desktop/tests/goal-graph-real-ipc.py'


def load_collector():
    names = {'sha', 'bounded_file', 'write_private_json', '_bound_identity_disappeared', 'collect_started_records'}
    tree = ast.parse(SUBJECT.read_text(), filename=str(SUBJECT))
    nodes = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names
             or isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id in ('REQUEST_FIELDS','OBSERVER_ACK_CODE') for t in node.targets)]
    support_tree = ast.parse(SUPPORT.read_text(), filename=str(SUPPORT))
    support_nodes = [n for n in support_tree.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))
                     and n.name in ('BoundaryRefusal', 'require', 'unique_object')]
    ns = {'hashlib': hashlib, 'json': json, 'os': os, 'Path': Path, 're': re,
          'sqlite3': sqlite3, 'stat': stat, 'time':time, 'MAX_HTTP_BYTES': 270336}
    exec(compile(ast.Module(body=support_nodes, type_ignores=[]), str(SUPPORT), 'exec'), ns)
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SUBJECT), 'exec'), ns)
    exec(ns['OBSERVER_ACK_CODE'],ns)
    return ns


class Fixture:
    """Uses actual collector bindings; only proc reads/getpgid are injected."""
    def __init__(self, root, bound=True):
        self.ns = load_collector()
        self.runtime, self.work, self.out = root / 'runtime', root / 'workspace', root / 'output'
        for p in (self.runtime, self.work, self.out):
            p.mkdir(mode=0o700)
        (self.work / 'reports').mkdir(mode=0o700)
        handles = self.runtime / 'factory-scoped-handles'
        handles.mkdir(mode=0o700)
        self.context = {'nonce': 'unit-only-nonce', 'connection_id': 'unit-connection', 'run_id': 'unit-run'}
        for key in ('provider', 'settings', 'companion'):
            path = root / key
            path.write_bytes(('Inert unit metadata only: '+key).encode())
            path.chmod(0o600)
            self.context[key] = str(path)
            self.context['program_sha256' if key == 'provider' else key+'_sha256'] = self.ns['sha'](path.read_bytes())
        self.provider, self.companion = 12124, 12126  # never queried in real /proc
        self.scope = {'id': 'unit-scope', 'root_task_id': 'unit-root', 'root_grant_id': 'unit-grant'}
        request = {key: None for key in self.ns['REQUEST_FIELDS']}
        request.update(schema_version=1, kind='agentbaton_factory_task_request', task_id='unit-root',
            root_task_id='unit-root', parent_task_id=None, goal='No real execution', handoff_results=[],
            context_continuation='new_attempt_explicit_handoff_only', acceptance_state='not_established')
        request_text = json.dumps(request, sort_keys=True)
        self.row = {'fixture_nonce': self.context['nonce'], 'program_sha256': self.context['program_sha256'],
            'settings_sha256': self.context['settings_sha256'], 'request_text': request_text,
            'request_sha256': self.ns['sha'](request_text.encode()), 'stage': 'parent-initial',
            'native_session_id': f'generated-factory-{self.provider}-'+self.context['nonce'],
            'owned_provider': {'pid': self.provider, 'pgid': self.provider, 'start_ticks': '23884'},
            'owned_companion': {'pid': self.companion, 'pgid': self.provider, 'start_ticks': '23886'},
            'provider_argv': [self.context['provider'], '--fixture-settings', self.context['settings']],
            'companion_argv': [self.context['companion'], 'factory-mcp', str(handles / 'unit-handle')]}
        self.path = self.work / 'reports' / f'fixture-request-{self.provider}.json'
        self.path.write_text(json.dumps(self.row))
        self.path.chmod(0o600)
        with sqlite3.connect(self.runtime / 'baton.sqlite3') as db:
            db.executescript('CREATE TABLE sessions(document TEXT); CREATE TABLE session_turns(session_id TEXT,document TEXT); CREATE TABLE factory_attempts(scope_id TEXT,session_id TEXT,document TEXT);')
            session = {'id': 'unit-session', 'native_session_id': self.row['native_session_id'],
                'task_id': 'unit-root', 'workspace_id': 'unit-workspace', 'connection_id': 'unit-connection',
                'grant_id': 'unit-grant', 'attempt_id': 'unit-attempt'}
            attempt = {'attempt_id': 'unit-attempt', 'task_id': 'unit-root', 'root_task_id': 'unit-root', 'run_id': 'unit-run'}
            db.execute('INSERT INTO sessions VALUES(?)', (json.dumps(session),))
            db.execute('INSERT INTO session_turns VALUES(?,?)', ('unit-session', json.dumps({'text': request_text})))
            db.execute('INSERT INTO factory_attempts VALUES(?,?,?)', ('unit-scope', 'unit-session', json.dumps(attempt)))
        self.verified = []
        self.reset()
        if bound:
            self.collect()  # actual full collector, live complete argv binds first
            assert len(self.verified) == 1
        self.original = copy.deepcopy(self.verified)
        self.reset()

    def reset(self):
        self.proc_steps, self.argv_overrides, self.pgid_overrides = {}, {}, {}
        self.proc_calls, self.argv_calls, self.pgid_calls = [], [], []

    def identity(self, pid, **changes):
        field = 'owned_provider' if pid == self.provider else 'owned_companion'
        result = {'pid': pid, 'parent_pid': 500 if pid == self.provider else self.provider,
                  'start_ticks': self.row[field]['start_ticks'], 'state': 'R'}
        result.update(changes)
        return result

    def proc(self, pid):
        self.proc_calls.append(pid)
        assert pid in (self.provider, self.companion), 'Unrelated proc read forbidden in unit'
        values = self.proc_steps.get(pid)
        value = values.pop(0) if values and len(values) > 1 else values[0] if values else self.identity(pid)
        if value is None:
            raise FileNotFoundError('unit-only exact PID absent')
        if isinstance(value, BaseException):
            raise value
        return copy.deepcopy(value)

    def collect(self, reaper=None):
        self.ns['support'] = SimpleNamespace(proc_identity=self.proc)
        real_open = Path.open
        def proc_open(path, *args, **kwargs):
            if str(path).startswith('/proc/'):
                pid = int(path.parts[2])
                assert path == Path('/proc', str(pid), 'cmdline') and pid in (self.provider, self.companion)
                self.argv_calls.append(pid)
                field = 'provider_argv' if pid == self.provider else 'companion_argv'
                value = self.argv_overrides.get(pid, b'python3\0'+b'\0'.join(v.encode() for v in self.row[field])+b'\0')
                if isinstance(value, BaseException):
                    raise value
                return io.BytesIO(value)
            return real_open(path, *args, **kwargs)
        def pgid(pid):
            self.pgid_calls.append(pid)
            assert pid in (self.provider, self.companion), 'Unrelated getpgid forbidden'
            value = self.pgid_overrides.get(pid, self.provider)
            if isinstance(value, BaseException):
                raise value
            return value
        with mock.patch.object(Path, 'open', proc_open), mock.patch.object(os, 'getpgid', pgid):
            return self.ns['collect_started_records'](self.runtime, {'id': 'unit-workspace', 'root': str(self.work)},
                self.scope, self.context, self.verified, self.out, reaper)


class BoundExitTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='factory-exit-unit-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def fixture(self, bound=True):
        return Fixture(self.root, bound)

    def retained(self, f):
        rows = f.collect()
        self.assertEqual(f.verified, f.original)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['request']['acceptance_state'], 'not_established')
        self.assertNotIn('stop_confirmed', rows[0])
        self.assertNotIn('task_accepted', rows[0])

    def test_bound_empty_provider_and_fresh_absence_retains_only_original_binding(self):
        f = self.fixture(); f.argv_overrides[f.provider] = b''
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        self.retained(f)
        self.assertNotIn(f.provider, f.pgid_calls)

    def test_bound_missing_cmdline_and_fresh_absence_retains_only_original_binding(self):
        f = self.fixture(); f.argv_overrides[f.provider] = FileNotFoundError()
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        self.retained(f)

    def test_empty_provider_still_live_never_passes_guard(self):
        f = self.fixture(); f.argv_overrides[f.provider] = b''
        with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.verified, f.original)

    def test_empty_provider_still_zombie_never_counts_as_disappeared(self):
        f = self.fixture(); f.argv_overrides[f.provider] = b''
        f.proc_steps[f.provider] = [f.identity(f.provider), f.identity(f.provider, state='Z')]
        with self.assertRaises(AssertionError): f.collect()

    def test_empty_provider_reused_after_read_never_counts_as_gone(self):
        f = self.fixture(); f.argv_overrides[f.provider] = b''
        f.proc_steps[f.provider] = [f.identity(f.provider), f.identity(f.provider, start_ticks='99999')]
        with self.assertRaises(AssertionError): f.collect()

    def test_present_initial_foreign_birth_is_rejected_before_argv_read(self):
        f = self.fixture(); f.proc_steps[f.provider] = [f.identity(f.provider, start_ticks='99999')]
        with self.assertRaises(AssertionError): f.collect()
        self.assertNotIn(f.provider, f.argv_calls)

    def test_unbound_empty_provider_does_not_gain_binding_even_if_later_absent(self):
        f = self.fixture(False); f.argv_overrides[f.provider] = b''
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.verified, [])

    def test_nonempty_invalid_bounded_argv_is_not_excused_by_disappearance(self):
        f = self.fixture()
        for observed in (b'incomplete', b'x'*8192+b'\0', b'wrong\0arguments\0'):
            with self.subTest(bytes=len(observed)):
                f.reset(); f.argv_overrides[f.provider] = observed
                f.proc_steps[f.provider] = [f.identity(f.provider), None]
                with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.verified, f.original)

    def test_complete_non_utf8_argv_still_fails_before_identity_fallback(self):
        f = self.fixture(); f.argv_overrides[f.provider] = b'\xff\0'
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        with self.assertRaises(UnicodeDecodeError): f.collect()

    def test_complete_valid_argv_then_getpgid_absent_retains_bound_provider(self):
        f = self.fixture(); f.pgid_overrides[f.provider] = ProcessLookupError()
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        self.retained(f)

    def test_getpgid_lookup_error_does_not_excuse_live_or_reused_identity(self):
        f = self.fixture()
        for after in (f.identity(f.provider), f.identity(f.provider, start_ticks='99999')):
            f.reset(); f.pgid_overrides[f.provider] = ProcessLookupError()
            f.proc_steps[f.provider] = [f.identity(f.provider), after]
            with self.assertRaises(ProcessLookupError): f.collect()

    def test_actual_wrong_pgid_never_passes_even_if_later_absent(self):
        f = self.fixture(); f.pgid_overrides[f.provider] = f.provider+10
        f.proc_steps[f.provider] = [f.identity(f.provider), None]
        with self.assertRaises(AssertionError): f.collect()

    def test_complete_valid_argv_then_final_identity_absent_retains_bound_provider(self):
        f = self.fixture(); f.proc_steps[f.provider] = [f.identity(f.provider), None, None]
        self.retained(f)

    def test_final_identity_reuse_never_passes(self):
        f = self.fixture(); f.proc_steps[f.provider] = [f.identity(f.provider), f.identity(f.provider, start_ticks='99999')]
        with self.assertRaises(AssertionError): f.collect()

    def test_unbound_valid_argv_then_identity_absent_is_not_a_new_binding(self):
        f = self.fixture(False); f.proc_steps[f.provider] = [f.identity(f.provider), None, None]
        with self.assertRaises(FileNotFoundError): f.collect()
        self.assertEqual(f.verified, [])

    def test_permission_errors_are_not_missing_identity(self):
        f = self.fixture()
        f.argv_overrides[f.provider] = PermissionError()
        with self.assertRaises(PermissionError): f.collect()
        f.reset(); f.argv_overrides[f.provider] = b''
        f.proc_steps[f.provider] = [f.identity(f.provider), PermissionError()]
        with self.assertRaises(PermissionError): f.collect()
        f.reset(); f.pgid_overrides[f.provider] = PermissionError()
        with self.assertRaises(PermissionError): f.collect()

    def test_changed_previously_bound_record_is_rejected_before_proc(self):
        f = self.fixture(); f.verified[0]['sha256'] = '0'*64
        f.argv_overrides[f.provider] = b''; f.proc_steps[f.provider] = [f.identity(f.provider), None]
        with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.proc_calls, [])

    def test_fresh_file_hash_or_core_relation_failure_cannot_reach_exit_fallback(self):
        f = self.fixture()
        Path(f.context['provider']).write_bytes(b'changed program')
        with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.proc_calls, [])
        Path(f.context['provider']).write_bytes(b'Inert unit metadata only: provider')
        with sqlite3.connect(f.runtime / 'baton.sqlite3') as db:
            db.execute('UPDATE session_turns SET document=?', (json.dumps({'text': 'different exact request'}),))
        with self.assertRaises(AssertionError): f.collect()
        self.assertEqual(f.proc_calls, [])

    def test_mcp_empty_argv_keeps_existing_late_reaper_path(self):
        f = self.fixture(); f.argv_overrides[f.companion] = b''
        class ReaperStub:
            calls = 0
            def reap(inner, old):
                self.assertEqual(old, f.original[0]); inner.calls += 1
                if inner.calls == 2: f.proc_steps[f.companion] = [None]
        reaper = ReaperStub()
        rows = f.collect(reaper)
        self.assertEqual(reaper.calls, 2)
        self.assertEqual(f.verified, f.original)
        self.assertEqual(len(rows), 1)

    def test_normal_complete_argv_still_uses_both_pgid_and_final_identity_checks(self):
        f = self.fixture(); self.retained(f)
        self.assertEqual(f.argv_calls, [f.provider, f.companion])
        self.assertEqual(f.pgid_calls, [f.provider, f.companion])
        self.assertEqual(f.proc_calls, [f.provider, f.provider, f.companion, f.companion])


    def test_initial_missing_provider_companion_or_both_never_first_binds(self):
        for i, missing in enumerate(((12124,), (12126,), (12124, 12126))):
            root = self.root / str(i); root.mkdir(mode=0o700)
            f = Fixture(root, bound=False)
            for pid in missing: f.proc_steps[pid] = [None]
            with self.subTest(missing=missing):
                with self.assertRaisesRegex(AssertionError, 'Previously bound'):
                    f.collect()
                self.assertEqual(f.verified, [])
                for pid in missing: self.assertNotIn(pid, f.argv_calls)
                self.assertFalse(list(f.out.glob('verified-*.json')))

    def test_initial_both_missing_retains_exact_previously_verified_history_only(self):
        f = self.fixture()
        f.proc_steps = {f.provider: [None], f.companion: [None]}
        self.retained(f)
        self.assertEqual(f.argv_calls, [])
        self.assertEqual(f.pgid_calls, [])
        self.assertEqual(f.proc_calls, [f.provider, f.companion])


class ObserverAckTests(unittest.TestCase):
    setUp=BoundExitTests.setUp
    fixture=BoundExitTests.fixture
    def marked(self):
        f=self.fixture(False)
        start=time.monotonic_ns()
        f.row['observer_context']={'scope_id':'unit-scope','attempt_id':'unit-attempt',
            'task_id':'unit-root','root_task_id':'unit-root','native_session_id':f.row['native_session_id'],
            'started_ns':start,'deadline_ns':start+5_000_000_000}
        f.path.write_text(json.dumps(f.row))
        return f

    def test_ack_requires_full_first_live_binding_then_matches_exact_bytes(self):
        f=self.marked(); f.collect()
        self.assertEqual(f.argv_calls,[f.provider,f.companion])
        self.assertEqual(f.pgid_calls,[f.provider,f.companion])
        self.assertEqual(len(f.verified),1)
        value=f.ns['observer_ack_value'](f.row,f.ns['sha'](f.path.read_bytes()))
        f.ns['observer_ack_read'](f.path,value)
        f.ns['observer_ack_wait'](f.path,f.row,value['record_sha256'])
        # The exact old binding can remain after disappearance; no reissued ACK.
        f.proc_steps={f.provider:[None],f.companion:[None]}
        f.collect();self.assertEqual(len(f.verified),1)

    def test_unbound_gone_never_publishes_ack(self):
        f=self.marked();f.proc_steps[f.provider]=[None]
        with self.assertRaises(AssertionError):f.collect()
        self.assertEqual(f.verified,[])
        self.assertFalse(any(p.exists() for p in f.ns['observer_ack_paths'](f.path)))

    def test_precreated_ack_or_ready_is_rejected_before_kernel_reads(self):
        for i,which in enumerate((0,1)):
            root=self.root/str(i);root.mkdir()
            f=Fixture(root,False);start=time.monotonic_ns()
            f.row['observer_context']={'scope_id':'unit-scope','attempt_id':'unit-attempt','task_id':'unit-root',
                'root_task_id':'unit-root','native_session_id':f.row['native_session_id'],
                'started_ns':start,'deadline_ns':start+5_000_000_000}
            f.path.write_text(json.dumps(f.row))
            path=f.ns['observer_ack_paths'](f.path)[which]
            path.write_bytes(b'forged') if which==0 else path.mkdir(mode=0o700)
            with self.assertRaises(AssertionError):f.collect()
            self.assertEqual(f.argv_calls,[]);self.assertEqual(f.verified,[])

    def test_wrong_context_or_expired_context_never_acknowledges(self):
        for i,field in enumerate(('attempt_id','scope_id','deadline_ns')):
            root=self.root/str(i);root.mkdir();f=Fixture(root,False)
            start=time.monotonic_ns()-6_000_000_000 if field=='deadline_ns' else time.monotonic_ns()
            f.row['observer_context']={'scope_id':'unit-scope','attempt_id':'unit-attempt','task_id':'unit-root',
                'root_task_id':'unit-root','native_session_id':f.row['native_session_id'],
                'started_ns':start,'deadline_ns':start+5_000_000_000}
            if field!='deadline_ns':f.row['observer_context'][field]='foreign'
            f.path.write_text(json.dumps(f.row))
            with self.assertRaises(AssertionError):f.collect()
            self.assertFalse(any(p.exists() for p in f.ns['observer_ack_paths'](f.path)))

    def test_changed_ack_wrong_hash_duplicate_publish_and_nonprivate_file_reject(self):
        f=self.marked();f.collect()
        value=f.ns['observer_ack_value'](f.row,f.ns['sha'](f.path.read_bytes()))
        with self.assertRaises(AssertionError):f.ns['observer_ack_publish'](f.path,value)
        wrong=copy.deepcopy(value);wrong['record_sha256']='0'*64
        with self.assertRaises(AssertionError):f.ns['observer_ack_read'](f.path,wrong)
        ack,ready=f.ns['observer_ack_paths'](f.path)
        ack.chmod(0o644)
        with self.assertRaises(AssertionError):f.ns['observer_ack_read'](f.path,value)
        ack.chmod(0o600);ack.write_bytes(b'changed')
        with self.assertRaises(AssertionError):f.collect()

    def test_symlink_hardlink_nonregular_and_incomplete_publication_reject(self):
        f=self.marked();value=f.ns['observer_ack_value'](f.row,f.ns['sha'](f.path.read_bytes()))
        ack,ready=f.ns['observer_ack_paths'](f.path)
        # Complete ACK bytes without the atomic ready publication are not consumable.
        ack.write_bytes(f.ns['observer_ack_bytes'](value));ack.chmod(0o600)
        with self.assertRaises(FileNotFoundError):f.ns['observer_ack_read'](f.path,value)
        ready.mkdir(mode=0o700)
        linked=ack.with_name('linked');os.link(ack,linked)
        with self.assertRaises(AssertionError):f.ns['observer_ack_read'](f.path,value)
        linked.unlink();ack.unlink();ack.symlink_to(f.path)
        with self.assertRaises(OSError):f.ns['observer_ack_read'](f.path,value)
        ack.unlink();os.mkfifo(ack,0o600)
        with self.assertRaises(AssertionError):f.ns['observer_ack_read'](f.path,value)

    def test_missing_observer_expiry_late_ack_and_interruption_never_succeed(self):
        f=self.marked();deadline=f.row['observer_context']['deadline_ns']
        clock=iter((deadline-1,deadline))
        with self.assertRaises(AssertionError):
            f.ns['observer_ack_wait'](f.path,f.row,'0'*64,now=lambda:next(clock),pause=lambda _:None)
        with self.assertRaises(SystemExit):
            f.ns['observer_ack_wait'](f.path,f.row,'0'*64,now=lambda:deadline-1,
                pause=lambda _:(_ for _ in ()).throw(SystemExit(143)))
        f.collect();clock=iter((deadline-1,deadline))
        with self.assertRaises(AssertionError):
            f.ns['observer_ack_wait'](f.path,f.row,f.ns['sha'](f.path.read_bytes()),
                now=lambda:next(clock),pause=lambda _:None)


class ChildGoalObserverWaitTests(unittest.TestCase):
    def functions(self, **changes):
        path=SUBJECT.with_name('factory-child-goal-real-ipc.py')
        tree=ast.parse(path.read_text())
        runtime=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='runtime_driver')
        nodes=[n for n in runtime.body if isinstance(n,ast.FunctionDef) and n.name in ('wait','capture')]
        events=[]
        ns={'host':object(),'workspace':object(),'scope':object(),'record_context':{'run_id':'unit-only-run'},
            'runtime':None,'verified':[],'graph_verified':[],'output':None,'reaper':None,'time':time,'clean':str,
            'pump':lambda:events.append('pump'),
            'base':SimpleNamespace(collect_started_records=lambda *a:events.append('factory_capture')),
            'collect_graph_records':lambda *a:events.append('graph_capture')}
        ns.update(changes)
        exec(compile(ast.Module(body=nodes,type_ignores=[]),str(path),'exec'),ns)
        return ns,events

    def test_ready_wait_captures_full_bound_record_before_each_read(self):
        ns,events=self.functions();steps=iter((False,True))
        def read():
            self.assertEqual(events[-2:],['factory_capture','graph_capture'])
            events.append('read');return next(steps)
        with mock.patch.object(time,'sleep',lambda _:None):
            self.assertTrue(ns['wait'](read,bool))
        self.assertEqual(events,['pump','factory_capture','graph_capture','read']*2)

    def test_capture_stays_guarded_and_failure_prevents_read(self):
        for missing in ({'host':None},{'workspace':None},{'scope':None},{'record_context':{}}):
            ns,events=self.functions(**missing)
            self.assertTrue(ns['wait'](lambda:True,bool))
            self.assertEqual(events,['pump'])
        def refused(*args):raise AssertionError('unit-only binding refused')
        ns,events=self.functions(base=SimpleNamespace(collect_started_records=refused))
        with self.assertRaisesRegex(AssertionError,'binding refused'):
            ns['wait'](lambda:self.fail('Read must not run after binding refusal'),bool)


class ChildGoalTimingTests(unittest.TestCase):
    def nodes(self):
        path=SUBJECT.with_name('factory-child-goal-real-ipc.py');tree=ast.parse(path.read_text())
        runtime=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='runtime_driver')
        return path,runtime

    def test_wire_timing_survives_success_backend_and_capture_failures(self):
        path,runtime=self.nodes()
        observed=next(n for n in ast.walk(runtime)if isinstance(n,ast.ClassDef)and n.name=='ObservedHost')
        # The extracted class closes over this real, bounded diagnostic object
        # in runtime_driver. Supply the same dependency without importing the
        # browser harness or changing any original timing/failure assertions.
        spec=importlib.util.spec_from_file_location('_wire_timing_diagnostics',ROOT/'scripts/record-child-goal-ci-evidence.py')
        diagnostics=importlib.util.module_from_spec(spec);spec.loader.exec_module(diagnostics)
        for fault in ('none','backend','capture'):
            with self.subTest(fault=fault):
                slots=[];finished=[];failed=[];ticks=iter((10,20,30,40));sent=[]
                finite=diagnostics.FiniteRuntimeDiagnostics()
                reply={'result':{'unit':'unchanged'}};raw=b'original unit bytes'
                class Host:
                    def wire(self,call):
                        sent.append(call)
                        if fault=='backend':raise TimeoutError('injected backend failure')
                        return reply,raw
                def begin(call):
                    slot={'method':call.method,'params':copy.deepcopy(call.params),'key':call.key,'revision':call.revision}
                    slots.append(slot);return slot
                def finish(slot,value):
                    self.assertIs(value,raw)
                    finished.append(dict(slot))
                    if fault=='capture':raise OSError('injected recorder failure')
                def fail(slot,error,stage):failed.append({**slot,'stage':stage})
                ns={'base':SimpleNamespace(PrivateHost=Host),'time':SimpleNamespace(monotonic_ns=lambda:next(ticks)),
                    'FixedCall':lambda method,params,key,revision:SimpleNamespace(method=method,params=params,key=key,revision=revision),
                    'wire_recorder':SimpleNamespace(begin=begin,finish=finish,failed=fail),'finite':finite}
                exec(compile(ast.Module(body=[observed],type_ignores=[]),str(path),'exec'),ns)
                call=SimpleNamespace(method='unit-fixed',params={'a':1},key='unit-key',revision=7)
                if fault=='none':self.assertEqual(ns['ObservedHost']().wire(call),(reply,raw))
                else:
                    with self.assertRaises(TimeoutError if fault=='backend'else OSError):ns['ObservedHost']().wire(call)
                self.assertEqual((sent[0].method,sent[0].params,sent[0].key,sent[0].revision),('unit-fixed',{'a':1},'unit-key',7))
                self.assertEqual(slots[0]['timing_ns'],{'started':10,'backend_started':20,'backend_finished':30,'finished':40})
                evidence=(failed if fault=='backend'else finished)[0]
                self.assertIs(evidence['timing_ns'],slots[0]['timing_ns'])
                self.assertEqual(evidence['timing_ns']['finished'],40)
                observation=finite.snapshot()['core_replies']
                self.assertEqual(observation['observation'],'unobserved'if fault=='backend'else 'observed')
                self.assertEqual(observation['captured_count'],None if fault=='backend'else 1)
                self.assertEqual(observation['errors'],None if fault=='backend'else [])

    def test_phase_stamps_are_finite_private_clock_metadata(self):
        path,runtime=self.nodes();phase=next(n for n in runtime.body if isinstance(n,ast.FunctionDef)and n.name=='phase_stamp')
        report={};ticks=iter((100,200));ns={'report':report,'time':SimpleNamespace(monotonic_ns=lambda:next(ticks))}
        exec(compile(ast.Module(body=[phase],type_ignores=[]),str(path),'exec'),ns)
        ns['phase_stamp']('owner_enabled_expect','start')
        try:raise AssertionError('unit-only original failure')
        except AssertionError:ns['phase_stamp']('owner_enabled_expect','end')
        self.assertEqual(report['owner_phase_stamps'],[
            {'phase':'owner_enabled_expect','edge':'start','monotonic_ns':100},
            {'phase':'owner_enabled_expect','edge':'end','monotonic_ns':200}])
        with self.assertRaises(AssertionError):ns['phase_stamp']('arbitrary private text','start')
        self.assertEqual(len(report['owner_phase_stamps']),2)

    def test_all_seven_actual_phase_blocks_keep_end_stamp_on_failure(self):
        path,runtime=self.nodes()
        blocks=[n for n in ast.walk(runtime)if isinstance(n,ast.Try)and len(n.finalbody)==1
            and isinstance(n.finalbody[0],ast.Expr)and isinstance(n.finalbody[0].value,ast.Call)
            and isinstance(n.finalbody[0].value.func,ast.Name)and n.finalbody[0].value.func.id=='phase_stamp']
        expected={'before_owner_read','before_owner_capture','owner_UI_refresh_click','owner_enabled_expect',
            'owner_click','owner_receipt_observe','owner_core_acceptance'}
        self.assertEqual(len(blocks),7)
        names=[]
        for block in blocks:
            end=block.finalbody[0].value
            self.assertEqual(len(end.args),2);self.assertFalse(end.keywords)
            self.assertIsInstance(end.args[0],ast.Constant);self.assertIsInstance(end.args[1],ast.Constant)
            self.assertEqual(end.args[1].value,'end');names.append(end.args[0].value)
        self.assertEqual(set(names),expected);self.assertEqual(len(set(names)),7)
        for name,block in zip(names,blocks):
            with self.subTest(phase=name):
                stamps=[];calls=[];original=RuntimeError('original unit failure')
                def refuse(*a,**k):calls.append((a,k));raise original
                button=SimpleNamespace(click=refuse);trace=object()
                query={'scope_id':'scope','task_id':'child','goal_run_id':'graph'}
                pump=lambda:None
                ns={'phase_stamp':lambda *a:stamps.append(a),'wait':refuse,'wait_child_goal':refuse,'capture':refuse,
                    'goal_panel':SimpleNamespace(get_by_role=lambda *a,**k:button),'owner':button,
                    'expect':lambda _:SimpleNamespace(to_be_enabled=refuse),
                    'wait_goal_owner_binding':refuse,'report':{'goal_owner_binding':trace},
                    'query':query,'owner_deadline':123.0,'pump':pump}
                with self.assertRaisesRegex(RuntimeError,'original unit failure')as error:
                    exec(compile(ast.Module(body=[block],type_ignores=[]),str(path),'exec'),ns)
                self.assertIs(error.exception,original)
                self.assertEqual(stamps,[(name,'end')]);self.assertEqual(len(calls),1)
                if name=='owner_receipt_observe':
                    self.assertEqual(len(calls[0][0]),2);self.assertIs(calls[0][0][0],trace);self.assertIs(calls[0][0][1],query)
                    self.assertEqual(calls[0][1],{'deadline':123.0,'pump':pump,'capture':refuse})
                elif name=='owner_core_acceptance':
                    self.assertEqual(len(calls[0][0]),1);self.assertTrue(callable(calls[0][0][0]));self.assertEqual(calls[0][1],{'deadline':123.0})



class BoundMcpLateDiagnosticTests(unittest.TestCase):
    setUp=BoundExitTests.setUp
    fixture=BoundExitTests.fixture

    def test_late_live_or_zombie_remains_refused_with_finite_state_diagnostic(self):
        for index,(state,parent) in enumerate((('S',12124),('Z',os.getpid()))):
            root=self.root/str(index);root.mkdir();f=Fixture(root)
            f.argv_overrides[f.companion]=b''
            f.proc_steps[f.companion]=[f.identity(f.companion,state='R'),f.identity(f.companion,state=state,parent_pid=parent)]
            calls=[];reaper=SimpleNamespace(reap=lambda old:calls.append(old))
            with self.assertRaisesRegex(AssertionError,'Bound MCP still exists after late adopted-child wait: ')as error:
                f.collect(reaper)
            detail=json.loads(str(error.exception).split(': ',1)[1])
            self.assertEqual(detail,{'stage':'bound_MCP_late_wait_still_present','before_state':'R','after_state':state,
                'after_parent_is_controller':parent==os.getpid(),'birth_matches':True,'pgid_matches':True,'pgid_error_type':None})
            self.assertEqual(len(calls),2);self.assertEqual(f.verified,f.original)
            self.assertNotIn('stop_confirmed',detail);self.assertNotIn('process_reaped',detail)

    def test_late_pgid_error_type_is_recorded_without_changing_refusal(self):
        f=self.fixture();f.argv_overrides[f.companion]=b''
        f.pgid_overrides[f.companion]=PermissionError('sensitive unit message must not be copied')
        with self.assertRaisesRegex(AssertionError,'Bound MCP still exists after late adopted-child wait: ')as error:
            f.collect(SimpleNamespace(reap=lambda _:None))
        detail=json.loads(str(error.exception).split(': ',1)[1])
        self.assertEqual(detail['pgid_error_type'],'PermissionError');self.assertIsNone(detail['pgid_matches'])
        self.assertNotIn('sensitive',str(error.exception));self.assertEqual(f.verified,f.original)


class BoundMcpDeferredDiscoveryTests(unittest.TestCase):
    setUp=BoundExitTests.setUp
    fixture=BoundExitTests.fixture

    def pending_fixture(self):
        f=self.fixture();f.argv_overrides[f.companion]=b''
        f.proc_steps[f.companion]=[f.identity(f.companion,state='Z',parent_pid=f.provider)]
        return f

    def test_nonadopted_bound_zombie_defers_row_without_changing_history_or_files(self):
        f=self.pending_fixture();before={str(p):p.read_bytes()for p in f.out.iterdir()}
        calls=[];rows=f.collect(SimpleNamespace(reap=lambda old:calls.append(old)))
        self.assertEqual(rows,[]);self.assertEqual(f.verified,f.original);self.assertEqual(len(calls),2)
        self.assertEqual({str(p):p.read_bytes()for p in f.out.iterdir()},before)
        self.assertNotIn('stop_confirmed',f.verified[0]);self.assertNotIn('process_reaped',f.verified[0])
        self.assertFalse(list(f.work.glob('reports/fixture-observed-*')))
        # Permanent nonadoption remains pending, never a fresh successful discovery.
        self.assertEqual(f.collect(SimpleNamespace(reap=lambda _:None)),[])

    def test_later_adopted_child_requires_existing_reaper_before_fresh_discovery(self):
        f=self.pending_fixture();self.assertEqual(f.collect(SimpleNamespace(reap=lambda _:None)),[])
        f.proc_steps[f.companion]=[f.identity(f.companion,state='Z',parent_pid=os.getpid())]
        waits=[]
        def reaper(old):
            self.assertEqual(old,f.original[0])
            current=f.proc_steps[f.companion][0]
            if current is not None and current['state']=='Z'and current['parent_pid']==os.getpid():
                waits.append(f.companion)  # injected known-child wait; no OS wait/reap claim
                f.proc_steps[f.companion]=[None]
        rows=f.collect(SimpleNamespace(reap=reaper))
        self.assertEqual(waits,[f.companion]);self.assertEqual(len(rows),1)
        self.assertEqual(f.verified,f.original);self.assertNotIn('stop_confirmed',rows[0])

    def test_pending_never_excuses_live_controller_wrong_identity_or_pgid(self):
        cases=('before-live','after-live','controller','wrong-pgid','pgid-permission','wrong-after-pid','wrong-initial-birth','malformed-argv','unbound')
        for index,case in enumerate(cases):
            root=self.root/str(index);root.mkdir();f=Fixture(root,bound=case!='unbound')
            f.argv_overrides[f.companion]=b''
            z=f.identity(f.companion,state='Z',parent_pid=f.provider)
            f.proc_steps[f.companion]=[dict(z),dict(z)]
            if case=='before-live':f.proc_steps[f.companion][0]['state']='R'
            if case=='after-live':f.proc_steps[f.companion][1]['state']='S'
            if case=='controller':
                for x in f.proc_steps[f.companion]:x['parent_pid']=os.getpid()
            if case=='wrong-pgid':f.pgid_overrides[f.companion]=99999
            if case=='pgid-permission':f.pgid_overrides[f.companion]=PermissionError('not copied')
            if case=='wrong-after-pid':f.proc_steps[f.companion][1]['pid']=99999
            if case=='wrong-initial-birth':f.proc_steps[f.companion][0]['start_ticks']='99999'
            if case=='malformed-argv':f.argv_overrides[f.companion]=b'nonempty-no-NUL'
            with self.subTest(case=case):
                with self.assertRaises(AssertionError):f.collect(SimpleNamespace(reap=lambda _:None))
                self.assertEqual(f.verified,f.original)


if __name__ == '__main__':
    unittest.main()
