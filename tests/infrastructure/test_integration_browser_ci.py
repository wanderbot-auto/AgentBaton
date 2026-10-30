"""Pure CI/source/virtual-tool tests, no browser/Core/Cargo/Git fixture runs."""
import ast
import contextlib
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import types
import unittest
from unittest import mock

import yaml
from tests.infrastructure.test_schema17_migration_workflow import (
    strip_integration_browser_ci_layer, verify_only_six_historical_target_changes, BROWSER_CI_STEPS)

ROOT=Path(__file__).resolve().parents[2]
WORKFLOW=ROOT/'.github/workflows/product.yml'
RUN='Actual integration frontend with real Core IPC and generated correctness'
PROJECT='Retain finite integration browser evidence without private runtime data'
HEAD='a'*40
SECRET='PRIVATE_CORE_PROPOSAL_TOKEN_DATABASE_PATH_CANARY'


def document():
    return yaml.load(WORKFLOW.read_text(),Loader=yaml.BaseLoader)


def step(name):
    rows=[s for s in document()['jobs']['frontend']['steps'] if s.get('name')==name]
    if len(rows)!=1:raise AssertionError('One exact new UI step required')
    return rows[0]


def program(name,marker):
    run=step(name)['run'];header="- <<'"+marker+"'\n"
    if run.count(header)!=1 or not run.endswith(marker+'\n'):raise AssertionError('Fixed inline Python expected')
    source=run.split(header,1)[1][:-len(marker+'\n')]
    namespace={'__name__':'integration_browser_ci_contract'}
    exec(compile(source,'<fixed-integration-browser-ci>','exec'),namespace)
    return namespace,source


