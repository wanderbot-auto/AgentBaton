"""Source wiring controls only; these do not execute historical tools or migration."""
from pathlib import Path
import shlex
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]


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
            self.assertIn('--target-dir "$CARGO_TARGET_DIR"', lines[2])
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


if __name__ == '__main__':
    unittest.main()
