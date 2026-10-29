"""Pure CGCI-01 projection controls, not real Core/browser acceptance."""
import copy
import ast
import hashlib
import json
import os
from pathlib import Path
import tempfile
import textwrap
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
P=types.ModuleType('child_goal_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')
exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)
SECRET='PRIVATE_SENTINEL_TOKEN_GRANT_RPC_HTML'

def inputs(case='normal'):
    m={'schema':15,'dirty':False,'git_head':'a'*40,'source_head':'a'*40,'compiled_core_source_head':'a'*40,
       'binaries':{'batond':'b'*64,'baton':'c'*64},'frontend_assets':{SECRET+'.html':'d'*64}}
    raw=json.dumps(m).encode();candidate,summary=P.candidate_summary(raw)
    r={'case':case,'status':'passed','candidate':candidate,'manifest_sha256':P.sha(raw),'harness_sha256':'e'*64,
       'checks':[{'id':key,'status':'passed','evidence':SECRET}for key in sorted(P.EXPECTED[case])],
       'cleanup_complete':True,'cleanup':[{'arbitrary_raw_proposal':SECRET}],'http_unexpected_errors':[],
       'private_owner_reviews':[{'token':SECRET}],'http':[SECRET],'observations':{'raw_db':SECRET},
       'screenshots':[{'path':'/private/'+SECRET+'.png','html':SECRET}]}
    return raw,candidate,summary,r

