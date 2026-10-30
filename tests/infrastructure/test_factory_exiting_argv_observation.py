"""Exact collector + generated SQLite/files; proc/UID/argv are injected only.

No product, process, signal or kernel-exit claim. Existing tests and their Root
finite timing injection are imported unchanged, not replaced by this module.
"""
import ast
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import sqlite3
import tempfile
import traceback
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('old_bound_exit_controls',Path(__file__).with_name('test_factory_bound_provider_exit.py'))
OLD=importlib.util.module_from_spec(spec);spec.loader.exec_module(OLD)

class ExitingArgvObservationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve();self.next=0

    def fixture(self,bound=True,cancel=True):
        root=self.root/str(self.next);self.next+=1;root.mkdir(mode=0o700)
        f=OLD.Fixture(root,bound)
        with sqlite3.connect(f.runtime/'baton.sqlite3')as db:
            db.execute('CREATE TABLE factory_runs(id TEXT PRIMARY KEY,scope_id TEXT,root_task_id TEXT,revision INTEGER,state TEXT,document TEXT)')
            value=dict(id=f.context['run_id'],scope_id=f.scope['id'],root_task_id=f.scope['root_task_id'],revision=3,state='cancel_requested'if cancel else 'running',cancel_requested=cancel)
            db.execute('INSERT INTO factory_runs VALUES(?,?,?,?,?,?)',(value['id'],value['scope_id'],value['root_task_id'],value['revision'],value['state'],json.dumps(value)))
        return f

    def collect(self,f,uid=None):
        real_stat=Path.stat
        def proc_stat(path,*args,**kwargs):
            if str(path).startswith('/proc/'):
                self.assertEqual(path,Path('/proc',str(f.provider)))
                return SimpleNamespace(st_uid=os.geteuid()if uid is None else uid)
            return real_stat(path,*args,**kwargs)
        with mock.patch.object(Path,'stat',proc_stat),mock.patch.object(os,'kill',side_effect=AssertionError('test must never signal a product PID')):
            return f.collect()

    def test_prior_exact_cancelled_empty_read_defers_without_discovery_mutation_or_stop(self):
        f=self.fixture();f.argv_overrides[f.provider]=b''
        before=copy.deepcopy(f.context);raw={p.name:p.read_bytes()for p in f.out.iterdir()};database=(f.runtime/'baton.sqlite3').read_bytes()
        self.assertEqual(self.collect(f),[])
        self.assertEqual(f.verified,f.original);self.assertEqual(f.context,before)
        self.assertEqual((f.runtime/'baton.sqlite3').read_bytes(),database)
        self.assertEqual({p.name:p.read_bytes()for p in f.out.iterdir()},raw)
        self.assertFalse(any('stop_confirmed'in r or 'task_accepted'in r for r in f.verified))
        f.reset();rows=self.collect(f);self.assertEqual(len(rows),1)
        self.assertEqual(f.verified,f.original)

    def test_first_empty_cannot_bind_even_with_committed_cancel(self):
        f=self.fixture(False);f.argv_overrides[f.provider]=b''
        with self.assertRaises(AssertionError):self.collect(f)
        self.assertEqual(f.verified,[]);self.assertNotIn('_verified_provider_parents',f.context)
        self.assertFalse(list(f.out.glob('verified-*')))

    def test_prior_without_cancel_or_with_wrong_cancel_binding_stays_rejected(self):
        for mutation in [None,'id','scope_id','root_task_id','flag_int','revision_float','state']:
            f=self.fixture(cancel=mutation is not None);f.argv_overrides[f.provider]=b''
            if mutation is not None:
                with sqlite3.connect(f.runtime/'baton.sqlite3')as db:
                    value=json.loads(db.execute('SELECT document FROM factory_runs').fetchone()[0])
                    if mutation in ('id','scope_id','root_task_id'):value[mutation]='foreign'
                    elif mutation=='flag_int':value['cancel_requested']=1
                    elif mutation=='revision_float':value['revision']=3.0
                    else:value['state']='running'
                    db.execute('UPDATE factory_runs SET document=?',(json.dumps(value),))
            with self.subTest(mutation=mutation),self.assertRaises(AssertionError):self.collect(f)
            self.assertEqual(f.verified,f.original)

    def test_cancel_missing_or_invalid_table_never_uses_context_self_report(self):
        f=self.fixture();f.argv_overrides[f.provider]=b''
        f.context['cancel_requested']=True
        with sqlite3.connect(f.runtime/'baton.sqlite3')as db:db.execute('DROP TABLE factory_runs')
        with self.assertRaises(AssertionError):self.collect(f)

    def test_no_original_parent_history_cannot_defer(self):
        f=self.fixture();f.argv_overrides[f.provider]=b'';f.context.pop('_verified_provider_parents')
        with self.assertRaises(AssertionError):self.collect(f)
        self.assertEqual(f.verified,f.original)

    def test_pid_birth_parent_pgid_and_uid_changes_are_refused(self):
        for change in ['pid','birth_before','birth_after','parent_before','parent_after','pgid','uid']:
            f=self.fixture();f.argv_overrides[f.provider]=b''
            if change=='pid':f.proc_steps[f.provider]=[{**f.identity(f.provider),'pid':54321}]
            elif change=='birth_before':f.proc_steps[f.provider]=[f.identity(f.provider,start_ticks='999')]
            elif change=='birth_after':f.proc_steps[f.provider]=[f.identity(f.provider),f.identity(f.provider,start_ticks='999')]
            elif change=='parent_before':f.proc_steps[f.provider]=[f.identity(f.provider,parent_pid=999)]
            elif change=='parent_after':f.proc_steps[f.provider]=[f.identity(f.provider),f.identity(f.provider,parent_pid=999)]
            elif change=='pgid':f.pgid_overrides[f.provider]=999
            with self.subTest(change=change),self.assertRaises(AssertionError):self.collect(f,uid=os.geteuid()+1 if change=='uid'else None)
            self.assertEqual(f.verified,f.original)

    def test_none_is_not_temporary_empty_and_nonempty_wrong_argv_still_refuses(self):
        for value in [FileNotFoundError(),b'wrong-program\0',b'wrong-program']:
            f=self.fixture();f.argv_overrides[f.provider]=value
            with self.subTest(value=type(value).__name__),self.assertRaises(AssertionError):self.collect(f)
            self.assertEqual(f.verified,f.original)

    def test_changed_file_nonce_or_core_binding_cannot_reach_defer(self):
        for mutation in ['nonce','bytes','session']:
            f=self.fixture();f.argv_overrides[f.provider]=b''
            if mutation=='session':
                with sqlite3.connect(f.runtime/'baton.sqlite3')as db:
                    row=json.loads(db.execute('SELECT document FROM sessions').fetchone()[0]);row['attempt_id']='foreign';db.execute('UPDATE sessions SET document=?',(json.dumps(row),))
            else:
                row=copy.deepcopy(f.row)
                if mutation=='nonce':row['fixture_nonce']='foreign'
                else:row['extra']='changed immutable record'
                f.path.write_text(json.dumps(row));f.path.chmod(0o600)
            with self.subTest(mutation=mutation),self.assertRaises(AssertionError):self.collect(f)
            self.assertEqual(f.verified,f.original)

    def test_companion_wrong_argv_is_not_hidden_by_provider_defer(self):
        f=self.fixture();f.argv_overrides[f.provider]=b'';f.argv_overrides[f.companion]=b'wrong\0'
        with self.assertRaises(AssertionError):self.collect(f)
        self.assertEqual(f.verified,f.original)

    def test_fresh_readonly_snapshot_can_observe_single_committed_cancel_after_old_snapshot(self):
        f=self.fixture(cancel=False);f.argv_overrides[f.provider]=b''
        with sqlite3.connect(f.runtime/'baton.sqlite3')as db:self.assertEqual(db.execute('PRAGMA journal_mode=WAL').fetchone()[0],'wal')
        original=f.proc;commits=[]
        def proc(pid):
            if not commits and pid==f.provider:
                with sqlite3.connect(f.runtime/'baton.sqlite3')as writer:
                    row=json.loads(writer.execute('SELECT document FROM factory_runs').fetchone()[0]);row.update(cancel_requested=True,state='cancel_requested')
                    writer.execute("UPDATE factory_runs SET state='cancel_requested',document=?",(json.dumps(row),));commits.append(True)
            return original(pid)
        f.proc=proc
        self.assertEqual(self.collect(f),[]);self.assertEqual(commits,[True]);self.assertEqual(f.verified,f.original)

    def test_final_identity_gate_still_rejects_live_empty_and_only_accepts_actual_absence(self):
        f=self.fixture();f.argv_overrides[f.provider]=b'';self.assertEqual(self.collect(f),[])
        tree=ast.parse(OLD.SUBJECT.read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in ('note_failure','inspect_owned_records')]
        f.ns['signal']=signal;f.ns['traceback']=traceback;exec(compile(ast.Module(body=nodes,type_ignores=[]),str(OLD.SUBJECT),'exec'),f.ns)
        f.reset();f.proc_steps[f.companion]=[None]
        real_read=Path.read_bytes
        def proc_read(path):
            if str(path).startswith('/proc/'):
                self.assertEqual(path,Path('/proc',str(f.provider),'cmdline'));return b''
            return real_read(path)
        with mock.patch.object(Path,'read_bytes',proc_read),mock.patch.object(os,'getpgid',return_value=f.provider),mock.patch.object(os,'kill',side_effect=AssertionError('no signal is permitted in this pure control'))as signal_call:
            report={'cleanup':[]};self.assertFalse(f.ns['inspect_owned_records'](report,f.verified,1,f.context['provider'],f.context['settings'],f.context['companion'],f.runtime));self.assertFalse(signal_call.called)
        f.proc_steps={f.provider:[None],f.companion:[None]};report={'cleanup':[]}
        with mock.patch.object(os,'kill',side_effect=AssertionError('no signal is permitted'))as signal_call:
            self.assertTrue(f.ns['inspect_owned_records'](report,f.verified,1,f.context['provider'],f.context['settings'],f.context['companion'],f.runtime));self.assertFalse(signal_call.called)
        self.assertEqual(f.verified,f.original)

if __name__=='__main__':unittest.main()
