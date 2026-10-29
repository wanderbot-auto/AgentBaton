#!/usr/bin/env python3
"""Actual built React/Unix Goal execution with generated, account-free ACP.

Only the private fixture host approves fixed local resources, outside HTTP.
The HTTP adapter never exposes approval, finalization or generic dispatch.
No synthetic SQL writes, TaskAccepted, vendor/account/model or native OS claim.
"""
from __future__ import annotations

import argparse
from functools import partial
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import uuid

from playwright.sync_api import expect, sync_playwright


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('Duplicate fixture adapter field')
        value[key] = item
    return value


def integer(value, minimum=0):
    return type(value) is int and minimum <= value <= 9_007_199_254_740_991


def linux_start_ticks(pid):
    try:
        return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
    except FileNotFoundError:
        return None


# The same generated program services every scenario. Its executable bytes are
# approved once before invocation; no per-run script mutation or real vendor.
FIXTURE = r'''
import json,os,re,sys
from pathlib import Path
if sys.argv[1:]==['--version']:
 print('AgentBaton generated Goal ACP 1.0');sys.exit(0)
assert sys.argv[1:]==[]
def send(value): print(json.dumps(value),flush=True)
pending=None
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':
  send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-owned-goal'}})
 elif m=='session/prompt':
  text=r['params']['prompt'][0]['text'];v=json.loads(text);pending=i
  assert set(v)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location'}
  assert v['kind']=='agentbaton_goal_task_request' and v['schema_version']==1
  assert v['workspace']['canonical_root']==os.getcwd() and v['workspace']['id']==v['node']['workspace_id']
  assert v['lineage']['attempt_id'] and v['plan_digest'] and v['return_location']['node_id']==v['node']['id']
  assert v['return_location']['run_id']==v['run_id'] and v['return_location']['attempt_id']==v['lineage']['attempt_id']
  assert v['inputs'][0]['artifact']['relative_path'] in ('file','inputs/file')
  assert v['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
  assert len(v['outputs']) in (1,2)
  if len(v['outputs'])==2:
   assert [(x['artifact']['artifact_id'],x['artifact']['relative_path']) for x in v['outputs']]==[('report','reports/report.txt'),('summary','reports/summary.txt')]
  assert v['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
  assert v['budget']['reservation']==v['node']['reservation'] and v['budget']['limits']==v['node']['budget']
  Path('reports').mkdir(exist_ok=True)
  Path('reports/task-request.json').write_text(text)
  ticks=Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19]
  Path('reports/fixture-owned.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'start_ticks':ticks}))
  if len(v['outputs'])==2:
   attempt=v['lineage']['attempt_id'];assert re.fullmatch(r'[A-Za-z0-9._:-]{1,128}',attempt)
   Path('reports/task-request-'+attempt+'.json').write_text(text)
   Path('reports/fixture-owned-'+attempt+'.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'start_ticks':ticks}))
  goal=v['goal']
  if goal in ('goal-report','goal-report-v2'):
   Path('reports/report.txt').write_bytes(b'fixed report' if goal=='goal-report' else b'fixed report v2')
  elif goal=='goal-partial-hang':
   Path('reports/report.txt').write_bytes(b'partial report')
  elif goal=='goal-summary':
   assert len(v['outputs'])==2 and v['node']['kind']=='summary' and v['node']['outputs']==[{'artifact_id':'summary','contract_id':'output:summary'}]
   Path('reports/summary.txt').write_bytes(b'fixed summary')
  elif goal!='hang':raise AssertionError('Unexpected fixed fixture goal')
  if goal in ('hang','goal-partial-hang'):
   Path('reports/fixture-hang').write_text(str(os.getpid()));continue
  send({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'generated-owned-goal','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'generated candidate only'}}}})
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}});pending=None
 elif m=='session/cancel' and pending is not None:
  send({'jsonrpc':'2.0','id':pending,'result':{'stopReason':'cancelled'}});pending=None
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--batond', type=Path, required=True)
    parser.add_argument('--candidate-manifest', type=Path, required=True)
    parser.add_argument('--frontend', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    binary, frontend, output = args.batond.resolve(strict=True), args.frontend.resolve(strict=True), args.output.resolve()
    assert (frontend / 'index.html').is_file()
    output.mkdir(parents=True, exist_ok=False)
    manifest_bytes = args.candidate_manifest.resolve(strict=True).read_bytes()
    manifest = json.loads(manifest_bytes)
    binary_entry = manifest['binaries']['batond']
    expected_binary = binary_entry['sha256'] if isinstance(binary_entry, dict) else binary_entry
    assert type(manifest.get('schema')) is int and manifest['schema'] == 15 and sha(binary.read_bytes()) == expected_binary, 'Explicit coherent schema15 candidate is required'
    report = {'status': 'failed_or_interrupted',
              'dimension': 'actual built React / Linux Chromium / fixed test-only HTTP / authenticated Unix Core / generated owned ACP process',
              'native_owner_dialog': 'not_run; fixed private host fixture outside HTTP approves actual Core snapshots',
              'provider': 'generated local ACP, no account/credential/network/model',
              'not_claimed': ['macOS', 'native WebView', 'real Provider/model', 'TaskAccepted', 'independent artifact acceptance', 'OS sandbox', 'whole workspace output quota', 'whole product completion'],
              'daemon_sha256': sha(binary.read_bytes()), 'manifest_sha256': sha(manifest_bytes),
              'candidate': manifest, 'harness_sha256': sha(Path(__file__).read_bytes()),
              'frontend_assets_sha256': {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()},
              'checks': [], 'requests': [], 'faults': [], 'observations': {}, 'screenshots': []}
    token = ''
    event_pump = lambda: None

    def safe(value):
        text = str(value)
        return text.replace(token, '[redacted generated fixture authentication]') if token else text

    def passed(name, **details):
        report['checks'].append({'name': name, 'status': 'passed', **details})

    def wait(read, predicate, seconds=15):
        deadline = time.monotonic() + seconds
        while True:
            event_pump()
            value = read()
            if predicate(value):
                return value
            assert time.monotonic() < deadline, safe(f'Observation timed out: {value}')
            time.sleep(.02)

    with tempfile.TemporaryDirectory(prefix='baton-goal-execution-') as temporary:
        root = Path(temporary).resolve()
        root.chmod(0o700)
        runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
        daemon_log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)}, stdout=daemon_log, stderr=subprocess.STDOUT, start_new_session=True)
        server = thread = None
        scenarios = {}
        provider = root / 'generated-owned-goal-acp'
        provider.write_text(f'#!{sys.executable}\n' + FIXTURE); provider.chmod(0o700)
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()), lambda row: row[0] and row[1] and row[2] is None)
            token = (runtime / 'session.token').read_text()

            def ipc(method, params, key=None, revision=None):
                frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': token, 'method': method, 'params': params}
                if key is not None:
                    frame['idempotency_key'] = key
                if revision is not None:
                    frame['expected_revision'] = revision
                raw = json.dumps(frame).encode() + b'\n'
                assert len(raw) <= 262_144
                with socket.socket(socket.AF_UNIX) as stream:
                    stream.settimeout(5); stream.connect(str(runtime / 'batond.sock')); stream.sendall(raw)
                    with stream.makefile('rb') as reader:
                        response = reader.readline(8_388_609)
                assert len(response) <= 8_388_608 and response.endswith(b'\n')
                return json.loads(response)

            def result(method, params, key=None, revision=None):
                response = ipc(method, params, key, revision)
                assert not response.get('error'), {'method': method, 'response': response}
                return response['result']

            def finalize(proposal):
                raw = json.dumps(proposal['snapshot'], ensure_ascii=False, separators=(',', ':')).encode()
                assert len(raw) <= 16_384, 'Complete compact host review; never truncate or raise the private review cap'
                report['observations'].setdefault('private_owner_reviews', []).append({'id': proposal['id'], 'kind': proposal['kind'], 'snapshot_utf8_bytes': len(raw), 'snapshot_sha256': sha(raw), 'review_format': 'complete compact UTF-8; host fixture, not OS dialog'})
                return result('approvals.finalize', {'proposal_id': proposal['id']}, 'finalize:' + proposal['id'], proposal['revision'])

            def owner(method, params, key, kind, revision=None):
                proposal = result(method, params, key, revision)
                assert proposal['kind'] == kind
                return finalize(proposal)

            def database():
                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
                    db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
                    assert db.execute('PRAGMA user_version').fetchone()[0] == 15
                    assert db.execute('SELECT count(*) FROM factory_child_goal_runs').fetchone()[0] == 0
                    tables = ['attempts', 'sessions', 'dispatch_intents', 'goal_runs', 'goal_run_nodes', 'goal_run_reservations', 'goal_plan_revisions', 'goal_node_evidence']
                    value = {name: db.execute('SELECT count(*) FROM ' + name).fetchone()[0] for name in tables}
                    value['sequence'] = db.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]
                    value['budgets'] = {scope: {'consumed': consumed, 'active': active} for scope, consumed, active in db.execute('SELECT scope,consumed,active FROM budgets ORDER BY scope')}
                    value['goal_ledgers'] = {scope: json.loads(raw) for scope, raw in db.execute('SELECT scope,document FROM goal_ledgers ORDER BY scope')}
                    value['execution_approvals'] = db.execute("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')='goal_execution'").fetchone()[0]
                    return value

            connection = owner('connections.propose', {'name': 'Generated owned Goal ACP; no account', 'provider': 'custom_acp', 'executable': str(provider), 'arguments': []}, 'fixture-connection', 'connection')
            wait(lambda: result('connections.list', {}), lambda rows: any(row['id'] == connection['id'] and row['probe_state'] == 'ready' for row in rows))
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}

            def prepare(name, node_goal, data=False, frontier=False):
                source = root / ('source-' + name); source.mkdir(mode=0o700); (source / 'file').write_bytes(b'baseline')
                def git(*argv):
                    return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *argv], env=git_env, text=True).strip()
                if not data:
                    git('init', '-q', '--initial-branch=main'); git('add', 'file'); git('commit', '-qm', 'generated fixed Goal input')
                project = result('projects.import', {'path': str(source), 'name': 'Generated Goal execution ' + name}, 'project:' + name)
                if data:
                    started = result('data_workspace_previews.start', {'project_id': project['id'], 'files': [{'artifact_id': 'source', 'relative_path': 'file', 'max_bytes': 1024, 'expected_sha256': sha(b'baseline')}], 'limits': {'max_file_bytes': 1024, 'max_total_bytes': 1024, 'max_files': 1}}, 'preview:' + name, project['revision'])
                    preview = wait(lambda: result('data_workspace_previews.get', {'preview_id': started['id']}), lambda row: row['state'] not in {'checking', 'cancel_requested'})
                    assert preview['state'] == 'verified'
                    proposal = result('data_workspaces.propose', {'preview_id': preview['id']}, 'copy:' + name, preview['revision'])
                    assert proposal['kind'] == 'data_workspace' and proposal['snapshot']['snapshot'] == preview['snapshot']
                    intent = proposal['snapshot']['intent']; source_pin = preview['snapshot']['contract']['source']
                    assert source_pin['canonical_root'] == str(source) and source_pin['device'] == source.stat().st_dev and source_pin['inode'] == source.stat().st_ino
                    managed = runtime / 'workspaces'; managed_pin = intent['managed_root']
                    assert managed_pin['canonical_root'] == str(managed) and managed_pin['device'] == managed.stat().st_dev and managed_pin['inode'] == managed.stat().st_ino
                    assert str(uuid.UUID(intent['creation_id'])) == intent['creation_id']
                    assert intent['destination'] == str(managed / ('data-' + intent['creation_id']))
                    creation = finalize(proposal)
                    copied = wait(lambda: result('data_workspaces.get', {'creation_id': creation['id']}), lambda row: row['state'] not in {'creating', 'cancel_requested'})
                    assert copied['state'] == 'ready' and copied['io_completion'] == 'worker_joined'
                    workspace = next(row for row in result('workspaces.list', {'project_id': project['id']}) if row['id'] == copied['workspace_id'])
                    assert workspace['identity']['kind']['kind'] == 'directory' and not (Path(workspace['root']) / '.git').exists()
                    input_path = 'inputs/file'
                else:
                    workspace = result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'workspace:' + name, project['revision'])
                    input_path = 'file'
                assert Path(workspace['root']).resolve().is_relative_to(runtime / 'workspaces')
                grant = owner('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'], 'max_attempts': 2, 'max_concurrency': 1, 'max_duration_ms': 600_000}, 'grant:' + name, 'grant')
                task = result('tasks.create', {'project_id': project['id'], 'goal': 'Generated bounded Goal ' + name}, 'task:' + name)
                budget = {'attempts': 2, 'parallel': 1, 'runtime_ms': 20_000, 'output_bytes': 8192, 'children': 1, 'reworks': 1, 'tokens': None, 'cost_microunits': None}
                outputs = [{'artifact': {'artifact_id': 'report', 'relative_path': 'reports/report.txt', 'max_bytes': 1024, 'expected_sha256': None}, 'validators': []}]
                if frontier:
                    outputs.append({'artifact': {'artifact_id': 'summary', 'relative_path': 'reports/summary.txt', 'max_bytes': 1024, 'expected_sha256': None}, 'validators': []})
                scope = owner('goal_scopes.propose', {'task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'], 'expected_grant_revision': grant['revision'], 'max_nodes': 2, 'new_max_children': 1, 'new_max_depth': 1, 'root_budget': budget, 'project_budget': budget, 'inputs': [{'artifact_id': 'fixed-input', 'relative_path': input_path, 'max_bytes': 1024, 'expected_sha256': sha(b'baseline')}], 'outputs': outputs, 'writable_prefixes': ['reports'], 'owner_acceptance_criteria': 'Owner reviews exact report; execution is candidate evidence only'}, 'scope:' + name, 'goal')
                scenario = {'source': source, 'project': project, 'workspace': workspace, 'task': task, 'grant': grant, 'scope': scope, 'node_goal': node_goal, 'data': data, 'frontier': frontier}
                scenarios[name] = scenario
                return scenario

            prepare('normal', 'goal-report')
            prepare('data', 'goal-report', data=True)
            prepare('cancel', 'goal-partial-hang')
            prepare('timeout', 'hang')
            prepare('frontier', 'goal-report', frontier=True)
            assert database()['attempts'] == database()['goal_runs'] == 0
            passed('private host approves generated local inputs and planning scopes; no execution or SQL-forged authority')

            def plan(scenario, revision=1):
                scope = scenario['scope']; contract = scope['outputs']['report']
                node_budget = {'attempts': 1 if revision == 1 else 2, 'parallel': 1, 'runtime_ms': 12_000, 'output_bytes': 2048 if revision == 1 else 4096, 'children': 0, 'reworks': 0 if revision == 1 else 1, 'tokens': None, 'cost_microunits': None}
                reservation = 1200 if scenario['node_goal'] == 'hang' else 8000 if scenario['node_goal'] == 'goal-partial-hang' else 4000
                node = {'id': 'produce-report', 'parent_id': None, 'kind': 'work', 'goal': scenario['node_goal'] if revision == 1 else 'goal-report-v2', 'dependencies': [], 'workspace_id': scope['workspace_id'], 'executor': {'kind': 'connection', 'binding': scope['binding'], 'program_sha256': scope['program_sha256']}, 'recipients': [], 'required_inputs': [{'goal_task_id': scenario['task']['id'], 'artifact_id': 'fixed-input', 'contract_id': scope['inputs']['fixed-input']['contract_id'], 'source': {'kind': 'approved_external', 'sha256': scope['inputs']['fixed-input']['sha256']}}], 'outputs': [{'artifact_id': 'report', 'contract_id': contract}], 'required_validators': scope['contracts'][contract], 'side_effect': 'workspace_write', 'write_set': [{'kind': 'workspace', 'path': 'reports/report.txt'}], 'budget': node_budget, 'reservation': {'runtime_ms': reservation, 'output_bytes': 2048, 'tokens': None, 'cost_microunits': None}, 'retry': {'max_attempts': 1 if revision == 1 else 2, 'max_reworks': 0 if revision == 1 else 1, 'retry_classes': [] if revision == 1 else ['validation_failed']}, 'idempotency_key': 'produce-report-once', 'external_action_id': None}
                nodes = [node]
                if scenario['frontier']:
                    assert revision == 1
                    summary = {**node, 'id': 'produce-summary', 'kind': 'summary', 'goal': 'goal-summary', 'outputs': [{'artifact_id': 'summary', 'contract_id': scope['outputs']['summary']}], 'required_validators': scope['contracts'][scope['outputs']['summary']], 'write_set': [{'kind': 'workspace', 'path': 'reports/summary.txt'}], 'idempotency_key': 'produce-summary-once'}
                    nodes.append(summary)
                return {'schema_version': 1, 'plan_id': 'browser-execution:' + scenario['task']['id'], 'goal': scope['goal'], 'scope_revision': scope['authority_revision'], 'revision': revision, 'expected_revision': revision - 1, 'acceptance_rules': scope['required_acceptance_rules'], 'nodes': nodes}

            def run_view(run_id):
                return result('goal_runs.get', {'run_id': run_id})

            def host_execute(scenario):
                scope = scenario['scope']; latest = result('goal_plans.get', {'scope_id': scope['id']})
                seq = result('system.snapshot', {})['sequence']
                proposal = result('goal_runs.propose', {'scope_id': scope['id'], 'expected_plan_revision': latest['proposal']['revision'], 'expected_ledger_revision': seq}, 'execution:' + scenario['task']['id'])
                assert proposal['kind'] == 'goal_execution' and proposal['snapshot']['new_workspace_authority'] is False
                assert proposal['snapshot']['plan'] == latest and proposal['snapshot']['scope'] == scope
                assert proposal['snapshot']['scope']['workspace_id'] == scenario['workspace']['id']
                assert proposal['snapshot']['scope']['program_sha256'] == sha(provider.read_bytes())
                assert proposal['snapshot']['execution'] == 'bounded_existing_workspace_only' and proposal['snapshot']['allow_in_scope_plan_revisions'] is True
                assert proposal['snapshot']['enforcement'] == {'filesystem_sandbox': False, 'network_sandbox': False, 'output_quota': False}
                current_grant = next(row for row in result('grants.list', {'project_id': scenario['project']['id']}) if row['id'] == scope['root_grant_id'])
                assert proposal['snapshot']['root_grant'] == current_grant
                assert proposal['snapshot']['existing_connection'] == {**current_grant['connection_bindings'][connection['id']], 'id': connection['id']}
                started = finalize(proposal)
                assert started['scope_id'] == scope['id'] and started['task_id'] == scenario['task']['id'] and started['task_accepted'] is False
                scenario['run_id'] = started['id']; scenario['execution_proposal'] = proposal
                return started

            def assert_reaped(scenario, view, node=None):
                selected_node = node is not None
                node = view['nodes'][-1] if node is None else node
                assert node['stop_confirmed'] is True
                suffix = '-' + node['attempt_id'] if selected_node else ''
                marker = Path(scenario['workspace']['root']) / ('reports/fixture-owned' + suffix + '.json')
                owned = json.loads(marker.read_text())
                try:
                    os.kill(owned['pid'], 0)
                except ProcessLookupError:
                    pass
                else:
                    raise AssertionError('Generated owned process remains alive despite stop confirmation')
                session = result('sessions.get', {'session_id': node['session_id']})
                assert session['state'] == 'stopped'
                assert view['task_accepted'] is False and view['run']['task_accepted'] is False
                assert result('tasks.get', {'task_id': scenario['task']['id']})['state'] != 'accepted'
                raw = (Path(scenario['workspace']['root']) / ('reports/task-request' + suffix + '.json')).read_bytes()
                request = json.loads(raw)
                assert request['run_id'] == view['run']['id'] and request['plan_revision'] == node['plan_revision'] and request['lineage']['attempt_id'] == node['attempt_id']
                assert request['node'] == node['node'] and request['workspace']['canonical_root'] == scenario['workspace']['root']
                assert request['inputs'][0]['artifact']['relative_path'] == ('inputs/file' if scenario['data'] else 'file')
                assert request['outputs'][0]['artifact']['relative_path'] == 'reports/report.txt'
                report['observations'].setdefault('complete_task_requests', []).append({'scenario': scenario['task']['id'], 'utf8_bytes': len(raw), 'sha256': sha(raw), 'request': request, 'owned_stop_observed': True})
                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:
                    db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')
                    rows = db.execute("SELECT sequence,document FROM events WHERE json_extract(document,'$.aggregate_id') IN(?,?,?) ORDER BY sequence LIMIT 513", [view['run']['id'], scenario['task']['id'], node['session_id']]).fetchall()
                    assert len(rows) <= 512 and all(len(document.encode()) <= 1_048_576 for _, document in rows)
                    events = [(sequence, json.loads(document)) for sequence, document in rows]
                    reserved = [(sequence, event) for sequence, event in events if event['kind'] == 'goal.node_reserved' and event['source'] == 'goal_core' and event['payload'].get('attempt_id') == node['attempt_id'] and event['payload'].get('session_id') == node['session_id']]
                    stopped = [(sequence, event) for sequence, event in events if event['kind'] == 'attempt.stopped' and event['source'] == 'user_action' and event['payload'].get('intent_id') == session['intent_id']]
                    supervisor = [(sequence, event) for sequence, event in events if event['kind'] == 'session.stopped' and event['source'] == 'baton_supervisor' and event['aggregate_id'] == node['session_id']]
                    settled = [(sequence, event) for sequence, event in events if event['kind'] == 'goal.node_settled' and event['source'] == 'goal_core' and event['payload'].get('attempt_id') == node['attempt_id'] and event['payload'].get('session_id') == node['session_id']]
                    assert len(reserved) == len(stopped) == len(supervisor) == len(settled) == 1
                    assert 0 < reserved[0][0] < stopped[0][0] < supervisor[0][0] < settled[0][0]
                    assert stopped[0][1]['payload']['stop_evidence'] == {'kind': 'executor_stop', 'executor_instance': node['session_id'], 'receipt_ref': 'owned-worker-stop:' + node['session_id']}
                    assert supervisor[0][1]['payload']['process_reaped'] is True
                    observed = settled[0][1]['payload']
                    assert observed['stop_confirmed'] is True and observed['state'] == node['state']
                    assert observed['actual_runtime_ms'] == node['actual_runtime_ms'] and observed['actual_output_bytes'] == node['actual_output_bytes']
                    assert observed['tokens'] is None and observed['cost_microunits'] is None and observed['task_accepted'] is False
                report['observations'].setdefault('owned_stop_receipt_chains', []).append({'run_id': view['run']['id'], 'attempt_id': node['attempt_id'], 'plan_revision': node['plan_revision'], 'events': [{'sql_sequence': sequence, 'document': event} for sequence, event in [reserved[0], stopped[0], supervisor[0], settled[0]]]})

            allowed_task_ids = {s['task']['id'] for s in scenarios.values()}
            allowed_project_ids = {s['project']['id'] for s in scenarios.values()}
            allowed_scope_ids = {s['scope']['id'] for s in scenarios.values()}
            allowed_scopes_by_task = {}
            for scenario in scenarios.values():
                allowed_scopes_by_task.setdefault(scenario['task']['id'], set()).add(scenario['scope']['id'])
            known_runs = {}
            fault = {'method': None, 'run_id': None}
            fault_lock = threading.Lock()
            fixed_read = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list', 'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}
            scoped_read = {'workspaces_list': ('workspaces.list', 'projectId', 'project_id', allowed_project_ids), 'grants_list': ('grants.list', 'projectId', 'project_id', allowed_project_ids), 'goal_scopes_list': ('goal_scopes.list', 'taskId', 'task_id', allowed_task_ids), 'goal_scopes_get': ('goal_scopes.get', 'scopeId', 'scope_id', allowed_scope_ids), 'goal_plans_get': ('goal_plans.get', 'scopeId', 'scope_id', allowed_scope_ids), 'goal_plans_list': ('goal_plans.list', 'scopeId', 'scope_id', allowed_scope_ids), 'goal_runs_list': ('goal_runs.list', 'taskId', 'task_id', allowed_task_ids)}
            # The production legacy-mode interlock also reads the distinct
            # Graph namespace. This grants only a fixture-task-bound list.
            scoped_read['goal_graph_runs_list'] = ('goal_graph_runs.list', 'taskId', 'task_id', allowed_task_ids)

            class Handler(http.server.SimpleHTTPRequestHandler):
                def log_message(self, *_):
                    pass

                def do_POST(self):
                    if self.path != '/__test_goal_execution_ipc':
                        self.send_error(404); return
                    if self.headers.get('Origin') != url or self.headers.get('Host') != url.removeprefix('http://'):
                        self.send_error(403); return
                    if self.headers.get_content_type() != 'application/json':
                        self.send_error(400); return
                    try:
                        size = int(self.headers.get('Content-Length', '0')); assert 0 < size <= 270_336
                        envelope = json.loads(self.rfile.read(size), object_pairs_hook=unique_object)
                        assert isinstance(envelope, dict) and set(envelope) <= {'command', 'args'} and 'command' in envelope
                        command, values = envelope['command'], envelope.get('args')
                        if values is None:
                            values = {}
                        assert isinstance(command, str) and isinstance(values, dict)
                        key = revision = None
                        if command in fixed_read:
                            assert not values; method, params = fixed_read[command], {}
                        elif command in scoped_read:
                            method, camel, snake, bounds = scoped_read[command]
                            assert set(values) == {camel} and values[camel] in bounds; params = {snake: values[camel]}
                        elif command == 'events_subscribe':
                            assert set(values) <= {'cursor', 'limit'} and integer(values.get('cursor', 0)) and integer(values.get('limit', 100), 1) and values.get('limit', 100) <= 100
                            method, params = 'events.subscribe', {'cursor': values.get('cursor', 0), 'limit': values.get('limit', 100)}
                        elif command in {'goal_plans_propose', 'goal_plans_revise', 'goal_plans_ready_preview'}:
                            store = command != 'goal_plans_ready_preview'
                            required = {'scopeId', 'expectedRevision'} | ({'planJson', 'idempotencyKey'} if store else set())
                            assert set(values) == required and values['scopeId'] in allowed_scope_ids
                            revision = values['expectedRevision']; assert integer(revision, 0 if command == 'goal_plans_propose' else 1)
                            method = {'goal_plans_propose': 'goal_plans.propose', 'goal_plans_revise': 'goal_plans.revise', 'goal_plans_ready_preview': 'goal_plans.ready_preview'}[command]
                            params = {'scope_id': values['scopeId']}
                            if store:
                                assert isinstance(values['planJson'], str) and 0 < len(values['planJson'].encode()) <= 262_144
                                params['plan_json'] = values['planJson']; key = values['idempotencyKey']
                        elif command in {'goal_runs_get', 'goal_runs_cancel', 'goal_run_advance_plan'}:
                            required = {'runId'} | ({'expectedRevision', 'idempotencyKey'} if command != 'goal_runs_get' else set()) | ({'expectedPlanRevision', 'expectedLedgerRevision'} if command == 'goal_run_advance_plan' else set())
                            assert set(values) == required and values['runId'] in known_runs
                            method = {'goal_runs_get': 'goal_runs.get', 'goal_runs_cancel': 'goal_runs.cancel', 'goal_run_advance_plan': 'goal_runs.advance_plan'}[command]
                            params = {'run_id': values['runId']}
                            if command != 'goal_runs_get':
                                revision, key = values['expectedRevision'], values['idempotencyKey']; assert integer(revision, 1)
                            if command == 'goal_run_advance_plan':
                                assert integer(values['expectedPlanRevision'], 1) and integer(values['expectedLedgerRevision'], 1)
                                params.update(expected_plan_revision=values['expectedPlanRevision'], expected_ledger_revision=values['expectedLedgerRevision'])
                        else:
                            self.send_error(403); return
                        if method in {'goal_plans.propose', 'goal_plans.revise', 'goal_runs.cancel', 'goal_runs.advance_plan'}:
                            assert isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key)
                        else:
                            assert key is None
                    except (AssertionError, ValueError, TypeError, KeyError):
                        self.send_error(400); return
                    try:
                        response = ipc(method, params, key, revision)
                        if method == 'goal_runs.list' and isinstance(response.get('result'), list):
                            for row in response['result']:
                                assert row['task_id'] == params['task_id'] and row['scope_id'] in allowed_scope_ids
                                known_runs[row['id']] = row['task_id']
                        if method == 'goal_graph_runs.list' and isinstance(response.get('result'), list):
                            for row in response['result']:
                                assert type(row['graph_contract_version']) is int and row['graph_contract_version'] == 1
                                assert row['run']['task_id'] == params['task_id'] and row['run']['scope_id'] in allowed_scopes_by_task[params['task_id']]
                        entry = {'method': method, 'params': params, 'key': key, 'revision': revision, 'response': response}
                        report['requests'].append(entry)
                        with fault_lock:
                            drop = method == fault['method'] and params.get('run_id') == fault['run_id'] and not response.get('error')
                            if drop:
                                fault['method'] = fault['run_id'] = None
                        if drop:
                            report['faults'].append({'injection': 'after actual authenticated Unix result; close HTTP before headers', 'request': entry, 'database_after_forwarding': database()})
                            self.close_connection = True
                            try:
                                self.connection.shutdown(socket.SHUT_RDWR)
                            except OSError:
                                pass
                            return
                        raw = json.dumps(response).encode()
                        self.send_response(200); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(raw))); self.send_header('Cache-Control', 'no-store'); self.end_headers(); self.wfile.write(raw)
                    except (BrokenPipeError, ConnectionResetError):
                        report['observations']['client_aborts'] = report['observations'].get('client_aborts', 0) + 1
                    except (AssertionError, OSError, ValueError) as failure:
                        report['observations']['adapter_error'] = safe(failure); self.send_error(502)

            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = f'http://127.0.0.1:{server.server_port}'
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--disable-dev-shm-usage'])
                context = browser.new_context(viewport={'width': 1488, 'height': 981}, locale='en-US')
                context.add_init_script("""localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_goal_execution_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed Goal execution adapter refused request');return r.json();}};""")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); errors = []; page.on('pageerror', lambda error: errors.append(str(error)))
                event_pump = lambda: page.wait_for_timeout(1)

                def shot(name, focus=None):
                    if focus is not None:
                        focus.evaluate("element=>element.scrollIntoView({block:'start'})")
                    path = output / (name + '.png'); page.screenshot(path=str(path))
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'viewport': page.viewport_size, 'dimension': 'actual Linux Chromium, not macOS/native'})

                def expand(details):
                    if not details.evaluate('(element)=>element.open'):
                        details.locator(':scope > summary').click()

                def open_scenario(name):
                    scenario = scenarios[name]
                    if page.get_by_test_id('task-thread').count():
                        page.get_by_role('button', name='Back to tasks', exact=True).click()
                    page.get_by_test_id('task-card').filter(has_text=scenario['task']['goal']).click()
                    page.locator('#tab-details').click()
                    panel = page.get_by_test_id('goal-planning'); expand(panel.locator(':scope > details'))
                    expect(panel.get_by_role('combobox', name='Selected Goal scope', exact=True)).to_have_value(scenario['scope']['id'])
                    execution = panel.get_by_test_id('goal-execution'); expand(execution.locator(':scope > details'))
                    expect(execution.locator('[aria-busy]')).to_have_attribute('aria-busy', 'false')
                    return panel, execution

                def refresh(execution):
                    execution.get_by_role('button', name='Refresh execution records', exact=True).click()
                    expect(execution.locator('[aria-busy]')).to_have_attribute('aria-busy', 'false')

                def store_plan(panel, scenario, revision=1):
                    detail = panel.locator('details').filter(has=page.get_by_text('Versioned plan proposal', exact=True)).last; expand(detail)
                    raw = '\n ' + json.dumps(plan(scenario, revision), indent=2) + '\n'
                    panel.get_by_role('textbox', name='Plan proposal JSON', exact=True).fill(raw)
                    panel.get_by_role('button', name='Store initial plan proposal' if revision == 1 else 'Store revised plan proposal', exact=True).click()
                    rows = wait(lambda: result('goal_plans.list', {'scope_id': scenario['scope']['id']}), lambda rows: any(row['proposal']['revision'] == revision for row in rows))
                    stored = next(row for row in rows if row['proposal']['revision'] == revision)
                    assert stored['state'] == 'validated_proposal' and stored['dispatched'] is False
                    assert stored['proposal'] == plan(scenario, revision)
                    return stored

                def browser_request(command, values):
                    return page.evaluate("""async({command,args})=>{const r=await fetch('/__test_goal_execution_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});return{status:r.status,body:r.ok?await r.json():null};}""", {'command': command, 'args': values})

                try:
                    page.goto(url)
                    panel, execution = open_scenario('normal')
                    before = database()
                    expect(execution.get_by_text('No execution runs for this Goal scope.', exact=True)).to_be_visible()
                    expect(execution.get_by_role('button', name='Review Goal execution in native dialog', exact=True)).to_be_disabled()
                    assert database() == before
                    passed('actual Goal execution is lazy/read-only with no stored plan and no authority from planning approval')
                    denied = ['goal_run_approve', 'goal_runs_propose', 'goal_runs_dispatch', 'approvals_finalize', 'generic_ipc', 'sessions_create', 'sessions_send', 'connections_propose', 'projects_import']
                    assert all(browser_request(command, {})['status'] == 403 for command in denied)
                    assert browser_request('goal_runs_get', {'runId': 'foreign-run'})['status'] == 400
                    assert browser_request('goal_graph_runs_list', {'taskId': 'foreign-task'})['status'] == 400
                    assert browser_request('goal_run_advance_plan', {'runId': 'foreign-run', 'expectedPlanRevision': 2, 'expectedLedgerRevision': 1, 'expectedRevision': 1, 'idempotencyKey': 'foreign'})['status'] == 400
                    assert database() == before
                    passed('HTTP denies approval/finalize/generic dispatch or execution and foreign run references without database writes')

                    scenario = scenarios['normal']; stored = store_plan(panel, scenario); before_execution = database()
                    assert before_execution['attempts'] == before_execution['goal_runs'] == 0
                    refresh(execution)
                    expect(execution.get_by_role('button', name='Review Goal execution in native dialog', exact=True)).to_be_enabled()
                    shot('01-stored-plan-no-execution')
                    started = host_execute(scenario)
                    first = wait(lambda: run_view(started['id']), lambda view: view['run']['state'] in {'awaiting_acceptance', 'blocked', 'failed', 'outcome_unknown'})
                    assert first['run']['state'] == 'awaiting_acceptance' and first['nodes'][0]['state'] == 'candidate'
                    assert first['nodes'][0]['actual_output_bytes'] == 12 and first['nodes'][0]['actual_runtime_ms'] > 0
                    assert first['nodes'][0]['tokens'] is None and first['nodes'][0]['cost_microunits'] is None
                    assert_reaped(scenario, first)
                    assert (Path(scenario['workspace']['root']) / 'reports/report.txt').read_bytes() == b'fixed report'
                    refresh(execution); expect(execution.get_by_text('Execution candidate; acceptance pending', exact=False).first).to_be_visible()
                    node_detail = execution.locator('section[aria-label="Node and attempt history"] details').first; expand(node_detail)
                    expect(node_detail.get_by_text('Not measured / unknown', exact=True).first).to_be_visible(); shot('02-real-generated-candidate', node_detail.locator(':scope > summary'))
                    first_db = database(); project_budget = 'project:' + scenario['project']['id']
                    assert first_db['budgets'][project_budget] == {'consumed': 1, 'active': 0}
                    task_budget = 'root:' + scenario['task']['id']; root_ledger = 'root-grant:' + scenario['grant']['id']
                    assert first_db['budgets'][task_budget] == {'consumed': 1, 'active': 0}
                    for scope_key in [project_budget, root_ledger]:
                        assert first_db['goal_ledgers'][scope_key]['observed_attempts'] == 1
                        assert first_db['goal_ledgers'][scope_key]['output_bytes'] == 12
                        assert first_db['goal_ledgers'][scope_key]['runtime_ms'] > 0
                        assert first_db['goal_ledgers'][scope_key]['tokens'] is None and first_db['goal_ledgers'][scope_key]['cost_microunits'] is None
                    assert first_db['execution_approvals'] == 1
                    report['observations']['normal_first'] = {'view': first, 'database': first_db}
                    passed('HTTP-external real Owner fixture starts full TaskRequest, real artifact and owned-reaped candidate without acceptance')

                    second_plan = store_plan(panel, scenario, 2); unchanged = run_view(started['id'])
                    assert unchanged['run']['plan_revision'] == 1 and database()['attempts'] == first_db['attempts']
                    before_cas = database(); sequence = result('system.snapshot', {})['sequence']
                    assert unchanged['run']['revision'] > 1 and sequence > 1
                    stale_responses = []
                    for expected_run, expected_ledger, key in [(1, sequence, 'stale-run-cas'), (unchanged['run']['revision'], sequence - 1, 'stale-ledger-cas')]:
                        refused = browser_request('goal_run_advance_plan', {'runId': started['id'], 'expectedPlanRevision': 2, 'expectedLedgerRevision': expected_ledger, 'expectedRevision': expected_run, 'idempotencyKey': key})
                        assert refused['status'] == 200 and refused['body']['error']['code'] == 'STALE_REVISION', refused
                        assert database() == before_cas
                        stale_responses.append(refused)
                    report['observations']['actual_stale_CAS_refusals'] = stale_responses
                    refresh(execution)
                    advance = execution.get_by_role('button', name='Adopt the validated new plan', exact=True); expect(advance).to_be_enabled()
                    with fault_lock:
                        fault.update(method='goal_runs.advance_plan', run_id=started['id'])
                    advance.click(); expect(execution.get_by_role('alert')).to_contain_text('Request outcome unknown')
                    shot('03-real-plan-advance-response-lost')
                    second = wait(lambda: run_view(started['id']), lambda view: view['run']['plan_revision'] == 2 and view['run']['state'] in {'awaiting_acceptance', 'blocked', 'failed', 'outcome_unknown'})
                    assert second['run']['state'] == 'awaiting_acceptance' and second['run']['plan_digest'] == second_plan['digest']
                    assert len(second['nodes']) == 2 and second['nodes'][0] == first['nodes'][0]
                    assert second['nodes'][1]['attempt_id'] != first['nodes'][0]['attempt_id'] and second['nodes'][1]['actual_output_bytes'] == 15
                    assert_reaped(scenario, second)
                    assert (Path(scenario['workspace']['root']) / 'reports/report.txt').read_bytes() == b'fixed report v2'
                    before_recovery = database()
                    assert before_recovery['execution_approvals'] == 1 and before_recovery['budgets'][project_budget] == {'consumed': 2, 'active': 0}
                    assert before_recovery['budgets'][task_budget] == {'consumed': 2, 'active': 0}
                    for scope_key in [project_budget, root_ledger]:
                        previous, current = first_db['goal_ledgers'][scope_key], before_recovery['goal_ledgers'][scope_key]
                        assert current['observed_attempts'] == 2 and current['output_bytes'] == 27
                        assert current['runtime_ms'] > previous['runtime_ms'] and current['limits'] == previous['limits']
                        assert current['tokens'] is None and current['cost_microunits'] is None
                    panel, execution = open_scenario('normal'); refresh(execution)
                    recover = execution.get_by_role('button', name='Recover the same execution request', exact=True); expect(recover).to_be_enabled(); recover.click()
                    expect(execution.get_by_role('button', name='Recover the same execution request', exact=True)).to_have_count(0)
                    assert database() == before_recovery
                    writes = [row for row in report['requests'] if row['method'] == 'goal_runs.advance_plan' and not row['response'].get('error')]
                    assert len(writes) == 2 and writes[0]['params'] == writes[1]['params'] and writes[0]['key'] == writes[1]['key'] and writes[0]['revision'] == writes[1]['revision']
                    # Transport request IDs and the response's current global
                    # journal revision are fresh observations. The immutable
                    # idempotent *result* must match the original receipt.
                    assert writes[0]['response']['result'] == writes[1]['response']['result']
                    before_read = database(); refresh(execution); assert database() == before_read
                    shot('04-same-scope-replan-history-kept', execution.get_by_role('heading', name='Node and attempt history', exact=True))
                    report['observations']['normal_second'] = {'view': second, 'database': before_recovery, 'exact_advance_replay': writes}
                    passed('UI stores rev2 then explicitly adopts same-scope plan; dropped real response recovers exact key without new Owner, attempt replay or budget/history reset')

                    scenario = scenarios['data']; panel, execution = open_scenario('data'); store_plan(panel, scenario); started = host_execute(scenario)
                    data_view = wait(lambda: run_view(started['id']), lambda view: view['run']['state'] in {'awaiting_acceptance', 'blocked', 'failed', 'outcome_unknown'})
                    assert data_view['run']['state'] == 'awaiting_acceptance' and data_view['nodes'][0]['actual_output_bytes'] == 12
                    assert_reaped(scenario, data_view); refresh(execution); shot('05-real-data-workspace-goal-candidate', execution.locator('summary').filter(has_text=started['id']))
                    assert (Path(scenario['workspace']['root']) / 'reports/report.txt').read_bytes() == b'fixed report'
                    assert not (Path(scenario['workspace']['root']) / '.git').exists() and (scenario['source'] / 'file').read_bytes() == b'baseline'
                    report['observations']['data_candidate'] = data_view
                    passed('native-confirmed actual selected-file Directory copy supplies real Goal TaskRequest inputs without Git initialization or source writes')

                    scenario = scenarios['cancel']; panel, execution = open_scenario('cancel'); store_plan(panel, scenario); started = host_execute(scenario)
                    marker = Path(scenario['workspace']['root']) / 'reports/fixture-hang'
                    live = wait(lambda: run_view(started['id']), lambda view: view['nodes'][0]['state'] == 'running' and marker.is_file())
                    refresh(execution); cancel = execution.get_by_role('button', name='Request Goal run cancellation', exact=True); expect(cancel).to_be_enabled()
                    with fault_lock:
                        fault.update(method='goal_runs.cancel', run_id=started['id'])
                    cancel.click(); expect(execution.get_by_role('alert')).to_contain_text('Request outcome unknown')
                    cancel_fault = next(row for row in report['faults'] if row['request']['method'] == 'goal_runs.cancel')
                    assert cancel_fault['request']['response']['result']['state'] == 'cancel_requested'
                    stopped = wait(lambda: run_view(started['id']), lambda view: view['run']['state'] in {'cancelled', 'outcome_unknown'})
                    assert stopped['run']['state'] == 'cancelled' and stopped['nodes'][0]['state'] == 'cancelled'
                    assert stopped['nodes'][0]['actual_output_bytes'] == 14
                    assert_reaped(scenario, stopped)
                    assert (Path(scenario['workspace']['root']) / 'reports/report.txt').read_bytes() == b'partial report'
                    before_read = database(); panel, execution = open_scenario('cancel'); refresh(execution)
                    expect(execution.get_by_role('button', name='Recover the same execution request', exact=True)).to_have_count(0)
                    assert database() == before_read
                    request = cancel_fault['request']; replay = ipc(request['method'], request['params'], request['key'], request['revision'])
                    assert not replay.get('error') and replay['result'] == request['response']['result'] and database() == before_read
                    assert len([row for row in report['requests'] if row['method'] == 'goal_runs.cancel' and row['params']['run_id'] == started['id']]) == 1
                    cancel_node = execution.locator('section[aria-label="Node and attempt history"] details').first; expand(cancel_node)
                    shot('06-real-owned-cancel-retains-partial-report', cancel_node.locator(':scope > summary'))
                    report['observations']['cancel'] = {'request_receipt': request['response'], 'terminal': stopped, 'partial_bytes': 14, 'no_automatic_replay': True}
                    passed('real partial-write Goal cancellation keeps request receipt separate from actual owned stop; dropped response reconciles by current reads without a second mutation')

                    scenario = scenarios['timeout']; panel, execution = open_scenario('timeout'); store_plan(panel, scenario); started = host_execute(scenario)
                    unknown = wait(lambda: run_view(started['id']), lambda view: view['run']['state'] in {'outcome_unknown', 'failed', 'cancelled', 'awaiting_acceptance'})
                    assert unknown['run']['state'] == 'outcome_unknown' and unknown['nodes'][0]['state'] == 'outcome_unknown'
                    assert unknown['nodes'][0]['actual_runtime_ms'] >= 1200 and unknown['nodes'][0]['actual_output_bytes'] is None
                    assert_reaped(scenario, unknown); refresh(execution)
                    expect(execution.get_by_text('Node effects are unknown or their states are unsupported. Owned stop does not resolve effects; new approval and plan adoption stay blocked.', exact=True)).to_be_visible()
                    expect(execution.get_by_role('button', name='Review Goal execution in native dialog', exact=True)).to_be_disabled()
                    execution.get_by_role('button', name='Request Goal run cancellation', exact=True).click()
                    still_unknown = wait(lambda: run_view(started['id']), lambda view: view['run']['cancel_requested'])
                    assert still_unknown['run']['state'] == 'outcome_unknown' and still_unknown['nodes'][0]['state'] == 'outcome_unknown'
                    assert still_unknown['nodes'][0]['stop_confirmed'] is True
                    refresh(execution)
                    timeout_node = execution.locator('section[aria-label="Node and attempt history"] details').first; expand(timeout_node)
                    shot('07-timeout-owned-stop-effects-still-unknown', timeout_node.locator(':scope > summary'))
                    page.set_viewport_size({'width': 390, 'height': 844}); shot('08-timeout-mobile-unknown', timeout_node.locator(':scope > summary')); assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                    report['observations']['timeout'] = {'before_cancel': unknown, 'after_cancel': still_unknown}
                    passed('actual1200ms owned timeout remains unknown after reaping and cancellation; UI blocks new approval/adoption and never fabricates TaskAccepted')

                    # Same approved initial plan: neither explicit replan nor a
                    # second Owner action may be needed to drain ready nodes.
                    page.set_viewport_size({'width': 1488, 'height': 981})
                    scenario = scenarios['frontier']; panel, execution = open_scenario('frontier')
                    initial_plan = store_plan(panel, scenario)
                    assert initial_plan['proposal']['revision'] == 1 and len(initial_plan['proposal']['nodes']) == 2
                    assert [node['kind'] for node in initial_plan['proposal']['nodes']] == ['work', 'summary']
                    assert all(node['dependencies'] == [] for node in initial_plan['proposal']['nodes'])
                    before_frontier = database(); started = host_execute(scenario)
                    frontier_view = wait(lambda: run_view(started['id']), lambda view: len(view['nodes']) == 2 and all(node['state'] == 'candidate' for node in view['nodes']))
                    assert frontier_view['run']['state'] == 'awaiting_acceptance' and frontier_view['run']['plan_revision'] == 1
                    assert [node['plan_revision'] for node in frontier_view['nodes']] == [1, 1]
                    assert len({node['attempt_id'] for node in frontier_view['nodes']}) == len({node['session_id'] for node in frontier_view['nodes']}) == 2
                    assert [node['actual_output_bytes'] for node in frontier_view['nodes']] == [12, 13]
                    for node in frontier_view['nodes']:
                        assert node['actual_runtime_ms'] > 0 and node['tokens'] is None and node['cost_microunits'] is None
                        assert_reaped(scenario, frontier_view, node)
                    workspace = Path(scenario['workspace']['root'])
                    assert (workspace / 'reports/report.txt').read_bytes() == b'fixed report'
                    assert (workspace / 'reports/summary.txt').read_bytes() == b'fixed summary'
                    after_frontier = database()
                    assert after_frontier['attempts'] == before_frontier['attempts'] + 2
                    assert after_frontier['sessions'] == before_frontier['sessions'] + 2
                    assert after_frontier['goal_run_reservations'] == before_frontier['goal_run_reservations'] + 2
                    assert after_frontier['execution_approvals'] == before_frontier['execution_approvals'] + 1
                    for key in ['project:' + scenario['project']['id'], 'root:' + scenario['task']['id']]:
                        assert after_frontier['budgets'][key] == {'consumed': 2, 'active': 0}
                    for key in ['project:' + scenario['project']['id'], 'root-grant:' + scenario['grant']['id']]:
                        ledger = after_frontier['goal_ledgers'][key]
                        assert ledger['observed_attempts'] == 2 and ledger['output_bytes'] == 25
                        assert ledger['runtime_ms'] == sum(node['actual_runtime_ms'] for node in frontier_view['nodes'])
                        assert ledger['limits'] == before_frontier['goal_ledgers'][key]['limits']
                    request_rows = [row for row in report['observations']['complete_task_requests'] if row['request']['run_id'] == started['id']]
                    assert len(request_rows) == 2 and [row['request']['node'] for row in request_rows] == [node['node'] for node in frontier_view['nodes']]
                    chains = [row for row in report['observations']['owned_stop_receipt_chains'] if row['run_id'] == started['id']]
                    assert len(chains) == 2 and chains[0]['events'][-1]['sql_sequence'] < chains[1]['events'][0]['sql_sequence'], 'parallel1 must stop/settle the first owned attempt before reserving the second'
                    refresh(execution)
                    history = execution.get_by_role('region', name='Node and attempt history', exact=True)
                    expect(history.locator('details')).to_have_count(2)
                    expect(history.locator('summary').nth(0)).to_contain_text('produce-report · candidate · Plan revision 1')
                    expect(history.locator('summary').nth(1)).to_contain_text('produce-summary · candidate · Plan revision 1')
                    shot('09-same-plan-work-and-summary-frontier', execution.get_by_role('heading', name='Node and attempt history', exact=True))
                    report['observations']['same_plan_frontier'] = {'initial_plan_digest': initial_plan['digest'], 'view': frontier_view, 'before': before_frontier, 'after': after_frontier, 'outputs': {'reports/report.txt': {'bytes': 12, 'sha256': sha(b'fixed report')}, 'reports/summary.txt': {'bytes': 13, 'sha256': sha(b'fixed summary')}}, 'sequential_owned_chains': chains, 'additional_advance_requests': len([row for row in report['requests'] if row['method'] == 'goal_runs.advance_plan' and row['params']['run_id'] == started['id']])}
                    assert report['observations']['same_plan_frontier']['additional_advance_requests'] == 0
                    passed('one initial approved plan automatically drains independent Work and Summary nodes with distinct outputs, owned-stop lineage and cumulative two-attempt budgets; UI shows both same-plan candidates')
                    for scenario in scenarios.values():
                        assert (scenario['source'] / 'file').read_bytes() == b'baseline'
                    assert not errors, errors
                    report['observations']['final_database'] = database(); report['observations']['page_errors'] = errors
                    passed('original generated sources unchanged, real scoped history retained and no browser runtime errors')
                    report['status'] = 'passed'
                except BaseException:
                    shot('failure-current-ui')
                    raise
                finally:
                    browser.close()
        except BaseException as failure:
            report['failure'] = {'type': type(failure).__name__, 'message': safe(failure), 'traceback': safe(traceback.format_exc())}
            raise AssertionError(safe(failure)) from None
        finally:
            if server is not None:
                server.shutdown(); server.server_close()
            if thread is not None:
                thread.join(timeout=5)
            daemon.terminate()
            try:
                daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(daemon.pid, signal.SIGKILL); daemon.wait()
            # Core owns child groups. Normal tests require its own stop/reap.
            # On a failed fixture, additionally signal ONLY a still-live exact
            # generated process identity/group, never global pkill/user PIDs.
            cleanup = []
            inspected = set()
            for scenario in scenarios.values():
                marker = Path(scenario['workspace']['root']) / 'reports/fixture-owned.json'
                for marker in [marker, *marker.parent.glob('fixture-owned-*.json')]:
                    if not marker.is_file():
                        continue
                    try:
                        owned = json.loads(marker.read_text()); pid = owned['pid']
                        identity = (pid, owned['start_ticks'])
                        if identity in inspected:
                            continue
                        inspected.add(identity)
                        if linux_start_ticks(pid) != owned['start_ticks']:
                            continue
                        assert owned['pgid'] == pid and os.getpgid(pid) == pid
                        argv = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
                        assert str(provider).encode() in argv
                        os.killpg(pid, signal.SIGKILL)
                        cleanup.append({'scenario': scenario['task']['id'], 'action': 'killed only still-live exact generated private process group after daemon teardown', 'stop_proof': False})
                        report['status'] = 'failed_or_interrupted'
                    except ProcessLookupError:
                        pass
                    except (AssertionError, OSError, ValueError, KeyError) as failure:
                        cleanup.append({'scenario': scenario['task']['id'], 'action': 'refused unsafe or unavailable cleanup identity', 'error': safe(failure), 'stop_proof': False})
                        report['status'] = 'failed_or_interrupted'
            report['observations']['failure_cleanup'] = cleanup
            daemon_log.close()
            (output / 'report.json').write_text(safe(json.dumps(report, ensure_ascii=False, indent=2)) + '\n')
            print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'report': str(output / 'report.json')}), flush=True)
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