def driver():
    spec=importlib.util.spec_from_file_location('_integration_ci_driver_pure',ROOT/'apps/desktop/tests/integration-real-ipc.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


@contextlib.contextmanager
def fixture():
    with tempfile.TemporaryDirectory() as temporary:
        root=Path(temporary).resolve();runner=root/'temp';runner.mkdir(mode=0o700)
        (root/'target/debug').mkdir(parents=True)
        hashes={}
        for name in ('batond','baton'):
            p=root/'target/debug'/name;p.write_bytes((name+' synthetic image').encode());p.chmod(0o500)
            hashes[name]=hashlib.sha256(p.read_bytes()).hexdigest()
        public=root/'artifacts/ui-browser';public.mkdir(parents=True)
        (public/'goal-execution-candidate.json').write_text(json.dumps({'schema':17,'git_head':HEAD,'dirty':False,'binaries':hashes}))
        (public/'frontend-build-record.json').write_text(json.dumps({'source':HEAD,'exit':0,'source_head_and_tracked_unchanged':True}))
        chromium=root/'real-installed-chromium';chromium.write_bytes(b'synthetic ELF only');chromium.chmod(0o500)
        previous=Path.cwd()
        try:
            os.chdir(root)
            with mock.patch.dict(os.environ,{'RUNNER_TEMP':str(runner)}):yield root,runner,chromium
        finally:os.chdir(previous)


class WorkflowSource(unittest.TestCase):
    def test_same_existing_jobs_checks_limits_actions_and_raw_whole_baseline_preserved(self):
        text=WORKFLOW.read_text();restored=strip_integration_browser_ci_layer(text)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),'599074c95afb9324470bc859f11added4b5232a877ba6e0486001e1bebf015fc')
        verify_only_six_historical_target_changes(text)
        d=document();self.assertEqual(set(d['jobs']),{'core','frontend','desktop'})
        self.assertEqual([d['jobs'][j]['timeout-minutes'] for j in ('core','frontend','desktop')],['25','20','60'])
        self.assertEqual(d['permissions'],{'contents':'read'})
        for job in d['jobs'].values():
            for s in job['steps']:
                if 'uses' in s:self.assertRegex(s['uses'],r'@[a-f0-9]{40}$')
        self.assertNotIn('continue-on-error',step(RUN));self.assertEqual(step(PROJECT)['if'],'always()')
        self.assertEqual(step(RUN)['id'],'integration_browser_driver')
        self.assertEqual(step(PROJECT)['env'],{'INTEGRATION_BROWSER_DRIVER_OUTCOME':'${{ steps.integration_browser_driver.outcome }}'})

    def test_exact_added_block_hash_count_job_and_position_cannot_be_weakened(self):
        text=WORKFLOW.read_text();headers=list(re.finditer(r'^      - .*\n',text,re.M))
        for name,index,_ in BROWSER_CI_STEPS:
            start,end=headers[index].start(),headers[index+1].start();block=text[start:end]
            for changed in [text[:start]+block.replace('0o600','0o644',1)+text[end:],
                text[:start]+block+block+text[end:],text[:start]+text[end:],
                text[:start]+block.replace('\n','\r\n')+text[end:]]:
                with self.subTest(name=name),self.assertRaises(ValueError):strip_integration_browser_ci_layer(changed)
        addition=text[headers[27].start():headers[29].start()];without=text.replace(addition,'',1)
        moved=without.replace('      - name: Real daemon and browser test dependencies\n',addition+'      - name: Real daemon and browser test dependencies\n',1)
        with self.assertRaises(ValueError):strip_integration_browser_ci_layer(moved)
        for old,new in [("'schema': 17,","'schema': 16,"),('timeout-minutes: 20','timeout-minutes: 21'),
            ("'--case', 'all'","'--case', 'normal'"),('if: always()','if: success()'),('contents: read','contents: write')]:
            self.assertIn(old,text)
            with self.subTest(old=old),self.assertRaises(ValueError):verify_only_six_historical_target_changes(text.replace(old,new,1))

    def test_schema_literal_is_current_and_actual_backend_probe_is_not_metadata_only(self):
        _,source=program(RUN,'INTEGRATION_BROWSER')
        tree=ast.parse(source)
        methods=[n.args[0].value for n in ast.walk(tree) if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute)
            and n.func.attr=='result' and n.args and isinstance(n.args[0],ast.Constant)]
        self.assertEqual(methods,['system.snapshot'])
        self.assertIn("snapshot['database_schema_version'] == 17",source)
        self.assertIn("snapshot['api_version'] == 1",source)
        self.assertIn('process.send_signal(signal.SIGTERM)',source)
        self.assertIn("'daemon_exit': 0, 'joined': True",source)
        self.assertNotIn('approvals.finalize',source)
        self.assertEqual(WORKFLOW.read_text().count("              'schema': 17,"),1)

    def test_no_new_raw_public_paths_or_private_logs_are_uploaded(self):
        _,source=program(RUN,'INTEGRATION_BROWSER');_,projection=program(PROJECT,'INTEGRATION_BROWSER_EVIDENCE')
        self.assertIn("temporary = pathlib.Path(os.environ['RUNNER_TEMP']).resolve(strict=True)",source)
        self.assertIn("private.mkdir(mode=0o700, exist_ok=False)",source)
        self.assertIn('os.O_EXCL | os.O_NOFOLLOW, 0o600',source)
        self.assertIn("'--output', str(private / 'driver')",source)
        self.assertIn("'artifacts/ui-browser/integration-browser'",projection)
        self.assertNotIn('copytree',projection);self.assertNotIn('read_text',projection)
        self.assertEqual(next(s for s in document()['jobs']['frontend']['steps'] if s.get('uses','').startswith('actions/upload-artifact'))['with']['path'],'artifacts/ui-browser/')


