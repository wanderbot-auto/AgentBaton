#!/usr/bin/env python3
"""Create real14 history using explicitly frozen tools, private Owner RPC and ACP.

Run only in the coordinator's process lease. No browser, model, account or OS
Owner-dialog claim. Never downgrade a database, insert approvals or change DDL.
The committed database is a SQLite backup of the joined real daemon's database.
"""
import argparse
import ast
import copy
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import time
import uuid

SOURCE = 'f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701'
HASHES = {'batond': '1c835dcd2b925baeb2f0131b225c1047bdc89c4452417a302fae34a133c4cf73',
          'baton': 'd90cf23761b83dc7eeeb92291b411a573e9cfb6ecde3750bc3bb376d2fe66797'}
ACP = r'''import json,os,sys
from pathlib import Path
if sys.argv[1:]==['--version']:
 print('Generated legacy migration ACP 1');sys.exit(0)
assert sys.argv[1:]==[]
pending=None
def reply(i,result):print(json.dumps({'jsonrpc':'2.0','id':i,'result':result}),flush=True)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':reply(i,{'protocolVersion':1,'agentCapabilities':{}})
 elif m=='session/new':reply(i,{'sessionId':'generated-legacy-history'})
 elif m=='session/prompt':
  pending=i;v=json.loads(r['params']['prompt'][0]['text'])
  Path('reports').mkdir(exist_ok=True)
  ticks=Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19]
  Path('reports/migration-owned-'+str(os.getpid())+'.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'start_ticks':ticks}))
  if v['kind']=='agentbaton_goal_task_request':
   assert v['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
   Path('reports').mkdir(exist_ok=True);Path('reports/report.txt').write_bytes(b'fixed legacy Graph candidate\n')
   reply(i,{'stopReason':'end_turn'});pending=None
  else:assert v['kind']=='agentbaton_factory_task_request'
 elif m=='session/cancel' and pending is not None:
  reply(pending,{'stopReason':'cancelled'});pending=None
'''

def sha(data):
    return hashlib.sha256(data).hexdigest()


def bounded(path, cap=4_194_304):
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode) and not info.st_mode & 0o022
    assert 0 < info.st_size <= cap
    return path.read_bytes()


def wait(read, predicate, seconds=20):
    deadline = time.monotonic() + seconds
    while True:
        value = read()
        if predicate(value):
            return value
        assert time.monotonic() < deadline, 'Bounded observation did not reach its required state'
        time.sleep(.02)


@contextmanager
def retained_private_runtime(report):
    root = Path(tempfile.mkdtemp(prefix='ab14-', dir='/tmp')).resolve()
    root.chmod(0o700)
    report['private_runtime'] = str(root)
    report['runtime_cleanup'] = 'retained_on_failure'
    # In particular, assertion/exception paths cannot erase the original DB,
    # WAL, joined-process evidence or private failure diagnostics.
    yield root
    shutil.rmtree(root)
    report['runtime_cleanup'] = 'removed_after_successful_owned_join_and_backup'


