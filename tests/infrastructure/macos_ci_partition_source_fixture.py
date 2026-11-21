"""Exact test-only inverse and action/graph contracts for the reviewed macOS partition."""
from hashlib import sha256
import re

import yaml


PREVIOUS_SHA256 = '80e53ff9052815bbdb5fa9eb2c132b4b4ba632b4e6245b096fb573206eef616c'
PARTITION_SHA256 = '854c9502d344002ff81535d62a002c9af63c96dbb5d01ea167c7724699f39b57'
PREFLIGHT_SHA256 = '182627a83a2adcf60ab27109c8f7ad6e112d079a8b6812e52bce5a27a4b47855'
DESKTOP = '  desktop:\n'
RELEASE = '  desktop_release:\n'
OLD_DESKTOP = '''  desktop:
    name: Product / ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    # Native tests, two application profiles, release audit and DMG packaging
    # must finish on the same exact checkout. Per-test/product limits stay fixed.
    timeout-minutes: 60
    strategy:
      fail-fast: false
      max-parallel: 2
      matrix:
        os: [macos-15]
    steps:
'''
CONDITIONAL_RETENTION = "        if: ${{ always() && steps.release_source_start.outcome == 'success' }}\n"
CHECKOUT = 'actions/checkout@11d5960a326750d5838078e36cf38b85af677262'
NODE = 'actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020'
PYTHON = 'actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065'
UPLOAD = 'actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02'
MATRIX = {'fail-fast': 'false', 'max-parallel': '2', 'matrix': {'os': ['macos-15']}}
DEPENDENCY_GUARD = 'Require successful macOS Core validation'
UPSTREAM_GATE_IDS = (
    'frontend', 'schema14_build', 'schema15_build', 'schema16_build', 'core_native',
    'schema14_migrate', 'schema15_migrate', 'schema16_migrate', 'migration_evidence',
    'rework_evidence', 'core_artifact',
)
NATIVE_UPLOAD_SCOPE = "${{ always() && steps.release_source_start.outcome == 'success' }}"
ANCHOR_STAGES = ('core-start', 'core-complete', 'release-check', 'release-check', 'release-complete')
UPSTREAM_OUTPUTS = {
    'source_receipt': '${{ steps.core_source_complete.outputs.source_receipt }}',
    'core_evidence_id': '${{ steps.core_artifact.outputs.artifact-id }}',
    'core_evidence_sha256': '${{ steps.core_artifact.outputs.artifact-digest }}',
}
RELEASE_INPUTS = {
    'MACOS_CORE_RECEIPT': '${{ needs.desktop.outputs.source_receipt }}',
    'MACOS_CORE_ARTIFACT_ID': '${{ needs.desktop.outputs.core_evidence_id }}',
    'MACOS_CORE_ARTIFACT_SHA256': '${{ needs.desktop.outputs.core_evidence_sha256 }}',
}
CORE_EVIDENCE = (
    'artifacts/native-smoke/factory-test-tools/probe-stdout.log',
    'artifacts/native-smoke/factory-test-tools/probe-stderr.log',
    'artifacts/native-smoke/factory-test-tools/probe-snapshot.stderr',
    'artifacts/native-smoke/factory-test-tools/probe-snapshot.json',
    'artifacts/native-smoke/factory-test-tools/probe-cleanup.json',
    'artifacts/native-smoke/factory-test-tools/probe-failure.json',
    'artifacts/native-smoke/factory-test-tools/manifest.json',
    'artifacts/native-smoke/legacy-migration/summary.json',
    'artifacts/native-smoke/factory-rework-progress.json',
)


def digest(text):
    return sha256(text.encode('utf-8')).hexdigest()


def require(value, message):
    if not value:
        raise ValueError(message)


def use(action, **inputs):
    return action, inputs


def uploaded(name, path, missing='warn'):
    return use(UPLOAD, name=name, path=path, **{'if-no-files-found': missing, 'retention-days': '14'})


