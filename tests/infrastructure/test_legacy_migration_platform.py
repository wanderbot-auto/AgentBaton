"""Pure platform/lifecycle/privacy controls; never macOS/Core evidence."""
import copy
import io
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import types
import tempfile
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
def load(name,path):
    module=types.ModuleType(name);module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module
BUILDER=load('platform_builder',ROOT/'scripts/prepare-legacy-migration-fixture.py')
PURE=load('old_pure_controls',ROOT/'crates/core/tests/fixtures/child_goal_legacy_migration/test_source_builder.py')
GEN=PURE.generator
PUBLIC=load('platform_public',ROOT/'scripts/record-legacy-migration-ci-evidence.py')

def darwin_metadata(architecture='arm64'):
    m=PURE.synthetic_metadata();m.update(platform='darwin',architecture=architecture,os_release='24.6.0',native_helper_sha256='d'*64)
    m['probe'].update(identity_method='darwin_owned_unreaped_group',leader_unreaped_until_signalling=True,group_quiescent_before_reaping=True,cleanup_errors=[])
    for b in m['binaries'].values():b.update(format='mach-o-64',architecture=architecture)
    return m

class NativeMigrationPlatformTests(unittest.TestCase):
    def test_original_linux_v1_shape_remains_valid_and_known_hash_is_narrow(self):
        m=PURE.synthetic_metadata();m['builder_sha256']=BUILDER.LEGACY_LINUX_BUILDER_SHA256
        self.assertEqual(BUILDER.validate_build_manifest(m),{'batond':'a'*64,'baton':'a'*64})
        BUILDER.verify_reviewed_builder(m,'b'*64)
        for key,value in [('platform','darwin'),('architecture','x86_64'),('builder_sha256','c'*64)]:
            bad=copy.deepcopy(m);bad[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):BUILDER.verify_reviewed_builder(bad,'b'*64)

    def test_both_macos_architectures_validate_only_complete_owned_join(self):
        for architecture in ['arm64','x86_64']:
            m=darwin_metadata(architecture);self.assertEqual(BUILDER.validate_build_manifest(m)['baton'],'a'*64)
            for key,value in [('leader_unreaped_until_signalling',False),('group_quiescent_before_reaping',False),('cleanup_errors',['unknown']),('forced_kill',True),('daemon_wait_exit',None),('identity_method','unknown')]:
                bad=copy.deepcopy(m);bad['probe'][key]=value
                with self.subTest(architecture=architecture,key=key),self.assertRaises(ValueError):BUILDER.validate_build_manifest(bad)

    def test_actual_macho_header_must_match_the_declared_native_architecture(self):
        for architecture,cpu in [('arm64',0x0100000c),('x86_64',0x01000007)]:
            raw=struct.pack('<II',0xfeedfacf,cpu)+b'pure synthetic header only'
            self.assertEqual(BUILDER.native_binary_metadata(raw,'darwin',architecture),{'format':'mach-o-64','architecture':architecture})
            with self.assertRaises(ValueError):BUILDER.native_binary_metadata(raw,'darwin','x86_64' if architecture=='arm64' else 'arm64')
        for raw in [b'',b'ELF.....']:
            with self.assertRaises(ValueError):BUILDER.native_binary_metadata(raw,'darwin','arm64')

    def test_darwin_owned_stop_holds_the_leader_until_group_quiescence_then_waits(self):
        events=[]
        child=mock.Mock(pid=12345)
        child.wait.side_effect=lambda **kw:events.append('wait') or 0
        helpers=types.SimpleNamespace(owned_child_exited=mock.Mock(side_effect=[False,True]),owned_group_is_quiescent=mock.Mock(side_effect=lambda pid:events.append('quiescent') or True))
        result={'owned_identity':{'pid':12345,'pgid':12345},'sigterm_sent':False,'forced_kill':False}
        with mock.patch.object(BUILDER.os,'killpg',side_effect=lambda pid,sig:events.append(sig.name)):
            BUILDER.stop_darwin_owned(child,helpers,result)
        self.assertEqual(events,['SIGTERM','quiescent','wait']);self.assertTrue(result['identity_gone_after_join'])
        child.poll.assert_not_called();child.send_signal.assert_not_called()

    def test_unknown_premature_exit_bad_group_or_bad_wait_cannot_be_success(self):
        for mode in ['unknown','premature','group','wait','wait_timeout','unjoined','unknown_quiescence']:
            child=mock.Mock(pid=12345);child.wait.return_value=1 if mode=='wait' else 0
            if mode=='wait_timeout':child.wait.side_effect=subprocess.TimeoutExpired('pure child',5)
            helpers=types.SimpleNamespace(owned_child_exited=mock.Mock(side_effect=OSError('pure unknown') if mode=='unknown' else [True] if mode=='premature' else [False,True]),owned_group_is_quiescent=mock.Mock(return_value=False if mode=='unjoined' else None if mode=='unknown_quiescence' else True))
            result={'owned_identity':{'pid':12345,'pgid':54321 if mode=='group' else 12345},'sigterm_sent':False,'forced_kill':False}
            with self.subTest(mode=mode),mock.patch.object(BUILDER.os,'killpg') as kill,mock.patch.object(BUILDER.time,'monotonic',side_effect=[0,16]):
                BUILDER.stop_darwin_owned(child,helpers,result)
                self.assertFalse(result['identity_gone_after_join'])
                if mode=='group':kill.assert_not_called();child.kill.assert_called_once()

    def test_provider_absence_is_not_a_birth_identity_or_an_unknown_ps_result(self):
        for code,stdout,stderr,valid in [(1,'','',True),(0,'12345 12345 Z\n','',False),(0,'12345 12345 S\n','',False),(1,'','denied',False),(0,'','',False),(2,'','',False)]:
            with self.subTest(code=code,stdout=stdout),mock.patch.object(BUILDER.subprocess,'run',return_value=subprocess.CompletedProcess([],code,stdout,stderr)):
                if valid:self.assertTrue(BUILDER.darwin_pid_absence(12345)['gone_after_join'])
                else:
                    with self.assertRaises(ValueError):BUILDER.darwin_pid_absence(12345)
        with mock.patch.object(BUILDER.subprocess,'run',side_effect=subprocess.TimeoutExpired('ps',1)),self.assertRaises(subprocess.TimeoutExpired):BUILDER.darwin_pid_absence(12345)

    def test_primary_core_join_requires_exact_same_session_and_attempt(self):
        class DB:
            def __init__(self,bad=False):self.bad=bad
            def execute(self,sql,args):
                if 'goal_run_nodes' in sql:rows=[{'stop_confirmed':True,'session_id':'session'}]
                else:rows=[{'id':'other' if self.bad else 'session','attempt_id':'attempt','state':'stopped'}]
                return types.SimpleNamespace(fetchall=lambda:[(json.dumps(r),)for r in rows])
        self.assertEqual(GEN.darwin_core_join(DB(),{'attempt_id':'attempt'},'scope')['session_id'],'session')
        with self.assertRaises(AssertionError):GEN.darwin_core_join(DB(True),{'attempt_id':'attempt'},'scope')

    def test_factory_primary_join_rejects_unconfirmed_or_mismatched_attempt(self):
        class DB:
            def __init__(self,bad=None):self.bad=bad
            def execute(self,sql,args):
                if 'factory_task_bindings' in sql:rows=[{'session_id':'session','handle':{'attempt_id':'attempt','confirmed_stop':self.bad!='unconfirmed'}}]
                elif 'factory_attempts' in sql:rows=[{'session_id':'session','attempt_id':'other' if self.bad=='attempt' else 'attempt','stop_confirmed':1}]
                else:rows=[{'id':'session','attempt_id':'attempt','state':'stopped'}]
                return types.SimpleNamespace(fetchall=lambda:[(json.dumps(r),)for r in rows])
        self.assertEqual(GEN.darwin_core_join(DB(),{'task_id':'task'},'scope')['source'],'factory_binding_attempt')
        for bad in ['unconfirmed','attempt']:
            with self.subTest(bad=bad),self.assertRaises(AssertionError):GEN.darwin_core_join(DB(bad),{'task_id':'task'},'scope')

    def test_generated_macos_acp_records_only_its_tuple_without_proc(self):
        for request in [{'kind':'agentbaton_factory_task_request','task_id':'synthetic-task'},
                        {'kind':'agentbaton_goal_task_request','lineage':{'attempt_id':'synthetic-attempt'},'outputs':[{'artifact':{'relative_path':'reports/report.txt'}}]}]:
            with self.subTest(kind=request['kind']),tempfile.TemporaryDirectory() as tmp:
                previous=Path.cwd()
                try:
                    os.chdir(tmp)
                    frame={'method':'session/prompt','id':1,'params':{'prompt':[{'text':json.dumps(request)}]}}
                    original_read=Path.read_text
                    def read(path,*args,**kwargs):
                        self.assertNotIn('/proc',str(path),'Darwin generated provider must not depend on Linux procfs')
                        return original_read(path,*args,**kwargs)
                    with mock.patch.object(GEN.sys,'platform','darwin'),mock.patch.object(GEN.sys,'argv',['synthetic']),mock.patch.object(GEN.sys,'stdin',io.StringIO(json.dumps(frame)+'\n')),mock.patch.object(GEN.sys,'stdout',io.StringIO()),mock.patch.object(Path,'read_text',read):
                        exec(compile(GEN.ACP,'<pure generated darwin ACP>','exec'),{})
                    marker=json.loads(next(Path('reports').glob('migration-owned-*.json')).read_text())
                    self.assertEqual(marker['identity_method'],'darwin_pid_absence_after_core_join');self.assertNotIn('start_ticks',marker)
                finally:os.chdir(previous)

    def test_public_macos_metadata_never_forwards_private_nested_fields(self):
        m=darwin_metadata();m['toolchain']={'cargo':'cargo 1.99.1 (abcdef12 2026-10-01)','rustc':'rustc 1.99.1 (abcdef12 2026-10-01)'}
        m['source']['raw_grant']={'token':'PRIVATE_SENTINEL'};m['probe']['raw_rpc']='PRIVATE_SENTINEL';m['binaries']['baton']['raw_db']='PRIVATE_SENTINEL'
        public=PUBLIC.public_build(m)
        self.assertEqual(public['platform'],'darwin');self.assertEqual(public['architecture'],'arm64')
        self.assertNotIn('PRIVATE_SENTINEL',json.dumps(public));self.assertNotIn('probe',public)
        bad=copy.deepcopy(m);bad['probe']['forced_kill']=True
        with self.assertRaises(ValueError):PUBLIC.public_build(bad)
        for key in ['architecture','os_release','native_helper_sha256']:
            bad=copy.deepcopy(m);bad[key]={'raw':'PRIVATE_SENTINEL'}
            with self.subTest(key=key),self.assertRaises(ValueError):PUBLIC.public_build(bad)

    def test_public_macos_fixture_requires_both_separate_stop_proofs(self):
        value={'status':'passed','schema':14,'platform':'darwin','architecture':'x86_64','os_release':'24.6.0','origin':'actual_frozen_schema14_core_rpc','source_commit':PUBLIC.SOURCE,'provenance_kind':'reviewed_source_build','daemon_exit':0,'forced_kill':False,'runtime_cleanup':'removed_after_successful_owned_join_and_backup','database_sha256':'a'*64,'counts':{n:1 for n in PUBLIC.TABLES},'daemon_identity_method':'darwin_owned_unreaped_group','daemon_sigterm_sent':True,'daemon_group_quiescent_before_reaping':True,'daemon_leader_unreaped_until_signalling':True,'daemon_cleanup_errors':[],'provider_stop_proof':{'method':'core_stop_confirmed_plus_pid_absence','observed':2,'core_join_confirmed':True,'pid_absence_confirmed':True,'raw_rpc':'PRIVATE_SENTINEL'}}
        self.assertNotIn('PRIVATE_SENTINEL',json.dumps(PUBLIC.public_fixture(value)))
        for field in ['core_join_confirmed','pid_absence_confirmed']:
            bad=copy.deepcopy(value);bad['provider_stop_proof'][field]=False
            with self.subTest(field=field),self.assertRaises(ValueError):PUBLIC.public_fixture(bad)

if __name__=='__main__':unittest.main()
