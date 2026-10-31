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


# Approved UI-only dev-debuginfo delta, normalized before the older diagnostic
# and whole-workflow layers. These raw-source pins authorize no arbitrary edits.
APPROVED_UI_PREVIEW_PROFILE_BASE = 'fbd5b3103794cc163ae91d9f3407fe2dfb90ea49'
APPROVED_UI_PREVIEW_PROFILE_BUILD_STEP_SHA256 = '7ea6ff61070a94e8e12f7c7da31954fed8356d7d5953b2d109564d82f72ef0c5'
APPROVED_UI_PREVIEW_PROFILE_CANDIDATE_SHA256 = '22801a9a25f763b0d26bcace5224b5d449ed6425758594fab791831dc380031f'
UI_PREVIEW_BUILD_BEFORE = '          cargo build --locked --bin batond --bin baton --features agentbaton-core/preview-evidence\n'
UI_PREVIEW_BUILD_AFTER = '          CARGO_PROFILE_DEV_DEBUG=0 cargo build --locked --bin batond --bin baton --features agentbaton-core/preview-evidence\n'
UI_PREVIEW_SCOPE_BEFORE = 'CI-built Linux debug evidence candidate; preview-evidence is non-default and release-prohibited, configured only by the private Data fixture; actual schema checked by read-only SQLite; not a release artifact'
UI_PREVIEW_SCOPE_AFTER = 'CI-built Linux dev evidence candidate; only this UI preview tool build sets CARGO_PROFILE_DEV_DEBUG=0 (debuginfo=0); dev opt-level/debug_assertions/panic defaults and existing package overrides are unchanged; preview-evidence is non-default and release-prohibited, configured only by the private Data fixture; actual schema checked by read-only SQLite; not a release artifact'


PRIVATE_BROWSER_PYTHON_BEFORE = '          python3 -m venv artifacts/browser-venv\n'
PRIVATE_BROWSER_PYTHON_AFTER = '          python3 scripts/prepare-browser-runtime.py --output artifacts/browser-venv\n'
PRIVATE_BROWSER_BUILD_STEP_SHA256 = '30ec3a29323f835e235254015eadf8375b4e9e9f1b113210dce5b0e46d1df1cc'


def strip_private_browser_python(text):
    """Reverse only the complete pinned browser dependency step's one command."""
    headers = list(re.finditer(r'^      - .*\n', text, re.MULTILINE))
    title = '      - name: Real daemon and browser test dependencies\n'
    if [i for i, h in enumerate(headers) if h.group(0) == title] != [25] or len(headers) <= 26:
        raise ValueError('Private browser dependency step count/position changed')
    if headers[26].group(0) != '      - name: Actual frontend with real Unix IPC and responsive screenshots\n':
        raise ValueError('Private browser dependency neighbor changed')
    start, end = headers[25].start(), headers[26].start()
    block = text[start:end]
    digest = hashlib.sha256(block.encode()).hexdigest()
    if digest == APPROVED_UI_PREVIEW_PROFILE_BUILD_STEP_SHA256:
        return text
    if digest != PRIVATE_BROWSER_BUILD_STEP_SHA256 or block.count(PRIVATE_BROWSER_PYTHON_AFTER) != 1:
        raise ValueError('Private browser dependency complete reviewed bytes changed')
    original = block.replace(PRIVATE_BROWSER_PYTHON_AFTER, PRIVATE_BROWSER_PYTHON_BEFORE, 1)
    if hashlib.sha256(original.encode()).hexdigest() != APPROVED_UI_PREVIEW_PROFILE_BUILD_STEP_SHA256:
        raise ValueError('Private browser exact one-command inverse changed')
    return text[:start] + original + text[end:]


