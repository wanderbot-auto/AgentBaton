"""Current workflow controls plus exact whole-source historical routing."""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/product.yml'
INVERSE = ROOT / 'docs/execution/host19-browser-ci-workflow/WORKFLOW_INVERSE.json'
BASE = 'bd97dcf2adbe2a439aa2253ed4d118cc069cc850'
HISTORICAL_SHA256 = 'edfffb07b5d5c8ecb3039962f5ad54e77cd187a4afd33f89941564b89603bff4'
RUN_STEP = 'Actual integration frontend with real Core IPC and generated correctness'
BROWSER_STEP = 'Actual frontend with real Unix IPC and responsive screenshots'
TOOL_STEP = 'Real daemon and browser test dependencies'
FRONTEND_STEP = 'Frontend validation and measured build provenance'


def sha(source):
    return hashlib.sha256(source.encode()).hexdigest()


def inverse_record():
    value = json.loads(INVERSE.read_bytes())
    if value['base'] != BASE or value['historical_sha256'] != HISTORICAL_SHA256:
        raise ValueError('Historical schema19 workflow baseline changed')
    return value


def restore_schema18_workflow(source, record=None):
    record = inverse_record() if record is None else record
    current = sha(source)
    if current == HISTORICAL_SHA256:
        return source
    if current != record['current_sha256']:
        raise ValueError('Complete schema19 workflow bytes changed')
    lines = source.splitlines(keepends=True)
    last = len(lines)
    for edit in reversed(record['edits']):
        start, end = edit['start'], edit['end']
        if not 0 <= start < end <= last or sha(''.join(lines[start:end])) != edit['current_sha256']:
            raise ValueError('Schema19 workflow inverse span changed')
        lines[start:end] = edit['historical'].splitlines(keepends=True)
        last = start
    restored = ''.join(lines)
    if sha(restored) != HISTORICAL_SHA256:
        raise ValueError('Schema19 inverse did not restore the exact schema18 workflow')
    return restored


def normalize_workflow(source):
    record = inverse_record()
    if sha(source) == record['current_sha256']:
        return restore_schema18_workflow(source, record)
    return source


def historical_workflow():
    return restore_schema18_workflow(WORKFLOW.read_bytes().decode())


def steps(raw=None):
    document = yaml.load(WORKFLOW.read_bytes() if raw is None else raw, Loader=yaml.BaseLoader)
    return {step['name']: step for step in document['jobs']['frontend']['steps'] if 'name' in step}


def integration_program():
    run = steps()[RUN_STEP]['run']
    prefix = "- <<'INTEGRATION_BROWSER'\n"
    if run.count(prefix) != 1 or not run.endswith('INTEGRATION_BROWSER\n'):
        raise AssertionError('Expected the actual inline Integration launch')
    return run.split(prefix)[1][:-len('INTEGRATION_BROWSER\n')]


def load_preparer():
    location = ROOT / 'scripts/prepare-schema19-browser-ci.py'
    spec = importlib.util.spec_from_file_location('_schema19_browser_ci_controls', location)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


P = load_preparer()
HEAD, TREE = 'a' * 40, 'b' * 40


@contextmanager
def workspace():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp).resolve()
        previous = Path.cwd()
        try:
            os.chdir(root)
            (root / P.CORE_TOOLS).mkdir(parents=True)
            (root / P.FRONTEND).mkdir(parents=True)
            (root / P.PUBLIC).mkdir(parents=True)
            (root / P.FRONTEND / 'index.html').write_text('generated fixed frontend')
            for name in P.TOOLS:
                path = root / P.CORE_TOOLS / name
                path.write_bytes(name.encode())
                path.chmod(0o700)
            for name in ('Cargo.toml', 'Cargo.lock'):
                (root / name).write_text(name)
            temp = root / 'temp'
            temp.mkdir(mode=0o700)
            with mock.patch.object(P, 'git_identity', return_value=(HEAD,TREE)):
                yield root, temp
        finally:
            os.chdir(previous)


