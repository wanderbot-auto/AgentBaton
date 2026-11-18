import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREFLIGHT = 'environment-preflight.yml'
PRODUCT = 'product.yml'
MARKER = 'SYNTHETIC_CREDENTIAL_PLACEHOLDER'
CHECKOUT = '11d5960a326750d5838078e36cf38b85af677262'
HEADER = "name: Workflow admission fixture\non: pull_request\npermissions: {contents: read}\n"
RUNNER_JOB = '''\
jobs:
  verify:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo ordinary
'''
CALL_JOB = '''\
jobs:
  verify:
    uses: ./.github/workflows/reusable.yaml
'''


def expression(source):
    return '${{ ' + source + ' }}'


class WorkflowCredentialIsolationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.fixture = Path(temporary.name)
        self.workflows = self.fixture / '.github/workflows'
        for name in (
            'scripts/check_gate.py',
            'scripts/validate_infrastructure.py',
            'infra/execution-gate.json',
            'infra/action-pins.json',
            'docs/execution/launch/APPROVAL.md',
            'package-lock.json',
            'tools/environment-smoke/Cargo.lock',
            f'.github/workflows/{PREFLIGHT}',
            f'.github/workflows/{PRODUCT}',
        ):
            target = self.fixture / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)

    def validator(self, root=None):
        return subprocess.run(
            [sys.executable, 'scripts/validate_infrastructure.py'],
            cwd=root or self.fixture, capture_output=True, text=True, timeout=15,
        )

    def write_workflow(self, body=RUNNER_JOB, *, filename='admission.yaml', header=HEADER):
        (self.workflows / filename).write_text(header + body, encoding='utf-8')

    def assert_admitted(self, files, pins):
        result = self.validator()
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout)
        self.assertEqual(output['workflow_files'], files)
        self.assertEqual(output['pinned_action_references'], pins)
        self.assertFalse(output['remote_ci_executed'])

    def assert_denied(self, reason, filename='admission.yaml'):
        result = self.validator()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse(result.stdout)
        self.assertIn(f'.github/workflows/{filename}', result.stderr)
        self.assertIn(reason, result.stderr)
        self.assertNotIn(MARKER, result.stderr)

    def test_actual_entry_checks_the_checked_in_workflows(self):
        result = self.validator(ROOT)
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout)
        self.assertGreaterEqual(output['workflow_files'], 2)
        self.assertEqual(output['scope'], 'static_infrastructure_only')
        self.assertFalse(output['remote_ci_executed'])
        self.assert_admitted(2, 15)

    def test_yaml_suffix_narrow_permissions_and_builtin_token(self):
        for token in (expression('github.token'), expression("github['token']")):
            with self.subTest(token=token):
                action = (
                    f'      - uses: actions/checkout@{CHECKOUT}\n'
                    f'        with:\n          persist-credentials: false\n          token: "{token}"\n'
                )
                job = RUNNER_JOB.replace('      - run: echo ordinary\n', action)
                self.write_workflow(job)
                self.assert_admitted(3, 16)
        self.write_workflow(RUNNER_JOB.replace('    steps:', '    permissions: {}\n    steps:'))
        self.assert_admitted(3, 15)

    def test_ignores_comments_and_literal_expression_arguments(self):
        job = RUNNER_JOB.replace('      - run: echo ordinary\n', '''\
      - run: |
          echo "$SECRETS_LIKE_LOCAL_VARIABLE"
          echo "${{ contains('literal secrets.placeholder }}', 'secrets') }}"
          echo "${{ steps.secrets.outputs.result }}"
''')
        self.write_workflow('# text mentioning secrets.placeholder and pull_request_target\n' + job)
        self.assert_admitted(3, 15)

    def test_explicit_job_and_workflow_permissions(self):
        self.write_workflow(header=HEADER.replace('{contents: read}', '{contents: write}'))
        self.assert_denied('CI credentials must stay read-only')
        for permissions in ('{contents: write}', '{contents: read, id-token: write}', 'write-all', 'read-all'):
            with self.subTest(permissions=permissions):
                self.write_workflow(RUNNER_JOB.replace(
                    '    timeout-minutes:', f'    Permissions: {permissions}\n    timeout-minutes:',
                ))
                self.assert_denied('.jobs.verify: job credentials must stay read-only')
        self.write_workflow(RUNNER_JOB + '        permissions: {contents: write}\n')
        self.assert_denied('.jobs.verify.steps[0]: workflow-level credential settings forbidden')

    def test_environment_and_privileged_triggers(self):
        for declaration in (f'Environment: {MARKER}', f'environment: {{name: {MARKER}}}'):
            with self.subTest(declaration=declaration):
                self.write_workflow(RUNNER_JOB.replace(
                    '    timeout-minutes:', f'    {declaration}\n    timeout-minutes:',
                ))
                self.assert_denied('CI environment binding forbidden')
        for events in ('pull_request_target', '[push, workflow_run]', '{pull_request_target: {}, push: {}}'):
            with self.subTest(events=events):
                self.write_workflow(header=HEADER.replace('pull_request', events))
                self.assert_denied('Privileged PR trigger forbidden')

    def test_reusable_calls_do_not_get_new_admission_or_secret_transfer(self):
        call_header = HEADER.replace('pull_request', 'workflow_call')
        self.write_workflow(filename='reusable.yaml', header=call_header)
        self.write_workflow(CALL_JOB)
        self.assert_denied('reusable workflow calls are not approved')
        for transfer in (
            'inherit',
            f'{{KEY: "{MARKER}"}}',
            '{KEY: "' + expression(f'secrets[{MARKER!r}]') + '"}',
        ):
            with self.subTest(transfer=transfer):
                self.write_workflow(CALL_JOB + f'    SeCrEtS: {transfer}\n')
                self.assert_denied('reusable workflow secret transfer forbidden')
        self.write_workflow()
        self.write_workflow(
            filename='reusable.yaml',
            header=call_header.replace(
                'on: workflow_call', f'on: {{workflow_call: {{secrets: {{{MARKER}: {{required: false}}}}}}}}',
            ),
        )
        self.assert_denied('reusable workflow secret declarations forbidden', filename='reusable.yaml')
        self.write_workflow(
            RUNNER_JOB.replace('    steps:', f'    environment: {MARKER}\n    steps:'),
            filename='reusable.yaml', header=call_header,
        )
        self.assert_denied('CI environment binding forbidden', filename='reusable.yaml')

    def test_secret_context_dot_index_and_whole_object_references(self):
        for source in (
            f'secrets.{MARKER}',
            f"SeCrEtS  [ '{MARKER}' ]",
            "format('embedded }} bracket: {0}', " + f"sEcReTs['{MARKER}'])",
            'toJSON(secrets)',
            "secrets['GITHUB_TOKEN']",
        ):
            with self.subTest(source=source):
                self.write_workflow(RUNNER_JOB + f'        env:\n          KEY: "{expression(source)}"\n')
                self.assert_denied('.jobs.verify: secret context forbidden')
        self.write_workflow(RUNNER_JOB + f"        if: SECRETS ['{MARKER}'] != ''\n")
        self.assert_denied('secret context forbidden')
        self.write_workflow(RUNNER_JOB + (
            "        env:\n          KEY: >-\n"
            "            " + expression(f"SeCrEtS\n              ['{MARKER}']") + '\n'
        ))
        self.assert_denied('secret context forbidden')
        fragments = expression('github.sha') + ':' + expression(f'secrets.{MARKER}')
        self.write_workflow(RUNNER_JOB + f'        env:\n          KEY: "{fragments}"\n')
        self.assert_denied('secret context forbidden')

    def test_aliases_keep_the_resolved_credential_semantics(self):
        job = RUNNER_JOB.replace('    steps:', '    permissions: *token\n    steps:')
        self.write_workflow(job, header=HEADER.replace('permissions:', 'permissions: &token'))
        self.assert_admitted(3, 15)
        self.write_workflow('env: &token {contents: write}\n' + job)
        self.assert_denied('job credentials must stay read-only')
        job = RUNNER_JOB.replace('    steps:', '    env: *credential\n    steps:')
        value = expression(f"secrets ['{MARKER}']")
        self.write_workflow(f'env: &credential {{VALUE: "{value}"}}\n' + job)
        self.assert_denied('secret context forbidden')

    def test_rejects_duplicate_and_ambiguous_yaml_without_echoing_source(self):
        for field in (
            'permissions: {contents: write}\n',
            'Permissions: {contents: write}\n',
            f'env: {{{MARKER}: one, {MARKER}: two}}\n',
            f'env: &defaults {{<<: {{KEY: {MARKER}}}}}\n',
            f'env: [{MARKER}\n',
        ):
            with self.subTest(field=field):
                self.write_workflow(field + RUNNER_JOB)
                self.assert_denied('invalid or ambiguous workflow YAML')

    def test_action_credential_slots(self):
        for inputs, reason in (
            ('          persist-credentials: true\n', 'checkout must disable credential persistence'),
            ('', 'checkout must disable credential persistence'),
            (f'          persist-credentials: false\n          ssh-key: {MARKER}\n', 'checkout SSH key forbidden'),
            (f'          persist-credentials: false\n          token: "{MARKER}"\n', 'action token must use the built-in GitHub token'),
            ('          persist-credentials: false\n          token: "${{ vars.KEY }}"\n', 'action token must use the built-in GitHub token'),
        ):
            with self.subTest(reason=reason, inputs=inputs):
                action = f'      - uses: actions/checkout@{CHECKOUT}\n'
                if inputs:
                    action += '        with:\n' + inputs
                self.write_workflow(RUNNER_JOB.replace('      - run: echo ordinary\n', action))
                self.assert_denied(reason)

    def test_registry_credentials_and_non_hosted_runners(self):
        for registry in (
            f'container: {{image: example/image, credentials: {{username: user, password: {MARKER}}}}}',
            f'services: {{db: {{image: example/image, credentials: {{username: user, password: {MARKER}}}}}}}',
        ):
            with self.subTest(registry=registry):
                self.write_workflow(RUNNER_JOB.replace('    steps:', f'    {registry}\n    steps:'))
                self.assert_denied('registry credentials forbidden')
        for selector in ('[self-hosted, macos-15]', '{group: example}', '${{ vars.HOST }}'):
            with self.subTest(selector=selector):
                self.write_workflow(RUNNER_JOB.replace('ubuntu-24.04', selector))
                self.assert_denied('approved GitHub-hosted runners required')

    def test_original_required_job_name_cannot_drift(self):
        path = self.workflows / PREFLIGHT
        path.write_text(path.read_text().replace('name: Environment / ', 'name: unrelated / '))
        result = self.validator()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Expected Linux and Apple Silicon preflight hosts', result.stderr)

    def test_original_timeout_pin_and_execution_gate_still_fail_closed(self):
        path = self.workflows / PREFLIGHT
        original = path.read_text()
        for before, after, reason in (
            ('timeout-minutes: 25', 'timeout-minutes: 999', 'Unbounded CI job'),
            (f'@{CHECKOUT}', '@v4', 'Unpinned action'),
            (f'@{CHECKOUT}', '@' + '0' * 40, 'Action pin mismatch'),
        ):
            with self.subTest(reason=reason):
                path.write_text(original.replace(before, after))
                result = self.validator()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(reason, result.stderr)
        path.write_text(original)
        gate_path = self.fixture / 'infra/execution-gate.json'
        gate = json.loads(gate_path.read_text())
        gate['production_release_authorized'] = True
        gate_path.write_text(json.dumps(gate))
        result = self.validator()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Separate approval required: production_release_authorized', result.stderr)


if __name__ == '__main__':
    unittest.main()
