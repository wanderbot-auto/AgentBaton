"""CI flow contracts only; no Cargo, Core, Native process or OS dialog runs."""
import contextlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import types
import unittest
from unittest import mock

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/product.yml'
LIB_STEP = 'Native library tests with smoke features'
DRIVER_STEP = 'Actual Native integration approval and early cancellation'
PROJECT_STEP = 'Retain finite Native integration evidence without private snapshots'
CASES = ('last-page', 'first-page')
SECRET = 'PRIVATE_CANARY_PROPOSAL_TOKEN_SNAPSHOT_PATH'


def workflow():
    return yaml.load(WORKFLOW.read_text(), Loader=yaml.BaseLoader)


def step(name):
    matches = [value for value in workflow()['jobs']['desktop']['steps'] if value.get('name') == name]
    if len(matches) != 1:
        raise AssertionError('One fixed Native integration CI step required')
    return matches[0]


def program(name, marker):
    run = step(name)['run']
    header = "python3 - <<'" + marker + "'\n"
    if run.count(header) != 1 or not run.endswith(marker + '\n'):
        raise AssertionError('Exact inline Python boundary required')
    source = run.split(header, 1)[1][:-len(marker + '\n')]
    namespace = {'__name__': 'integration_ci_contract'}
    exec(compile(source, '<fixed-native-integration-ci>', 'exec'), namespace)
    return namespace


@contextlib.contextmanager
def fixture():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        (root / 'scripts').mkdir()
        for name in ('record-integration-owner-ci-evidence.py', 'native-owner-dialog-smoke.py'):
            shutil.copyfile(ROOT / 'scripts' / name, root / 'scripts' / name)
        temporary = root / 'runner-temp'
        temporary.mkdir(mode=0o700)
        previous = Path.cwd()
        try:
            os.chdir(root)
            with mock.patch.dict(os.environ, {'RUNNER_TEMP': str(temporary)}):
                yield root, temporary
        finally:
            os.chdir(previous)


def private_json(path, value):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_text(json.dumps(value))
    path.chmod(0o600)


def seed_reports(temporary, exits=None):
    private = temporary / 'agentbaton-native-integration'
    private.mkdir(mode=0o700)
    private_json(private / 'driver-status.json', {
        case: {'driver_exit': 0 if exits is None else exits[index]}
        for index, case in enumerate(CASES)
    })
    for case in CASES:
        # Only wrapper baseline extraction is under test. The subprocess mock
        # below does NOT claim the real projector accepts this minimal report.
        private_json(private / case / 'cancel/owner-dialog.json', {
            'integration_approval_database': {'before': {'sequence': 7}},
            'private_canary': SECRET,
        })
    return private


def fake_process(namespace, returns):
    calls = []
    values = iter(returns)

    def run(command, **kwargs):
        calls.append((command, kwargs))
        value = next(values)
        kwargs['stdout'].write(SECRET.encode())
        if isinstance(value, BaseException):
            raise value
        return types.SimpleNamespace(returncode=value)

    namespace['subprocess'] = types.SimpleNamespace(
        run=run, STDOUT=subprocess.STDOUT, SubprocessError=subprocess.SubprocessError)
    return calls


