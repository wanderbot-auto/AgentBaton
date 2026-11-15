#!/usr/bin/env python3
"""Genuine old15 Task history from fixed-source product RPC, never DDL synthesis.

Everything retained here is private input evidence. No Provider, Agent login,
Native approval or worktree creation is requested. Default invocation only
prints usage; executing this producer requires Root's separate runtime window.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import time
import uuid

REPO = Path(__file__).resolve().parents[5]
HELPER_PATH = REPO/'scripts/prepare-schema15-migration-fixture.py'
spec = importlib.util.spec_from_file_location('schema15_helper',HELPER_PATH)
H = importlib.util.module_from_spec(spec);spec.loader.exec_module(H)

def snapshot(path):
    with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True) as db:
        db.execute('PRAGMA query_only=ON')
        H.require(db.execute('PRAGMA user_version').fetchone()[0] == 15,'Actual tool did not create schema15')
        H.require(not db.execute('PRAGMA foreign_key_check').fetchall(),'Actual schema15 contains foreign key errors')
        names = [r[0] for r in db.execute("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")]
        counts = {name:db.execute('SELECT count(*) FROM "'+name.replace('"','""')+'"').fetchone()[0] for name in names}
        return {'schema':15,'query_only':True,'foreign_key_errors':0,'counts':counts,
                'sequence':db.execute('SELECT COALESCE(max(sequence),0) FROM events').fetchone()[0]}

def logical_snapshot(path):
    with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True) as db:
        db.execute('PRAGMA query_only=ON')
        schema=db.execute('SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name').fetchall()
        rows={name:db.execute('SELECT rowid,* FROM "'+name.replace('"','""')+'" ORDER BY rowid').fetchall()
              for kind,name,_,_ in schema if kind=='table'}
        return schema,rows,db.execute('PRAGMA user_version').fetchone()[0]

def wait_ready(runtime, child, native):
    until = time.monotonic()+15
    while not ((runtime/'session.token').exists() and (runtime/'batond.sock').exists()):
        H.require(native.owned_child_exited(child.pid) is False and time.monotonic()<until,'Old15 daemon did not start within the controller bound')
        time.sleep(.02)
    endpoint = (runtime/'batond.sock').lstat()
    H.require(stat.S_ISSOCK(endpoint.st_mode) and endpoint.st_uid == os.geteuid() and stat.S_IMODE(endpoint.st_mode)==0o600,'Private socket required')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--frozen',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--build-manifest',type=Path)
    parser.add_argument('--build-evidence',type=Path)
    args = parser.parse_args()
    output = args.output.absolute();output.mkdir(mode=0o700,parents=True,exist_ok=False);output=output.resolve()
    common, common_sha = H.common();native, native_sha = common.load_native_helpers()
    report = {'contract':'agentbaton.actual-task-schema15.v1','status':'failed_or_interrupted','schema':15,
              'origin':'actual_source15_core_rpc','source_commit':H.SOURCE,'source_tree':H.TREE,
              'platform':sys.platform,'architecture':platform.machine(),'os_release':platform.release(),
              'generator_sha256':H.sha(Path(__file__).read_bytes()),'helper_sha256':H.sha(HELPER_PATH.read_bytes()),
              'common_helper_sha256':common_sha,'native_helper_sha256':native_sha,
              'not_claimed':['nonempty Goal/Factory history','Native approval','real Provider','Task binding creation','write-after-migration fault','product completion']}
    child = None;provenance = None;completed = False;error = None
    try:
        H.require(shutil.disk_usage(output).free>=200*1024*1024,'Preserve coordinator free-space guard')
        frozen = args.frozen.resolve(strict=True)
        provenance = H.provenance(frozen,args.build_manifest,args.build_evidence)
        report.update(provenance_kind=provenance['kind'],binaries={n:r['sha256'] for n,r in provenance['records'].items()},tool_metadata=provenance['records'])
        raw = provenance['raw_manifest'];report['tool_provenance_sha256'] = H.sha(raw)
        (output/'tool-provenance.json').write_bytes(raw);(output/'tool-provenance.json').chmod(0o600)
        if 'raw_build_evidence' in provenance:
            raw_build=provenance['raw_build_evidence'];report['original_build_evidence_sha256']=H.sha(raw_build)
            (output/'original-build-evidence.json').write_bytes(raw_build);(output/'original-build-evidence.json').chmod(0o600)
            report['historical_toolchain_limit']=provenance['toolchain_limit']
        root = Path(tempfile.mkdtemp(prefix='ab15-task-',dir='/tmp')).resolve();root.chmod(0o700)
        report['retained_private_root']=str(root)
        runtime=root/'r';runtime.mkdir(mode=0o700);home=root/'home';home.mkdir(mode=0o700);scratch=root/'tmp';scratch.mkdir(mode=0o700)
        source=root/'source';source.mkdir(mode=0o700);(source/'input.txt').write_text('Generated source for actual15 Task migration. 雪\n')
        source_hash=H.sha((source/'input.txt').read_bytes())
        H.require(len(str(runtime/'batond.sock').encode())<=100,'Canonical Unix socket path exceeds bound')
        env={'PATH':os.environ.get('PATH','/usr/bin:/bin'),'HOME':str(home),'TMPDIR':str(scratch),'LANG':'C.UTF-8','BATON_RUNTIME_DIR':str(runtime)}
        report['argv']=[str(frozen/'batond')];report['runtime_mode']='0o700';report['socket_utf8_bytes']=len(str(runtime/'batond.sock').encode())
        log_fd=os.open(output/'daemon.log',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
        with os.fdopen(log_fd,'wb') as log:
            report['daemon']={'sigterm_sent':False,'forced_kill':False}
            child=subprocess.Popen(report['argv'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            report['daemon']['owned_identity']=common.darwin_owned_identity(child)
            wait_ready(runtime,child,native)
            report['initial_probe']=snapshot(runtime/'baton.sqlite3')
            H.require(report['initial_probe']['sequence']==0 and report['initial_probe']['counts']['tasks']==0,'Old15 runtime was not fresh')
            H.require(all(report['initial_probe']['counts'][n]==0 for n in H.FACTORY),'Fresh old15 Factory tables are not empty')
            token=H.read_regular(runtime/'session.token',1024).decode()
            transcript_fd=os.open(output/'rpc-private.jsonl',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
            with os.fdopen(transcript_fd,'wb') as transcript:
                def call(method,params,key=None,revision=None,expected_error=None):
                    request={'api_version':1,'request_id':str(uuid.uuid4()),'method':method,'params':params}
                    if key is not None:request['idempotency_key']=key
                    if revision is not None:request['expected_revision']=revision
                    wire=json.dumps({**request,'token':token},allow_nan=False,separators=(',',':')).encode()+b'\n'
                    H.require(len(wire)<=262144,'RPC request exceeds private frame bound')
                    with socket.socket(socket.AF_UNIX) as connection:
                        connection.settimeout(10);connection.connect(str(runtime/'batond.sock'));connection.sendall(wire)
                        with connection.makefile('rb') as reader:reply_raw=reader.readline(4_194_305)
                    H.require(0<len(reply_raw)<=4_194_304 and reply_raw.endswith(b'\n'),'RPC response exceeds private frame bound')
                    reply=json.loads(reply_raw,object_pairs_hook=H.unique)
                    transcript.write((json.dumps({'request_without_token':request,'response_raw_utf8':reply_raw.decode()},ensure_ascii=False)+'\n').encode());transcript.flush()
                    if expected_error:
                        H.require(reply.get('error',{}).get('code')==expected_error,'Expected old15 rejection was not observed');return reply
                    H.require(not reply.get('error') and 'result'in reply,'Actual old15 RPC failed')
                    return reply['result']
                initial=call('system.snapshot',{})
                H.require(initial['api_version']==1 and initial['sequence']==0 and initial['tasks']==[],'Wrong actual old15 IPC initial snapshot')
                cli=subprocess.run([str(frozen/'baton'),'snapshot'],env=env,capture_output=True,timeout=10)
                H.require(cli.returncode==0 and len(cli.stdout)<=4_194_304 and len(cli.stderr)<=262144,'Actual companion snapshot failed')
                cli_value=json.loads(cli.stdout,object_pairs_hook=H.unique)
                H.require(cli_value['result']['api_version']==1 and cli_value['result']['sequence']==0,'Companion does not match empty old15 runtime')
                report['actual_cli_probe']={'exit':0,'api_version':1,'sequence':0}
                project=call('projects.import',{'path':str(source),'name':'Actual15 migration generated directory'},'actual15-project')
                tasks=[]
                for index,(project_id,goal) in enumerate([(project['id'],'Actual15 registered draft 雪'),(project['id'],'Actual15 cancelled history'),('local','Actual15 personal draft')]):
                    key='actual15-create-'+str(index)
                    task=call('tasks.create',{'project_id':project_id,'goal':goal},key)
                    H.require('code_workspace_binding'not in task and task['state']=='draft','Old15 Task shape was not genuine')
                    H.require(H.strict_equal(task,call('tasks.create',{'project_id':project_id,'goal':goal},key)),'Exact old receipt did not replay')
                    tasks.append({'id':task['id'],'project_id':project_id,'goal':goal,'create_key':key,'state':'draft','revision':task['revision']})
                cancelled=call('tasks.cancel',{'task_id':tasks[1]['id']},revision=tasks[1]['revision'])
                H.require(cancelled['state']=='cancelled' and cancelled['revision']>tasks[1]['revision'],'Actual task cancellation was not persisted')
                tasks[1].update(state=cancelled['state'],revision=cancelled['revision'])
                replay=call('tasks.create',{'project_id':tasks[1]['project_id'],'goal':tasks[1]['goal']},tasks[1]['create_key'])
                H.require(replay['state']=='draft' and replay['revision']==1,'Historical receipt was rewritten by later cancellation')
                call('tasks.create',{'project_id':tasks[0]['project_id'],'goal':'Conflicting same key'},tasks[0]['create_key'],expected_error='CONFIG_CONFLICT')
                final=call('system.snapshot',{});H.require(len(final['tasks'])==3,'Actual old15 Task history count changed')
                report['tasks']=tasks;report['project_id']=project['id'];report['final_api_sequence']=final['sequence']
            completed=True
    except BaseException as caught:
        error=caught;report['failure_kind']=type(caught).__name__
    finally:
        try:
            if child is not None:
                # This reviewed helper holds the original leader unreaped,
                # observes its private group, signals and wait()s the owned PID.
                common.stop_darwin_owned(child,native,report['daemon'])
                H.require(report['daemon'].get('identity_gone_after_join') is True,'Owned old15 daemon did not SIGTERM/join0')
            if provenance is not None:
                report['tool_metadata_after']=H.tool_records(frozen)
                H.require(H.strict_equal(report['tool_metadata_after'],provenance['records']),'Frozen pair changed during generation')
            if completed and error is None:
                H.require(H.sha((source/'input.txt').read_bytes())==source_hash and sorted(p.name for p in source.iterdir())==['input.txt'],'Registered original directory was changed')
                final_probe=snapshot(runtime/'baton.sqlite3');report['final_probe']=final_probe
                H.require(final_probe['counts']['tasks']==3 and final_probe['counts']['events']>=5 and final_probe['counts']['idempotency']>=4,'Missing genuine Task history')
                H.require(all(final_probe['counts'][n]==0 for n in H.FACTORY),'Unexpected Factory authority was created')
                H.require(all(final_probe['counts'][n]==0 for n in H.EMPTY_AUTHORITY),'Task history must not grant execution or provider authority')
                H.require(final_probe['sequence']==report['final_api_sequence'],'Original event sequence changed after joined shutdown')
                destination=output/'actual-schema15.sqlite3'
                fd=os.open(destination,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600);os.close(fd)
                with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True) as original,sqlite3.connect(destination) as backup:
                    original.execute('PRAGMA query_only=ON');original.backup(backup)
                H.require(snapshot(destination)==final_probe,'Joined SQLite backup differs from actual original15')
                H.require(logical_snapshot(destination)==logical_snapshot(runtime/'baton.sqlite3'),'Backup changed old raw rows/rowids or schema')
                report['backup_logical_exact']=True
                report['database_sha256']=H.sha(H.read_regular(destination,16_777_216))
                report['database_bytes']=destination.stat().st_size
                H.require(H.sha(Path(__file__).read_bytes())==report['generator_sha256'] and H.sha(HELPER_PATH.read_bytes())==report['helper_sha256'],'Producer source changed during generation')
                H.require(H.common()[1]==common_sha and common.load_native_helpers()[1]==native_sha,'Lifecycle helper source changed')
                report['status']='passed'
        except BaseException as caught:
            report['status']='failed_or_interrupted';report['cleanup_or_snapshot_failure_kind']=type(caught).__name__
            if error is None:error=caught
        common.publish_private_json(output/'manifest.json',report)
    if error is not None:raise error
    H.require(report['status']=='passed','Actual old15 generation did not complete')

if __name__=='__main__':main()
