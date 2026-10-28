#!/usr/bin/env python3
"""Built Factory UI -> fixed test HTTP -> actual authenticated Core13 Unix IPC.

Owner review/finalization and resource creation exist only in the private host
fixture. Generated ACP/MCP is an account-free test program, never a real model.
Default invocation records pending dependencies and exits2 without a process.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from functools import partial
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import uuid

SUPPORT_PATH = Path(__file__).with_name('goal-graph-real-ipc.py')
_spec = importlib.util.spec_from_file_location('_factory_existing_ipc_support', SUPPORT_PATH)
support = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = support
_spec.loader.exec_module(support)
FixedCall = support.FixedCall
BoundaryRefusal = support.BoundaryRefusal
require = support.require
safe_integer = support.safe_integer
unique_object = support.unique_object
MAX_HTTP_BYTES, MAX_IPC_BYTES, MAX_RESPONSE_BYTES = 270_336, 262_144, 4_194_304
MAX_FACTORY_REVIEW_BYTES, MAX_LEGACY_REVIEW_BYTES = 1_048_576, 16_384
MUTATIONS = frozenset({'factory_runs.start', 'factory_runs.cancel'})
READS = frozenset({'system.snapshot', 'projects.list', 'connections.list', 'sessions.list',
    'workspaces.list', 'grants.list', 'events.subscribe', 'goal_scopes.list',
    'factory_scopes.get', 'factory_scopes.list', 'factory_runs.get'})
PENDING = ('Root immutable matching schema13 batond and sibling baton with manifest hashes',
    'Root actual composed built Factory frontend and asset hashes',
    'Core-author confirmation of actual Factory runtime/ledger and private MCP contract')


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


@dataclass(frozen=True)
class Bounds:
    # Only filled from private authenticated actual Core results, never HTTP.
    project_id: str
    task_id: str
    scope_id: str


def decode_envelope(value, bounds: Bounds):
    require(type(value) is dict and set(value) <= {'command', 'args'} and 'command' in value)
    command, args = value['command'], value.get('args')
    require(isinstance(command, str))
    if args is None:
        args = {}
    require(type(args) is dict)
    simple = {'system_snapshot': 'system.snapshot', 'projects_list': 'projects.list',
        'connections_list': 'connections.list', 'sessions_list': 'sessions.list'}
    scoped = {'workspaces_list': ('workspaces.list', 'projectId', 'project_id', bounds.project_id),
        'grants_list': ('grants.list', 'projectId', 'project_id', bounds.project_id),
        'goal_scopes_list': ('goal_scopes.list', 'taskId', 'task_id', bounds.task_id),
        'factory_scopes_list': ('factory_scopes.list', 'rootTaskId', 'root_task_id', bounds.task_id),
        'factory_scopes_get': ('factory_scopes.get', 'scopeId', 'scope_id', bounds.scope_id),
        'factory_runs_get': ('factory_runs.get', 'scopeId', 'scope_id', bounds.scope_id)}
    if command in simple:
        require(not args)
        return FixedCall(simple[command], {})
    if command in scoped:
        method, camel, snake, expected = scoped[command]
        require(set(args) == {camel} and isinstance(args[camel], str) and args[camel] == expected)
        return FixedCall(method, {snake: expected})
    if command == 'events_subscribe':
        require(set(args) <= {'cursor', 'limit'})
        cursor, limit = args.get('cursor', 0), args.get('limit', 100)
        require(safe_integer(cursor) and safe_integer(limit, 1) and limit <= 100)
        return FixedCall('events.subscribe', {'cursor': cursor, 'limit': limit})
    if command in {'factory_runs_start', 'factory_runs_cancel'}:
        require(set(args) == {'scopeId', 'expectedRevision', 'idempotencyKey'})
        require(isinstance(args['scopeId'], str) and args['scopeId'] == bounds.scope_id)
        require(safe_integer(args['expectedRevision'], 1))
        key = args['idempotencyKey']
        require(isinstance(key, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,128}', key) is not None)
        return FixedCall('factory_runs.start' if command.endswith('start') else 'factory_runs.cancel',
            {'scope_id': bounds.scope_id}, key, args['expectedRevision'])
    # Includes Owner approve/propose/finalize/decline, revoke (host-only here),
    # generic IPC, session dispatch and all four scoped Agent tools.
    raise BoundaryRefusal(403, 'Outside fixed Factory browser fixture boundary')


def decode_http(raw, *, origin, host, content_type, expected_origin, bounds):
    require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}', expected_origin) is not None)
    require(1 <= int(expected_origin.rsplit(':', 1)[1]) <= 65535)
    if origin != expected_origin or host != expected_origin.removeprefix('http://'):
        raise BoundaryRefusal(403, 'Exact private loopback Origin and Host required')
    require(content_type == 'application/json' and 0 < len(raw) <= MAX_HTTP_BYTES)
    try:
        envelope = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
            parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
    except (UnicodeError, ValueError) as failure:
        raise BoundaryRefusal(400, 'Malformed complete JSON request') from failure
    return decode_envelope(envelope, bounds)


class PrivateHost:
    """Private Owner token stays outside HTTP/results/ACP settings."""
    def __init__(self, runtime):
        self.runtime = runtime.resolve(strict=True)
        token_path = self.runtime / 'session.token'
        require(stat.S_IMODE(token_path.stat().st_mode) == 0o600)
        self.token = token_path.read_text()

    def wire(self, call):
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'token': self.token,
            'method': call.method, 'params': call.params}
        if call.key is not None:
            frame['idempotency_key'] = call.key
        if call.revision is not None:
            frame['expected_revision'] = call.revision
        raw = json.dumps(frame, separators=(',', ':'), allow_nan=False).encode() + b'\n'
        require(len(raw) <= MAX_IPC_BYTES)
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(5)
            stream.connect(str(self.runtime / 'batond.sock'))
            stream.sendall(raw)
            with stream.makefile('rb') as reader:
                response = reader.readline(MAX_RESPONSE_BYTES + 1)
        require(0 < len(response) <= MAX_RESPONSE_BYTES and response.endswith(b'\n'))
        value = json.loads(response, object_pairs_hook=unique_object)
        require(type(value) is dict)
        return value, response  # original Core response bytes for HTTP, no u64 rewrite

    def result(self, method, params, key=None, revision=None):
        value, _ = self.wire(FixedCall(method, params, key, revision))
        assert not value.get('error'), {'method': method, 'response': value}
        return value['result']


CONNECTION_PIN_FIELDS = frozenset({'protocol_route', 'revision', 'environment_id', 'identity_ref',
    'host_id', 'config_revision', 'executable_sha256', 'capability_snapshot_id'})


def review_factory(proposal, selection, project, task, grant, connection, workspace, companion):
    """Python keeps original serde integers; representation hash is not Core digest."""
    require(proposal.get('kind') == 'factory' and isinstance(proposal.get('id'), str)
        and safe_integer(proposal.get('revision'), 1))
    snapshot = proposal['snapshot']
    require(set(snapshot) == {'schema_version', 'selection', 'bindings', 'contract_digest'})
    require(snapshot['schema_version'] == 1 and snapshot['selection'] == selection)
    require(re.fullmatch('[a-f0-9]{64}', snapshot['contract_digest']) is not None)
    b = snapshot['bindings']
    require(set(b) == {'project', 'root_task', 'approved_grant', 'connections', 'workspaces',
        'inputs', 'outputs', 'companion', 'guarantees', 'start_contract'})
    require(b['project'] == project and b['root_task'] == task and b['approved_grant'] == grant)
    require(task['state'] == 'draft' and not task.get('acceptance_ref') and not task.get('acceptance_sequence'))
    require(b['connections'] == [connection] and b['workspaces'] == [workspace])
    pin = grant['connection_bindings'][connection['id']]
    require(set(pin) == CONNECTION_PIN_FIELDS and all(pin[k] == connection[k] for k in pin))
    require(safe_integer(pin['revision'], 1) and all(isinstance(pin[k], str) and pin[k]
        for k in CONNECTION_PIN_FIELDS - {'revision'}))
    require(re.fullmatch('[a-f0-9]{64}', pin['executable_sha256']) is not None)
    require(workspace['identity'] == grant['workspace_bindings'][workspace['id']])
    require(connection['host_id'] in grant['scope']['host_ids'])
    require(all(r in grant['scope']['recipients'] for r in selection['recipients']))
    for rows, requirements, is_input in [(b['inputs'], selection['inputs'], True), (b['outputs'], selection['outputs'], False)]:
        require(len(rows) == len(requirements))
        for row, expected in zip(rows, requirements):
            contract = row['contract'] if is_input else row
            require(contract['requirement'] == expected and contract['canonical_root'] == workspace['root'])
            require(contract['root_device'] == workspace['identity']['device'] and contract['root_inode'] == workspace['identity']['inode'])
            if is_input:
                require(row['sha256'] == expected['expected_sha256'] and row['bytes'] == len(b'fixed input\n'))
    require(b['companion'] == {'executable': str(companion), 'executable_sha256': sha(companion.read_bytes()),
        'argument_template': ['factory-mcp', '${SESSION_HANDLE_FILE}'],
        'credential_transport': 'private_0600_file_reference', 'global_daemon_token_available': False})
    require(set(b['guarantees']) == {'filesystem_sandbox', 'network_sandbox', 'account_isolation',
        'output_quota', 'continuous_input_immutability', 'complete_process_tree_containment'}
        and all(v is False for v in b['guarantees'].values()))
    start = b['start_contract']
    require(set(start) == {'mode', 'root_task_request_digest', 'context_continuation', 'implicit_start_on_approval', 'new_workspace_authority'})
    require(start['mode'] == 'single_scope_single_run' and start['context_continuation'] == 'new_attempt_explicit_handoff_only'
        and start['implicit_start_on_approval'] is False and start['new_workspace_authority'] is False
        and re.fullmatch('[a-f0-9]{64}', start['root_task_request_digest']) is not None)
    raw = json.dumps(proposal, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()
    require(len(raw) <= MAX_FACTORY_REVIEW_BYTES)
    return {'authority': 'private authenticated generated Owner fixture, NOT real OS dialog',
        'proposal_id': proposal['id'], 'complete_review_encoding_bytes': len(raw),
        'complete_review_encoding_sha256': sha(raw), 'snapshot': snapshot}


def preflight(args):
    assert args.batond and args.candidate_manifest and args.frontend, 'Supply frozen batond/manifest and composed built frontend'
    binary = args.batond.resolve(strict=True)
    raw = args.candidate_manifest.resolve(strict=True).read_bytes()
    manifest = json.loads(raw, object_pairs_hook=unique_object)
    assert manifest['schema'] == 13, 'Only actual schema13 candidate is Factory runtime evidence'
    companion = binary.with_name('baton').resolve(strict=True)
    for name, path in [('batond', binary), ('baton', companion)]:
        assert manifest['binaries'][name] == sha(path.read_bytes()), (name, 'frozen binary SHA mismatch')
        assert path.is_file() and os.access(path, os.X_OK)
    assert companion.stat().st_size <= 128 * 1024 * 1024, 'Core companion original128MiB binding cap; never strip frozen executable'
    frontend = args.frontend.resolve(strict=True)
    assert (frontend / 'index.html').is_file()
    paths = list(frontend.rglob('*'))
    assert len(paths) <= 4096 and not any(p.is_symlink() for p in paths)
    assert sum(p.stat().st_size for p in paths if p.is_file()) <= 64 * 1024 * 1024
    return binary, companion, frontend, manifest, raw


GENERATED_ACP = r'''import hashlib,json,os,stat,subprocess,sys
if '--version' in sys.argv:
 print('Generated account-free Factory ACP 1');sys.exit(0)
settings_path=sys.argv[sys.argv.index('--fixture-settings')+1]
mcp=None;mid=0
def emit(value):print(json.dumps(value),flush=True)
def response(request,value):emit({'jsonrpc':'2.0','id':request['id'],'result':value})
def identity(pid):
 tail=open('/proc/'+str(pid)+'/stat').read().rsplit(')',1)[1].split()
 return {'pid':pid,'pgid':os.getpgid(pid),'start_ticks':tail[19]}
def mcp_call(method,params,allow_error=False):
 global mid
 mid+=1;frame={'jsonrpc':'2.0','id':mid,'method':method,'params':params}
 raw=json.dumps(frame);assert len(raw.encode())<=65536
 mcp.stdin.write(raw+'\n');mcp.stdin.flush()
 raw=mcp.stdout.readline(1048577);assert raw.endswith('\n') and len(raw.encode())<=1048576
 value=json.loads(raw);assert value['id']==mid
 if not allow_error:assert 'error' not in value,value
 return value if allow_error else value['result']
def tool(name,args):
 packet=mcp_call('tools/call',{'name':name,'arguments':args})
 value=json.loads(packet['content'][0]['text']);assert 'error' not in value,value
 return value['result']
def close_mcp():
 global mcp
 if mcp is not None:
  mcp.stdin.close();mcp.wait(timeout=1);mcp=None
def file_hash(path):return hashlib.sha256(open(path,'rb').read()).hexdigest()
try:
 for line in sys.stdin:
  request=json.loads(line);method=request.get('method')
  if method=='initialize':
   assert not any(k in os.environ for k in ['GH_TOKEN','GITHUB_TOKEN','OPENAI_API_KEY','ANTHROPIC_API_KEY'])
   response(request,{'protocolVersion':1,'agentCapabilities':{}})
  elif method=='session/new':
   settings=json.load(open(settings_path));servers=request['params']['mcpServers'];assert len(servers)==1
   server=servers[0];assert server['name']=='agentbaton-factory' and server['env']==[]
   assert server['command']==settings['companion'] and server['args'][0]=='factory-mcp' and len(server['args'])==2
   credential=server['args'][1];assert os.path.isabs(credential)
   assert stat.S_IMODE(os.stat(credential).st_mode)==0o600 and stat.S_IMODE(os.stat(os.path.dirname(credential)).st_mode)==0o700
   # Never open the scoped credential or owner session.token; only Baton reads it.
   mcp=subprocess.Popen([server['command']]+server['args'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
   mcp_call('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'generated-browser-fixture','version':'1'}})
   assert [t['name'] for t in mcp_call('tools/list',{})['tools']]==['delegate_task','get_task_status','get_task_result','cancel_task']
   response(request,{'sessionId':'generated-factory-native'})
  elif method=='session/prompt':
   raw=request['params']['prompt'][0]['text'];assert len(raw.encode())<=131072;c=json.loads(raw)
   assert set(c)=={'schema_version','kind','task_id','root_task_id','parent_task_id','goal','input_artifacts','output_artifacts','handoff_result','handoff_results','handoff_digest','context_continuation','acceptance_state'}
   assert c['schema_version']==1 and c['kind']=='agentbaton_factory_task_request'
   assert c['context_continuation']=='new_attempt_explicit_handoff_only' and c['acceptance_state']=='not_established'
   assert c['root_task_id']==settings['task_id']
   status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']
   denied=mcp_call('tools/call',{'name':'approvals.finalize','arguments':{'proposal_id':'forbidden'}},True)
   assert 'error' in denied
   assert len(c['input_artifacts'])==1
   source=c['input_artifacts'][0];contract=source['contract'];requirement=contract['requirement']
   assert requirement['relative_path']=='input.txt' and requirement['artifact_id']=='input'
   assert open('input.txt','rb').read()==b'fixed input\n' and file_hash('input.txt')==source['sha256']==requirement['expected_sha256']
   root_stat=os.stat('.');assert os.path.realpath('.')==contract['canonical_root'] and root_stat.st_dev==contract['root_device'] and root_stat.st_ino==contract['root_inode']
   stage='child' if c['parent_task_id'] is not None else ('parent-resumed' if c['handoff_results'] else 'parent-initial')
   os.makedirs('reports',exist_ok=True)
   proof={'stage':stage,'request_text':raw,'request_sha256':hashlib.sha256(raw.encode()).hexdigest(),
    'owned_provider':identity(os.getpid()),'owned_companion':identity(mcp.pid),'denied_owner_tool':denied}
   with open('reports/fixture-request-'+str(os.getpid())+'.json','w')as f:json.dump(proof,f)
   with open('reports/fixture-stages.txt','a')as f:f.write(stage+'\n')
   if settings['case']=='cancel':
    assert stage=='parent-initial'
    continue  # actual live Provider awaits real session/cancel; no end_turn success
   if settings['case']=='unknown':
    close_mcp();print('generated malformed ACP protocol',flush=True);continue
   if stage=='parent-initial':
    child=tool('delegate_task',{'goal':'Write the fixed generated child report','connection_id':settings['connection_id'],
     'workspace_id':settings['workspace_id'],'input_artifact_ids':['input'],'output_artifact_ids':['report-child'],
     'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},
     'wait_for_child':True,'idempotency_key':'generated-child'})
    assert child['parent_task_id']==c['task_id'] and child['state']=='blocked_on_live_parent'
    early=tool('get_task_result',{'task_id':child['task_id']});assert early['task']['result_available'] is False and early['acceptance_state']=='not_established'
   else:
    if stage=='parent-resumed':
     assert len(c['handoff_results'])==1 and c['handoff_digest'] and c['handoff_result']['handoff_digest']==c['handoff_digest']
     result=c['handoff_results'][0];assert result['task']['confirmed_stop'] is True and result['task']['result_available'] is True
     assert result['task']['parent_task_id']==c['task_id'] and result['acceptance_state']=='not_established'
     assert tool('get_task_result',{'task_id':result['task']['task_id']})['task']['task_id']==result['task']['task_id']
     for artifact in result['artifacts']:
      assert file_hash(artifact['relative_path'])==artifact['sha256'] and os.stat(artifact['relative_path']).st_size==artifact['bytes']
    artifact_id='report-child' if stage=='child' else 'report-parent'
    output=next(a for a in c['output_artifacts'] if a['requirement']['artifact_id']==artifact_id)
    path=output['requirement']['relative_path'];assert path=='reports/'+artifact_id+'.txt'
    assert os.path.realpath(path).startswith(os.path.realpath('.')+'/')
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_TRUNC|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb')as f:f.write((stage+' '+artifact_id+' report\n').encode())
   response(request,{'stopReason':'end_turn'})
  elif method=='session/cancel':
   close_mcp()
finally:
 close_mcp()
'''


def runtime_driver(args, output, attribution, prepared):
    binary, companion, frontend, manifest, manifest_raw = prepared
    try:
        from playwright.sync_api import expect, sync_playwright
    except ImportError as failure:
        (output / 'preflight-refusal.json').write_text(json.dumps({**attribution, 'status': 'preflight_refused',
            'actual_runtime': 'not_run', 'reason': str(failure), 'no_process_token_read_or_listener': True}, indent=2) + '\n')
        return 2
    compile(GENERATED_ACP, '<generated-account-free-acp>', 'exec')
    assets = {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()}
    report = {**attribution, 'status': 'failed_or_interrupted', 'case': args.case,
        'dimension': 'actual built UI/Linux Chromium/fixed HTTP/authenticated Core13/generated ACP and real private Baton MCP',
        'not_claimed': ['Native Owner OS dialog', 'macOS', 'real Provider/model', 'true response-drop recovery',
            'Provider disconnect side-effect recovery', 'complete Factory fault matrix', 'full product completion'],
        'candidate': manifest, 'candidate_manifest_sha256': sha(manifest_raw), 'frontend_assets_sha256': assets,
        'checks': [], 'private_owner_reviews': [], 'http': [], 'http_unexpected_errors': [], 'readonly_aborts': [],
        'observations': {}, 'screenshots': [], 'cleanup': []}
    token = ''; pump = lambda: None
    def clean(value):
        return str(value).replace(token, '[redacted generated Owner token]') if token else str(value)
    def wait(read, predicate, seconds=15):
        deadline = time.monotonic() + seconds
        while True:
            pump(); value = read()
            if predicate(value):
                return value
            assert time.monotonic() < deadline, clean(('Observation timed out', value))
            time.sleep(.02)
    def passed(name):
        report['checks'].append({'name': name, 'status': 'passed'})
    with tempfile.TemporaryDirectory(prefix='baton-factory-') as temporary:
        root = Path(temporary).resolve(); root.chmod(0o700)
        runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
        settings = root / 'generated-settings.json'
        provider = root / 'generated-factory-acp'
        provider.write_text('#!' + sys.executable + '\n' + GENERATED_ACP); provider.chmod(0o700)
        report['generated_program_sha256'] = sha(provider.read_bytes())
        log = (output / 'daemon.log').open('wb')
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        server = thread = browser = None; workspace = host = scope = None
        try:
            wait(lambda: ((runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists(), daemon.poll()),
                lambda v: v[0] and v[1] and v[2] is None)
            host = PrivateHost(runtime); token = host.token
            def owner_resource(method, params, kind, key, revision=None):
                proposal = host.result(method, params, key, revision)
                assert proposal['kind'] == kind
                raw = json.dumps(proposal, ensure_ascii=False, separators=(',', ':')).encode()
                assert len(raw) <= MAX_LEGACY_REVIEW_BYTES
                report['private_owner_reviews'].append({'kind': kind, 'proposal': proposal,
                    'authority': 'explicit private generated-resource Owner fixture; not OS dialog'})
                return host.result('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
            created = owner_resource('connections.propose', {'name': 'Generated Factory account-free ACP', 'provider': 'custom_acp',
                'executable': str(provider), 'arguments': ['--fixture-settings', str(settings)]}, 'connection', 'factory:connection')
            connection = wait(lambda: host.result('connections.list', {}),
                lambda rows: any(c['id'] == created['id'] and c['probe_state'] == 'ready' for c in rows))
            connection = next(c for c in connection if c['id'] == created['id'])
            source = root / 'source'; source.mkdir(mode=0o700)
            (source / 'input.txt').write_bytes(b'fixed input\n'); (source / '.gitignore').write_text('reports/\n')
            git_env = {**os.environ, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_LAZY_FETCH': '1'}
            def git(*argv):
                return subprocess.check_output(['git', '-C', str(source), '-c', 'user.name=Generated fixture',
                    '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null', *argv], env=git_env, text=True).strip()
            git('init', '-q', '--initial-branch=main'); git('add', 'input.txt', '.gitignore'); git('commit', '-qm', 'generated fixed Factory input')
            project = host.result('projects.import', {'path': str(source), 'name': 'Generated Factory runtime'}, 'factory:project')
            workspace = host.result('workspaces.create', {'project_id': project['id'], 'expected_head': git('rev-parse', 'HEAD')}, 'factory:workspace', project['revision'])
            workspace_root = Path(workspace['root']).resolve(strict=True)
            assert workspace_root.is_relative_to(runtime / 'workspaces')
            grant = owner_resource('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'],
                'connection_id': connection['id'], 'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 30_000}, 'grant', 'factory:grant')
            task = host.result('tasks.create', {'project_id': project['id'], 'goal': 'Generated bounded parent child new-parent Factory'}, 'factory:task')
            settings_value = {'case': args.case, 'task_id': task['id'], 'connection_id': connection['id'],
                'workspace_id': workspace['id'], 'companion': str(companion)}
            settings.write_text(json.dumps(settings_value)); settings.chmod(0o600)
            report['observations']['nonsecret_generated_settings'] = settings_value
            report['generated_settings_sha256'] = sha(settings.read_bytes())
            budget = {'attempts': 8, 'parallel': 1, 'runtime_ms': 30_000, 'output_bytes': 12288,
                'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
            selection = {'root_task_id': task['id'], 'expected_task_revision': task['revision'], 'grant_id': grant['id'],
                'expected_grant_revision': grant['revision'], 'connection_ids': [connection['id']], 'workspace_ids': [workspace['id']],
                'inputs': [{'artifact_id': 'input', 'relative_path': 'input.txt', 'max_bytes': 1024, 'expected_sha256': sha(b'fixed input\n')}],
                'outputs': [{'artifact_id': key, 'relative_path': 'reports/' + key + '.txt', 'max_bytes': 4096, 'expected_sha256': None}
                    for key in ['report-child', 'report-parent']], 'recipients': [], 'max_depth': 2, 'max_children': 1, 'max_total_nodes': 3,
                'root_budget': budget, 'project_budget': dict(budget), 'ttl_ms': 30_000,
                'capabilities': {'delegate_task': True, 'resume_parent_as_new_attempt': True, 'child_planning': False,
                    'child_goal_execution': False, 'create_workspace': False}}
            proposal = host.result('factory_scopes.propose', selection, 'factory:scope')
            report['private_owner_reviews'].append(review_factory(proposal, selection, project, task, grant, connection, workspace, companion))
            scope = host.result('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])
            assert scope['id'] == scope['approval_id'] == proposal['id'] and scope['selection'] == selection
            assert scope['bindings'] == proposal['snapshot']['bindings'] and scope['contract_digest'] == proposal['snapshot']['contract_digest']
            assert host.result('sessions.list', {}) == []
            stale, _ = host.wire(FixedCall('factory_runs.start', {'scope_id': scope['id']}, 'factory:wrong-cas', scope['revision'] + 1))
            assert stale['error']['code'] == 'STALE_REVISION' and host.result('sessions.list', {}) == []
            report['observations']['actual_wrong_start_CAS_refusal'] = stale
            bounds = Bounds(project['id'], task['id'], scope['id'])
            passed('actual generated resources and complete private Factory approval; approval starts no Provider')
            lock = threading.Lock(); active = {}; phase = {'value': 'serving'}
            class Server(http.server.ThreadingHTTPServer):
                daemon_threads = False; block_on_close = True
                def handle_error(self, request, client_address):
                    with lock:
                        report['http_unexpected_errors'].append({'stage': 'uncaught_handler', 'traceback': clean(traceback.format_exc())})
            class Handler(http.server.SimpleHTTPRequestHandler):
                def setup(self):
                    super().setup(); self.connection.settimeout(5)
                def log_message(self, *_):
                    pass
                def list_directory(self, _):
                    self.send_error(403); return None
                def do_POST(self):
                    context = {'stage': 'headers', 'method': None, 'phase': phase['value']}
                    identity = id(self)
                    with lock:
                        active[identity] = context
                    try:
                        if self.path != '/__test_factory_ipc':
                            self.send_error(404); return
                        try:
                            require(self.headers.get('Transfer-Encoding') is None)
                            require(len(self.headers.get_all('Content-Length', [])) == 1)
                            require(all(len(self.headers.get_all(k, [])) == 1 for k in ['Origin', 'Host', 'Content-Type']))
                            length = self.headers['Content-Length']
                            require(re.fullmatch('[1-9][0-9]{0,5}', length) is not None and int(length) <= MAX_HTTP_BYTES)
                            raw = self.rfile.read(int(length)); require(len(raw) == int(length))
                            call = decode_http(raw, origin=self.headers.get('Origin', ''), host=self.headers.get('Host', ''),
                                content_type=self.headers.get('Content-Type', ''), expected_origin=url, bounds=bounds)
                        except BoundaryRefusal as failure:
                            self.send_error(failure.status); return
                        context.update(method=call.method, stage='core_call')
                        try:
                            value, reply = host.wire(call)
                            with lock:
                                report['http'].append({'method': call.method, 'params': call.params, 'key': call.key, 'revision': call.revision,
                                    'request_body_sha256': sha(raw), 'original_core_reply_sha256': sha(reply), 'response': value})
                            context['stage'] = 'response_headers'
                            self.send_response(200); self.send_header('Content-Type', 'application/json')
                            self.send_header('Content-Length', str(len(reply))); self.end_headers()
                            context['stage'] = 'response_body'; self.wfile.write(reply); context['stage'] = 'complete'
                        except (BrokenPipeError, ConnectionResetError) as failure:
                            if call.method in READS and context['stage'] == 'response_body':
                                report['readonly_aborts'].append({**context, 'type': type(failure).__name__})
                            else:
                                report['http_unexpected_errors'].append({**context, 'type': type(failure).__name__, 'message': clean(failure)})
                        except Exception as failure:
                            report['http_unexpected_errors'].append({**context, 'type': type(failure).__name__, 'message': clean(failure)})
                            self.send_error(502)
                    finally:
                        with lock:
                            active.pop(identity, None)
            server = Server(('127.0.0.1', 0), partial(Handler, directory=str(frontend)))
            url = 'http://127.0.0.1:' + str(server.server_port)
            thread = threading.Thread(target=server.serve_forever); thread.start()
            with sync_playwright() as playwright:
                browser = playwright.chromium.launch(executable_path=args.chromium, headless=True, args=['--no-sandbox'])
                context = browser.new_context(viewport={'width': 1488, 'height': 1400}, locale='en-US')
                context.add_init_script("localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('/__test_factory_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error('Fixed Factory fixture refused request');return r.json();}};")
                context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(url + '/') else route.abort())
                page = context.new_page(); errors = []; page.on('pageerror', lambda failure: errors.append(str(failure)))
                pump = lambda: page.wait_for_timeout(1)
                def shot(name, focus):
                    focus.evaluate("e=>e.scrollIntoView({block:'start'})")
                    path = output / (name + '.png'); page.screenshot(path=str(path))
                    report['screenshots'].append({'path': str(path), 'sha256': sha(path.read_bytes()), 'dimension': 'Linux Chromium; not native/Mac'})
                try:
                    page.goto(url); page.get_by_test_id('task-card').filter(has_text=task['goal']).click(); page.locator('#tab-details').click()
                    panel = page.get_by_test_id('factory-runtime')
                    assert not panel.locator(':scope > details').evaluate('e=>e.open')
                    assert not any(r['method'].startswith('factory_') for r in report['http'])
                    panel.locator(':scope > details > summary').click()
                    expect(panel.get_by_role('button', name='Start approved Factory run', exact=True)).to_be_enabled()
                    expect(panel.get_by_text('Approval is recorded; execution has not started.', exact=True)).to_be_visible()
                    denied = []
                    for command in ['factory_scope_approve', 'factory_scopes_propose', 'factory_scopes_revoke',
                            'approvals_finalize', 'approvals_decline', 'generic_ipc', 'delegate_task', 'get_task_status', 'get_task_result', 'cancel_task']:
                        status = page.evaluate("async command=>(await fetch('/__test_factory_ipc',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args:{}})})).status", command)
                        assert status == 403; denied.append(command)
                    report['observations']['http_authority_denied'] = denied
                    shot('01-real-approved-scope-not-started', panel.get_by_role('button', name='Start approved Factory run', exact=True))
                    passed('built UI reads genuine scope; folded load and explicit start; HTTP denies Owner and Agent tools')
                    panel.get_by_role('button', name='Start approved Factory run', exact=True).click()
                    if args.case == 'cancel':
                        wait(lambda: list((workspace_root / 'reports').glob('fixture-request-*.json')), bool)
                        panel.get_by_role('button', name='Refresh Factory records', exact=True).click()
                        expect(panel.get_by_role('button', name='Request Factory cancellation', exact=True)).to_be_enabled()
                        panel.get_by_role('button', name='Request Factory cancellation', exact=True).click()
                        requested = wait(lambda: [r for r in report['http'] if r['method'] == 'factory_runs.cancel'], bool)[0]['response']
                        assert not requested.get('error') and requested['result']['state'] == 'cancel_requested'
                        assert requested['result']['cancel_requested'] is True
                        report['observations']['actual_cancel_intent_receipt'] = requested
                    terminal = {'normal': 'completed', 'cancel': 'cancelled', 'unknown': 'outcome_unknown'}[args.case]
                    detail = wait(lambda: host.result('factory_runs.get', {'scope_id': scope['id']}), lambda d: d['run']['state'] == terminal)
                    report['observations']['terminal'] = detail
                    assert detail['run']['scope_id'] == scope['id'] and detail['run']['context_continuation'] == 'new_attempt_explicit_handoff_only'
                    assert len(detail['tasks']) == (2 if args.case == 'normal' else 1)
                    if args.case in {'normal', 'cancel'}:
                        assert detail['run']['active_session_id'] is None
                        assert all(t['attempt_id'] is not None and t['confirmed_stop'] is True for t in detail['tasks'])
                    if args.case == 'cancel':
                        assert detail['run']['cancel_requested'] is True
                    records = [json.loads(p.read_text()) for p in sorted((workspace_root / 'reports').glob('fixture-request-*.json'))]
                    expected_stages = ['child', 'parent-initial', 'parent-resumed'] if args.case == 'normal' else ['parent-initial']
                    assert sorted(r['stage'] for r in records) == expected_stages
                    for row in records:
                        assert sha(row['request_text'].encode()) == row['request_sha256']
                        row['request'] = json.loads(row['request_text'])
                    report['observations']['actual_task_requests'] = records
                    if args.case == 'normal':
                        assert (workspace_root / 'reports/fixture-stages.txt').read_text() == 'parent-initial\nchild\nparent-resumed\n'
                        for artifact_id, content in [('report-child', b'child report-child report\n'), ('report-parent', b'parent-resumed report-parent report\n')]:
                            path = workspace_root / 'reports' / (artifact_id + '.txt')
                            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
                            try:
                                before = os.fstat(fd); actual = os.read(fd, 4097); after = os.fstat(fd)
                                assert stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and actual == content
                                assert (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
                            finally:
                                os.close(fd)
                            report['observations'].setdefault('declared_files', []).append({'artifact_id': artifact_id, 'bytes': len(content),
                                'sha256': sha(content), 'observed_device': before.st_dev, 'observed_inode': before.st_ino,
                                'evidence_kind': 'independent bounded nofollow FD read; not a fabricated Core receipt'})
                        parent = [r for r in records if r['stage'].startswith('parent')]
                        assert len({r['owned_provider']['pid'] for r in parent}) == 2
                        assert parent[0]['request']['task_id'] == parent[1]['request']['task_id'] == task['id']
                    with sqlite3.connect('file:' + str(runtime / 'baton.sqlite3') + '?mode=ro', uri=True) as db:
                        report['observations']['actual_sqlite_schema'] = db.execute('PRAGMA user_version').fetchone()[0]
                        assert report['observations']['actual_sqlite_schema'] == 13
                        attempts = db.execute('SELECT count(*) FROM factory_attempts').fetchone()[0]
                        assert attempts == (3 if args.case == 'normal' else 1)
                        assert db.execute('SELECT count(*) FROM execution_grants').fetchone()[0] == 1
                        assert db.execute("SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'").fetchone()[0] == 0
                        budgets = [dict(zip(['scope', 'consumed', 'active'], row)) for row in db.execute('SELECT scope,consumed,active FROM budgets')]
                        report['observations']['budgets'] = budgets
                        roots = [r for r in budgets if r['scope'].startswith('root:')]
                        assert len(roots) == 1 and roots[0]['consumed'] == attempts, roots
                        if args.case in {'normal', 'cancel'}:
                            assert roots[0]['active'] == 0, roots
                            assert db.execute("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'").fetchone()[0] == 0
                            assert db.execute("SELECT count(*) FROM workspace_leases WHERE holder!=''").fetchone()[0] == 0
                            assert db.execute('SELECT count(*) FROM factory_scoped_handles WHERE revoked=0').fetchone()[0] == 0
                        events = [json.loads(raw) for raw, in db.execute('SELECT document FROM events ORDER BY sequence')]
                        report['observations']['events'] = events
                        owned_stops = [e for e in events if e['kind'] == 'session.stopped' and e['payload'].get('process_reaped') is True and e['payload'].get('no_process_started') is False]
                        report['observations']['actual_owned_session_stops'] = owned_stops
                        if args.case in {'normal', 'cancel'}:
                            assert len(owned_stops) == attempts
                    starts = [r for r in report['http'] if r['method'] == 'factory_runs.start']
                    assert len(starts) == 1 and not starts[0]['response'].get('error')
                    original_start = starts[0]
                    replay, _ = host.wire(FixedCall('factory_runs.start', original_start['params'], original_start['key'], original_start['revision']))
                    assert not replay.get('error') and replay['result'] == original_start['response']['result']
                    conflict, _ = host.wire(FixedCall('factory_runs.start', original_start['params'], original_start['key'], original_start['revision'] + 1))
                    assert conflict['error']['code'] == 'CONFIG_CONFLICT'
                    report['observations']['same_key_historical_start_replay'] = {'reply': replay, 'different_payload_error': conflict,
                        'dimension': 'ordinary same-key receipt replay; NOT a dropped-response test'}
                    passed('actual scoped MCP/owned attempts persist exact lineage and cumulative budgets; no TaskAccepted or new Grant')
                    panel.get_by_role('button', name='Refresh Factory records', exact=True).click()
                    region = panel.get_by_role('region', name='Factory run', exact=True)
                    expect(region).to_contain_text(terminal)
                    expect(panel.get_by_text('Completed Factory processing does not accept the task.', exact=True)).to_be_visible()
                    shot('02-real-' + terminal + '-task-history', region)
                    assert not errors and not report['http_unexpected_errors'], (errors, report['http_unexpected_errors'])
                    assert sha(Path(__file__).read_bytes()) == attribution['harness_sha256']
                    assert {str(p.relative_to(frontend)): sha(p.read_bytes()) for p in sorted(frontend.rglob('*')) if p.is_file()} == assets
                    assert sha(binary.read_bytes()) == manifest['binaries']['batond'] and sha(companion.read_bytes()) == manifest['binaries']['baton']
                    passed('actual built Factory UI shows terminal history without acceptance or source/asset drift')
                    report['status'] = 'passed'
                except BaseException:
                    try:
                        shot('failure-current-ui', page.locator('body'))
                    except Exception as capture_failure:
                        report['failure_capture_error'] = clean(capture_failure)
                    raise
                finally:
                    phase['value'] = 'browser_closing'; browser.close(); browser = None
        except BaseException as failure:
            report['failure'] = {'type': type(failure).__name__, 'message': clean(failure), 'traceback': clean(traceback.format_exc())}
            if host is not None:
                try:
                    report['observations']['failure_snapshot'] = host.result('system.snapshot', {})
                    if scope is not None:
                        report['observations']['failure_factory_response'] = host.wire(FixedCall('factory_runs.get', {'scope_id': scope['id']}))[0]
                except Exception as read_failure:
                    report['observations']['failure_diagnostic_error'] = clean(read_failure)
        finally:
            pump = lambda: None
            if server is not None:
                server.shutdown()
            if thread is not None:
                thread.join(timeout=5)
                report['cleanup'].append({'action': 'HTTP thread join', 'thread_alive_after_join': thread.is_alive()})
                if thread.is_alive():
                    report['status'] = 'failed_or_interrupted'
            if server is not None:
                server.server_close()
                report['cleanup'].append({'action': 'HTTP handlers joined', 'active_handlers': list(active.values())})
                if active:
                    report['status'] = 'failed_or_interrupted'
            if daemon.poll() is None:
                daemon.terminate()
            forced = False
            try:
                exitcode = daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                forced = True; os.killpg(daemon.pid, signal.SIGKILL); exitcode = daemon.wait()
            report['cleanup'].append({'action': 'private daemon waited', 'pid': daemon.pid, 'exitcode': exitcode, 'forced_sigkill': forced})
            if exitcode != 0 or forced:
                report['status'] = 'failed_or_interrupted'
            owned = []
            if workspace is not None:
                for path in (Path(workspace['root']) / 'reports').glob('fixture-request-*.json'):
                    try:
                        row = json.loads(path.read_text())
                        for field in ['owned_provider', 'owned_companion']:
                            expected = row[field]; pid = expected['pid']
                            assert type(pid) is int and pid > 1 and expected['pgid'] > 1
                            try:
                                current = support.proc_identity(pid)
                            except FileNotFoundError:
                                current = None
                            alive = current is not None and current['start_ticks'] == expected['start_ticks']
                            observation = {'expected': expected, 'observed': current, 'original_identity_gone': not alive, 'signal_sent': False}
                            if alive:
                                # Only this exact recorded private fixture child, never global pgid/pkill.
                                assert os.getpgid(pid) == expected['pgid']
                                raw_argv = Path(f'/proc/{pid}/cmdline').read_bytes()
                                assert len(raw_argv) <= 8192 and raw_argv.endswith(b'\0')
                                argv = [p.decode() for p in raw_argv[:-1].split(b'\0')]
                                if field == 'owned_provider':
                                    assert expected['pgid'] == pid and str(provider) in argv
                                    assert argv[-2:] == ['--fixture-settings', str(settings)]
                                else:
                                    assert expected['pgid'] == row['owned_provider']['pid']
                                    assert len(argv) == 3 and argv[:2] == [str(companion), 'factory-mcp']
                                    assert Path(argv[2]).resolve().is_relative_to(runtime / 'factory-scoped-handles')
                                # PID can disappear/reuse during argv reads: recheck immediately.
                                assert support.proc_identity(pid)['start_ticks'] == expected['start_ticks']
                                os.kill(pid, signal.SIGKILL); observation['signal_sent'] = True
                                report['status'] = 'failed_or_interrupted'
                            owned.append(observation)
                    except Exception as cleanup_failure:
                        report['status'] = 'failed_or_interrupted'
                        report['cleanup'].append({'action': 'refused unsafe/malformed fixture identity cleanup', 'failure': clean(cleanup_failure)})
            report['cleanup'].append({'action': 'exact recorded generated identities inspected', 'observations': owned})
            if report['http_unexpected_errors']:
                report['status'] = 'failed_or_interrupted'
            log.close()
            path = output / 'daemon.log'
            if token:
                path.write_bytes(path.read_bytes().replace(token.encode(), b'[redacted generated Owner token]'))
            (output / 'report.json').write_text(clean(json.dumps(report, ensure_ascii=False, indent=2)) + '\n')
    print(json.dumps({'status': report['status'], 'checks': len(report['checks']), 'screenshots': len(report['screenshots']), 'report': str(output / 'report.json')}))
    return 0 if report['status'] == 'passed' else 1


def boundary_self_check():
    bounds = Bounds('project', 'task', 'scope')
    rows = []
    def valid(name, command, args, expected):
        actual = decode_envelope({'command': command, 'args': args}, bounds)
        assert actual == expected
        rows.append({'name': name, 'status': 'passed'})
    def invalid(name, envelope, status):
        try:
            decode_envelope(envelope, bounds)
        except BoundaryRefusal as failure:
            assert failure.status == status, (name, failure.status)
        else:
            raise AssertionError(name)
        rows.append({'name': name, 'status': 'passed'})
    for command, method in [('system_snapshot', 'system.snapshot'), ('projects_list', 'projects.list'),
            ('connections_list', 'connections.list'), ('sessions_list', 'sessions.list')]:
        valid(command, command, {}, FixedCall(method, {}))
    for command, camel, snake, identity, method in [
            ('factory_scopes_list', 'rootTaskId', 'root_task_id', 'task', 'factory_scopes.list'),
            ('factory_scopes_get', 'scopeId', 'scope_id', 'scope', 'factory_scopes.get'),
            ('factory_runs_get', 'scopeId', 'scope_id', 'scope', 'factory_runs.get'),
            ('workspaces_list', 'projectId', 'project_id', 'project', 'workspaces.list'),
            ('grants_list', 'projectId', 'project_id', 'project', 'grants.list'),
            ('goal_scopes_list', 'taskId', 'task_id', 'task', 'goal_scopes.list')]:
        valid(command, command, {camel: identity}, FixedCall(method, {snake: identity}))
        invalid(command + ' foreign', {'command': command, 'args': {camel: 'foreign'}}, 400)
        invalid(command + ' authority extra', {'command': command, 'args': {camel: identity, 'token': 'untrusted'}}, 400)
    for command, method in [('factory_runs_start', 'factory_runs.start'), ('factory_runs_cancel', 'factory_runs.cancel')]:
        args = {'scopeId': 'scope', 'expectedRevision': 3, 'idempotencyKey': 'fixed-key'}
        valid(command, command, args, FixedCall(method, {'scope_id': 'scope'}, 'fixed-key', 3))
        for label, patch in [('null key', {'idempotencyKey': None}), ('empty key', {'idempotencyKey': ''}),
                ('bad key', {'idempotencyKey': 'a\nkey'}), ('long key', {'idempotencyKey': 'a' * 129}),
                ('bool CAS', {'expectedRevision': True}), ('zero CAS', {'expectedRevision': 0}),
                ('float CAS', {'expectedRevision': 1.0}), ('unsafe CAS', {'expectedRevision': 2**53}),
                ('foreign scope', {'scopeId': 'foreign'}), ('opaque authority', {'token': 'bad'})]:
            invalid(command + ' ' + label, {'command': command, 'args': {**args, **patch}}, 400)
    for command in ['factory_scope_approve', 'factory_scopes_propose', 'factory_scopes_revoke',
            'approvals_finalize', 'approvals_decline', 'generic_ipc', 'sessions_dispatch',
            'delegate_task', 'get_task_status', 'get_task_result', 'cancel_task']:
        invalid('deny ' + command, {'command': command, 'args': {}}, 403)
    origin = 'http://127.0.0.1:43123'
    def web(raw, **patch):
        return decode_http(raw, origin=patch.get('origin', origin), host=patch.get('host', '127.0.0.1:43123'),
            content_type=patch.get('content_type', 'application/json'), expected_origin=origin, bounds=bounds)
    assert web(b'{"command":"system_snapshot"}') == FixedCall('system.snapshot', {})
    rows.append({'name': 'exact loopback HTTP positive', 'status': 'passed'})
    for name, raw, patch, expected in [('duplicate', b'{"command":"system_snapshot","command":"generic_ipc"}', {}, 400),
            ('UTF8', b'\xff', {}, 400), ('nonfinite', b'{"command":"x","args":{"x":NaN}}', {}, 400),
            ('oversize', b'x' * (MAX_HTTP_BYTES + 1), {}, 400), ('foreign Origin', b'{}', {'origin': 'http://example.invalid'}, 403),
            ('foreign Host', b'{}', {'host': 'example.invalid'}, 403), ('MIME', b'{}', {'content_type': 'text/plain'}, 400)]:
        try:
            web(raw, **patch)
        except BoundaryRefusal as failure:
            assert failure.status == expected
        else:
            raise AssertionError(name)
        rows.append({'name': name, 'status': 'passed'})
    return {'dimension': 'pure fixed HTTP boundary controls; no Core/browser/Provider',
        'status': 'passed', 'count': len(rows), 'checks': rows}


def fixture_self_check():
    """Pure review/AST and version-only local generated process, not Core."""
    rows = []
    def passed(name):
        rows.append({'name': name, 'status': 'passed'})
    compile(GENERATED_ACP, '<generated-account-free-acp>', 'exec')
    passed('generated ACP strict Python compiles')
    wire_path = Path(__file__).parents[1] / 'src-tauri/tests/fixtures/factory-wire-bundle-0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64.json'
    raw = wire_path.read_bytes()
    assert sha(raw) == '0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64'
    passed('saved actual Store bundle original UTF8 SHA verified, not a runtime replay')
    with tempfile.TemporaryDirectory(prefix='factory-review-unit-') as temporary:
        directory = Path(temporary); companion = directory / 'generated-companion-binding-unit'
        companion.write_bytes(b'Unit-only companion binding, never executed\n')
        wire = json.loads(raw); proposal, selection = wire['proposal'], wire['selection']
        bindings = proposal['snapshot']['bindings']
        # This modified DTO is explicitly a unit fixture, never a Core result.
        bindings['companion']['executable'] = str(companion)
        bindings['companion']['executable_sha256'] = sha(companion.read_bytes())
        expected = {key: json.loads(json.dumps(bindings[key])) for key in ['project', 'root_task', 'approved_grant', 'connections', 'workspaces']}
        def review(value):
            return review_factory(value, selection, expected['project'], expected['root_task'], expected['approved_grant'],
                expected['connections'][0], expected['workspaces'][0], companion)
        assert review(proposal)['proposal_id'] == proposal['id']
        passed('complete private Factory review unit accepts exact bound typed DTO')
        for name in ['kind', 'task', 'grant-pin', 'project', 'host', 'output-inode', 'implicit-start', 'input-hash', 'oversize']:
            changed = json.loads(json.dumps(proposal)); b = changed['snapshot']['bindings']
            if name == 'kind': changed['kind'] = 'goal'
            if name == 'task': b['root_task']['id'] = 'foreign'
            if name == 'grant-pin': b['approved_grant']['connection_bindings'][b['connections'][0]['id']]['config_revision'] = 'foreign'
            if name == 'project': del b['project']
            if name == 'host': b['connections'][0]['host_id'] = 'foreign'
            if name == 'output-inode': b['outputs'][0]['root_inode'] += 1
            if name == 'implicit-start': b['start_contract']['implicit_start_on_approval'] = True
            if name == 'input-hash': b['inputs'][0]['sha256'] = 'a' * 64
            if name == 'oversize': changed['padding'] = 'x' * MAX_FACTORY_REVIEW_BYTES
            try:
                review(changed)
            except (BoundaryRefusal, KeyError):
                passed('full private review refuses ' + name)
            else:
                raise AssertionError(name)
        program = directory / 'generated-acp-unit.py'; program.write_text(GENERATED_ACP)
        result = subprocess.run([sys.executable, str(program), '--version'], capture_output=True, timeout=2, check=True)
        assert result.stdout == b'Generated account-free Factory ACP 1\n' and result.stderr == b''
        passed('actual generated program version-only process exits; no MCP/Core/model started')
    return {'dimension': 'review DTO derivative + source compile + generated version-only unit; NOT actual Core/browser/Factory',
        'status': 'passed', 'count': len(rows), 'checks': rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--fixture-self-check', action='store_true')
    parser.add_argument('--case', choices=['normal', 'cancel', 'unknown'])
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--candidate-manifest', type=Path)
    parser.add_argument('--batond', type=Path)
    parser.add_argument('--frontend', type=Path)
    parser.add_argument('--chromium', default='/usr/bin/chromium')
    args = parser.parse_args()
    assert sum([args.self_check, args.fixture_self_check, bool(args.case)]) <= 1
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    attribution = {'harness_sha256': sha(Path(__file__).read_bytes()), 'support_sha256': sha(SUPPORT_PATH.read_bytes()), 'schema_required': 13}
    if args.self_check:
        report = {**attribution, **boundary_self_check()}
        (output / 'boundary-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.fixture_self_check:
        report = {**attribution, **fixture_self_check()}
        (output / 'fixture-unit-checks.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'status': report['status'], 'count': report['count']})); return 0
    if args.case:
        try:
            prepared = preflight(args)
        except (AssertionError, OSError, ValueError, KeyError) as failure:
            report = {**attribution, 'status': 'preflight_refused', 'actual_runtime': 'not_run',
                'reason': str(failure), 'no_process_token_read_or_listener': True}
            (output / 'preflight-refusal.json').write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps(report)); return 2
        return runtime_driver(args, output, attribution, prepared)
    report = {**attribution, 'status': 'pending_dependencies', 'actual_runtime': 'not_run', 'pending': list(PENDING)}
    (output / 'pending-runtime.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report)); return 2


if __name__ == '__main__':
    raise SystemExit(main())
