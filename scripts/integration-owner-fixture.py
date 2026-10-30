#!/usr/bin/env python3
"""Private generated Core seed and OS loop for Native integration acceptance.

Imported only by the explicit Integration runner. No schema/receipt is inserted
by SQL. All resource permissions come from actual private Core proposals.
"""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]


def require(value, message='Generated integration fixture refused'):
    if not value:
        raise ValueError(message)


class ResourceClient:
    PROPOSALS = {'connections.propose': 'connection', 'grants.propose': 'grant',
                 'goal_scopes.propose': 'goal', 'goal_runs.propose': 'goal_execution',
                 'command_validation_policies.propose': 'command_validation'}
    METHODS = frozenset({'system.snapshot', 'connections.list', 'projects.import',
        'tasks.create', 'tasks.get', 'workspaces.resolve', 'workspaces.prepare_task_git', 'workspaces.list',
        'goal_plans.propose', 'goal_runs.get', 'sessions.get', 'integration_context.get',
        'integration_policies.propose'})

    def __init__(self, runtime, owner):
        self.runtime, self.owner, self.proposals, self.transcript = runtime, owner, {}, []

    def call(self, method, params, key=None, revision=None):
        if method == 'approvals.finalize':
            identifier = params.get('proposal_id') if type(params) is dict else None
            expected = self.proposals.get(identifier)
            require(expected is not None and self.owner.json_exact(params, {'proposal_id': identifier})
                    and self.owner.json_exact(revision, expected['revision']) and key == 'proposal:' + identifier,
                    'Only this generated connection/Grant/Goal/Command resource can be bootstrap-finalized; NEVER integration')
        else:
            require(method in self.METHODS or method in self.PROPOSALS, 'Not a fixed Integration fixture preparation method')
        frame = {'api_version': 1, 'request_id': str(uuid.uuid4()), 'method': method, 'params': params,
                 'token': (self.runtime / 'session.token').read_text()}
        if key is not None: frame['idempotency_key'] = key
        if revision is not None: frame['expected_revision'] = revision
        wire = json.dumps(frame, allow_nan=False).encode() + b'\n'
        require(len(wire) <= 262144, 'Complete private RPC exceeds unchanged Core request bound')
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(20)
            stream.connect(str(self.runtime / 'batond.sock'))
            stream.sendall(wire)
            with stream.makefile('rb') as reader:
                raw = reader.readline(4_194_305)
        require(0 < len(raw) <= 4_194_304 and raw.endswith(b'\n'), 'Incomplete private Core reply')
        value = json.loads(raw)
        self.transcript.append({'method': method, 'params': params, 'key': key, 'revision': revision, 'response': value})
        require('error' not in value and 'result' in value, 'Actual generated resource request failed: ' + method)
        result = value['result']
        if method in self.PROPOSALS:
            require(result['kind'] == self.PROPOSALS[method] and result['consumed'] is False)
            require(len(self.owner.compact(result)) <= 16384, 'Original resource review cap unchanged')
            self.proposals[result['id']] = result
        if method == 'integration_policies.propose':
            require(result['kind'] == 'integration' and result['consumed'] is False)
            # Deliberately never place the target in bootstrap-finalize permits.
            require(result['id'] not in self.proposals)
        return result

    def approved(self, method, params, key, revision=None):
        require(method in self.PROPOSALS, 'Target integration cannot be host-auto-finalized')
        proposal = self.call(method, params, key, revision)
        return self.call('approvals.finalize', {'proposal_id': proposal['id']}, 'proposal:' + proposal['id'], proposal['revision'])


def wait(call, predicate, seconds=5):
    deadline = time.monotonic() + seconds
    while True:
        value = call()
        if predicate(value): return value
        if time.monotonic() >= deadline: raise TimeoutError('Original bounded generated fixture observation expired')
        time.sleep(.05)