class ChildGoalProjectionTests(unittest.TestCase):
    def test_only_explicit_fields_leave_all_three_synthetic_case_reports(self):
        for case in P.CASES:
            _,m,s,r=inputs(case);v=P.project(r,m,s,case,0,'e'*64)
            self.assertEqual(v['status'],'passed');self.assertFalse(v['screenshots_published'])
            self.assertNotIn(SECRET,json.dumps(v));self.assertNotIn('candidate',v)
            self.assertEqual(set(v['checks'][0]),{'id','status'})

    def test_nonzero_or_late_error_cleanup_or_missing_check_never_passes(self):
        _,m,s,r=inputs()
        self.assertEqual(P.project(r,m,s,'normal',1,'e'*64)['status'],'failed')
        unknown=P.project(r,m,s,'normal',None,'e'*64)
        self.assertEqual(unknown['status'],'failed');self.assertIsNone(unknown['exit_code']);self.assertEqual(unknown['command_state'],'unobserved')
        for key,value in [('cleanup_complete',False),('failure',{'message':SECRET}),('http_unexpected_errors',[SECRET]),('checks',r['checks'][:-1])]:
            bad=copy.deepcopy(r);bad[key]=value
            with self.subTest(key=key):self.assertEqual(P.project(bad,m,s,'normal',0,'e'*64)['status'],'failed')

    def test_unknown_duplicate_or_wrong_case_check_is_refused_without_raw_error(self):
        _,m,s,r=inputs()
        for checks in [[{'id':SECRET,'status':'passed'}],r['checks']+[r['checks'][0]],[{'id':'CG03','status':'passed'}]]:
            bad=copy.deepcopy(r);bad['checks']=checks
            with self.assertRaises(P.Refused)as failure:P.project(bad,m,s,'normal',0,'e'*64)
            self.assertNotIn(SECRET,str(failure.exception))

    def test_wrong_manifest_source_tools_harness_or_report_case_is_refused(self):
        _,m,s,r=inputs()
        for key,value in [('candidate',{}),('manifest_sha256','f'*64),('harness_sha256','f'*64),('case','cancel'),('status','pending_dependencies')]:
            bad=copy.deepcopy(r);bad[key]=value
            with self.subTest(key=key),self.assertRaises(P.Refused):P.project(bad,m,s,'normal',0,'e'*64)
        for key in ['source_head','compiled_core_source_head','binaries','frontend_assets']:
            bad=copy.deepcopy(m);bad[key]={'private_grant':SECRET}
            with self.subTest(key=key),self.assertRaises(P.Refused):P.candidate_summary(json.dumps(bad).encode())

    def test_cli_publishes_only_summary_preserving_private_raw_originals(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);private=root/'private';private.mkdir(mode=0o700)
            raw,m,s,r=inputs();r['harness_sha256']=P.sha((ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py').read_bytes())
            original=json.dumps(r).encode();report=private/'report.json';report.write_bytes(original);report.chmod(0o600)
            for name in ['runtime.raw.sqlite3','wire-reply-private.json','private-owner.json','snapshot.html','screenshot.png']:(private/name).write_text(SECRET)
            manifest=root/'manifest.json';manifest.write_bytes(raw);output=root/'public'
            argv=['project','--private-dir',str(private),'--manifest',str(manifest),'--case','normal','--exit-code','0','--output',str(output)]
            with mock.patch.object(os,'environ',dict(os.environ)),mock.patch('sys.argv',argv):self.assertEqual(P.main(),0)
            self.assertEqual([p.name for p in output.iterdir()],['summary.json'])
            self.assertNotIn(SECRET,(output/'summary.json').read_text());self.assertEqual(report.read_bytes(),original)
            self.assertEqual((private/'runtime.raw.sqlite3').read_text(),SECRET)

    def test_missing_preflight_symlink_and_bad_json_remain_failed_and_private(self):
        for mode in ['missing','preflight','symlink','bad_json']:
            with self.subTest(mode=mode),tempfile.TemporaryDirectory()as tmp:
                root=Path(tmp);private=root/'private';private.mkdir(mode=0o700);raw,_,_,_=inputs()
                manifest=root/'manifest.json';manifest.write_bytes(raw);output=root/'public'
                if mode=='preflight':(private/'preflight-refusal.json').write_text(SECRET)
                if mode=='symlink':(private/'report.json').symlink_to(manifest)
                if mode=='bad_json':(private/'report.json').write_text(SECRET);(private/'report.json').chmod(0o600)
                argv=['project','--private-dir',str(private),'--manifest',str(manifest),'--case','normal','--exit-code','1','--output',str(output)]
                with mock.patch('sys.argv',argv):self.assertEqual(P.main(),1)
                value=json.loads((output/'summary.json').read_text());self.assertEqual(value['status'],'failed');self.assertNotIn(SECRET,json.dumps(value))

    def test_duplicate_json_keys_and_nonprivate_or_oversized_input_are_refused(self):
        with self.assertRaises(P.Refused):P.decode(b'{"case":"normal","case":"cancel"}')
        with tempfile.TemporaryDirectory()as tmp:
            p=Path(tmp)/'report';p.write_bytes(b'{}');p.chmod(0o644)
            with self.assertRaises(P.Refused):P.regular_bytes(p,100,private=True)
            p.chmod(0o600)
            with self.assertRaises(P.Refused):P.regular_bytes(p,1,private=True)

    def test_final_publish_failure_never_exposes_passed_final_name(self):
        for operation in ['fchmod','replace']:
            with self.subTest(operation=operation),tempfile.TemporaryDirectory()as tmp:
                path=Path(tmp)/'summary.json'
                with mock.patch.object(P.os,operation,side_effect=OSError(SECRET)),self.assertRaises(OSError):P.publish(path,{'status':'passed'})
                self.assertFalse(path.exists());self.assertTrue(path.with_name('summary.json.pending').exists())

    def test_private_ci_driver_and_projection_failures_still_attempt_all_cases(self):
        import subprocess
        import sys
        source=(ROOT/'.github/workflows/product.yml').read_text().split('          import os\n',1)[1].split('\n          PY',1)[0]
        program=compile(ast.parse('import os\n'+textwrap.dedent(source)),'<pure CGCI private orchestration>','exec')
        class Calls:
            CalledProcessError=subprocess.CalledProcessError;SubprocessError=subprocess.SubprocessError;STDOUT=subprocess.STDOUT
            def __init__(self,root,fail):self.root=root;self.fail=fail;self.seen=[]
            def run(self,argv,**kwargs):
                self_outer.assertTrue(kwargs['check']);self_outer.assertEqual(kwargs['stderr'],self.STDOUT)
                kwargs['stdout'].write(SECRET.encode())
                case=argv[argv.index('--case')+1];driver='factory-child-goal-real-ipc.py'in argv[1];self.seen.append((driver,case))
                output=Path(argv[argv.index('--output')+1])
                if driver:
                    self_outer.assertTrue(output.is_relative_to(self.root/'agentbaton-child-goal-ci'))
                    output.mkdir(mode=0o700);(output/'runtime.raw.sqlite3').write_text(SECRET)
                else:
                    self_outer.assertEqual(str(output),'artifacts/ui-browser/factory-child-goal-'+case)
                    output.mkdir(parents=True);(output/'summary.json').write_text('{"status":"failed"}'if self.fail else '{"status":"passed"}')
                if self.fail and (not driver or case!='lost-start'):raise self.CalledProcessError(7,argv)
        self_outer=self
        for fail in [False,True]:
            with self.subTest(fail=fail),tempfile.TemporaryDirectory()as tmp:
                previous=Path.cwd();root=Path(tmp);calls=Calls(root,fail)
                try:
                    os.chdir(root)
                    scope={'pathlib':__import__('pathlib'),'subprocess':calls,'sys':sys,'json':json,'frontend':Path('frontend'),'planning_manifest':Path('manifest'),'chromium':'same-reviewed-chromium'}
                    with mock.patch.dict(os.environ,{'RUNNER_TEMP':str(root)}):
                        if fail:
                            with self.assertRaises(SystemExit)as error:exec(program,scope)
                            self.assertNotIn(SECRET,str(error.exception));self.assertTrue(all(c in str(error.exception)for c in P.CASES))
                        else:exec(program,scope)
                    self.assertEqual(calls.seen,[(driver,case)for case in P.CASES for driver in [True,False]])
                    public=list(Path('artifacts/ui-browser').rglob('*'))
                    self.assertTrue(all(p.is_dir()or p.name=='summary.json'for p in public))
                    self.assertTrue(all(SECRET not in p.read_text()for p in public if p.is_file()))
                    self.assertEqual((root/'agentbaton-child-goal-ci').stat().st_mode&0o777,0o700)
                finally:os.chdir(previous)

if __name__=='__main__':unittest.main()
