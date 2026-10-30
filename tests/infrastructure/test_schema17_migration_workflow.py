"""Source wiring controls only; these do not execute historical tools or migration."""
from pathlib import Path
import copy
import hashlib
import shlex
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]
BASELINE_PRODUCT_SHA256 = '4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d'
HISTORICAL_BUILDERS = {
    14: 'scripts/prepare-legacy-migration-fixture.py',
    15: 'scripts/prepare-schema15-migration-fixture.py',
    16: 'scripts/prepare-schema16-migration-fixture.py',
}


def historical_targets(job):
    """Read only the fixed builder argv; never run Cargo or historical tools."""
    current = next(step for step in job['steps']
                   if 'cargo build --workspace --locked --bins' in step.get('run', ''))
    if 'export CARGO_TARGET_DIR="$GITHUB_WORKSPACE/target"' not in current['run'].splitlines():
        raise ValueError('Current Cargo target contract changed')
    targets = {}
    for schema, builder in HISTORICAL_BUILDERS.items():
        commands = [shlex.split(line) for step in job['steps'] for line in step.get('run', '').splitlines()
                    if line.startswith('python3 ' + builder + ' ')]
        if len(commands) != 1 or commands[0].count('--target-dir') != 1:
            raise ValueError('One exact historical target argument is required')
        argv = commands[0]
        at = argv.index('--target-dir')
        if at + 1 == len(argv):
            raise ValueError('Historical target argument is missing')
        targets[schema] = argv[at + 1]
    if len(set(targets.values())) != 3:
        raise ValueError('Historical target directories must be distinct')
    if any(target in ('$CARGO_TARGET_DIR', '${CARGO_TARGET_DIR}', '$GITHUB_WORKSPACE/target')
           for target in targets.values()):
        raise ValueError('Historical targets cannot reuse the current Cargo target')
    if any(target != f'$RUNNER_TEMP/agentbaton-legacy{schema}-target'
           for schema, target in targets.items()):
        raise ValueError('Historical target must use its exact private revision path')
    return targets


def verify_only_six_historical_target_changes(text):
    """Reverse exactly six approved argv edits, preserving every other byte.

    The digest pins the assigned82773a3 product workflow, not an external git
    object/network lookup. Future workflow changes need their own reviewed
    baseline update; this guard cannot silently normalize tests or permissions.
    """
    workflow = yaml.load(text, Loader=yaml.BaseLoader)
    for lane in ('core', 'desktop'):
        historical_targets(workflow['jobs'][lane])
    seen = {schema: 0 for schema in HISTORICAL_BUILDERS}
    original = []
    for line in text.splitlines(keepends=True):
        for schema, builder in HISTORICAL_BUILDERS.items():
            if line.lstrip().startswith('python3 ' + builder + ' '):
                expected = f'--target-dir "$RUNNER_TEMP/agentbaton-legacy{schema}-target"'
                if line.count(expected) != 1:
                    raise ValueError('Only the exact historical target argv can be normalized')
                line = line.replace(expected, '--target-dir "$CARGO_TARGET_DIR"')
                seen[schema] += 1
                break
        original.append(line)
    if seen != {14: 2, 15: 2, 16: 2}:
        raise ValueError('Exactly six historical target arguments are required')
    digest = hashlib.sha256(''.join(original).encode('utf-8')).hexdigest()
    if digest != BASELINE_PRODUCT_SHA256:
        raise ValueError('Workflow changed outside the approved six target arguments')


