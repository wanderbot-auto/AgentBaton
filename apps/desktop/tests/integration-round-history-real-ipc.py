#!/usr/bin/env python3
"""Optional real Core17/Chromium three-round history fixture; only --run executes.

Reuses the unchanged two-case driver's finite HTTP/private Owner boundary. The
new fixture alone selects 12 attempts, 20s shared runtime, 30s Grant/permission,
2s stages and the existing 500+500ms owned native shell/FIFO protocol. It is not
production Native/macOS, a real Provider or a new product terminal entry.
"""
from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import traceback

BASE_PATH = Path(__file__).with_name('integration-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('_integration_three_round_base', BASE_PATH)
D = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(D)
ROOT = D.ROOT
require = D.require
SCRIPT = ('test -s reports/report.txt || exit 21; printf \'%s\\n\' "$$" > "$1" || exit 22; '
    'IFS= read -r reply < "$2" || exit 23; [ "$reply" = continue ] || exit 24')


def own_source(expected):
    relative = 'apps/desktop/tests/integration-round-history-real-ipc.py'
    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == expected)
    require(subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=no'], cwd=ROOT) == b'')
    raw = subprocess.check_output(['git', 'show', 'HEAD:' + relative], cwd=ROOT)
    require(raw == Path(__file__).read_bytes(), 'Three-round driver must be the clean tracked source')
    return {'sha256': D.digest(raw), 'source': expected, 'tracked_clean': True}


def selected(context, target, reference):
    value = D.expected_selection(context, target, reference)
    value.update(max_rounds=3, on_target_advance='bounded_descendant_reprepare', ttl_ms=30000)
    for reservation in [value['stage_reservations']['preparation'], value['stage_reservations']['cas'],
            value['stage_reservations']['validators'][0]['reservation']]:
        reservation['runtime_ms'] = 2000
    return value


def headroom(context):
    for domain in ('root', 'project'):
        limits = context['bindings'][domain + '_budget']; usage = context['shared_usage'][domain]
        require(limits['attempts'] == 12 and limits['runtime_ms'] == 20000 and limits['output_bytes'] == 8192)
        require(usage['active'] == 0 and usage['attempts'] == 1
            and D.integer(usage['runtime_ms']) and D.integer(usage['output_bytes']))
        require(usage['attempts'] + 9 <= limits['attempts']
            and usage['runtime_ms'] + 18000 <= limits['runtime_ms']
            and usage['output_bytes'] + 6912 <= limits['output_bytes'],
            'Real original Graph usage leaves insufficient conservative three-round headroom')


class RoundHost(D.Host):
    """Only this new fixture's original proposals are selected differently."""
    def __init__(self, runtime, gate):
        super().__init__(runtime)
        self.gate = gate

    def approved(self, method, params, key, kind, revision=None):
        params = copy.deepcopy(params)
        if method == 'grants.propose':
            require(params['max_attempts'] == 8 and params['max_duration_ms'] == 30000)
            params['max_attempts'] = 12
        elif method == 'goal_scopes.propose':
            # Bootstrap aliases the same original budget in both domains.
            # Validate both original values before mutating either deep copy.
            for domain in ('root_budget', 'project_budget'):
                require(params[domain]['attempts'] == 8 and params[domain]['runtime_ms'] == 20000
                    and params[domain]['output_bytes'] == 8192)
            for domain in ('root_budget', 'project_budget'):
                params[domain]['attempts'] = 12
        elif method == 'command_validation_policies.propose':
            require(params['wall_time_ms'] == params['cleanup_time_ms'] == 500
                and params['expected_exit_codes'] == [0] and params['mode'] == 'native_executable'
                and params['script'] is None)
            params['arguments'] = ['-c', SCRIPT, 'bounded-reprepare',
                str(self.gate.ready_path), str(self.gate.release_path)]
        return super().approved(method, params, key, kind, revision)


def proc_identity(pid):
    """Linux kernel identity only; no argv scan or signal authority."""
    require(D.integer(pid, 2))
    path = Path('/proc') / str(pid)
    raw = (path / 'stat').read_bytes()
    require(len(raw) <= 8192)
    end = raw.rfind(b')'); require(end > 0 and raw[:raw.index(b' ')].isdigit())
    fields = raw[end + 2:].split(); require(len(fields) >= 20)
    identity = {'pid': int(raw[:raw.index(b' ')]), 'parent': int(fields[1]),
        'group': int(fields[2]), 'birth': fields[19].decode('ascii'), 'uid': path.stat().st_uid}
    require(identity['pid'] == pid and fields[0] != b'Z')
    return identity


def original_gone(identity):
    try:
        current = proc_identity(identity['pid'])
    except FileNotFoundError:
        return True
    # A zombie with the original birth is not an absent/reaped original child.
    return current['birth'] != identity['birth']


class RoundGate:
    """One owned thread observes real FIFO readiness and moves the exact ref.

    No Core request, browser wait, IPC retry or deadline renewal occurs after a
    ready marker. Fixed generated-repository Git children are owned/reaped in
    this thread. Original Command and stage clocks remain independently active.
    """
    def __init__(self, root):
        self.root = root
        self.ready_path, self.release_path = (root / n for n in ('round-ready.fifo', 'round-release.fifo'))
        self.files = []
        try:
            for path in (self.ready_path, self.release_path):
                os.mkfifo(path, 0o600)
                self.files.append(os.open(path, os.O_RDWR | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC))
        except BaseException:
            for fd in self.files: os.close(fd)
            self.files = []
            raise
        self.pins = [(os.fstat(fd).st_dev, os.fstat(fd).st_ino) for fd in self.files]
        self.stop = threading.Event(); self.thread = None; self.error = None
        self.records, self.heads = [], []

    def check(self):
        for fd, path, identity in zip(self.files, (self.ready_path, self.release_path), self.pins):
            held, current = os.fstat(fd), path.lstat()
            require(stat.S_ISFIFO(current.st_mode) and (held.st_dev, held.st_ino) == identity
                == (current.st_dev, current.st_ino) and current.st_nlink == 1
                and current.st_uid == held.st_uid == os.geteuid()
                and stat.S_IMODE(current.st_mode) == 0o600)

    def git(self, project, args, deadline):
        require(not self.stop.is_set() and time.monotonic() < deadline)
        env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
        env.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1',
            GIT_TERMINAL_PROMPT='0', GIT_NO_LAZY_FETCH='1')
        child = None
        with tempfile.TemporaryFile(dir=self.root) as out, tempfile.TemporaryFile(dir=self.root) as err:
            try:
                child = subprocess.Popen(['git', '-C', str(project), '-c', 'user.name=Generated integration browser',
                    '-c', 'user.email=fixture@example.invalid', '-c', 'core.hooksPath=/dev/null',
                    '-c', 'commit.gpgsign=false', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false', *args],
                    env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=err)
                while child.poll() is None:
                    require(not self.stop.is_set() and time.monotonic() < deadline,
                        'Original 500ms Host advance bound expired')
                    time.sleep(.001)
                require(child.returncode == 0 and time.monotonic() < deadline)
                require(os.fstat(out.fileno()).st_size <= 128 and os.fstat(err.fileno()).st_size <= 4096)
                out.seek(0); err.seek(0); raw, diagnostic = out.read(129), err.read(4097)
                require(len(raw) <= 128 and diagnostic == b'' and time.monotonic() < deadline)
                return raw.decode('ascii').strip()
            finally:
                if child is not None:
                    if child.poll() is None: child.kill()
                    child.wait()

    def run(self, fixture, daemon_pid):
        try:
            self.heads = [fixture['target']]
            for round_number in range(1, 4):
                waiting = time.monotonic() + 5
                raw = b''
                while not raw.endswith(b'\n'):
                    require(not self.stop.is_set() and time.monotonic() < waiting,
                        'Original five-second FIFO readiness observation expired')
                    self.check()
                    try: part = os.read(self.files[0], 64)
                    except BlockingIOError: part = b''
                    raw += part
                    require(len(raw) <= 32)
                    if not raw.endswith(b'\n'): time.sleep(.001)
                deadline = time.monotonic() + .5
                require(re.fullmatch(rb'[0-9]+\n', raw) is not None)
                identity = proc_identity(int(raw[:-1]))
                require(identity['parent'] == daemon_pid and identity['group'] == identity['pid']
                    and identity['uid'] == os.geteuid() and os.getpgid(identity['pid']) == identity['pid'])
                expected = self.heads[-1]
                if round_number < 3:
                    tree = self.git(fixture['source'], ['rev-parse', expected + '^{tree}'], deadline)
                    next_head = self.git(fixture['source'], ['commit-tree', tree, '-p', expected,
                        '-m', 'owned UI descendant round ' + str(round_number)], deadline)
                    require(re.fullmatch('[a-f0-9]{40}', next_head) is not None and next_head != expected)
                    self.git(fixture['source'], ['merge-base', '--is-ancestor', expected, next_head], deadline)
                    self.git(fixture['source'], ['update-ref', '--no-deref', D.TARGET, next_head, expected], deadline)
                    require(self.git(fixture['source'], ['rev-parse', D.TARGET], deadline) == next_head)
                    self.heads.append(next_head)
                require(not self.stop.is_set() and proc_identity(identity['pid']) == identity and time.monotonic() < deadline)
                self.check(); require(os.write(self.files[1], b'continue\n') == 9)
                require(time.monotonic() < deadline)
                self.records.append({'round': round_number, 'identity': identity,
                    'expected_head': expected, 'advanced': round_number < 3})
        except BaseException as error:
            self.error = {'kind': type(error).__name__, 'traceback': traceback.format_exc()}

    def start(self, fixture, daemon_pid):
        require(self.thread is None)
        self.thread = threading.Thread(target=self.run, args=(fixture, daemon_pid), name='owned-three-round-fifo')
        self.thread.start()

    def healthy(self):
        require(self.error is None, 'Owned target-advance observer failed; see private gate evidence')

    def close(self):
        self.stop.set()
        if self.thread is not None:
            self.thread.join(5); require(not self.thread.is_alive(), 'Owned FIFO/Git thread did not join')
        for fd in self.files: os.close(fd)
        self.files = []
        return {'thread_joined': self.thread is None or not self.thread.is_alive()}


def terminal_evidence(host, fixture, accepted, policy, done, gate):
    gate.healthy()
    require(len(gate.records) == len(gate.heads) == 3)
    run, attempts = done['run'], done['attempts']
    require(run['state'] == 'confirmed_local_ref' and run['round'] == 3
        and run['policy_id'] == policy['id'] and run['policy_revision'] == 1
        and run['expected_target_head'] == gate.heads[2] and run['target_ref'] == D.TARGET
        and run['acceptance_ref'] == accepted['task_acceptance']['id']
        and run['acceptance_digest'] == D.digest(D.compact(accepted['task_acceptance']))
        and run['last_error'] is None and run['active_stage'] is None
        and run['io_completion'] == 'worker_joined' and done['fence']['state'] == 'released')
    require(D.git(fixture['source'], 'rev-parse', D.TARGET) == run['integration_commit'])
    require(D.source_image(fixture['source']) == fixture['project_image']
        and D.source_image(fixture['managed']) == fixture['source_image'])
    require([a['round'] for a in attempts] == [1, 1, 2, 2, 3, 3, 3]
        and [a['kind'] for a in attempts] == ['preparation', 'validator'] * 3 + ['cas']
        and len({a['attempt_id'] for a in attempts}) == 7)
    receipts = run['validation_receipts']
    require([r['round'] for r in receipts] == [1, 2, 3]
        and [r['passed'] for r in receipts] == [False, False, True]
        and len({r['integration_commit'] for r in receipts}) == 3)
    with D.database(host.runtime) as db:
        require(D.exact(D.rows(db, 'command_validation_policies', policy=fixture['command']['id']), fixture['original_command'][0])
            and D.exact(D.rows(db, 'command_validation_runs', policy=fixture['command']['id']), fixture['original_command'][1]))
        events = [(n, D.decode(raw)) for n, raw in db.execute('SELECT sequence,document FROM events ORDER BY sequence')]
        def event(kind, aggregate):
            values = [(n, e) for n, e in events if e['kind'] == kind and e['aggregate_id'] == aggregate]
            require(len(values) == 1); return values[0]
        require(accepted['task_acceptance']['sequence'] < event('integration.queued', run['id'])[0])
        owner = event('approval.confirmed', policy['id']); approved = event('integration.policy_approved', policy['id'])
        require(owner[1]['source'] == 'local_owner_confirmation' and owner[0] < approved[0]
            and approved[1]['payload']['owner_sequence'] == owner[0])
        observed = [(n, e) for n, e in events if e['kind'] == 'integration.validator_observed' and e['aggregate_id'] == run['id']]
        require(len(observed) == 3)
        for index, receipt in enumerate(receipts):
            a = attempts[index * 2 + 1]; n, e = observed[index]; view = e['payload']['view']
            require(receipt['attempt_id'] == a['attempt_id'] and receipt['sequence'] == n
                and receipt['stop_confirmed'] is True and receipt['worker_joined'] is True
                and receipt['result_digest'] == D.digest(D.compact(view))
                and receipt['actual_runtime_ms'] == a['actual_runtime_ms']
                and receipt['actual_output_bytes'] == a['actual_output_bytes']
                and receipt['definition_digest'] == policy['bindings']['validators'][0]['definition_digest'])
            require(view['process_started'] is True and view['exit_code'] == 0 and view['exit_signal'] is None
                and view['passed'] is (index == 2) and view['reason'] == ('exited' if index == 2 else 'artifact_contract_failed')
                and view['exit_observed_within_budget'] is True and view['cleanup_completed_within_budget'] is True
                and view['cleanup']['owned_leader_reaped'] is True and view['cleanup']['all_capture_pipes_observed_eof'] is True
                and view['goal_io']['state'] == 'observed' and view['task_acceptance'] == 'not_established'
                and view['integration_authorized'] is False and view['file_integrity']['passed'] is True
                and view['approval_id'] == policy['approval_id'] and view['policy_revision'] == receipt['definition_digest'])
            contract, old = view['contract'], fixture['command']['contract']
            for name in ('arguments', 'mode', 'script', 'argv0', 'image_policy', 'environment_sha256',
                    'expected_exit_codes', 'wall_time_ms', 'cleanup_time_ms', 'stdout_limit', 'stderr_limit', 'require'):
                require(D.exact(contract[name], old[name]))
            version = contract['file_contract']['subject']['version']
            subject = contract['file_contract']['subject']
            require(subject['task_id'] == fixture['task']['id'] and subject['attempt_id'] == a['attempt_id']
                and subject['spec_revision'] == policy['selection']['expected_spec_revision']
                and subject['input_revision'] == policy['bindings']['source']['head']
                and version['kind'] == 'git' and version['expected_target_head'] == gate.heads[index] and version['target_ref'] == D.TARGET
                and version['integration_candidate'] == receipt['integration_commit']
                and version['tree'] == receipt['integration_tree'] and contract['cwd']['kind']['head'] == receipt['integration_commit']
                and D.exact(view['file_integrity']['subject'], subject)
                and D.exact(view['file_integrity']['policy_contract'], contract['file_contract']))
            for field in ('canonical_root', 'root_device', 'root_inode'):
                require(D.exact(contract['program'][field], old['program'][field]))
            for field in ('artifact_id', 'relative_path', 'expected_sha256'):
                require(D.exact(contract['program']['requirement'][field], old['program']['requirement'][field]))
            cwd = contract['cwd']
            managed = D.decode(db.execute('SELECT document FROM managed_workspaces WHERE root=?', (cwd['canonical_root'],)).fetchone()[0])
            require(D.exact(managed['identity'], cwd) and managed['base_revision'] == receipt['integration_commit']
                and cwd['canonical_root'] != str(fixture['managed']))
            for collection, name, path, raw in [('inputs', 'fixed-input', 'inputs/file', b'baseline'),
                    ('outputs', 'report', 'reports/report.txt', D.REPORT)]:
                values = [value for value in view['file_integrity'][collection] if value['artifact_id'] == name]
                bounds = [value for value in contract['file_contract'][collection] if value['requirement']['artifact_id'] == name]
                require(len(values) == len(bounds) == 1)
                value, bound = values[0], bounds[0]
                require(value['relative_path'] == path and value['size'] == len(raw)
                    and value['expected_sha256'] == value['observed_sha256'] == D.digest(raw) and value['state'] == 'passed'
                    and value['root_device'] == cwd['device'] and value['root_inode'] == cwd['inode']
                    and bound['canonical_root'] == cwd['canonical_root'] and bound['requirement']['expected_sha256'] == D.digest(raw))
            require(D.git(fixture['managed'], 'rev-parse', receipt['integration_commit'] + '^{tree}') == receipt['integration_tree']
                and D.git(fixture['managed'], 'show', receipt['integration_commit'] + ':reports/report.txt') == D.REPORT.decode())
            for stream in ('stdout', 'stderr'):
                capture = view[stream]
                require(capture['captured_bytes'] == capture['observed_bytes_at_least'] == 0
                    and capture['sha256_of_captured'] == D.digest(b'') and capture['truncated'] is False
                    and capture['eof_observed'] is True)
        advances = [(n, e) for n, e in events if e['kind'] == 'integration.descendant_target_observed' and e['aggregate_id'] == run['id']]
        require(len(advances) == 2)
        for index, (n, e) in enumerate(advances):
            value = e['payload']
            require(value['from_round'] == index + 1 and value['next_round'] == index + 2
                and value['from_head'] == gate.heads[index] and value['to_head'] == gate.heads[index + 1]
                and value['stopped_validator_target_only'] is True and value['original_validation_passed'] is False
                and value['new_permission'] is False and value['read_worker_joined'] is True
                and value['validation_sequence'] == receipts[index]['sequence']
                and value['validation_result_digest'] == receipts[index]['result_digest']
                and receipts[index]['sequence'] < n)
        cas = event('integration.cas_observed', run['id']); record = cas[1]['payload']['record']; io = cas[1]['payload']['io']
        require(receipts[2]['sequence'] < cas[0] and cas[1]['payload']['historical_cas_confirmed'] is True
            and record['expected_target_head'] == gate.heads[2] and record['candidate_commit'] == receipts[2]['integration_commit']
            == run['integration_commit'] and record['target_ref'] == D.TARGET and record['state'] == 'confirmed_local_ref'
            and record['grant_ref'] == policy['approval_id']
            and io['owned_leader_reaped'] is True and io['stdout_eof'] is True and io['stderr_eof'] is True
            and attempts[6]['actual_output_bytes'] == io['stdout_retained_bytes'] + io['stderr_retained_bytes'])
        for attempt in attempts:
            require(attempt['state'] == 'finished' and attempt['settled'] is True and attempt['io_completion'] == 'worker_joined'
                and D.integer(attempt['actual_runtime_ms']) and attempt['actual_runtime_ms'] <= attempt['reservation']['runtime_ms']
                and D.integer(attempt['actual_output_bytes']) and attempt['actual_output_bytes'] <= attempt['reservation']['output_bytes'])
            joined = [(n, e) for n, e in events if e['kind'] == 'integration.attempt_joined'
                and e['aggregate_id'] == run['id'] and e['payload']['attempt_id'] == attempt['attempt_id']]
            require(len(joined) == 1 and joined[0][0] == attempt['receipt_sequence']
                and joined[0][1]['payload']['reservation_refunded'] is False)
        graph_cost = D.decode(db.execute('SELECT document FROM goal_run_reservations WHERE run_id=?',
            (fixture['graph']['run']['id'],)).fetchone()[0]); require(graph_cost['settled'] is True)
        total_ms = graph_cost['actual_runtime_ms'] + sum(a['actual_runtime_ms'] for a in attempts)
        total_bytes = graph_cost['actual_output_bytes'] + sum(a['actual_output_bytes'] for a in attempts)
        require(0 < total_ms <= 20000 and 0 < total_bytes <= 8192)
        for domain, identity in [('root', policy['bindings']['root_task_id']), ('project', policy['bindings']['project_id'])]:
            usage = done['shared_usage'][domain]
            require(usage['attempts'] == 8 and usage['active'] == 0 and usage['runtime_ms'] == total_ms
                and usage['output_bytes'] == total_bytes and usage['tokens'] is None and usage['cost_microunits'] is None
                and db.execute('SELECT consumed,active FROM budgets WHERE scope=?', (domain + ':' + identity,)).fetchone() == (8, 0))
        require(db.execute('SELECT count(*) FROM integrations').fetchone() == (1,)
            and db.execute('SELECT count(*) FROM integration_attempts').fetchone() == (7,)
            and db.execute("SELECT count(*) FROM workspace_leases WHERE holder!=''").fetchone() == (0,)
            and db.execute("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'").fetchone() == (0,))
    graph = host.result('goal_graph_runs.get', {'run_id': fixture['graph']['run']['id']})
    require(all(D.exact(graph[k], accepted[k]) for k in ('run', 'candidates', 'validations', 'owner_decisions', 'task_acceptance')))
    require(all(original_gone(record['identity']) for record in gate.records))
    return {'rounds': 3, 'receipts': [False, False, True], 'integration_stages': 7, 'total_attempts': 8,
        'actual_shared_runtime_ms': total_ms, 'actual_shared_output_bytes': total_bytes,
        'cas_count': 1, 'source_and_index_unchanged': True, 'original_command_unchanged': True}


def run_case(args, binary, companion, frontend, attribution):
    from playwright.sync_api import sync_playwright, expect
    output = args.output
    private = output / 'private'; private.mkdir(mode=0o700)
    root = Path(tempfile.mkdtemp(prefix='ab-round-ui-', dir='/tmp')).resolve(); root.chmod(0o700)
    runtime = root / 'r'; runtime.mkdir(mode=0o700)
    require(len(str(runtime / 'batond.sock').encode()) < 104)
    report = {'case': 'actual_three_round_history', 'status': 'failed_or_interrupted', 'inputs': attribution,
        'dimension': 'Linux Chromium/built React/finite TEST bridge/actual Core17/generated ACP/private Owner host',
        'not_claimed': ['OS native dialog', 'macOS', 'real Provider/model', 'OS sandbox', 'full product acceptance'],
        'checks': [], 'screenshots': [], 'cleanup': {}}
    daemon = engine = browser = server = server_thread = host = reviews = gate = None
    phase, passed, cleanup_errors = 'startup', False, []
    log = (private / 'daemon.log').open('xb')
    def checked(name, **facts): report['checks'].append({'name': name, 'status': 'passed', **facts})
    def cleanup(name, fn):
        try: report['cleanup'][name] = fn()
        except BaseException as error:
            cleanup_errors.append(name); report['cleanup'][name] = {'kind': type(error).__name__}
    try:
        engine = sync_playwright().start()
        browser = engine.chromium.launch(executable_path=str(args.chromium), headless=True,
            args=['--no-sandbox', '--disable-dev-shm-usage'])
        report['browser_version'] = browser.version
        daemon = subprocess.Popen([str(binary)], env={**os.environ, 'BATON_RUNTIME_DIR': str(runtime)},
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        D.observe(lambda: (daemon.poll(), (runtime / 'session.token').is_file(), (runtime / 'batond.sock').exists()),
            lambda v: v == (None, True, True), seconds=10)
        require(stat.S_IMODE((runtime / 'session.token').stat().st_mode) == 0o600
            and stat.S_ISSOCK((runtime / 'batond.sock').lstat().st_mode)
            and stat.S_IMODE((runtime / 'batond.sock').stat().st_mode) == 0o600)
        gate = RoundGate(root); host = RoundHost(runtime, gate)
        phase = 'real_three_round_fixture'
        fixture = D.bootstrap(host, root, False); headroom(fixture['context'])
        bounds = D.Boundary(fixture['task']['id'], fixture['project']['id'], fixture['scope']['id'],
            fixture['target'], [fixture['definition']['reference']])
        reviews = D.Reviews(host, bounds, fixture['context'], fixture['definition'],
            selected(fixture['context'], fixture['target'], fixture['definition']['reference']))
        server, server_thread = D.serve(frontend, host, bounds, reviews, attribution)
        context = browser.new_context(viewport={'width': 1440, 'height': 1100}, locale='en-US')
        context.add_init_script("localStorage.setItem('agentbaton.language','en');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{const r=await fetch('" + D.ENDPOINT + "',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});if(!r.ok)throw Error('Finite integration TEST bridge refused');return r.json();}};")
        context.route('**/*', lambda route: route.continue_() if route.request.url.startswith(server.origin + '/') else route.abort())
        page = context.new_page(); page_errors = []; page.on('pageerror', lambda _: page_errors.append('uncaught_browser_error'))
        def pump(): reviews.service(); gate.healthy(); page.wait_for_timeout(10)
        def shot(name, focus):
            focus.scroll_into_view_if_needed(); expect(focus).to_be_in_viewport(ratio=1)
            path = output / (name + '.png'); box = focus.bounding_box(); page.screenshot(path=str(path))
            report['screenshots'].append({'name': name, 'sha256': D.digest(path.read_bytes()),
                'bytes': path.stat().st_size, 'viewport': page.viewport_size, 'focus_box_before_capture_observation': box,
                'focus_viewport_asserted_before_capture': True, 'pixel_synchronized': False})
        phase = 'actual_UI_selection_and_private_review'
        page.goto(server.origin + '/', wait_until='networkidle')
        page.locator('[data-testid="task-card"][data-task-id="' + bounds.task + '"]').click()
        page.get_by_role('tab', name='Details', exact=True).click(); panel = page.get_by_test_id('task-integration')
        panel.locator(':scope > summary').click()
        expect(panel.get_by_role('button', name='Refresh integration records')).to_be_enabled()
        panel.get_by_label('Existing local target branch').select_option(D.TARGET)
        panel.get_by_label('Maximum preparation rounds').fill('3')
        panel.get_by_label('If the target advances').select_option('bounded_descendant_reprepare')
        panel.get_by_label('Permission lifetime (milliseconds)').fill('30000')
        panel.get_by_text('Fresh candidate correctness checks', exact=True).click()
        panel.get_by_role('checkbox', name=re.compile('Add check command:')).check()
        panel.get_by_text('Stage reservation', exact=True).click()
        runtimes = panel.get_by_label(re.compile(r' · Runtime reservation \(milliseconds\)$'))
        outputs = panel.get_by_label(re.compile(r' · Output reservation \(bytes\)$'))
        require(runtimes.count() == outputs.count() == 3)
        for index, value in enumerate(('1024', '256', '1024')):
            runtimes.nth(index).fill('2000'); outputs.nth(index).fill(value)
        panel.get_by_role('button', name='Review branch permission in native dialog').click()
        D.observe(lambda: len(reviews.completed), lambda n: n == 1, pump=pump)
        policy = reviews.completed[0]['policy']
        require(D.exact(policy['selection'], reviews.expected) and D.integration_counts(runtime) == (1, 0, 0, 0))
        checked('actual UI max3 selection and complete private-host reviewed/consumed permission', **reviews.completed[0]['facts'])
        phase = 'real_owner_acceptance_and_owned_target_advances'
        gate.start(fixture, daemon.pid)
        accepted = D.accept_graph(host, fixture)
        # Core's watcher may already have started the policy. The UI action must
        # return that same run, never create a second attempt domain or retry.
        panel.get_by_role('button', name='Refresh integration records').click()
        expect(panel.get_by_role('button', name='Check accepted task for the same run')).to_be_enabled()
        panel.get_by_role('button', name='Check accepted task for the same run').click()
        def read_terminal():
            gate.healthy()
            runs = host.result('integrations.list', {'task_id': bounds.task}); require(len(runs) <= 1)
            return None if not runs else host.result('integrations.get', {'integration_id': runs[0]['id']})
        def finished(value):
            if value is None: return False
            require(value['run']['state'] not in ('failed', 'blocked', 'outcome_unknown', 'stale_target', 'cancelled'),
                'Three-round runtime did not establish expected confirmation')
            return value['run']['state'] == 'confirmed_local_ref'
        done = D.observe(read_terminal, finished, pump=pump)
        starts = [a for a in host.audit if a['method'] == 'integrations.start']
        require(len(starts) == 1 and starts[0]['response']['result']['run']['id'] == done['run']['id'])
        facts = terminal_evidence(host, fixture, accepted, policy, done, gate)
        checked('actual false,false,true receipts and charged seven-stage/same-policy sole exact CAS', **facts)
        phase = 'actual_UI_readonly_round_history'
        panel.get_by_role('button', name='Refresh integration records').click()
        history = panel.get_by_role('region', name='Integration history ' + done['run']['id'], exact=True)
        expect(history.get_by_text('confirmed_local_ref', exact=True)).to_be_visible()
        expect(history.get_by_role('heading', name='Confirmed local branch update', exact=True)).to_be_visible()
        history.get_by_text('Actual validation receipts', exact=True).click()
        for receipt in done['run']['validation_receipts']:
            group = history.get_by_role('group', name='Actual validation receipts · Round ' + str(receipt['round']) + ' · ' + receipt['attempt_id'], exact=True)
            expect(group).to_be_visible()
            expect(group.get_by_text(('passed' if receipt['passed'] else 'failed') + ' · stop_confirmed=true · worker_joined=true', exact=True)).to_be_visible()
            expect(group.locator('dt', has_text=re.compile('^Round$')).locator('xpath=following-sibling::dd[1]')).to_have_text(str(receipt['round']))
            expect(group.locator('dt', has_text=re.compile('^Merged candidate$')).locator('xpath=following-sibling::dd[1]')).to_have_text(receipt['integration_commit'] + receipt['integration_tree'])
            for label, key in [('Actual runtime (milliseconds)', 'actual_runtime_ms'), ('Actual retained output (bytes)', 'actual_output_bytes')]:
                expect(group.locator('dt', has_text=re.compile('^' + re.escape(label) + '$')).locator('xpath=following-sibling::dd[1]')).to_have_text(str(receipt[key]))
            shot('round-' + str(receipt['round']) + '-receipt-desktop', group)
        for attempt in done['attempts']:
            group = history.get_by_role('group', name='Attempts / active · Round ' + str(attempt['round']) + ' · ' + attempt['attempt_id'], exact=True)
            expect(group).to_be_visible(); expect(group.get_by_text('finished · worker_joined · settled', exact=True)).to_be_visible()
            for label, key in [('Round', 'round'), ('Actual runtime (milliseconds)', 'actual_runtime_ms'), ('Actual retained output (bytes)', 'actual_output_bytes')]:
                expect(group.locator('dt', has_text=re.compile('^' + re.escape(label) + '$')).locator('xpath=following-sibling::dd[1]')).to_have_text(str(attempt[key]))
        # Read-only history is rendered from actual Core records; no document,
        # receipt or usage value is injected by the bridge or page script.
        history_container = history.locator('xpath=..')
        history_container.get_by_text('Shared budget', exact=True).click()
        for domain, label in [('root', 'Root task'), ('project', 'Project')]:
            usage = history_container.locator('dl[aria-label="Observed shared usage ' + label + '"]')
            for name, value in [('Attempts / active', '8 / 0'), ('Runtime (milliseconds)', str(done['shared_usage'][domain]['runtime_ms'])),
                    ('Output (bytes)', str(done['shared_usage'][domain]['output_bytes'])), ('Tokens', 'Unknown / not measured'), ('Cost (microunits)', 'Unknown / not measured')]:
                expect(usage.locator('dt', has_text=re.compile('^' + re.escape(name) + '$')).locator('xpath=following-sibling::dd[1]')).to_have_text(value)
        shot('actual-shared-budget-desktop', history_container.locator('dl[aria-label="Observed shared usage Root task"]'))
        page.set_viewport_size({'width': 390, 'height': 844})
        for receipt in done['run']['validation_receipts']:
            group = history.get_by_role('group', name='Actual validation receipts · Round ' + str(receipt['round']) + ' · ' + receipt['attempt_id'], exact=True)
            shot('round-' + str(receipt['round']) + '-receipt-mobile', group)
        require(page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1') and not page_errors and not server.faults)
        checked('actual DOM preserves every round candidate, failed earlier receipts, settled fees and unknown model cost; desktop/mobile')
        report['http_method_counts'] = dict(D.Counter(a['method'] for a in server.calls))
        passed = True
    except BaseException as error:
        report['failure'] = {'stage': phase, 'kind': type(error).__name__}
        D.private_json(private / 'failure.json', {'stage': phase, 'traceback': traceback.format_exc()})
    finally:
        if gate is not None: gate.stop.set()  # Prevent any later Host ref work before owned shutdown.
        if reviews is not None: cleanup('private_reviews_stopped', lambda: (reviews.stop(), True)[1])
        if browser is not None: cleanup('browser_closed', lambda: (browser.close(), True)[1])
        if engine is not None: cleanup('playwright_stopped', lambda: (engine.stop(), True)[1])
        if server is not None: cleanup('http_shutdown', lambda: (server.shutdown(), server.server_close(), True)[2])
        if server_thread is not None:
            def join_http(): server_thread.join(5); require(not server_thread.is_alive()); return True
            cleanup('http_thread_joined', join_http)
        if daemon is not None:
            def stop_daemon():
                require(daemon.poll() is None, 'Daemon exited before owned shutdown')
                daemon.send_signal(signal.SIGTERM)
                try: code = daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    daemon.kill(); daemon.wait(timeout=5); raise D.Refusal('Forced kill cannot count as clean shutdown')
                require(code == 0); return {'exit': code, 'joined': True, 'forced_kill': False}
            cleanup('daemon_joined', stop_daemon)
        if gate is not None:
            cleanup('owned_gate_thread_joined', gate.close)
            D.private_json(private / 'gate.json', {'records': gate.records, 'heads': gate.heads, 'error': gate.error})
            if gate.error is not None: passed = False
            def stopped_identities(): require(all(original_gone(v['identity']) for v in gate.records)); return True
            cleanup('original_validator_identities_gone_after_daemon_join', stopped_identities)
        log.close()
        if host is not None: cleanup('private_transcript_saved', lambda: (D.private_json(private / 'transcript.json', host.audit), True)[1])
        def unchanged():
            require(D.pinned_file(binary) == attribution['tools']['batond'] and D.pinned_file(companion) == attribution['tools']['baton']
                and D.assets(frontend) == attribution['assets'] and D.source_guard(attribution['source']) == attribution['source_guard']
                and own_source(attribution['source']) == attribution['round_driver']
                and D.pinned_file(args.chromium, D.MAX_CHROMIUM_IMAGE) == attribution['runtime_tools']['chromium']
                and D.pinned_file(Path(sys.executable).resolve(strict=True)) == attribution['runtime_tools']['python'])
            return True
        cleanup('tools_assets_sources_unchanged', unchanged)
        if server is not None and server.faults: passed = False
        if passed and not cleanup_errors: cleanup('fixture_removed', lambda: (shutil.rmtree(root), True)[1])
        if not passed or cleanup_errors:
            D.private_json(private / 'retained.json', {'root': str(root), 'after_owned_cleanup_attempts': True, 'cleanup_errors': cleanup_errors})
            report['fixture_retained_privately'] = root.exists()
        report['status'] = 'passed' if passed and not cleanup_errors else 'failed_or_interrupted'
        D.private_json(output / 'report.json', report)
    return report['status'] == 'passed'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('batond', 'candidate-manifest', 'frontend', 'frontend-build-record', 'chromium', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--expected-source-head', required=True); parser.add_argument('--run', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    require(stat.S_IMODE(args.output.stat().st_mode) == 0o700 and args.output.stat().st_uid == os.geteuid() and not args.output.is_symlink())
    try:
        require(sys.platform == 'linux', 'This real proc-identity/browser fixture is Linux only')
        binary, companion, frontend, attribution = D.preflight(args)
        attribution['round_driver'] = own_source(args.expected_source_head)
    except BaseException as error:
        D.private_json(args.output / 'report.json', {'status': 'preflight_refused', 'actual_runtime': 'NOT_RUN',
            'kind': type(error).__name__, 'no_product_token_or_listener': True}); return 2
    if not args.run:
        D.private_json(args.output / 'report.json', {'status': 'NOT_RUN', 'reason': '--run required', 'inputs': attribution}); return 2
    try:
        from playwright.sync_api import sync_playwright  # noqa: F401; dependency check only
    except ImportError as error:
        D.private_json(args.output / 'report.json', {'status': 'preflight_refused', 'actual_runtime': 'NOT_RUN',
            'kind': type(error).__name__, 'no_product_token_or_listener': True}); return 2
    result = run_case(args, binary, companion, frontend, attribution)
    print(json.dumps({'status': 'passed' if result else 'failed_or_interrupted', 'dimension': 'Linux three-round actual IPC/browser only'}))
    return 0 if result else 1


if __name__ == '__main__':
    raise SystemExit(main())