class IntegrationNativeCiTests(unittest.TestCase):
    def test_both_mac_lanes_keep_existing_jobs_limits_actions_and_permissions(self):
        document = workflow()
        self.assertEqual(set(document['jobs']), {'core', 'frontend', 'desktop'})
        desktop = document['jobs']['desktop']
        self.assertEqual(desktop['name'], 'Product / ${{ matrix.os }}')
        self.assertEqual(desktop['strategy']['matrix']['os'], ['macos-15', 'macos-15-intel'])
        self.assertEqual(desktop['strategy']['fail-fast'], 'false')
        self.assertEqual(desktop['strategy']['max-parallel'], '2')
        self.assertEqual(desktop['timeout-minutes'], '60')
        self.assertEqual(document['jobs']['core']['timeout-minutes'], '25')
        self.assertEqual(document['permissions'], {'contents': 'read'})
        for job in document['jobs'].values():
            for value in job['steps']:
                if 'uses' in value:
                    self.assertRegex(value['uses'], r'@[0-9a-f]{40}$')

    def test_original_default_and_owner_binary_commands_are_not_replaced(self):
        steps = workflow()['jobs']['desktop']['steps']
        original = next(value for value in steps if value.get('name') == 'Core and native host tests')['run']
        self.assertIn('cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml\n', original)
        owner = next(value for value in steps if value.get('name') == 'Actual native Owner Grant dialog and private database acceptance')['run']
        self.assertIn('--features native-smoke,custom-protocol --bin agentbaton-native-owner-dialog-smoke', owner)
        self.assertIn('--output artifacts/native-smoke/owner-dialog', owner)
        factory = next(value for value in steps if value.get('name') == 'Actual Native Factory planning approval and unchanged Grant evidence')['run']
        self.assertIn('--kind factory_planning', factory)
        names = [value.get('name') for value in steps]
        self.assertLess(names.index('Actual Native Factory planning approval and unchanged Grant evidence'), names.index(LIB_STEP))
        self.assertLess(names.index(LIB_STEP), names.index(DRIVER_STEP))

    def test_feature_library_configuration_is_explicit_debug_locked_and_serial(self):
        value = step(LIB_STEP)
        self.assertEqual(value['env'], {'RUST_TEST_THREADS': '1'})
        self.assertEqual(value['run'].split(), [
            'cargo', 'test', '--locked', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml',
            '--features', 'native-smoke,custom-protocol', '--lib', '--', '--test-threads=1'])

    def test_existing_always_retention_release_audit_and_package_remain(self):
        steps = workflow()['jobs']['desktop']['steps']
        for name in ('Retain bounded native migration metadata without private history',
                     'Retain finite rework progress without private runtime data', PROJECT_STEP):
            self.assertEqual(next(value for value in steps if value.get('name') == name)['if'], 'always()')
        uploads = [value for value in steps if value.get('uses', '').startswith('actions/upload-artifact@')]
        self.assertTrue(any(value.get('if') == 'always()' and value['with']['path'] == 'artifacts/native-smoke/' for value in uploads))
        audit = next(value for value in steps if value.get('name') == 'Audit the actual default release application for test controls')
        self.assertEqual(audit['run'], 'bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop')
        self.assertTrue(any(value.get('run') == 'bash scripts/package-development-macos.sh' for value in steps))
        for name in (LIB_STEP, DRIVER_STEP, PROJECT_STEP):
            self.assertNotIn('continue-on-error', step(name))

    def test_two_real_driver_commands_reuse_tools_and_keep_private_logs(self):
        code = program(DRIVER_STEP, 'INTEGRATION_NATIVE')
        calls = fake_process(code, [0, 0])
        with fixture() as (_, temporary):
            code['run_integration_cases']()
            self.assertEqual(len(calls), 2)
            for index, (command, kwargs) in enumerate(calls):
                self.assertEqual(command[1:5], ['scripts/native-owner-dialog-smoke.py', '--kind', 'integration', '--binary'])
                self.assertEqual(command[5], 'apps/desktop/src-tauri/target/debug/agentbaton-native-owner-dialog-smoke')
                self.assertEqual(command[6:10], ['--daemon', 'target/debug/batond', '--companion', 'target/debug/baton'])
                self.assertEqual(command[10:12], ['--output', str(temporary / 'agentbaton-native-integration' / CASES[index])])
                self.assertEqual(command[12:], [] if index == 0 else ['--integration-cancel-at', '0'])
                self.assertIs(kwargs['check'], False)
            private = temporary / 'agentbaton-native-integration'
            self.assertEqual(stat.S_IMODE(private.stat().st_mode), 0o700)
            for value in private.iterdir():
                self.assertEqual(stat.S_IMODE(value.stat().st_mode), 0o600)

    def test_driver_failure_still_runs_second_case_and_cannot_pass(self):
        code = program(DRIVER_STEP, 'INTEGRATION_NATIVE')
        calls = fake_process(code, [7, 0])
        with fixture() as (_, temporary):
            with self.assertRaisesRegex(SystemExit, '1'):
                code['run_integration_cases']()
            self.assertEqual(len(calls), 2)
            saved = json.loads((temporary / 'agentbaton-native-integration/driver-status.json').read_text())
            self.assertEqual(saved, {'last-page': {'driver_exit': 7}, 'first-page': {'driver_exit': 0}})

    def test_driver_launch_exception_is_unknown_not_zero_and_second_case_runs(self):
        code = program(DRIVER_STEP, 'INTEGRATION_NATIVE')
        calls = fake_process(code, [OSError(SECRET), 0])
        with fixture() as (_, temporary):
            with self.assertRaises(SystemExit):
                code['run_integration_cases']()
            self.assertEqual(len(calls), 2)
            self.assertIsNone(json.loads((temporary / 'agentbaton-native-integration/driver-status.json').read_text())['last-page']['driver_exit'])

    def test_existing_private_directory_is_never_reused_or_overwritten(self):
        code = program(DRIVER_STEP, 'INTEGRATION_NATIVE')
        calls = fake_process(code, [0, 0])
        with fixture() as (_, temporary):
            private = temporary / 'agentbaton-native-integration'
            private.mkdir(mode=0o700)
            (private / 'original').write_text(SECRET)
            with self.assertRaises(FileExistsError):
                code['run_integration_cases']()
            self.assertEqual(calls, [])
            self.assertEqual((private / 'original').read_text(), SECRET)

    def test_runner_temp_alias_is_resolved_before_private_output(self):
        code = program(DRIVER_STEP, 'INTEGRATION_NATIVE')
        calls = fake_process(code, [0, 0])
        with fixture() as (root, temporary):
            alias = root / 'temporary-alias'
            alias.symlink_to(temporary, target_is_directory=True)
            with mock.patch.dict(os.environ, {'RUNNER_TEMP': str(alias)}):
                code['run_integration_cases']()
            self.assertEqual(calls[0][0][11], str(temporary / 'agentbaton-native-integration/last-page'))

    def test_projection_success_requires_both_actual_driver_exit_codes(self):
        for exits in ([0, 0], [1, 0], [0, None]):
            with self.subTest(exits=exits), fixture() as (root, temporary):
                seed_reports(temporary, exits)
                code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
                calls = fake_process(code, [0, 0])
                if exits == [0, 0]:
                    code['project_integration_cases']()
                else:
                    with self.assertRaises(SystemExit):
                        code['project_integration_cases']()
                self.assertEqual(len(calls), 2)
                summary = json.loads((root / 'artifacts/native-smoke/integration-execution.json').read_text())
                self.assertEqual(summary['status'], 'driver_and_projection_passed' if exits == [0, 0] else 'failed')
                self.assertIs(summary['runtime_acceptance'], False)
                self.assertIs(summary['private_history_published'], False)
                self.assertNotIn(SECRET, json.dumps(summary))
                for command, _ in calls:
                    self.assertEqual(command[1], 'scripts/record-integration-owner-ci-evidence.py')
                    self.assertEqual(command[-2:], ['--baseline-sequence', '7'])
                self.assertEqual(stat.S_IMODE((root / 'artifacts/native-smoke/integration-execution.json').stat().st_mode), 0o600)

    def test_projection_failure_is_not_washed_by_successful_driver(self):
        with fixture() as (root, temporary):
            seed_reports(temporary)
            code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
            calls = fake_process(code, [1, 0])
            with self.assertRaises(SystemExit):
                code['project_integration_cases']()
            self.assertEqual(len(calls), 2)
            summary = json.loads((root / 'artifacts/native-smoke/integration-execution.json').read_text())
            self.assertIs(summary['cases']['last-page']['complete'], False)

    def test_missing_reports_publish_only_finite_failure_and_fail(self):
        with fixture() as (root, _):
            code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
            calls = fake_process(code, [])
            with self.assertRaises(SystemExit):
                code['project_integration_cases']()
            self.assertEqual(calls, [])
            summary = json.loads((root / 'artifacts/native-smoke/integration-execution.json').read_text())
            self.assertEqual(summary['status'], 'failed')
            self.assertTrue(all(value == {'driver_exit': None, 'projection_exit': None, 'complete': False} for value in summary['cases'].values()))

    def test_baseline_bool_negative_overflow_and_float_fail_before_projector(self):
        for value in (True, -1, 1 << 64, 7.0):
            with self.subTest(value=value), fixture() as (_, temporary):
                private = seed_reports(temporary)
                private_json(private / 'last-page/cancel/owner-dialog.json', {'integration_approval_database': {'before': {'sequence': value}}})
                code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
                calls = fake_process(code, [0])
                with self.assertRaises(SystemExit):
                    code['project_integration_cases']()
                self.assertEqual(len(calls), 1)
                self.assertIn('first-page', calls[0][0][3])

    def test_invalid_driver_status_never_becomes_success_or_public_secret(self):
        for value in ({'last-page': {'driver_exit': True}, 'first-page': {'driver_exit': 0}},
                      {'last-page': {'driver_exit': 0}, 'first-page': {'driver_exit': 0}, SECRET: SECRET}):
            with self.subTest(value=value), fixture() as (root, temporary):
                private = seed_reports(temporary)
                private_json(private / 'driver-status.json', value)
                code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
                fake_process(code, [0, 0])
                with self.assertRaises(SystemExit):
                    code['project_integration_cases']()
                raw = (root / 'artifacts/native-smoke/integration-execution.json').read_text()
                self.assertNotIn(SECRET, raw)
                self.assertEqual(json.loads(raw)['status'], 'failed')

    def test_projection_exception_keeps_second_case_and_unknown_exit(self):
        with fixture() as (root, temporary):
            seed_reports(temporary)
            code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
            calls = fake_process(code, [OSError(SECRET), 0])
            with self.assertRaises(SystemExit):
                code['project_integration_cases']()
            self.assertEqual(len(calls), 2)
            self.assertIsNone(json.loads((root / 'artifacts/native-smoke/integration-execution.json').read_text())['cases']['last-page']['projection_exit'])

    def test_native_report_symlink_is_refused_without_reading_target(self):
        with fixture() as (root, temporary):
            private = seed_reports(temporary)
            target = root / 'untrusted-report'
            target.write_text(SECRET)
            report = private / 'last-page/cancel/owner-dialog.json'
            report.unlink()
            report.symlink_to(target)
            code = program(PROJECT_STEP, 'INTEGRATION_EVIDENCE')
            calls = fake_process(code, [0])
            with self.assertRaises(SystemExit):
                code['project_integration_cases']()
            self.assertEqual(len(calls), 1)
            self.assertEqual(target.read_text(), SECRET)

    def test_existing_runner_cancel_option_is_phase_scoped_not_cancel_only(self):
        source = (ROOT / 'scripts/integration-owner-fixture.py').read_text()
        self.assertIn("for phase in ['cancel', 'approve']:", source)
        self.assertIn("pending_error(owner, after, before) if phase == 'cancel' else approved_error", source)
        self.assertIn('deadline = time.monotonic()+85', source)
        self.assertIn("reply.get('error',{}).get('side_effect_state') == 'not_started'", source)
        self.assertNotIn('integration-cancel-only', step(DRIVER_STEP)['run'])


if __name__ == '__main__':
    unittest.main()