def prepare(tmp):
    frontend = P.PUBLIC / 'frontend-build-record.json'
    core = tmp / 'agentbaton-browser-core-build.json'
    with mock.patch.object(P.subprocess, 'run', return_value=SimpleNamespace(returncode=0)) as run:
        assert P.timed_build('frontend', frontend) == 0
        with mock.patch.dict(os.environ, {'CARGO_PROFILE_DEV_DEBUG':'0'}):
            assert P.timed_build('core', core) == 0
        assert run.call_args_list == [mock.call(P.NPM, check=False), mock.call(P.CARGO, check=False)]
    return core, frontend


class CandidateSourceAndInputs(unittest.TestCase):
    def test_current_workflow_reverses_every_byte_and_refuses_unreviewed_variants(self):
        current = WORKFLOW.read_bytes().decode()
        old = historical_workflow()
        self.assertEqual(sha(old), HISTORICAL_SHA256)
        self.assertEqual(restore_schema18_workflow(old), old)
        for changed in (current + '# unreviewed\n', current.replace('\n', '\r\n'),
                current.replace('contents: read', 'contents: write', 1),
                current.replace('timeout-minutes: 20', 'timeout-minutes: 21', 1),
                current.replace('freeze --runner-temp', 'core --runner-temp', 1),
                current.replace("snapshot['database_schema_version'] == 19",
                    "snapshot['database_schema_version'] == 18", 1)):
            with self.subTest(sha=sha(changed)), self.assertRaises(ValueError):
                restore_schema18_workflow(changed)

    def test_only_original_approved_steps_change_and_candidate_precedes_consumers(self):
        current_raw = WORKFLOW.read_bytes()
        previous_raw = historical_workflow()
        current, previous = (yaml.load(raw, Loader=yaml.BaseLoader) for raw in (current_raw, previous_raw))
        self.assertEqual({k:v for k,v in current.items() if k != 'jobs'}, {k:v for k,v in previous.items() if k != 'jobs'})
        self.assertEqual(current['jobs'].keys(), previous['jobs'].keys())
        for key in ('core', 'desktop'):
            self.assertEqual(current['jobs'][key], previous['jobs'][key])
        actual, old = current['jobs']['frontend'], previous['jobs']['frontend']
        self.assertEqual({k:v for k,v in actual.items() if k != 'steps'}, {k:v for k,v in old.items() if k != 'steps'})
        self.assertEqual(len(actual['steps']), len(old['steps']))
        for now, then in zip(actual['steps'],old['steps']):
            self.assertEqual({k:v for k,v in now.items() if k != 'run'}, {k:v for k,v in then.items() if k != 'run'})
            if now.get('name') not in {FRONTEND_STEP,TOOL_STEP,BROWSER_STEP,RUN_STEP}:
                self.assertEqual(now, then)
            if 'run' in now:
                subprocess.run(['bash','-n'], input=now['run'], text=True, check=True)
        source = steps()[BROWSER_STEP]['run']
        freeze = source.index('freeze --runner-temp')
        for name in ('data-workspace', 'task-workspace-binding', 'goal-execution','goal-graph','factory',
                'factory-child-planning','factory-child-goal'):
            with self.subTest(consumer=name):
                self.assertLess(freeze, source.index('/'+name+'-real-ipc.py'))
        self.assertIn("--candidate-manifest artifacts/ui-browser/goal-execution-candidate.json", source)
        self.assertIn("'--notification-probe',", integration_program())
        for line in source.splitlines():
            if line.startswith('artifacts/browser-venv/bin/python apps/desktop/tests/'):
                self.assertIn('--chromium "$AGENTBATON_CI_CHROMIUM"', line)
        self.assertLess(source.index("<<'CHROMIUM'"),source.index('/quiet-canvas-real-ipc.py'))

    def test_exact_outputs_bind_both_builds_and_readonly_private_executables(self):
        with workspace() as (root, tmp):
            core, front = prepare(tmp)
            original = P.CORE_TOOLS / 'batond'
            link = P.CORE_TOOLS / 'cargo-managed-link'
            os.link(original, link)
            # Capture the hardlink count actually observed just after a Cargo build.
            with mock.patch.object(P.subprocess, 'run', return_value=SimpleNamespace(returncode=0)), \
                    mock.patch.dict(os.environ, {'CARGO_PROFILE_DEV_DEBUG':'0'}):
                core.unlink()
                self.assertEqual(P.timed_build('core', core), 0)
            private = tmp/'agentbaton-browser-candidate'
            P.prepare_candidate(core, front, private)
            first = P.PUBLIC/'goal-execution-candidate.json'
            second = P.PUBLIC/'factory-child-planning-candidate.json'
            self.assertEqual(first.read_bytes(), second.read_bytes())
            data = P.verify_pair(first.read_bytes(), front.read_bytes(), core.read_bytes(), HEAD, TREE)
            self.assertEqual(data['schema'],19)
            self.assertEqual(data['core_build_record_sha256'],P.sha(core.read_bytes()))
            self.assertEqual(data['frontend_build_record_sha256'],P.sha(front.read_bytes()))
            for name in P.TOOLS:
                tool = private/'tools'/name
                pin = tool.stat()
                self.assertEqual((stat.S_IMODE(pin.st_mode),pin.st_nlink),(0o500,1))
                self.assertEqual(P.sha(tool.read_bytes()),data['binaries'][name])
                self.assertFalse(tool.samefile(P.CORE_TOOLS/name))

    def test_drifted_tool_assets_config_build_types_or_source_refuse_before_freezing(self):
        for label in ('tools','asset','asset_symlink','configuration','source','tree','bool_exit','failed_build'):
            with self.subTest(label=label), workspace() as (root, tmp):
                core, front = prepare(tmp)
                if label=='tools':
                    (P.CORE_TOOLS/'baton').write_bytes(b'wrong')
                elif label=='asset':
                    (P.FRONTEND/'added.js').write_text('wrong')
                elif label=='asset_symlink':
                    (P.FRONTEND/'alias.js').symlink_to(P.FRONTEND.absolute()/'index.html')
                elif label=='configuration':
                    Path('Cargo.lock').write_text('wrong')
                else:
                    record = P.decode(front.read_bytes())
                    key, value = {'source':('source','c'*40),'tree':('git_tree','c'*40),
                        'bool_exit':('exit',False),'failed_build':('exit',1)}[label]
                    record[key] = value
                    front.write_bytes(P.encode(record))
                with self.assertRaises((OSError,ValueError)):
                    P.prepare_candidate(core,front,tmp/'frozen')
                self.assertFalse((P.PUBLIC/'goal-execution-candidate.json').exists())

    def test_schema_and_same_raw_build_record_are_required(self):
        with workspace() as (_, tmp):
            core, front = prepare(tmp)
            P.prepare_candidate(core,front,tmp/'frozen')
            manifest = P.decode((P.PUBLIC/'goal-execution-candidate.json').read_bytes())
            raw = front.read_bytes()
            for schema in (18,20,True,19.0,'19'):
                with self.subTest(schema=schema), self.assertRaises(ValueError):
                    P.verify_pair(P.encode({**manifest,'schema':schema}),raw,core.read_bytes(),HEAD,TREE)
            with self.assertRaises(ValueError):
                P.verify_pair(P.encode(manifest),raw+b' ',core.read_bytes(),HEAD,TREE)
            with self.assertRaises(ValueError):
                P.verify_pair(P.encode(manifest),raw,core.read_bytes()+b' ',HEAD,TREE)


