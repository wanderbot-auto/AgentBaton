#!/usr/bin/env python3
"""Real daemon Unix IPC and SQLite history package; synthetic data, no Provider calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time
import uuid

ROOT=Path(__file__).resolve().parents[1]

def digest(data): return hashlib.sha256(data).hexdigest()

def rpc(runtime,method,params=None,key=None,drop_reply=False):
    frame=dict(api_version=1,request_id=str(uuid.uuid4()),token=(runtime/'session.token').read_text(),method=method,params=params or {})
    if key: frame['idempotency_key']=key
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(8);stream.connect(str(runtime/'batond.sock'));stream.sendall(json.dumps(frame).encode()+b'\n')
        if drop_reply:
            # Keep the authenticated request alive until the server has made
            # the fixed publication durable, then discard the unread response.
            # Closing immediately can legitimately abandon before admission.
            published=runtime/'history-exports'/(params['preview_id']+'.history.json')
            deadline=time.monotonic()+5
            while not published.exists():
                if time.monotonic()>=deadline: raise TimeoutError('publication did not reach the lost-response boundary')
                time.sleep(.01)
            return None
        with stream.makefile('rb') as incoming:
            result=json.loads(incoming.readline(1048577))
        if 'error' in result: raise RuntimeError(json.dumps(result['error']))
        return result['result']

def stop(process):
    if process.poll() is None: process.terminate()
    process.wait(timeout=10)

def start(runtime,log):
    binary=Path(os.environ.get('CARGO_TARGET_DIR',str(ROOT/'target')))/'debug/batond'
    process=subprocess.Popen([str(binary)],env=dict(os.environ,BATON_RUNTIME_DIR=str(runtime)),stdout=log,stderr=log)
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        if process.poll() is not None: raise RuntimeError('daemon exited before readiness')
        try: rpc(runtime,'system.snapshot');return process
        except (OSError,ValueError,RuntimeError): time.sleep(.03)
    stop(process);raise TimeoutError('daemon readiness timeout')

def business_hash(runtime):
    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True) as db:
        return digest(json.dumps({name:db.execute('SELECT * FROM '+name+' ORDER BY 1').fetchall() for name in ('projects','tasks','events','idempotency','dispatch_intents')},sort_keys=True).encode())

def seed(runtime,log):
    process=start(runtime,log)
    try:
        source=runtime/'project-source';source.mkdir(exist_ok=True)
        other=runtime/'other-source';other.mkdir(exist_ok=True)
        project=rpc(runtime,'projects.import',dict(path=str(source),name='History export 中文'),key='project')
        second=rpc(runtime,'projects.import',dict(path=str(other),name='Excluded project'),key='other-project')
        task=rpc(runtime,'tasks.create',dict(project_id=project['id'],goal='Read task API_KEY=KNOWN-CREDENTIAL'),key='task')
        other_task=rpc(runtime,'tasks.create',dict(project_id=second['id'],goal='CROSS-PROJECT-GOAL'),key='other-task')
    finally: stop(process)
    with sqlite3.connect(runtime/'baton.sqlite3') as db:
        for aggregate,payload in ((task['id'],{'text':'已有会话正文 hello'}),(task['id'],{'text':'Authorization: Bearer KNOWN-TEXT-CREDENTIAL','raw':{'password':'KNOWN-RAW-CREDENTIAL'}}),(other_task['id'],{'text':'CROSS-PROJECT-TEXT'})):
            document=dict(event_id=str(uuid.uuid4()),aggregate_id=aggregate,revision=1,kind='task.message_queued',occurred_at='2026-11-17T00:00:00Z',source='synthetic-history-fixture',payload=payload)
            db.execute('INSERT INTO events(document) VALUES(?)',(json.dumps(document),))
    return project

def validate_package(path,reference):
    data=path.read_bytes();assert digest(data)==reference['package_sha256'];package=json.loads(data)
    assert package['manifest']['format']=='agentbaton-history' and package['manifest']['restorable'] is False
    for file in package['files'].values(): assert digest(file['content'].encode())==file['sha256']
    text=data.decode();assert '已有会话正文 hello' in text
    for value in ('KNOWN-CREDENTIAL','KNOWN-TEXT-CREDENTIAL','KNOWN-RAW-CREDENTIAL','CROSS-PROJECT-TEXT','CROSS-PROJECT-GOAL'): assert value not in text
    return data

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=ROOT/'artifacts/history-export/ipc');args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='ab-hexp-') as temporary, (args.output/'daemon.log').open('w') as log:
        runtime=Path(temporary).resolve();runtime.chmod(0o700);project=seed(runtime,log);before=business_hash(runtime);process=start(runtime,log)
        try:
            selection=dict(version=1,project_id=project['id'],from_sequence=0,to_sequence=None,tasks=True,messages=True,events=True)
            preview=rpc(runtime,'history_exports.preview',selection);ref={key:preview[key] for key in ('preview_id','selection_digest','package_sha256')}
            assert rpc(runtime,'history_exports.get',ref)['state']=='preview'
            rpc(runtime,'history_exports.publish',ref,drop_reply=True)
            deadline=time.monotonic()+8
            while True:
                try:
                    result=rpc(runtime,'history_exports.get',ref)
                    if result['state']=='ready': break
                except RuntimeError as e:
                    if 'WORKSPACE_BUSY' not in str(e): raise
                assert time.monotonic()<deadline;time.sleep(.03)
            path=runtime/'history-exports'/result['filename'];data=validate_package(path,ref);inode=path.stat().st_ino
            assert rpc(runtime,'history_exports.publish',ref)['state']=='ready' and path.stat().st_ino==inode
            changed=dict(ref,selection_digest='0'*64)
            try: rpc(runtime,'history_exports.publish',changed);raise AssertionError('tamper accepted')
            except RuntimeError as error: assert 'INPUT_CHANGED' in str(error)
            stop(process);process=start(runtime,log);assert rpc(runtime,'history_exports.get',ref)['state']=='ready'
            assert before==business_hash(runtime)
            destination=args.output/'sample.history.json';destination.write_bytes(data);destination.chmod(0o600)
            record=dict(status='passed',dimension='real_daemon_unix_ipc_sqlite_synthetic_history',provider_calls='not_run',native_webview='not_run',selection=selection,reference=ref,sha256=digest(data),bytes=len(data),business_hash_before=before,business_hash_after=business_hash(runtime),checks=['manifest','file_sha256','redaction','cross_project','unknown_reply_reconcile','no_overwrite_retry','tampered_selection','daemon_restart','business_history_unchanged'],artifact=str(destination.resolve()))
            (args.output/'report.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
        finally: stop(process)
if __name__=='__main__': main()