class DriverWrapper(unittest.TestCase):
    def run_fake(self,namespace,root,chromium,exit_code=0,probe_error=False):
        calls=[];module=driver()
        namespace['git']=lambda *args:HEAD if args==('rev-parse','HEAD') else ''
        namespace['source_driver']=lambda:module
        namespace['chromium_path']=lambda:chromium
        def probe(*args):
            if probe_error:raise ValueError(SECRET)
        namespace['actual_schema_probe']=probe
        def run(command,**kwargs):
            calls.append((command,kwargs));kwargs['stdout'].write(SECRET.encode())
            if isinstance(exit_code,Exception):raise exit_code
            return types.SimpleNamespace(returncode=exit_code)
        namespace['subprocess']=types.SimpleNamespace(run=run,STDOUT=subprocess.STDOUT)
        result=namespace['run_integration_browser']()
        return result,calls

    def test_exact_same_source_manifest_build_existing_venv_and_installed_browser_flags(self):
        code,_=program(RUN,'INTEGRATION_BROWSER')
        with fixture() as (root,temporary,chromium):
            result,calls=self.run_fake(code,root,chromium)
            self.assertEqual(result,0);self.assertEqual(len(calls),1)
            command,kwargs=calls[0]
            self.assertEqual(command[1:6],['apps/desktop/tests/integration-real-ipc.py','--run','--case','all','--batond'])
            self.assertEqual(command[6],str(temporary/'agentbaton-integration-browser-ci/tools/batond'))
            self.assertEqual(command[-4:],['--chromium',str(chromium),'--output',str(temporary/'agentbaton-integration-browser-ci/driver')])
            self.assertIn(HEAD,command);self.assertIs(kwargs['check'],False)
            private=temporary/'agentbaton-integration-browser-ci'
            status=json.loads((private/'status.json').read_text());self.assertEqual(status,{'source':HEAD,'driver_exit':0,'probe_passed':True})
            self.assertEqual(stat.S_IMODE(private.stat().st_mode),0o700)
            for name in ['driver.log','status.json','candidate.json','frontend-build-record.json']:
                self.assertEqual(stat.S_IMODE((private/name).stat().st_mode),0o600)
            for name in ('batond','baton'):
                self.assertEqual(stat.S_IMODE((private/'tools'/name).stat().st_mode),0o500)
                self.assertEqual((private/'tools'/name).read_bytes(),(root/'target/debug'/name).read_bytes())

    def test_driver_nonzero_bool_unobserved_or_probe_failure_is_not_green(self):
        for exit_code,probe_error in [(1,False),(False,False),(OSError(SECRET),False),(0,True)]:
            with self.subTest(exit=exit_code,probe=probe_error),fixture() as (root,temporary,chromium):
                code,_=program(RUN,'INTEGRATION_BROWSER');result,calls=self.run_fake(code,root,chromium,exit_code,probe_error)
                self.assertEqual(result,1)
                value=json.loads((temporary/'agentbaton-integration-browser-ci/status.json').read_text())
                if probe_error:self.assertEqual(calls,[]);self.assertIs(value['probe_passed'],False)
                if isinstance(exit_code,Exception):self.assertIsNone(value['driver_exit'])
                self.assertNotIn(SECRET,json.dumps(value))

    def test_private_root_reuse_refuses_without_overwriting_old_evidence(self):
        code,_=program(RUN,'INTEGRATION_BROWSER')
        with fixture() as (root,temporary,chromium):
            private=temporary/'agentbaton-integration-browser-ci';private.mkdir(mode=0o700);(private/'old').write_text(SECRET)
            with self.assertRaises(FileExistsError):self.run_fake(code,root,chromium)
            self.assertEqual((private/'old').read_text(),SECRET)

    def test_runner_temp_alias_resolves_before_any_driver_path(self):
        code,_=program(RUN,'INTEGRATION_BROWSER')
        with fixture() as (root,temporary,chromium):
            alias=root/'alias';alias.symlink_to(temporary,target_is_directory=True)
            with mock.patch.dict(os.environ,{'RUNNER_TEMP':str(alias)}):result,calls=self.run_fake(code,root,chromium)
            self.assertEqual(result,0);self.assertIn(str(temporary/'agentbaton-integration-browser-ci/tools/batond'),calls[0][0])

    def test_paired_manifest_source_or_schema_mismatch_stops_before_driver(self):
        for field,bad in [('schema',16),('schema',True),('git_head','f'*40),('dirty',True)]:
            with self.subTest(field=field),fixture() as (root,temporary,chromium):
                path=root/'artifacts/ui-browser/goal-execution-candidate.json';value=json.loads(path.read_text());value[field]=bad;path.write_text(json.dumps(value))
                code,_=program(RUN,'INTEGRATION_BROWSER');result,calls=self.run_fake(code,root,chromium)
                self.assertEqual(result,1);self.assertEqual(calls,[])