# Fixed account-free ACP. It consumes the exact legacy TaskRequest and writes
# ONLY the one approved ignored report. No arbitrary model text/commands.
PROVIDER = r'''#!/usr/bin/env python3
import hashlib,json,os,sys
from pathlib import Path
if sys.argv[1:]==['--version']:
 print('AgentBaton integration Owner generated ACP 1.0');sys.exit(0)
assert not sys.argv[1:]
def send(v):print(json.dumps(v),flush=True)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-integration-owner-resource'}})
 elif m=='session/prompt':
  v=json.loads(r['params']['prompt'][0]['text'])
  assert set(v)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location'}
  assert v['schema_version']==1 and v['kind']=='agentbaton_goal_task_request' and v['goal']=='goal-report'
  assert v['workspace']['canonical_root']==os.getcwd() and v['workspace']['id']==v['node']['workspace_id']
  assert v['node']['id']=='produce-report' and len(v['inputs'])==len(v['outputs'])==1
  assert v['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
  assert v['budget']['limits']==v['node']['budget'] and v['budget']['reservation']==v['node']['reservation']
  binding=v['inputs'][0];artifact=binding['artifact'];assert artifact['relative_path']=='inputs/file'
  assert Path('inputs/file').read_bytes()==b'baseline' and hashlib.sha256(b'baseline').hexdigest()==binding['sha256']
  output=v['outputs'][0]['artifact'];assert output['artifact_id']=='report' and output['relative_path']=='reports/report.txt'
  Path('reports').mkdir(exist_ok=True);Path('reports/report.txt').write_bytes(b'fixed report')
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}})
 else:raise AssertionError('Unknown generated protocol method')
'''