def assert_action_inventory(product, preflight, *, partitioned):
    """Check exact Action and input at each use, across both workflows."""
    checkout = use(CHECKOUT, **{'persist-credentials': 'false'})
    checkout_sha = use(CHECKOUT, ref='${{ github.sha }}', **{'persist-credentials': 'false'})
    node = use(NODE, **{'node-version': '24'})
    python = use(PYTHON, **{'python-version': '3.12'})
    native = uploaded('native-webview-${{ matrix.os }}-${{ github.sha }}', 'artifacts/native-smoke/')
    development = uploaded('development-${{ matrix.os }}-${{ github.sha }}', 'artifacts/development/', 'error')
    expected = {
        'core': [checkout, python, uploaded('provider-contract-${{ github.sha }}', 'artifacts/provider-contract/')],
        'frontend': [checkout, node, python, uploaded('frontend-real-ipc-${{ github.sha }}', 'artifacts/ui-browser/')],
        'desktop': [checkout, node, native, development],
    }
    if partitioned:
        expected['desktop'] = [checkout_sha, node, uploaded(
            'native-core-${{ matrix.os }}-${{ github.sha }}', '\n'.join(CORE_EVIDENCE) + '\n')]
        expected['desktop_release'] = [checkout_sha, node, native, development]
    expected_environment = {'preflight': [
        checkout, python, node, uploaded('environment-${{ matrix.os }}-${{ github.sha }}', 'artifacts/', 'error'),
    ]}

    def inventory(document):
        return {
            job: [(step['uses'], step.get('with', {})) for step in value['steps'] if 'uses' in step]
            for job, value in document['jobs'].items()
        }

    require(inventory(product) == expected, 'Product Actions or approved inputs changed')
    require(inventory(preflight) == expected_environment, 'Environment Actions or approved inputs changed')
    return sum(map(len, expected.values())) + sum(map(len, expected_environment.values()))


def validate_partition_graph(document):
    """Check the concrete split, including the entry gate and diagnostic isolation."""
    require(document['on'] == {'pull_request': {'branches': ['main']}, 'workflow_dispatch': ''},
            'Product event changed')
    require(document['permissions'] == {'contents': 'read'}, 'Product permissions changed')
    require(document['concurrency'] == {'group': 'product-${{ github.ref }}', 'cancel-in-progress': 'true'},
            'Product concurrency changed')
    jobs = document['jobs']
    require(set(jobs) == {'core', 'frontend', 'desktop', 'desktop_release'}, 'Product job set changed')
    require([(jobs[name]['name'], jobs[name]['runs-on'], jobs[name]['timeout-minutes'])
             for name in ('core', 'frontend')] == [
                 ('Product / Rust', 'ubuntu-24.04', '25'), ('Product / UI', 'ubuntu-24.04', '20'),
             ], 'Linux required jobs changed')
    for job, minutes in (('desktop', '60'), ('desktop_release', '30')):
        lane = jobs[job]
        require(lane['runs-on'] == '${{ matrix.os }}' and lane['strategy'] == MATRIX
                and lane['timeout-minutes'] == minutes, 'macOS runner or timeout changed')
        require(not any(field in lane or any(field in step for step in lane['steps'])
                        for field in ('permissions', 'continue-on-error', 'environment')),
                'macOS permission or failure handling changed')
    core, release = jobs['desktop'], jobs['desktop_release']
    require(core['name'] == 'Product / ${{ matrix.os }} Core and migrations'
            and release['name'] == 'Product / ${{ matrix.os }}', 'macOS required check ownership changed')
    require('if' not in core and 'needs' not in core and release['needs'] == 'desktop'
            and release['if'] == '${{ always() }}', 'macOS dependency must always evaluate')
    require(core['outputs'] == UPSTREAM_OUTPUTS, 'macOS Core evidence must come from completion and upload')
    before, after = core['steps'], release['steps']
    require(len(before) == 16 and len(after) == 22, 'macOS step inventory changed')
    first = after[0]
    require(first.get('name') == DEPENDENCY_GUARD and set(first) == {'name', 'env', 'run'}
            and first['env'] == {'MACOS_CORE_RESULT': '${{ needs.desktop.result }}', **RELEASE_INPUTS}
            and after[1]['uses'] == CHECKOUT, 'Dependency must refuse before release checkout')
    gates = before[-1]['env']['MACOS_CORE_GATE_OUTCOMES']
    require(gates == ' '.join(f'{name}=${{{{ steps.{name}.outcome }}}}' for name in UPSTREAM_GATE_IDS)
            and tuple(step['id'] for step in before[4:15]) == UPSTREAM_GATE_IDS,
            'macOS Core gate result sources changed')
    bindings = (before[1], before[-1], after[2], after[19], after[-1])
    require(tuple(step['env']['MACOS_SOURCE_STAGE'] for step in bindings) == ANCHOR_STAGES
            and all(step['run'] == bindings[0]['run'] and 'if' not in step for step in bindings)
            and all(all(step['env'].get(key) == value for key, value in RELEASE_INPUTS.items())
                    for step in bindings[2:]), 'macOS source bindings or stage inputs changed')
    require(after[2]['id'] == 'release_source_start'
            and [step['if'] for step in after if 'if' in step] == [NATIVE_UPLOAD_SCOPE, NATIVE_UPLOAD_SCOPE]
            and after[14]['if'] == after[15]['if'] == NATIVE_UPLOAD_SCOPE
            and after[15]['id'] == 'native_artifact' and after[20]['id'] == 'development_artifact',
            'macOS diagnostic evidence or release path changed')
    require(before[-1]['env']['MACOS_CORE_START_CONTEXT'] == '${{ steps.core_source_start.outputs.source_context }}'
            and before[-1]['env']['MACOS_CORE_ARTIFACT_ID'] == UPSTREAM_OUTPUTS['core_evidence_id']
            and before[-1]['env']['MACOS_CORE_ARTIFACT_SHA256'] == UPSTREAM_OUTPUTS['core_evidence_sha256']
            and after[-1]['env']['MACOS_NATIVE_ARTIFACT_ID'] == '${{ steps.native_artifact.outputs.artifact-id }}'
            and after[-1]['env']['MACOS_NATIVE_ARTIFACT_SHA256'] == '${{ steps.native_artifact.outputs.artifact-digest }}'
            and after[-1]['env']['MACOS_DEVELOPMENT_ARTIFACT_ID'] == '${{ steps.development_artifact.outputs.artifact-id }}'
            and after[-1]['env']['MACOS_DEVELOPMENT_ARTIFACT_SHA256'] == '${{ steps.development_artifact.outputs.artifact-digest }}',
            'Receipt evidence must come from the actual upload roles')
    require(after[7] == jobs['core']['steps'][11] and after[6]['run'].splitlines() == [
        'set -euo pipefail', 'cargo build --release --locked --bin batond --bin baton',
        'bash scripts/prepare-desktop-sidecars.sh',
    ], 'Native runner must build fresh local release inputs before fresh18 and Native tests')