class ProjectionWrapper(unittest.TestCase):
    def test_always_projection_runs_even_if_driver_missing_and_cannot_wash_failed_step(self):
        for outcome,exit_code in [('success',0),('failure',0),('skipped',0),('success',1),('success',False)]:
            with self.subTest(outcome=outcome,exit=exit_code),fixture() as (root,temporary,_):
                code,_=program(PROJECT,'INTEGRATION_BROWSER_EVIDENCE');calls=[]
                def run(command,**kwargs):
                    calls.append((command,kwargs));kwargs['stdout'].write(SECRET.encode());return types.SimpleNamespace(returncode=exit_code)
                code['subprocess']=types.SimpleNamespace(check_output=lambda *a,**k:HEAD+'\n',run=run,STDOUT=subprocess.STDOUT)
                with mock.patch.dict(os.environ,{'INTEGRATION_BROWSER_DRIVER_OUTCOME':outcome}):result=code['project_integration_browser']()
                self.assertEqual(result,0 if outcome=='success' and type(exit_code) is int and exit_code==0 else 1)
                self.assertEqual(len(calls),1);self.assertEqual(calls[0][0][1],'scripts/record-integration-browser-ci-evidence.py')
                self.assertEqual(calls[0][0][-2:],['--driver-step-outcome',outcome])
                self.assertEqual(stat.S_IMODE((temporary/'agentbaton-integration-browser-ci/projection.log').stat().st_mode),0o600)

    def test_projection_private_alias_or_world_mode_refuses_before_subprocess(self):
        for kind in ['alias','world']:
            with self.subTest(kind=kind),fixture() as (root,temporary,_):
                code,_=program(PROJECT,'INTEGRATION_BROWSER_EVIDENCE');private=temporary/'agentbaton-integration-browser-ci'
                if kind=='alias':
                    other=root/'other';other.mkdir(mode=0o700);private.symlink_to(other,target_is_directory=True)
                else:private.mkdir(mode=0o755);private.chmod(0o755)
                run=mock.Mock();code['subprocess']=types.SimpleNamespace(check_output=lambda *a,**k:HEAD+'\n',run=run,STDOUT=subprocess.STDOUT)
                with self.assertRaises(ValueError):code['project_integration_browser']()
                run.assert_not_called()


class ChromiumImageBounds(unittest.TestCase):
    def test_only_chromium_cap_is_512_and_preflight_postcheck_both_use_it(self):
        d=driver();self.assertEqual(d.MAX_CHROMIUM_IMAGE,536870912)
        self.assertEqual(inspect.signature(d.pinned_file).parameters['limit'].default,268435456)
        source=(ROOT/'apps/desktop/tests/integration-real-ipc.py').read_text()
        self.assertEqual(source.count('pinned_file(args.chromium, MAX_CHROMIUM_IMAGE)'),2)
        self.assertIn("'python': pinned_file(Path(sys.executable).resolve(strict=True))",source)

    def test_virtual_image_above_256_below_512_accepts_complete_read_but_above512_refuses(self):
        # Synthetic logical file: reused1MiB byte blocks and a fake hash avoid
        # allocating/reading a real300MiB file. No Chromium/process is run.
        d=driver();block=b'\0'*1048576
        def attempt(size,limit):
            info=types.SimpleNamespace(st_mode=stat.S_IFREG|0o755,st_dev=1,st_ino=2,st_size=size,
                st_uid=os.geteuid(),st_nlink=1,st_mtime_ns=0,st_ctime_ns=0)
            class Stream:
                remaining=size
                def __enter__(self):return self
                def __exit__(self,*_):pass
                def fileno(self):return 42
                def read(self,count):
                    n=min(count,self.remaining);self.remaining-=n
                    return block if n==len(block) else block[:n]
            hashed=types.SimpleNamespace(update=lambda _:None,hexdigest=lambda:'a'*64)
            with mock.patch.object(Path,'lstat',return_value=info),mock.patch.object(d.os,'open',return_value=42) as opened,\
                mock.patch.object(d.os,'fdopen',return_value=Stream()),mock.patch.object(d.os,'fstat',return_value=info),\
                mock.patch.object(d.hashlib,'sha256',return_value=hashed):
                return d.pinned_file(Path('/synthetic/installed-chromium'),limit)
        with self.assertRaises(d.Refusal):attempt(268435457,268435456)
        self.assertEqual(attempt(326880088,d.MAX_CHROMIUM_IMAGE)['bytes'],326880088)
        with self.assertRaises(d.Refusal):attempt(536870913,d.MAX_CHROMIUM_IMAGE)


if __name__=='__main__':
    unittest.main()