def load_graph_helpers():
    spec = importlib.util.spec_from_file_location('_integration_seed_goal_helpers', ROOT / 'apps/desktop/tests/goal-graph-real-ipc.py')
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def tool_copy(source, destination, *, fixed_system_program=False):
    source = Path(source).absolute()
    fields = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mode, s.st_mtime_ns, s.st_ctime_ns, s.st_uid)
    before = source.lstat()
    require(type(fixed_system_program) is bool)
    allowed_uids = (0, os.geteuid()) if fixed_system_program else (os.geteuid(),)
    require(stat.S_ISREG(before.st_mode) and before.st_uid in allowed_uids and before.st_mode & 0o111
            and not before.st_mode & 0o022 and 0 < before.st_size <= 268435456)
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        require(fields(before) == fields(os.fstat(fd)))
        with os.fdopen(os.dup(fd), 'rb') as stream: raw = stream.read(268435457)
        require(len(raw) == before.st_size and fields(before) == fields(os.fstat(fd)) == fields(source.lstat()))
    finally: os.close(fd)
    with os.fdopen(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o500), 'wb') as stream:
        stream.write(raw); stream.flush(); os.fsync(stream.fileno()); os.fchmod(stream.fileno(), 0o500)
    require(destination.stat().st_nlink == 1 and destination.read_bytes() == raw)
    return {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


@contextmanager
def bootstrap(owner, daemon_path, companion_path, output):
    """Actual Core only, when Root explicitly runs it. No OS success fabricated."""
    installed = output / 'private-tools'; installed.mkdir(mode=0o700)
    tools = {name: tool_copy(source, installed / name) for name, source in [('batond', daemon_path), ('baton', companion_path)]}
    private = output / 'bootstrap'; private.mkdir(mode=0o700)
    daemon = None; client = None; generated = {}; failure = None
    with tempfile.TemporaryDirectory(prefix='ab-int-own-', dir=Path('/tmp').resolve()) as temporary:
        root = Path(temporary).resolve(); root.chmod(0o700)
        runtime = root / 'r'; runtime.mkdir(mode=0o700)
        env = os.environ.copy(); env['BATON_RUNTIME_DIR'] = str(runtime)
        source = root / 'source'; source.mkdir(mode=0o700)
        (source/'inputs').mkdir(); (source/'inputs/file').write_bytes(b'baseline')
        (source/'.gitignore').write_text('reports/\n')
        git_env = {k: v for k, v in env.items() if not k.startswith('GIT_')}
        git_env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null', GIT_TERMINAL_PROMPT='0', GIT_NO_LAZY_FETCH='1')
        def git(*argv):
            return subprocess.check_output(['git', '-C', str(source), '-c', 'core.hooksPath=/dev/null',
                '-c', 'init.templateDir=', '-c', 'user.name=Integration Fixture', '-c', 'user.email=fixture@example.invalid', *argv],
                env=git_env, timeout=5, text=True).strip()
        git('init', '-q', '--initial-branch=main'); git('add', '.gitignore', 'inputs/file'); git('commit', '-qm', 'Generated integration approval input')
        target_ref = 'refs/heads/native-integration-target'; git('branch', 'native-integration-target')
        target_head = git('rev-parse', target_ref)
        provider = root / 'provider'; provider.write_text(PROVIDER); provider.chmod(0o700)
        # Private copy avoids changing or relying on multiply linked system-bin
        # metadata; its identity is approved normally by Command policy.
        validator_program = root / 'fixed-sh'; tool_copy(Path('/bin/sh').resolve(strict=True), validator_program, fixed_system_program=True)
        owner.enable_owned_subreaper()
        logfd = os.open(private/'daemon.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        try:
            with os.fdopen(logfd, 'wb', buffering=0) as log:
                daemon = subprocess.Popen([str(installed/'batond')], env=env, stdout=log, stderr=subprocess.STDOUT,
                                          start_new_session=True, umask=0o077)
                client = ResourceClient(runtime, owner)
                wait(lambda: (runtime/'session.token').is_file() and (runtime/'batond.sock').exists(), bool, 10)
                snapshot = client.call('system.snapshot', {})
                require(type(snapshot['database_schema_version']) is int and snapshot['database_schema_version'] == 17, 'Actual schema17 Core required')
                project = client.call('projects.import', {'path': str(source), 'name': 'Native independent integration generated'}, 'integration-owner:project')
                task = client.call('tasks.create', {'project_id': project['id'], 'goal': 'Generated independent integration approval'}, 'integration-owner:task')
                preview = client.call('workspaces.resolve', {'task_id': task['id'], 'expected_task_revision': task['revision'],
                    'expected_project_revision': project['revision'], 'host_id': 'local'})
                require(preview['state'] == 'needs_git_workspace' and preview['execution_authorized'] is False)
                identity = preview['source']; source_pin = {k: identity[k] for k in ['canonical_root', 'device', 'inode']}
                source_pin.update(common_dir=identity['kind']['common_dir'], head=identity['kind']['head'])
                receipt = client.call('workspaces.prepare_task_git', {'task_id': task['id'], 'expected_spec_revision': 1,
                    'expected_project_revision': project['revision'], 'host_id': 'local', 'expected_source_pin': source_pin,
                    'preparation_key': preview['preparation_key']}, 'integration-owner:prepare', task['revision'])
                require(receipt['identity_revalidated'] is True and receipt['execution_authorized'] is False and receipt['binding']['state'] == 'ready')
                task = client.call('tasks.get', {'task_id': task['id']})
                workspace = next(w for w in client.call('workspaces.list', {'project_id': project['id']}) if w['id'] == receipt['binding']['workspace_id'])
                connection = client.approved('connections.propose', {'name': 'Generated integration ACP', 'provider': 'custom_acp', 'executable': str(provider), 'arguments': []}, 'integration-owner:connection')
                wait(lambda: client.call('connections.list', {}), lambda rows: any(c['id'] == connection['id'] and c['probe_state'] == 'ready' for c in rows))
                grant = client.approved('grants.propose', {'project_id': project['id'], 'workspace_id': workspace['id'], 'connection_id': connection['id'],
                    'max_attempts': 8, 'max_concurrency': 1, 'max_duration_ms': 300000}, 'integration-owner:grant')
                graph = load_graph_helpers()
                selection = graph.scope_selection(task, grant, count=1, max_nodes=2)
                scope = client.approved('goal_scopes.propose', selection, 'integration-owner:scope')
                plan = graph.scoped_agent_plan(scope, 1, 'goal-report')
                client.call('goal_plans.propose', {'scope_id': scope['id'], 'plan_json': json.dumps(plan)}, 'integration-owner:plan', 0)
                run = client.approved('goal_runs.propose', {'scope_id': scope['id'], 'expected_plan_revision': 1,
                    'expected_ledger_revision': client.call('system.snapshot', {})['sequence']}, 'integration-owner:bootstrap')
                detail = wait(lambda: client.call('goal_runs.get', {'run_id': run['id']}), lambda d: d['run']['state'] == 'awaiting_acceptance')
                node = detail['nodes'][0]
                require(node['stop_confirmed'] is True and detail['task_accepted'] is False)
                require(client.call('sessions.get', {'session_id': node['session_id']})['state'] == 'stopped')
                require((Path(workspace['root'])/'reports/report.txt').read_bytes() == b'fixed report')
                command = {'session_id': node['session_id'], 'inputs': selection['inputs'], 'outputs': [selection['outputs'][0]['artifact']],
                    'executable': str(validator_program), 'mode': 'native_executable', 'script': None,
                    'arguments': ['-c', 'test -s reports/report.txt'], 'environment': {}, 'expected_exit_codes': [0],
                    'wall_time_ms': 500, 'cleanup_time_ms': 500, 'stdout_limit': 128, 'stderr_limit': 128,
                    'require': {k: False for k in ['filesystem_isolation', 'network_isolation', 'process_tree_containment',
                        'execution_time_input_immutability', 'auxiliary_source_immutability', 'sealed_executable_image']}}
                definition = client.approved('command_validation_policies.propose', command, 'integration-owner:definition')
                context = client.call('integration_context.get', {'task_id': task['id'], 'goal_scope_id': scope['id']})
                reference = {'kind': 'command', 'policy_id': definition['id'], 'revision': definition['revision']}
                require(any(v['reference'] == reference for v in context['available_validators']))
                reservation = lambda runtime_ms, output_bytes: {'runtime_ms': runtime_ms, 'output_bytes': output_bytes, 'tokens': None, 'cost_microunits': None}
                intent = {'task_id': task['id'], 'expected_task_revision': context['task_revision'], 'expected_spec_revision': 1,
                    'goal_scope_id': scope['id'], 'expected_scope_revision': scope['authority_revision'], 'host_id': 'local',
                    'target_ref': target_ref, 'expected_target_head': target_head, 'max_rounds': 1,
                    'on_target_advance': 'block', 'ttl_ms': 300000,
                    'stage_reservations': {'preparation': reservation(1000, 1024),
                        'validators': [{'validator': reference, 'reservation': reservation(1500, 256)}], 'cas': reservation(500, 1024)}}
                proposal = client.call('integration_policies.propose', intent, owner.INTEGRATION_KEY)
                require(owner.json_exact(proposal['snapshot']['intent'], intent))
                generated.update(runtime=runtime, env=env, proposal=proposal,
                    invoke={'selection': intent, 'idempotencyKey': owner.INTEGRATION_KEY}, tools=tools)
                yield generated
                require(git('rev-parse', target_ref) == target_head, 'Policy-only approval must not update target branch')
        except BaseException as error:
            failure = {'type': type(error).__name__, 'message': str(error)}
            raise
        finally:
            try: owner.save(private/'core-private-transcript.json', client.transcript if client else [])
            finally:
                cleanup = owner.stop_owned(daemon, 'daemon'); generated['daemon_cleanup'] = cleanup
                owner.save(private/'daemon-cleanup.json', cleanup)
                owner.save(private/'state.json', {'original_failure': failure, 'cleanup': cleanup, 'tools': tools})
                if not cleanup['ok'] and failure is None:
                    raise RuntimeError('Owned generated daemon did not stop cleanly')


def database(runtime):
    """One actual readonly transaction; bound all selected raw bytes first."""
    spec = importlib.util.spec_from_file_location('_integration_owner_table_names', ROOT / 'scripts/prepare-schema16-migration-fixture.py')
    helper = importlib.util.module_from_spec(spec); spec.loader.exec_module(helper)
    tables = tuple(sorted((*helper.SCHEMA_TABLES, 'integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences')))
    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro', uri=True) as db:
        db.execute('PRAGMA query_only=ON'); db.execute('PRAGMA trusted_schema=OFF'); db.execute('BEGIN')
        require(db.execute('PRAGMA user_version').fetchone()[0] == 17)
        require(tuple(r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")) == tables)
        columns, sizes, remaining, total_rows = {}, {}, 8_388_608, 0
        for table in tables:
            columns[table] = [r[1] for r in db.execute('PRAGMA table_info("'+table+'")')]
            require(columns[table] and len(columns[table]) <= 64)
            expression = '+'.join('coalesce(length(CAST("'+c.replace('"','""')+'" AS BLOB)),0)' for c in columns[table])
            count, size = db.execute('SELECT count(*),coalesce(sum('+expression+'),0) FROM "'+table+'"').fetchone()
            remaining -= size; total_rows += count
            require(remaining >= 0 and total_rows <= 100000, 'Private readonly raw history exceeds fixture bound')
            sizes[table] = count
        encode_cell = lambda v: {'sqlite_blob_hex': v.hex()} if type(v) is bytes else v
        rows = {table: [[encode_cell(v) for v in row] for row in db.execute('SELECT rowid,* FROM "'+table+'" ORDER BY rowid')] for table in tables}
        db.rollback()
    return {'read_only': True, 'schema_version': 17, 'columns': columns, 'counts': sizes, 'rows': rows,
            'sequence': max((r[1] for r in rows['events']), default=0)}


def row_objects(value, table):
    return [dict(zip(['rowid', *value['columns'][table]], row)) for row in value['rows'][table]]


def pending_error(owner, value, before):
    return None if owner.json_exact(value, before) else 'Pending integration review changed actual private database history'


def approved_error(owner, value, before, proposal, response):
    if (value['schema_version'] != 17 or response.get('error') is not None or type(response.get('result')) is not dict
            or value['sequence'] != before['sequence'] + 2 or value['columns'] != before['columns']):
        return 'Missing exact integration response/schema/two-event transaction'
    policy = response['result']
    expected_policy = {'id': proposal['id'], 'approval_id': proposal['id'], 'revision': 1,
        'selection': proposal['snapshot']['intent'], 'bindings': proposal['snapshot']['bindings'],
        'snapshot_digest': owner.digest(proposal['snapshot']), 'revoked': False}
    if any(not owner.json_exact(policy.get(k), v) for k, v in expected_policy.items()):
        return 'Target policy differs from complete approved intent/bindings/digest'
    exception_tables = {'approval_proposals', 'integration_policies', 'events', 'idempotency', 'sqlite_sequence'}
    if any(not owner.json_exact(before['rows'][t], value['rows'][t]) for t in value['rows'] if t not in exception_tables):
        return 'Integration approval changed existing execution/Task/Goal/Factory history'
    for table in ['integrations', 'integration_attempts', 'integration_ref_fences']:
        if value['counts'][table] != 0 or before['counts'][table] != 0:
            return 'Policy approval cannot execute integration'
    if before['counts']['integration_policies'] != 0 or value['counts']['integration_policies'] != 1:
        return 'Expected exactly one new integration policy'
    policies = row_objects(value, 'integration_policies')
    if (policies[0]['id'] != proposal['id'] or not owner.json_exact(json.loads(policies[0]['document']), policy)
            or policies[0]['task_id'] != policy['selection']['task_id'] or policies[0]['spec_revision'] != policy['selection']['expected_spec_revision']
            or policies[0]['goal_scope_id'] != policy['selection']['goal_scope_id'] or policies[0]['approval_id'] != proposal['id']
            or type(policies[0]['revision']) is not int or policies[0]['revision'] != 1):
        return 'Actual persisted policy/result/SQL identity mismatch'
    old_proposals = row_objects(before, 'approval_proposals'); new_proposals = row_objects(value, 'approval_proposals')
    if len(old_proposals) != len(new_proposals): return 'Approval row count changed'
    expected_proposal = dict(proposal, consumed=True)  # Core c932 keeps original proposal CAS.
    for old, new in zip(old_proposals, new_proposals):
        if old['id'] != proposal['id']:
            if not owner.json_exact(old, new): return 'Other resource proposal changed'
        elif (old['rowid'] != new['rowid'] or old['id'] != new['id']
              or not owner.json_exact(json.loads(new['document']), expected_proposal)
              or not owner.json_exact(json.loads(new['result']), policy)):
            return 'Target proposal original revision/consumed/result binding mismatch'
    old_keys, new_keys = before['rows']['idempotency'], value['rows']['idempotency']
    if len(new_keys) not in (len(old_keys), len(old_keys)+1) or not owner.json_exact(new_keys[:len(old_keys)], old_keys):
        return 'Existing idempotency history changed or unrelated keys were inserted'
    old_events, new_events = before['rows']['events'], value['rows']['events']
    if len(new_events) != len(old_events)+2 or not owner.json_exact(new_events[:-2], old_events):
        return 'Original event history changed or unrelated events appeared'
    events = row_objects(value, 'events')[-2:]
    wanted = [dict(sequence=before['sequence']+1, aggregate_id=proposal['id'], revision=proposal['revision'],
        kind='approval.confirmed', source='local_owner_confirmation',
        payload={'kind':'integration','proposal_id':proposal['id'],'consumed':True}),
        dict(sequence=before['sequence']+2, aggregate_id=proposal['id'], revision=1,
        kind='integration.policy_approved', source='integration_core', payload={
            'snapshot_digest':policy['snapshot_digest'], 'owner_sequence':before['sequence']+1,
            'scope_digest':policy['bindings']['goal_scope_digest'], 'binding_digest':policy['bindings']['binding_digest'],
            'automatic_watch_armed':True})]
    for row, expected in zip(events, wanted):
        event = json.loads(row['document'])
        if not owner.json_exact(event.get('sequence'), 0) or row['sequence'] != expected['sequence']:
            return 'SQL/journal sequence mismatch'
        if any(not owner.json_exact(event.get(k), v) for k, v in expected.items() if k != 'sequence'):
            return 'Unique integration Owner/policy event payload/source/revision mismatch'
    return None


def run(owner, binary, daemon_path, companion_path, output, cancel_at=None):
    require(sys.platform == 'darwin', 'NOT_RUN: actual macOS NativeParent required')
    output = output.absolute(); output.mkdir(mode=0o700, parents=True, exist_ok=False); output = output.resolve()
    primary = None
    with owner.native_binary_witness(binary, output) as native:
        with bootstrap(owner, daemon_path, companion_path, output) as generated:
            proposal, invoke, runtime = generated['proposal'], generated['invoke'], generated['runtime']
            source = (ROOT/'apps/desktop/src-tauri/src/integration_approval.rs').read_text()
            before = database(runtime)
            require(all(before['counts'][t] == 0 for t in owner.INTEGRATION_TABLES))
            results = []
            for phase in ['cancel', 'approve']:
                phase_out = output/phase; phase_out.mkdir(mode=0o700)
                fixture, pages, contexts = owner.integration_fixture(proposal, invoke, runtime, phase_out, before, source, phase, cancel_at)
                require(pending_error(owner, database(runtime), before) is None)
                owner.save(phase_out/'fixture.json', fixture)
                owner.save(phase_out/'expected-display.json', {'pages': pages, 'contexts': contexts})
                owner.save(phase_out/'before.json', before)
                app = None; failure = None; permits = set()
                limit = fixture['cancel_at']+1 if phase == 'cancel' else len(pages)
                try:
                    require(owner.native_binary_check(native)['stable'])
                    app = subprocess.Popen([str(binary.resolve()), '--integration', str(phase_out/'fixture.json')],
                        env=generated['env'], start_new_session=True, umask=0o077)
                    deadline = time.monotonic()+85
                    while not owner.exited_without_reaping(app) and time.monotonic() < deadline:
                        for index in range(limit):
                            key = f'{phase}-page-{index+1:04}'
                            ready = phase_out/(key+'-sheet-ready.json')
                            if ready.exists() and index not in permits:
                                observed = json.loads(ready.read_text())
                                require(owner.integration_presentation_error(observed, fixture, pages, contexts, index) is None,
                                        'Native complete page/text/layout/nonce mismatch')
                                require(index == 0 or index-1 in permits, 'Native integration page order mismatch')
                                require(pending_error(owner, database(runtime), before) is None, 'Pre-click database must remain exact')
                                owner.save(phase_out/(key+'-database-permit.json'), {
                                    'phase':phase,'presentation_index':index,'presentation_count':len(pages),
                                    'proposal_id':proposal['id'],'proposal_revision':proposal['revision'],
                                    'before_click_sequence':before['sequence'],'review_nonce':fixture['review_nonce'],
                                    'integration_page':contexts[index]})
                                permits.add(index)
                        time.sleep(.05)
                    require(owner.exited_without_reaping(app) and permits == set(range(limit)), 'Original 85s OS/page-permit bound expired')
                    reply = json.loads((phase_out/(phase+'-reply.json')).read_text())
                    after = database(runtime); owner.save(phase_out/'after.json', after)
                    require((pending_error(owner, after, before) if phase == 'cancel' else approved_error(owner, after, before, proposal, reply)) is None)
                    if phase == 'cancel':
                        require(reply.get('error',{}).get('code') == 'PERMISSION_DENIED'
                                and reply.get('error',{}).get('side_effect_state') == 'not_started' and 'result' not in reply)
                    actual = json.loads((phase_out/'owner-dialog.json').read_text())
                    require(actual.get('status') == 'os_button_paths_completed_database_check_passed')
                    require(owner.integration_database_error(actual.get('integration_approval_database'), phase, before['sequence']) is None)
                    require(actual['integration_approval_database']['after']['sequence'] == after['sequence'])
                    # These stay PRIVATE. Actual visual review is still required,
                    # never replaced by a pixel hash or this signature check.
                    images = {}
                    for index in range(limit):
                        name=f'{phase}-page-{index+1:04}-sheet.png'; raw=(phase_out/name).read_bytes()
                        require(len(raw)>=64 and raw[:8]==b'\x89PNG\r\n\x1a\n' and raw[12:16]==b'IHDR'
                                and min(int.from_bytes(raw[16:20],'big'),int.from_bytes(raw[20:24],'big'))>=20)
                        images[name]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'visual_review':'pending'}
                    results.append({'phase':phase,'pages_observed':limit,'total_pages':len(pages),'images':images,
                                    'integration_approval_database':actual['integration_approval_database']})
                except BaseException as error:
                    failure={'type':type(error).__name__,'message':str(error)}
                    try: owner.save(phase_out/'original-failure.json',failure)
                    except Exception as secondary: print('SECONDARY integration diagnostic failure: '+type(secondary).__name__, file=sys.stderr)
                    raise
                finally:
                    owner._finish_factory_app(app, native, phase_out, failure)
        require(generated['daemon_cleanup']['ok'])
    owner.save(output/'result.json',{'status':'actual_integration_cancel_and_same_key_approval_visual_review_pending',
        'phases':results,'daemon_cleanup':generated['daemon_cleanup'], 'schema':17,
        'dimension':'macos_own_process_real_sheet','real_provider':'not_run','human_owner_approval':'not_claimed',
        'harness_binary_sha256':native['sha256'],'tools':generated['tools']})