def without_once(block, line):
    require(block.count(line) == 1, 'Exact macOS inverse edit absent or duplicated')
    return block.replace(line, '', 1)


def raw_steps(job):
    require(job.count('    steps:\n') == 1, 'Expected exact macOS job steps boundary')
    steps = job.split('    steps:\n', 1)[1]
    edges = [match.start() for match in re.finditer(r'^      - ', steps, re.M)] + [len(steps)]
    require(edges[0] == 0, 'Unrecognized initial macOS step')
    return [steps[start:end] for start, end in zip(edges, edges[1:])]


def restore_partitioned_workflow(text):
    """Reverse only the reviewed split, leaving all old assertions on exact bytes."""
    pin = digest(text)
    if pin == PREVIOUS_SHA256:
        return text
    require(pin == PARTITION_SHA256, 'Unknown complete macOS partition workflow')
    validate_partition_graph(yaml.load(text, Loader=yaml.BaseLoader))

    require(text.count(DESKTOP) == text.count(RELEASE) == 1, 'Changed macOS job boundaries')
    start, cut = text.index(DESKTOP), text.index(RELEASE)
    before, after = raw_steps(text[start:cut]), raw_steps(text[cut:])
    checkout = without_once(before[0], '          ref: ${{ github.sha }}\n')
    original_gates = [
        without_once(block, f'        id: {name}\n')
        for block, name in zip(before[4:14], UPSTREAM_GATE_IDS[:-1], strict=True)
    ]
    native = without_once(after[15], '        id: native_artifact\n')
    development = without_once(after[20], '        id: development_artifact\n')
    old_steps = [checkout, *before[2:4], *original_gates[:8], *after[7:14],
                 after[14].replace(CONDITIONAL_RETENTION, '        if: always()\n', 1),
                 *original_gates[8:], native.replace(CONDITIONAL_RETENTION, '        if: always()\n', 1),
                 *after[16:19], development]
    restored = text[:start] + OLD_DESKTOP + ''.join(old_steps)
    require(digest(restored) == PREVIOUS_SHA256, 'macOS partition inverse did not restore exact previous workflow')
    return restored


def normalize_partitioned_workflow(text):
    """Feed legacy adapters only the admitted inverse or their own input."""
    if digest(text) == PARTITION_SHA256:
        return restore_partitioned_workflow(text)
    return text