def strip_approved_ui_preview_profile(text):
    """Reverse only the exact reviewed UI build step and candidate scope."""
    text = strip_private_browser_python(text)
    headers = list(re.finditer(r'^      - .*\n', text, re.MULTILINE))
    name = '      - name: Real daemon and browser test dependencies\n'
    matches = [i for i, h in enumerate(headers) if h.group(0) == name]
    if matches != [25] or len(headers) <= 27:
        raise ValueError('Reviewed UI preview build step count/position changed')
    if headers[26].group(0) != '      - name: Actual frontend with real Unix IPC and responsive screenshots\n':
        raise ValueError('Reviewed UI preview build neighbor changed')
    build_start, build_end = headers[25].start(), headers[26].start()
    build = text[build_start:build_end]
    if hashlib.sha256(build.encode('utf-8')).hexdigest() != APPROVED_UI_PREVIEW_PROFILE_BUILD_STEP_SHA256:
        raise ValueError('Reviewed UI preview build complete raw block changed')
    definition = '          candidate = {\n'
    write = "          pathlib.Path('artifacts/ui-browser/goal-execution-candidate.json').write_text(json.dumps(candidate, indent=2) + '\\n')\n"
    if text.count(definition) != 1 or text.count(write) != 1:
        raise ValueError('One exact reviewed UI preview candidate required')
    start, end = text.index(definition), text.index(write) + len(write)
    if not headers[26].start() < start < end < headers[27].start():
        raise ValueError('Reviewed UI preview candidate position changed')
    candidate = text[start:end]
    if hashlib.sha256(candidate.encode('utf-8')).hexdigest() != APPROVED_UI_PREVIEW_PROFILE_CANDIDATE_SHA256:
        raise ValueError('Reviewed UI preview candidate complete raw block changed')
    if build.count(UI_PREVIEW_BUILD_AFTER) != 1 or candidate.count(UI_PREVIEW_SCOPE_AFTER) != 1:
        raise ValueError('Reviewed UI preview profile literals changed')
    # Replace the later span first; every byte outside these two precise edits
    # still passes through all existing raw-workflow pins below unchanged.
    text = text[:start] + candidate.replace(UI_PREVIEW_SCOPE_AFTER, UI_PREVIEW_SCOPE_BEFORE, 1) + text[end:]
    return text[:build_start] + build.replace(UI_PREVIEW_BUILD_AFTER, UI_PREVIEW_BUILD_BEFORE, 1) + text[build_end:]


# Exact reviewed TaskWorkspace diagnostic layer. The old browser/native/raw
# workflow pins below are unchanged. No runtime git or network lookup is used.
APPROVED_TASK_WORKSPACE_DIAGNOSTIC_SOURCE = '3dc7f0f231fd63dcf829cc7b1e6d0488b0743313'
APPROVED_TASK_WORKSPACE_DIAGNOSTIC_FUNCTION_SHA256 = '4cd9ae4b6ec58defa7ddfd693c9e567bfa972596ceb0500fbabea35a2b852bf9'
PRE_DIAGNOSTIC_TASK_WORKSPACE_FUNCTION_SHA256 = '28da00e8807fd758282b8e9beebe005fd669daf8618b0ac41e77dda0d8e72b20'
PRE_DIAGNOSTIC_TASK_WORKSPACE_FUNCTION_BYTES = 4189


def strip_approved_task_workspace_launch_diagnostics(text):
    """Restore only the complete exact reviewed function, never arbitrary AST."""
    definition = '          def run_task_workspace_evidence():\n'
    call = '          run_task_workspace_evidence()\n'
    if text.count(definition) != 1 or text.count(call) != 1:
        raise ValueError('One exact reviewed TaskWorkspace definition/call required')
    start, end = text.index(definition), text.index(call)
    if end <= start:
        raise ValueError('Reviewed TaskWorkspace call must follow its definition')
    reviewed = text[start:end]
    if hashlib.sha256(reviewed.encode('utf-8')).hexdigest() != APPROVED_TASK_WORKSPACE_DIAGNOSTIC_FUNCTION_SHA256:
        raise ValueError('Reviewed TaskWorkspace diagnostic function bytes changed')
    fixture = ROOT / 'tests/infrastructure/fixtures/task_workspace_launch_diagnostics/pre_diagnostics_function.py.txt'
    try:
        original = fixture.read_bytes()
    except OSError:
        raise ValueError('Pinned original TaskWorkspace function unavailable') from None
    if (len(original) != PRE_DIAGNOSTIC_TASK_WORKSPACE_FUNCTION_BYTES
            or hashlib.sha256(original).hexdigest() != PRE_DIAGNOSTIC_TASK_WORKSPACE_FUNCTION_SHA256):
        raise ValueError('Pinned original TaskWorkspace function bytes changed')
    return text[:start] + original.decode('utf-8') + text[end:]