def source_function(path, name, namespace):
    # Read only an existing pure plan constructor; never run harness main,
    # Playwright imports, hidden fixtures or an alternative Core implementation.
    raw = bounded(path)
    tree = ast.parse(raw, filename=str(path))
    node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name)
    exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), 'exec'), namespace)
    return sha(raw)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--frozen', type=Path, required=True)
    p.add_argument('--repository', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    frozen = args.frozen.resolve(strict=True)
    repo = args.repository.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    original_manifest = bounded(frozen / 'manifest.json')
    manifest = json.loads(original_manifest)
    assert manifest['git_head'] == SOURCE and manifest['schema'] == 14
    assert manifest['binaries'] == HASHES
    # These are the sole actual Core executables. Verify before any spawn and
    # after shutdown; never rewrite, strip or build the original pair.
    for name, digest in HASHES.items():
        info = (frozen / name).lstat()
        assert stat.S_ISREG(info.st_mode) and info.st_mode & 0o111 and not info.st_mode & 0o022
        assert sha((frozen / name).read_bytes()) == digest
    ns = {'copy': copy, 'sha': sha}
    source_hashes = {}
    graph_source = repo / 'apps/desktop/tests/goal-graph-real-ipc.py'
    for name in ['scope_selection', 'scoped_agent_plan']:
        source_hashes[str(graph_source.relative_to(repo))] = source_function(graph_source, name, ns)
    child_source = repo / 'crates/core/tests/fixtures/factory_child_planning_acp.py'
    source_hashes[str(child_source.relative_to(repo))] = source_function(child_source, 'child_plan', ns)
    report = {'status': 'failed_or_interrupted', 'schema': 14,
              'origin': 'actual_frozen_schema14_core_rpc', 'source_commit': SOURCE,
              'frozen_manifest_sha256': sha(original_manifest), 'binaries': HASHES,
              'generator_sha256': sha(Path(__file__).read_bytes()), 'plan_constructor_sources': source_hashes,
              'native_owner': 'trusted_private_host_fixture_not_os_dialog',
              'provider': 'generated_local_acp_no_model_or_credentials',
              'forced_kill': False, 'requests': [], 'private_reviews': [],
              'not_claimed': ['macOS', 'native dialog', 'real Provider', 'TaskAccepted', 'product completion', 'active worker recovery']}
    daemon = None
    try:
        with retained_private_runtime(report) as root:
            runtime = root / 'r'; runtime.mkdir(mode=0o700)
            assert len(str(runtime / 'batond.sock').encode()) <= 100
            provider = root / 'generated-acp'
            provider.write_text('#!' + sys.executable + '\n' + ACP); provider.chmod(0o700)
            clean_env = {k: v for k, v in os.environ.items() if k in ('PATH','LANG','LC_ALL','TMPDIR','HOME')}
            clean_env['BATON_RUNTIME_DIR'] = str(runtime)
            with (output / 'daemon.log').open('wb') as log:
                daemon = subprocess.Popen([str(frozen / 'batond')], env=clean_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()), lambda r: r[0] and r[1] and r[2] is None)
                    assert stat.S_IMODE((runtime / 'session.token').stat().st_mode) == 0o600
                    token = (runtime / 'session.token').read_text()
                    def call(method, params, key=None, revision=None, allow_error=False):
                        request = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'method': method, 'params': params}
                        if key is not None: request['idempotency_key'] = key
                        if revision is not None: request['expected_revision'] = revision
                        frame = {**request, 'token': token}
                        raw = json.dumps(frame, separators=(',', ':'), allow_nan=False).encode() + b'\n'
                        assert len(raw) <= 262_144
                        with socket.socket(socket.AF_UNIX) as stream:
                            stream.settimeout(10); stream.connect(str(runtime / 'batond.sock')); stream.sendall(raw)
                            with stream.makefile('rb') as reader: response = reader.readline(8_388_609)
                        assert len(response) <= 8_388_608 and response.endswith(b'\n')
                        reply = json.loads(response)
                        # Original value semantics and raw JSON retained without
                        # authentication. Owner token never enters the transcript.
                        report['requests'].append({'request': request, 'response_raw_utf8': response.decode(), 'response': reply})
                        if allow_error: return reply
                        assert not reply.get('error'), {'method': method, 'error': reply.get('error')}
                        return reply['result']
                    def approve(method, params, key, kind):
                        proposal = call(method, params, key)
                        assert proposal['kind'] == kind and proposal['consumed'] is False
                        review = json.dumps(proposal['snapshot'], separators=(',', ':'), ensure_ascii=False).encode()
                        assert 0 < len(review) <= (1_048_576 if kind == 'goal_graph_execution' else 262_144)
                        result = call('approvals.finalize', {'proposal_id': proposal['id']}, 'finalize:' + proposal['id'], proposal['revision'])
                        report['private_reviews'].append({'proposal': proposal, 'result': result, 'snapshot_sha256': sha(review), 'snapshot_utf8_bytes': len(review)})
                        return proposal, result
                    _, connection = approve('connections.propose', {'name': 'Generated migration-only ACP', 'provider': 'custom_acp', 'executable': str(provider), 'arguments': []}, 'legacy:connection', 'connection')
                    wait(lambda: call('connections.list', {}), lambda rows: any(c['id'] == connection['id'] and c['probe_state'] == 'ready' for c in rows))
                    source = root / 'source'; source.mkdir(mode=0o700); (source / 'file').write_bytes(b'baseline')
                    git_env = {**clean_env, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0'}
                    def git(*argv):
                        return subprocess.check_output(['git','-C',str(source),'-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','core.hooksPath=/dev/null',*argv], env=git_env, text=True).strip()
                    git('init','-q','--initial-branch=main'); git('add','file'); git('commit','-qm','fixed generated migration input')
                    project = call('projects.import', {'path': str(source), 'name': 'Actual legacy migration fixture'}, 'legacy:project')
                    workspace = call('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse','HEAD')}, 'legacy:workspace', project['revision'])
                    _, grant = approve('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 600_000}, 'legacy:grant', 'grant')
                    task = call('tasks.create', {'project_id': project['id'], 'goal': 'Fixed actual Native Graph migration history'}, 'legacy:goal-task')
                    selection = ns['scope_selection'](task, grant, count=1, max_nodes=4)
                    selection['inputs'][0]['relative_path'] = 'file'
                    selection['root_budget']['children'] = 3
                    selection['project_budget']['children'] = 3
                    _, scope = approve('goal_scopes.propose', selection, 'legacy:goal-scope', 'goal')
                    plan = ns['scoped_agent_plan'](scope, 1, 'goal-report')
                    call('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': json.dumps(plan, indent=2)+'\n'}, 'legacy:goal-plan', 0)
                    graph_proposal, graph_result = approve('goal_graph_runs.propose', {'scope_id': scope['id'], 'expected_plan_revision': 1, 'expected_ledger_revision': call('system.snapshot', {})['sequence']}, 'legacy:graph', 'goal_graph_execution')
                    graph = wait(lambda: call('goal_graph_runs.get', {'run_id': graph_result['run']['id']}), lambda d: d['run']['state'] in ('awaiting_acceptance','blocked','failed','cancelled'))
                    assert graph['run']['state'] == 'awaiting_acceptance' and graph['task_accepted'] is False
                    # Real cancellation joins workers before the immutable
                    # history snapshot. Never patch states in SQLite.
                    call('goal_graph_runs.cancel', {'run_id': graph['run']['id']}, 'legacy:cancel-graph', graph['run']['revision'])
                    graph = wait(lambda: call('goal_graph_runs.get', {'run_id': graph_result['run']['id']}), lambda d: d['run']['state'] == 'cancelled')
                    factory_task = call('tasks.create', {'project_id': project['id'], 'goal': 'Fixed Factory14 planning history only'}, 'legacy:factory-task')
                    current_grant = next(g for g in call('grants.list', {'project_id': project['id']}) if g['id'] == grant['id'])
                    budget = {'attempts':8,'parallel':1,'runtime_ms':10_000,'output_bytes':8192,'children':3,'reworks':0,'tokens':None,'cost_microunits':None}
                    artifact = {'artifact_id':'report','relative_path':'reports/report.txt','max_bytes':1024,'expected_sha256':None}
                    factory_selection = {'root_task_id':factory_task['id'],'expected_task_revision':factory_task['revision'],'grant_id':grant['id'],'expected_grant_revision':current_grant['revision'],'connection_ids':[connection['id']],'workspace_ids':[workspace['id']],
                        'inputs':[{'artifact_id':'input','relative_path':'file','max_bytes':1024,'expected_sha256':sha(b'baseline')}], 'outputs':[artifact], 'recipients':[], 'max_depth':1,'max_children':3,'max_total_nodes':4,'root_budget':budget,'project_budget':budget,'ttl_ms':600_000,
                        'capabilities':{'delegate_task':True,'resume_parent_as_new_attempt':True,'child_planning':True,'child_goal_execution':False,'create_workspace':False},
                        'child_planning_policy':{'max_nodes':8,'max_revisions':16,'max_plan_bytes':262144,'outputs':[{'artifact':artifact,'validators':[]}],'writable_prefixes':['reports'],'owner_acceptance_criteria':'Store historical plan only; separate Owner acceptance is required'}}
                    _, factory_scope = approve('factory_scopes.propose', factory_selection, 'legacy:factory-scope', 'factory')
                    call('factory_runs.start', {'scope_id':factory_scope['id']}, 'legacy:factory-start', factory_scope['revision'])
                    wait(lambda: list(runtime.glob('workspaces/**/reports/migration-owned-*.json')), lambda markers: len(markers) >= 2)
                    def context_read():
                        return call('factory_child_plans.context', {'scope_id':factory_scope['id'],'task_id':factory_task['id']}, allow_error=True)
                    context_reply = wait(context_read, lambda r: not r.get('error'))
                    child_plan = ns['child_plan'](context_reply['result'],1)
                    raw_plan = json.dumps(child_plan,indent=2)+'\n'
                    stored = call('factory_child_plans.submit', {'scope_id':factory_scope['id'],'task_id':factory_task['id'],'plan_json':raw_plan,'expected_plan_revision':0}, 'legacy:factory-plan', factory_scope['revision'])
                    assert stored['planning_only'] is True and stored['dispatched'] is False and stored['task_accepted'] is False and stored['plan_json'] == raw_plan
                    run = call('factory_runs.get', {'scope_id':factory_scope['id']})
                    call('factory_runs.cancel', {'scope_id':factory_scope['id']}, 'legacy:cancel-factory', run['run']['revision'])
                    wait(lambda: call('factory_runs.get', {'scope_id':factory_scope['id']}), lambda r: r['run']['state'] == 'cancelled')
                    report.update(native_goal_scope_id=scope['id'],native_graph_run_id=graph_result['run']['id'],native_graph_approval_id=graph_proposal['id'],factory_scope_id=factory_scope['id'])
                finally:
                    if daemon.poll() is None: daemon.send_signal(signal.SIGTERM)
                    try: report['daemon_exit'] = daemon.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        report['forced_kill'] = True
                        daemon.kill(); report['daemon_exit'] = daemon.wait(timeout=5)
                assert report['daemon_exit'] == 0 and not report['forced_kill']
                report['owned_processes'] = []
                for marker in sorted(runtime.glob('workspaces/**/reports/migration-owned-*.json')):
                    owned = json.loads(bounded(marker,1024))
                    process = Path('/proc') / str(owned['pid']) / 'stat'
                    try: current = process.read_text().rsplit(')',1)[1].split()[19]
                    except FileNotFoundError: current = None
                    owned['gone_after_join'] = current != owned['start_ticks']
                    report['owned_processes'].append(owned)
                assert len(report['owned_processes']) >= 2 and all(p['gone_after_join'] for p in report['owned_processes'])
                database = runtime / 'baton.sqlite3'
                with sqlite3.connect(database.as_uri()+'?mode=ro',uri=True) as db:
                    assert db.execute('PRAGMA user_version').fetchone()[0] == 14
                    assert not db.execute('PRAGMA foreign_key_check').fetchall()
                    names = ['goal_scopes','goal_runs','goal_graph_capabilities','factory_scopes','factory_runs','factory_child_plan_revisions','factory_child_plan_heads']
                    report['counts'] = {n: db.execute('SELECT count(*) FROM '+n).fetchone()[0] for n in names}
                    assert all(report['counts'].values())
                    assert db.execute("SELECT count(*) FROM goal_runs WHERE state IN ('queued','running','cancel_requested','outcome_unknown')").fetchone()[0] == 0
                    assert db.execute("SELECT count(*) FROM goal_graph_jobs WHERE state IN ('prepared','running','cancel_requested')").fetchone()[0] == 0
                    assert db.execute("SELECT count(*) FROM factory_runs WHERE state IN ('queued','starting','running','waiting_child','resuming','cancel_requested','outcome_unknown')").fetchone()[0] == 0
                    destination = output / 'actual-schema14.sqlite3'
                    destination.touch(mode=0o600,exist_ok=False)
                    with sqlite3.connect(destination) as backup: db.backup(backup)
                report['database_sha256'] = sha(destination.read_bytes())
                report['status'] = 'passed'
    finally:
        for name,digest in HASHES.items():
            assert sha((frozen/name).read_bytes()) == digest, 'Original frozen tool changed'
        (output/'manifest.json').write_text(json.dumps(report,indent=2)+'\n')
        (output/'manifest.json').chmod(0o600)
    assert report['status'] == 'passed'


if __name__ == '__main__':
    main()