class Genuine16WorkflowWiring(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.load((ROOT / '.github/workflows/product.yml').read_text(), Loader=yaml.BaseLoader)

    def lanes(self):
        lanes = []
        for job in self.workflow['jobs'].values():
            if any('Build genuine schema16' in step.get('name', '') for step in job['steps']):
                lanes.append(job)
        self.assertEqual(len(lanes), 2)  # Rust plus the original two-platform macOS matrix.
        return lanes

    def test_existing_job_names_permissions_timeouts_and_matrix_are_retained(self):
        self.assertEqual(self.workflow['permissions'], {'contents': 'read'})
        self.assertEqual(self.workflow['concurrency'], {'group': 'product-${{ github.ref }}', 'cancel-in-progress': 'true'})
        self.assertEqual({j['name'] for j in self.workflow['jobs'].values()},
                         {'Product / Rust', 'Product / UI', 'Product / ${{ matrix.os }}'})
        self.assertEqual(self.workflow['jobs']['core']['timeout-minutes'], '25')
        mac = next(j for j in self.workflow['jobs'].values() if 'strategy' in j)
        self.assertEqual(mac['timeout-minutes'], '60')
        self.assertEqual(mac['strategy']['matrix']['os'], ['macos-15', 'macos-15-intel'])

    def test_build_and_generate_use_exact_source_builder_and_private_pair_before_current_build(self):
        for job in self.lanes():
            steps = job['steps']
            build = next(s for s in steps if s.get('name', '').startswith('Build genuine schema16'))
            lines = build['run'].splitlines()
            self.assertEqual(lines[:2], ['set -euo pipefail', 'export CARGO_TARGET_DIR="$GITHUB_WORKSPACE/target"'])
            self.assertEqual(shlex.split(lines[2])[:2], ['python3', 'scripts/prepare-schema16-migration-fixture.py'])
            self.assertIn('--repository "$GITHUB_WORKSPACE"', lines[2])
            self.assertIn('--target-dir "$RUNNER_TEMP/agentbaton-legacy16-target"', lines[2])
            self.assertEqual(shlex.split(lines[3]), ['python3', 'scripts/generate-schema16-migration-fixture.py',
                '--frozen', '$RUNNER_TEMP/agentbaton-legacy16-build/frozen',
                '--build-manifest', '$RUNNER_TEMP/agentbaton-legacy16-build/frozen/source-build-manifest.json',
                '--output', '$RUNNER_TEMP/agentbaton-legacy16-fixture'])
            current = next(s for s in steps if 'cargo build --workspace --locked --bins' in s.get('run', ''))
            self.assertLess(steps.index(build), steps.index(current))
            self.assertIn('agentbaton-schema17-tools', current['run'])

    def test_required_gate_passes_manifest_file_and_executes_real_test_and_clippy_fail_hard(self):
        for job in self.lanes():
            gate = next(s for s in job['steps'] if s.get('name', '').startswith('Migrate genuine nonempty schema16'))
            self.assertNotIn('if', gate)
            self.assertNotIn('continue-on-error', gate)
            self.assertEqual(gate['env'], {'AGENTBATON_ACTUAL_SCHEMA16_FIXTURE': '${{ runner.temp }}/agentbaton-legacy16-fixture/manifest.json', 'RUST_TEST_THREADS': '1'})
            self.assertEqual(gate['run'].splitlines(), ['set -euo pipefail',
                'cargo test --locked -p agentbaton-core --features actual-schema16-migration-tests --test integration_actual16_migration -- --nocapture --test-threads=1',
                'cargo clippy --locked -p agentbaton-core --features actual-schema16-migration-tests --test integration_actual16_migration -- -D warnings'])

    def test_older_fourteen_and_fifteen_gates_and_always_finite_publishers_remain(self):
        for job in self.lanes():
            runs = '\n'.join(s.get('run', '') for s in job['steps'])
            for producer in ['scripts/prepare-legacy-migration-fixture.py', 'scripts/prepare-schema15-migration-fixture.py',
                             'crates/core/tests/fixtures/child_goal_legacy_migration/generate.py',
                             'crates/core/tests/fixtures/task_workspace_actual15_migration/generate.py']:
                self.assertIn(producer, runs)
            for feature, target in [('actual-legacy-migration-tests', 'factory_child_goal_legacy_migration'),
                                    ('actual-schema15-migration-tests', 'task_workspace_actual15_migration')]:
                self.assertIn('cargo test --locked -p agentbaton-core --features '+feature+' --test '+target+' -- --nocapture --test-threads=1', runs)
                self.assertIn('cargo clippy --locked -p agentbaton-core --features '+feature+' --test '+target+' -- -D warnings', runs)
            publisher = next(s for s in job['steps'] if 'scripts/record-legacy-migration-ci-evidence.py' in s.get('run', ''))
            self.assertEqual(publisher['if'], 'always()')
            self.assertNotIn('legacy16-fixture', next(s for s in job['steps'] if s.get('uses', '').startswith('actions/upload-artifact'))['with']['path'])

    def test_three_historical_targets_are_distinct_from_each_other_and_current_target(self):
        for job in self.lanes():
            self.assertEqual(historical_targets(job), {
                schema: f'$RUNNER_TEMP/agentbaton-legacy{schema}-target'
                for schema in (14, 15, 16)
            })

    def test_shared_historical_or_current_targets_fail_closed_in_each_lane(self):
        for lane in ('core', 'desktop'):
            for schema in (14, 15, 16):
                for bad in ('$CARGO_TARGET_DIR', '${CARGO_TARGET_DIR}', '$GITHUB_WORKSPACE/target',
                            '$RUNNER_TEMP/agentbaton-legacy' + str(15 if schema == 14 else 14) + '-target',
                            '$RUNNER_TEMP/agentbaton-shared-legacy-target',
                            '$RUNNER_TEMP/agentbaton-legacy' + str(schema) + '-target/../target'):
                    with self.subTest(lane=lane, schema=schema, bad=bad):
                        job = copy.deepcopy(self.workflow['jobs'][lane])
                        step = next(s for s in job['steps'] if HISTORICAL_BUILDERS[schema] in s.get('run', ''))
                        step['run'] = step['run'].replace(
                            f'--target-dir "$RUNNER_TEMP/agentbaton-legacy{schema}-target"',
                            f'--target-dir "{bad}"')
                        with self.assertRaises(ValueError):
                            historical_targets(job)

    def test_missing_or_duplicate_target_arguments_fail_closed(self):
        for lane in ('core', 'desktop'):
            for schema in (14, 15, 16):
                expected = f'--target-dir "$RUNNER_TEMP/agentbaton-legacy{schema}-target"'
                for replacement in ('', expected + ' ' + expected):
                    with self.subTest(lane=lane, schema=schema, duplicate=bool(replacement)):
                        job = copy.deepcopy(self.workflow['jobs'][lane])
                        step = next(s for s in job['steps'] if HISTORICAL_BUILDERS[schema] in s.get('run', ''))
                        step['run'] = step['run'].replace(expected, replacement)
                        with self.assertRaises(ValueError):
                            historical_targets(job)

    def test_exact_six_argument_reverse_normalization_retains_the_entire_old_workflow(self):
        verify_only_six_historical_target_changes((ROOT / '.github/workflows/product.yml').read_text())

    def test_reverse_normalization_does_not_hide_any_other_workflow_change(self):
        original = (ROOT / '.github/workflows/product.yml').read_text()
        for old, new in [
            ('timeout-minutes: 60', 'timeout-minutes: 61'),
            ('contents: read', 'contents: write'),
            ('cargo test --workspace --locked', 'cargo test --workspace --locked -- --ignored'),
            ('--source "$CARGO_TARGET_DIR/debug"', '--source "$RUNNER_TEMP/agentbaton-legacy14-target/debug"'),
            ('--output "$RUNNER_TEMP/agentbaton-legacy14-fixture"', '--output "$RUNNER_TEMP/other-fixture"'),
        ]:
            with self.subTest(old=old):
                self.assertIn(old, original)
                with self.assertRaises(ValueError):
                    verify_only_six_historical_target_changes(original.replace(old, new, 1))


if __name__ == '__main__':
    unittest.main()
