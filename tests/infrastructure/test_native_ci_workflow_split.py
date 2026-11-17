"""Raw DAG/source contracts; no Cargo, native process or remote CI execution."""
import copy
import hashlib
import json
from pathlib import Path
import unittest

import yaml
from tests.infrastructure.native_ci_workflow_split import (
    BASE_SHA256, ORIGINAL_STEP_SHA256, restore_native_ci_split)

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/product.yml'


def require(value):
    if not value:
        raise ValueError('Unsafe split DAG')


def validate_actual(document):
    require(document['permissions'] == {'contents': 'read'})
    require(document['concurrency'] == {'group': 'product-${{ github.ref }}', 'cancel-in-progress': 'true'})
    jobs = document['jobs']
    require(set(jobs) == {'core', 'frontend', 'desktop', 'native', 'desktop_acceptance'})
    core, native, gate = (jobs[k] for k in ('desktop', 'native', 'desktop_acceptance'))
    require(core['runs-on'] == '${{ matrix.os }}' and core['strategy'] == {
        'fail-fast': 'false', 'max-parallel': '2', 'matrix': {'os': ['macos-15']}})
    require(core['timeout-minutes'] == '60')
    require(native['runs-on'] == '${{ matrix.os }}' and native['strategy'] == core['strategy']
            and native['timeout-minutes'] == '30' and native['needs'] == 'desktop')
    require(gate['name'] == 'Product / macos-15' and gate['needs'] == ['desktop', 'native']
            and gate['if'] == 'always()' and gate['runs-on'] == 'ubuntu-24.04'
            and gate['timeout-minutes'] == '5')
    require(jobs['core']['name'] == 'Product / Rust' and jobs['frontend']['name'] == 'Product / UI')
    require(jobs['core']['timeout-minutes'] == '25' and jobs['frontend']['timeout-minutes'] == '20')
    for name, job in jobs.items():
        require('continue-on-error' not in job and 'permissions' not in job)
        if name != 'desktop_acceptance':
            require('if' not in job)
        for step in job['steps']:
            require('continue-on-error' not in step and 'timeout-minutes' not in step)
            if step.get('uses', '').startswith('actions/checkout@'):
                require(step['with'] == {'persist-credentials': 'false'})
    require(len([j for j in jobs.values() if j['name'] == 'Product / macos-15']) == 1)
    for job in (core, native, gate):
        require(job['env']['NATIVE_CI_PR_HEAD'] == '${{ github.event.pull_request.head.sha || github.sha }}')
    require('NATIVE_CI_CANCELLED' not in gate['env'])
    require(gate['env']['NATIVE_CI_NEEDS'] == '${{ toJSON(needs) }}')
    gate_runs = '\n'.join(s.get('run', '') for s in gate['steps'])
    cancel = next(s for s in gate['steps'] if s.get('name') == 'Refuse cancelled workflow')
    require(cancel['if'] == 'cancelled()' and cancel['run'] == 'exit 1')
    for step in gate['steps']:
        if 'native-ci-handoff.py aggregate' in step.get('run', ''):
            require(step['if'] == '${{ !cancelled() && success() }}'
                    and step['env']['NATIVE_CI_CANCELLED'] == 'false')
    require("needs[name]['result'] != 'success'" in gate_runs
            and "os.environ['NATIVE_CI_CANCELLED'] != 'false'" in gate_runs
            and 'native-ci-handoff.py aggregate' in gate_runs)
    downloads = [s for j in (native, gate) for s in j['steps'] if s.get('uses', '').startswith('actions/download-artifact@')]
    require(len(downloads) == 3)
    require([s['with']['artifact-ids'] for s in downloads] == [
        '${{ needs.desktop.outputs.artifact_id }}', '${{ needs.desktop.outputs.artifact_id }}', '${{ needs.native.outputs.artifact_id }}'])
    for step in downloads:
        require(set(step['with']) == {'artifact-ids', 'path', 'merge-multiple'}
                and step['with']['merge-multiple'] == 'true')
    native_steps = native['steps']
    runs = '\n'.join(s.get('run', '') for s in native_steps)
    require('npm --prefix apps/desktop ci --ignore-scripts --no-audit --no-fund' in runs
            and 'npm --prefix apps/desktop run build' in runs
            and 'cargo build --locked --bin batond --bin baton' in runs
            and 'bash scripts/prepare-desktop-sidecars.sh' in runs)
    names = [s.get('name', '') for s in native_steps]
    order = ['Validate current-run handoff and rebuild actual consumer companions',
             'Actual macOS WebView and daemon lifecycle acceptance',
             'Verify actual reconstructed Native companion bytes',
             'Actual native Owner Grant dialog and private database acceptance',
             'Actual Native integration approval and early cancellation',
             'Build desktop UI and application',
             'Audit the actual default release application for test controls',
             'Package development application and evidence',
             'Seal complete Native and default application acceptance']
    require([names.index(n) for n in order] == sorted(names.index(n) for n in order))
    complete_index = names.index(order[-1])
    require(native_steps[complete_index - 1]['uses'].startswith('actions/upload-artifact@')
            and native_steps[complete_index - 1]['with']['path'] == 'artifacts/development/'
            and 'if' not in native_steps[complete_index - 1]
            and native_steps[complete_index]['if'] == '${{ !cancelled() && success() }}'
            and native_steps[complete_index]['env']['NATIVE_CI_CANCELLED'] == 'false')
    pins = json.loads((ROOT / 'infra/action-pins.json').read_text())['actions']
    for job in jobs.values():
        for step in job['steps']:
            if 'uses' in step:
                action, commit = step['uses'].split('@')
                require(len(commit) == 40 and pins[action]['commit'] == commit)


