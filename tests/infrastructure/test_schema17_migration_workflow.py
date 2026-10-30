"""Source wiring controls only; these do not execute historical tools or migration."""
from pathlib import Path
import copy
import hashlib
import re
import shlex
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]
BASELINE_PRODUCT_SHA256 = '4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d'
# Independently reviewed Native CI addition, not a replacement for the old pin.
# Runtime validation uses these exact raw UTF8 blocks, never a git/network lookup.
APPROVED_NATIVE_CI_SOURCE = 'ea9bcd12fc78016922c86b40ed84108dbf918b70'
APPROVED_NATIVE_CI_STEPS = (
    ('Native library tests with smoke features', 42,
     'e03239832ab58f2c1a37d3f75fca37644fcf17076bd023b41570ae8c9c0aa46f'),
    ('Actual Native integration approval and early cancellation', 43,
     '0c4e0ccdb9cc5f9e3a8168e6edb865772a75c2c8b629bfb3ba1bfad91710d5e5'),
    ('Retain finite Native integration evidence without private snapshots', 44,
     '0d6868d45c646b8ddf689dbb287712fce1ab493e59fba3f8d385d64010915135'),
)
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



# New UI addition is stripped as its own exact layer BEFORE the older Native
# positional pins. The old Native/raw-workflow and six-argv pins remain intact.
BROWSER_CI_BASE_SHA256 = '599074c95afb9324470bc859f11added4b5232a877ba6e0486001e1bebf015fc'
BROWSER_CI_STEPS = (
    ('Actual integration frontend with real Core IPC and generated correctness', 27,
     'a1db5e7447f25ac6dd8f7a3fff8005e071971cb75c0aee109ace816b23ece680'),
    ('Retain finite integration browser evidence without private runtime data', 28,
     '3065cc48dea7cb751aebea051ff9f8d84f4b0c090d003161ddb770043c34a431'),
)


def strip_integration_browser_ci_layer(text):
    """Reverse ONLY the two exact adjacent UI blocks and one schema literal."""
    headers = list(re.finditer(r'^      - .*\n', text, re.MULTILINE))
    indices = []
    for name, expected_index, expected_sha in BROWSER_CI_STEPS:
        matches = [i for i, header in enumerate(headers)
                   if header.group(0) == '      - name: ' + name + '\n']
        if matches != [expected_index] or expected_index + 1 >= len(headers):
            raise ValueError('Integration browser CI step count/position changed')
        block = text[headers[expected_index].start():headers[expected_index + 1].start()]
        if hashlib.sha256(block.encode()).hexdigest() != expected_sha:
            raise ValueError('Integration browser CI complete raw block changed')
        indices.append(expected_index)
    if indices != [27, 28]:
        raise ValueError('Integration browser CI adjacency changed')
    if headers[26].group(0) != '      - name: Actual frontend with real Unix IPC and responsive screenshots\n':
        raise ValueError('Integration browser CI preceding step changed')
    if headers[29].group(0) != '      - uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02\n':
        raise ValueError('Integration browser CI following artifact step changed')
    first, end = headers[27].start(), headers[29].start()
    jobs = list(re.finditer(r'^  [A-Za-z_][A-Za-z0-9_-]*:\n', text, re.MULTILINE))
    preceding = [header for header in jobs if header.start() < first]
    if not preceding or preceding[-1].group(0) != '  frontend:\n' or any(first <= h.start() < end for h in jobs):
        raise ValueError('Integration browser addition must remain in original UI job')
    result = text[:first] + text[end:]
    current = "          candidate = {\n              'schema': 17,\n              'git_head': git('rev-parse', 'HEAD'),\n"
    previous = current.replace("'schema': 17", "'schema': 16")
    if result.count(current) != 1 or result.count("              'schema': 17,\n") != 1:
        raise ValueError('One exact existing candidate schema literal is required')
    result = result.replace(current, previous, 1)
    if hashlib.sha256(result.encode()).hexdigest() != BROWSER_CI_BASE_SHA256:
        raise ValueError('Workflow changed outside exact browser blocks/schema literal')
    return result