class IntegrationProbeControls(unittest.TestCase):
    def probe(self, snapshot, *, exit_code=0, early=False, forced=False, drift=False):
        temp = tempfile.TemporaryDirectory(dir='/tmp')
        self.addCleanup(temp.cleanup)
        private = Path(temp.name).resolve()
        tools = private / 'tools'
        tools.mkdir()
        namespace = {'__name__':'actual_workflow_probe_controls'}
        exec(compile(integration_program(), '<actual Integration workflow body>', 'exec'), namespace)
        signals, requests, checks = [], [], []
        daemon = mock.Mock()
        daemon.poll.side_effect = [None, 0] if early else lambda: None

        def wait(timeout):
            if forced and timeout == 10:
                raise subprocess.TimeoutExpired('owned probe', timeout)
            return exit_code

        daemon.wait.side_effect = wait
        daemon.send_signal.side_effect = signals.append

        def start(command, *, env, stdout, stderr, start_new_session):
            self.assertEqual(command, [str(tools / 'batond')])
            self.assertTrue(start_new_session)
            runtime = Path(env['BATON_RUNTIME_DIR'])
            self.assertEqual(runtime, private/'probe-runtime')
            self.assertEqual(stat.S_IMODE(runtime.stat().st_mode),0o700)
            token = runtime/'session.token'
            token.write_text('PRIVATE_TOKEN')
            token.chmod(0o600)
            # Real endpoint metadata only. The schema reply comes from the
            # explicitly synthetic Host below, never from this test socket.
            channel = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.addCleanup(channel.close)
            channel.bind(str(runtime/'batond.sock'))
            (runtime/'batond.sock').chmod(0o600)
            return daemon

        class Host:
            def __init__(self, runtime):
                if runtime != private/'probe-runtime':
                    raise AssertionError(runtime)
            def result(self, method, params):
                requests.append((method, params))
                return snapshot

        def verify():
            checks.append(True)
            if drift and len(checks) == 2:
                raise ValueError('private tool changed during probe')

        binding = {'source':HEAD,'manifest_sha256':'b'*64,'build_record_sha256':'c'*64,
            'tools_sha256':{'batond':'d'*64,'baton':'e'*64}}
        record = private/'schema-probe.json'
        with mock.patch.object(namespace['subprocess'],'Popen',side_effect=start):
            if (type(snapshot.get('database_schema_version')) is not int or snapshot['database_schema_version'] != 19
                    or type(snapshot.get('api_version')) is not int or snapshot['api_version'] != 1
                    or type(exit_code) is not int or exit_code != 0 or early or forced or drift):
                with self.assertRaises(ValueError):
                    namespace['actual_schema_probe'](SimpleNamespace(Host=Host),tools,private,binding,verify)
                self.assertFalse(record.exists())
            else:
                namespace['actual_schema_probe'](SimpleNamespace(Host=Host),tools,private,binding,verify)
                self.assertEqual(stat.S_IMODE(record.stat().st_mode),0o600)
                self.assertEqual(json.loads(record.read_bytes()),
                    {**binding,'api_version':1,'database_schema_version':19,'daemon_exit':0,'joined':True})
                self.assertNotIn('PRIVATE_TOKEN',record.read_text())
                self.assertEqual(len(checks),2)
        self.assertEqual(requests,[('system.snapshot',{})])
        if early:
            self.assertEqual(signals,[])
        else:
            self.assertEqual(signals,[namespace['signal'].SIGTERM])
        self.assertEqual(daemon.kill.call_count,int(forced))

    def test_probe_only_seals_fresh_typed_schema_and_actual_owned_wait(self):
        self.probe({'database_schema_version':19,'api_version':1})
        for schema in (18,20,19.0,True,'19'):
            with self.subTest(schema=schema):
                self.probe({'database_schema_version':schema,'api_version':1})
        for api in (19,1.0,True,'1'):
            with self.subTest(api=api):
                self.probe({'database_schema_version':19,'api_version':api})

    def test_probe_never_seals_nonzero_exit_early_exit_forced_stop_or_mutated_input(self):
        good = {'database_schema_version':19,'api_version':1}
        for control in ({'exit_code':7},{'exit_code':False},{'early':True},{'forced':True},{'drift':True}):
            with self.subTest(control=control):
                self.probe(good,**control)


if __name__ == '__main__':
    unittest.main()