# Exact integration setup diagnostic layer; older whole-workflow pins remain.
INTEGRATION_SETUP_STEP_SHA256 = 'dd33c3a86f61185a9cefb2e313ea228830c8679bc3efb0807eaf80568d3ae003'
INTEGRATION_SETUP_NOTIFICATION_STEP_SHA256 = '1c98001b8fd0a1f95f2e3c0d0100c76240f0082168be7a4beaabfeac0e8c733e'
INTEGRATION_SETUP_ORIGINAL_NOTIFICATION_SHA256 = 'df3ddde5681355b95ae69e79bdaae674a6531f68c438102007dde0ca82d5727d'
INTEGRATION_SETUP_NOTIFICATION_FLAG = "                          '--notification-probe',\n"
INTEGRATION_SETUP_NOTIFICATION_ANCHOR = "                          '--frontend', 'apps/desktop/dist', '--frontend-build-record', str(private / 'frontend-build-record.json'),\n"
INTEGRATION_SETUP_ORIGINAL_SHA256 = 'a1db5e7447f25ac6dd8f7a3fff8005e071971cb75c0aee109ace816b23ece680'
INTEGRATION_SETUP_ORIGINAL_BYTES = 9731


def strip_integration_setup_diagnostics(text):
    """Replace only a complete pinned step with its complete pinned original."""
    text = strip_private_browser_python(text)
    headers = list(re.finditer(r'^      - .*\n', text, re.MULTILINE))
    title = '      - name: Actual integration frontend with real Core IPC and generated correctness\n'
    if [i for i, header in enumerate(headers) if header.group(0) == title] != [27] or len(headers) <= 28:
        raise ValueError('Integration setup step count/position changed')
    start, end = headers[27].start(), headers[28].start()
    step_sha = hashlib.sha256(text[start:end].encode()).hexdigest()
    if step_sha not in (INTEGRATION_SETUP_STEP_SHA256, INTEGRATION_SETUP_NOTIFICATION_STEP_SHA256):
        raise ValueError('Integration setup complete reviewed bytes changed')
    fixture = ROOT / 'tests/infrastructure/fixtures/integration_setup_diagnostics/pre_diagnostics_step.yml.txt'
    try:
        original = fixture.read_bytes()
    except OSError:
        raise ValueError('Pinned original integration setup step unavailable') from None
    if len(original) != INTEGRATION_SETUP_ORIGINAL_BYTES or hashlib.sha256(original).hexdigest() != INTEGRATION_SETUP_ORIGINAL_SHA256:
        raise ValueError('Pinned original integration setup step changed')
    restored = original.decode('utf-8')
    if step_sha == INTEGRATION_SETUP_NOTIFICATION_STEP_SHA256:
        # Restore the entire independently pinned pre-diagnostic flagged step.
        # Integration must then apply the original notification inverse next.
        if restored.count(INTEGRATION_SETUP_NOTIFICATION_ANCHOR) != 1:
            raise ValueError('Exact original notification insertion point changed')
        restored = restored.replace(INTEGRATION_SETUP_NOTIFICATION_ANCHOR,
            INTEGRATION_SETUP_NOTIFICATION_FLAG + INTEGRATION_SETUP_NOTIFICATION_ANCHOR, 1)
        if hashlib.sha256(restored.encode()).hexdigest() != INTEGRATION_SETUP_ORIGINAL_NOTIFICATION_SHA256:
            raise ValueError('Complete original notification step inverse changed')
    return text[:start] + restored + text[end:]


def strip_integration_browser_ci_layer(text):
    """Restore the reviewed diagnostic layer, then exact UI blocks/schema."""
    text = strip_integration_setup_diagnostics(text)
    text = strip_approved_ui_preview_profile(text)
    text = strip_approved_task_workspace_launch_diagnostics(text)
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