def strip_approved_native_ci_steps(text):
    """Remove only the exact three approved adjacent desktop step blocks."""
    headers = list(re.finditer(r'^      - .*\n', text, re.MULTILINE))
    indices = []
    for name, expected_index, expected_digest in APPROVED_NATIVE_CI_STEPS:
        matches = [index for index, header in enumerate(headers)
                   if header.group(0) == '      - name: ' + name + '\n']
        if len(matches) != 1 or matches[0] != expected_index:
            raise ValueError('Reviewed Native CI step name, count or position changed')
        index = matches[0]
        if index + 1 >= len(headers):
            raise ValueError('Reviewed Native CI block boundary is missing')
        block = text[headers[index].start():headers[index + 1].start()]
        if hashlib.sha256(block.encode('utf-8')).hexdigest() != expected_digest:
            raise ValueError('Reviewed Native CI raw block bytes changed')
        indices.append(index)
    if indices != list(range(indices[0], indices[0] + 3)):
        raise ValueError('Reviewed Native CI steps must stay consecutive')
    before = '      - name: Actual Native Factory planning approval and unchanged Grant evidence\n'
    after = '      - name: Retain bounded native migration metadata without private history\n'
    if (headers[indices[0] - 1].group(0) != before
            or headers[indices[-1] + 1].group(0) != after):
        raise ValueError('Reviewed Native CI neighbors changed')
    first, end = headers[indices[0]].start(), headers[indices[-1] + 1].start()
    jobs = list(re.finditer(r'^  [a-zA-Z_][a-zA-Z0-9_-]*:\n', text, re.MULTILINE))
    preceding = [header for header in jobs if header.start() < first]
    if (not preceding or preceding[-1].group(0) != '  desktop:\n'
            or any(first <= header.start() < end for header in jobs)):
        raise ValueError('Reviewed Native CI steps must remain in desktop')
    return text[:first] + text[end:]


def verify_only_six_historical_target_changes(text):
    """Reverse exactly six approved argv edits, preserving every other byte.

    The digest pins the assigned82773a3 product workflow, not an external git
    object/network lookup. Future workflow changes need their own reviewed
    baseline update; this guard cannot silently normalize tests or permissions.
    """
    text = strip_integration_browser_ci_layer(text)
    text = strip_approved_native_ci_steps(text)
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

    def test_reviewed_native_blocks_refuse_changed_duplicate_missing_or_moved_steps(self):
        original = (ROOT / '.github/workflows/product.yml').read_bytes().decode('utf-8')
        verify_only_six_historical_target_changes(original)
        headers = list(re.finditer(r'^      - .*\n', original, re.MULTILINE))
        blocks = []
        changes = [
            ('--lib -- --test-threads=1', '--lib -- --ignored --test-threads=1'),
            ("'--integration-cancel-at', '0'", "'--integration-cancel-at', '1'"),
            ("'complete': driver == 0 and projection == 0", "'complete': projection == 0"),
        ]
        for (name, _, _), (old, new) in zip(APPROVED_NATIVE_CI_STEPS, changes):
            start = original.index('      - name: ' + name + '\n')
            end = next(header.start() for header in headers if header.start() > start)
            block = original[start:end]
            self.assertIn(old, block)
            blocks.append(block)
            without = original[:start] + original[end:]
            move_at = without.index('      - name: Toolchain\n')
            variants = {
                'content': original[:start] + block.replace(old, new, 1) + original[end:],
                'raw bytes': original[:start] + block.replace('\n', '\r\n') + original[end:],
                'duplicate': original[:start] + block + block + original[end:],
                'missing': without,
                'moved': without[:move_at] + block + without[move_at:],
            }
            for mutation, text in variants.items():
                with self.subTest(step=name, mutation=mutation):
                    with self.assertRaises(ValueError):
                        verify_only_six_historical_target_changes(text)
        # A hash-only stripper would incorrectly accept moving the complete
        # unchanged addition as a unit. Its exact position is independently bound.
        addition = ''.join(blocks)
        self.assertEqual(original.count(addition), 1)
        without = original.replace(addition, '', 1)
        move_at = without.index('      - name: Actual Native Factory planning approval and unchanged Grant evidence\n')
        moved = without[:move_at] + addition + without[move_at:]
        with self.subTest(mutation='entire addition moved'):
            with self.assertRaises(ValueError):
                verify_only_six_historical_target_changes(moved)
        # The old whole-workflow guard still rejects an unrelated legacy byte.
        with self.subTest(mutation='old retention changed'):
            self.assertIn('retention-days: 14', original)
            with self.assertRaises(ValueError):
                verify_only_six_historical_target_changes(original.replace('retention-days: 14', 'retention-days: 15', 1))


if __name__ == '__main__':
    unittest.main()