class WorkflowSplit(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text()
        self.document = yaml.load(self.text, Loader=yaml.BaseLoader)

    def test_complete_byte_inverse_preserves_every_original_command_and_pin(self):
        original = restore_native_ci_split(self.text)
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(), BASE_SHA256)
        self.assertEqual(BASE_SHA256, 'edfffb07b5d5c8ecb3039962f5ad54e77cd187a4afd33f89941564b89603bff4')
        self.assertEqual(len(ORIGINAL_STEP_SHA256), 25)
        self.assertEqual(restore_native_ci_split(original), original)

    def test_actual_dag_fresh_preparation_original_budgets_pins_and_protected_gate(self):
        validate_actual(self.document)

    def test_unknown_new_byte_old_command_permission_or_budget_cannot_be_normalized(self):
        for before, after in [('timeout-minutes: 30', 'timeout-minutes: 31'),
                ('contents: read', 'contents: write'), ('--bundles app', '--bundles dmg'),
                ('needs: [desktop, native]', 'needs: native'), ('if: always()', 'if: success()'),
                ('merge-multiple: true', 'merge-multiple: false')]:
            self.assertIn(before, self.text)
            with self.subTest(before=before), self.assertRaises(ValueError):
                restore_native_ci_split(self.text.replace(before, after, 1))
        with self.assertRaises(ValueError):
            restore_native_ci_split(self.text + '# unreviewed byte\n')

    def test_raw_dag_negative_conditions_are_checked_without_inverse(self):
        changes = [
            ('desktop_acceptance', 'needs', ['native']),
            ('desktop_acceptance', 'if', 'success()'),
            ('desktop_acceptance', 'continue-on-error', 'true'),
            ('native', 'runs-on', 'ubuntu-24.04'),
            ('native', 'timeout-minutes', '31'),
            ('native', 'if', 'always()'),
            ('desktop', 'timeout-minutes', '61')]
        for job, key, value in changes:
            changed = copy.deepcopy(self.document)
            changed['jobs'][job][key] = value
            with self.subTest(job=job, key=key), self.assertRaises(ValueError):
                validate_actual(changed)
        changed = copy.deepcopy(self.document)
        changed['jobs']['native']['steps'][-3]['continue-on-error'] = 'true'
        with self.assertRaises(ValueError): validate_actual(changed)


if __name__ == '__main__':
    unittest.main()
