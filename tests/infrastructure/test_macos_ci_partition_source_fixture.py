"""Exercise the actual reviewed CI entry and identity code using private local Git."""
from contextlib import chdir, redirect_stdout
from copy import deepcopy
from io import StringIO
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import yaml

from tests.infrastructure.macos_ci_partition_source_fixture import (
    CHECKOUT, DEPENDENCY_GUARD, NATIVE_GATE_IDS, PACKAGE_DEPENDENCY_GUARD, PACKAGE_SHA256,
    PARTITION_SHA256, PREFLIGHT_SHA256, PREVIOUS_SHA256, UPSTREAM_GATE_IDS,
    assert_action_inventory, digest, restore_package_partitioned_workflow, restore_partitioned_workflow,
    validate_package_graph, validate_partition_graph,
)


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / '.github/workflows'
MARKER = 'MACOS_SOURCE_BINDING'


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def current_workflow_text():
    return (WORKFLOWS / 'product.yml').read_bytes().decode()


def workflow_text():
    return restore_package_partitioned_workflow(current_workflow_text())


def load(text):
    return yaml.load(text, Loader=yaml.BaseLoader)


class MacosPartitionSourceTests(unittest.TestCase):
    def test_exact_partition_restores_the_entire_previous_input(self):
        raw = workflow_text()
        self.assertEqual(digest(raw), PARTITION_SHA256)
        parent = restore_partitioned_workflow(raw)
        self.assertEqual(digest(parent), PREVIOUS_SHA256)
        self.assertEqual(restore_partitioned_workflow(parent), parent)
        preflight = (WORKFLOWS / 'environment-preflight.yml').read_bytes().decode()
        self.assertEqual(digest(preflight), PREFLIGHT_SHA256)
        self.assertEqual(assert_action_inventory(load(raw), load(preflight), partitioned=True), 18)

    def test_raw_partition_refuses_unknown_old_or_new_bytes(self):
        raw = workflow_text()
        mutations = [
            ('timeout-minutes: 30', 'timeout-minutes: 31'),
            ('    needs: desktop\n', '    needs: core\n'),
            ('        id: release_source_start\n', '        id: another_check\n'),
            ('      - *fresh_schema18_gate\n', '      - *fresh_schema18_gate\n' * 2),
            ("'--integration-cancel-at', '0'", "'--integration-cancel-at', '1'"),
            ('      - name: Package development application and evidence\n', ''),
            ('  contents: read\n', '  contents: write\n'),
        ]
        for old, new in mutations:
            with self.subTest(old=old):
                self.assertIn(old, raw)
                with self.assertRaisesRegex(ValueError, 'Unknown complete'):
                    restore_partitioned_workflow(raw.replace(old, new, 1))
        for altered in (raw + '# unknown\n', '\ufeff' + raw, raw.replace('\n', '\r\n')):
            with self.assertRaisesRegex(ValueError, 'Unknown complete'):
                restore_partitioned_workflow(altered)

    def test_graph_validation_does_not_rely_on_digest_to_refuse_bad_topology(self):
        actual = load(workflow_text())
        validate_partition_graph(actual)
        cases = {
            'conditional required job': lambda core, release: release.update({'if': '${{ success() }}'}),
            'wrong dependency': lambda core, release: release.update(needs='core'),
            'duplicate check name': lambda core, release: core.update(name=release['name']),
            'longer release timeout': lambda core, release: release.update({'timeout-minutes': '60'}),
            'intel runner': lambda core, release: release['strategy']['matrix'].update(os=['macos-15-intel']),
            'failures allowed': lambda core, release: release['steps'][0].update({'continue-on-error': 'true'}),
            'checkout before gate': lambda core, release: release['steps'].reverse(),
            'false result source': lambda core, release: release['steps'][0]['env'].update(MACOS_CORE_RESULT='success'),
            'forged upload source': lambda core, release: core['outputs'].update(core_evidence_id='123'),
            'skippable source check': lambda core, release: release['steps'][2].update({'if': 'false'}),
            'unconditional native diagnostics': lambda core, release: release['steps'][14].update({'if': 'always()'}),
            'wrong evidence role': lambda core, release: release['steps'][-1]['env'].update(
                MACOS_DEVELOPMENT_ARTIFACT_ID='${{ steps.native_artifact.outputs.artifact-id }}'),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                changed = deepcopy(actual)
                mutate(changed['jobs']['desktop'], changed['jobs']['desktop_release'])
                with self.assertRaises(ValueError):
                    validate_partition_graph(changed)

    def test_action_inventory_refuses_substitution_and_private_evidence(self):
        product = load(workflow_text())
        environment = load((WORKFLOWS / 'environment-preflight.yml').read_text())
        cases = [
            ('desktop_release', 1, 'with', 'persist-credentials', 'true'),
            ('desktop_release', 1, 'with', 'ref', '${{ github.event.pull_request.head.sha }}'),
            ('desktop_release', 3, None, 'uses', CHECKOUT),
            ('desktop', 14, 'with', 'path', 'artifacts/**/*.sqlite3'),
            ('desktop_release', 20, 'with', 'name', 'incorrect-package'),
        ]
        for job, index, container, key, value in cases:
            with self.subTest(job=job, index=index, key=key):
                changed = deepcopy(product)
                step = changed['jobs'][job]['steps'][index]
                (step if container is None else step[container])[key] = value
                with self.assertRaisesRegex(ValueError, 'Product Actions'):
                    assert_action_inventory(changed, environment, partitioned=True)
        changed_env = deepcopy(environment)
        changed_env['jobs']['preflight']['steps'][0]['uses'] = CHECKOUT.replace('@', '@0', 1)
        with self.assertRaisesRegex(ValueError, 'Environment Actions'):
            assert_action_inventory(product, changed_env, partitioned=True)


class MacosReceiptFixture:
    workflow_input = staticmethod(workflow_text)

    @classmethod
    def setUpClass(cls):
        raw = cls.workflow_input()
        restore_partitioned_workflow(raw)
        jobs = load(raw)['jobs']
        core, release = jobs['desktop']['steps'], jobs['desktop_release']['steps']
        cls.first = release[0]
        anchor = core[1]['run'].split(f"<<'{MARKER}'\n", 1)[1].removesuffix(f'{MARKER}\n')
        cls.binding = compile(anchor, '<actual macOS CI source binding>', 'exec')
        owner = tempfile.TemporaryDirectory(prefix='baton-macos-ci-')
        cls.addClassCleanup(owner.cleanup)
        cls.root = Path(owner.name).resolve()
        seed = cls.root / 'seed'
        seed.mkdir()
        git_env = {
            'PATH': os.environ['PATH'], 'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_ALLOW_PROTOCOL': 'file',
            'GIT_AUTHOR_NAME': 'CI Fixture', 'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
            'GIT_COMMITTER_NAME': 'CI Fixture', 'GIT_COMMITTER_EMAIL': 'fixture@example.invalid',
        }

        def git(*args):
            return subprocess.check_output(['git', '-C', str(seed), *args], env=git_env, text=True).strip()

        git('init', '-q', '--template=')
        (seed / 'README').write_text('fixture\n')
        git('add', 'README')
        cls.tree = git('write-tree')
        cls.base = git('commit-tree', cls.tree, '-m', 'base')
        cls.head = git('commit-tree', cls.tree, '-p', cls.base, '-m', 'head')
        cls.merge = git('commit-tree', cls.tree, '-p', cls.base, '-p', cls.head, '-m', 'merge')
        git('update-ref', 'refs/heads/fixture', cls.merge)
        cls.checkout = cls.root / 'checkout'
        git('clone', '-q', '--template=', '--depth', '1', '--branch', 'fixture', seed.as_uri(), str(cls.checkout))
        cls.assert_shallow(cls.checkout, git_env)
        cls.event = cls.root / 'event.json'
        cls.output = cls.root / 'outputs.txt'
        cls.payload = {
            'number': 33,
            'repository': {'id': 1434422493, 'full_name': 'wanderbot-auto/AgentBaton'},
            'pull_request': {'base': {'ref': 'main', 'sha': cls.base}, 'head': {'sha': cls.head}},
        }
        cls.env = {
            **git_env, 'GITHUB_EVENT_NAME': 'pull_request', 'GITHUB_EVENT_PATH': str(cls.event),
            'GITHUB_REPOSITORY': 'wanderbot-auto/AgentBaton', 'GITHUB_REPOSITORY_ID': '1434422493',
            'GITHUB_SHA': cls.merge, 'GITHUB_REF': 'refs/pull/33/merge',
            'GITHUB_WORKFLOW_REF': 'wanderbot-auto/AgentBaton/.github/workflows/product.yml@refs/pull/33/merge',
            'GITHUB_WORKFLOW_SHA': cls.merge, 'GITHUB_RUN_ID': '44792307596', 'GITHUB_RUN_ATTEMPT': '2',
            'GITHUB_OUTPUT': str(cls.output),
            'MACOS_CORE_ARTIFACT_ID': '7001', 'MACOS_CORE_ARTIFACT_SHA256': 'a' * 64,
        }
        cls.outcomes = ' '.join(f'{gate}=success' for gate in UPSTREAM_GATE_IDS)

    @staticmethod
    def assert_shallow(checkout, env):
        is_shallow = subprocess.check_output(
            ['git', '-C', str(checkout), 'rev-parse', '--is-shallow-repository'], env=env, text=True,
        ).strip()
        if is_shallow != 'true':
            raise AssertionError('Missing real shallow merge fixture')

    def run_binding(self, stage, *, payload=None, uname=None, **changes):
        self.event.write_text(encoded(self.payload if payload is None else payload))
        self.output.unlink(missing_ok=True)
        environment = {**self.env, 'MACOS_SOURCE_STAGE': stage, **changes}
        host = uname if uname is not None else SimpleNamespace(sysname='Darwin', machine='arm64')
        stdout = StringIO()
        with chdir(self.checkout), patch.dict(os.environ, environment, clear=True):
            with patch('os.uname', return_value=host), redirect_stdout(stdout):
                exec(self.binding, {'__name__': '__main__'})
        published = self.output.read_text().strip() if self.output.exists() else ''
        return published, stdout.getvalue().strip()

    def core_receipt(self):
        start, _ = self.run_binding('core-start')
        published, observed = self.run_binding(
            'core-complete', MACOS_CORE_START_CONTEXT=start.removeprefix('source_context='),
            MACOS_CORE_GATE_OUTCOMES=self.outcomes,
        )
        self.assertEqual(published, 'source_receipt=' + observed)
        return observed


class MacosPartitionReceiptTests(MacosReceiptFixture, unittest.TestCase):
    def test_first_step_fails_closed_without_checkout_or_execution(self):
        self.assertEqual(self.first['name'], DEPENDENCY_GUARD)
        inputs = {'MACOS_CORE_RESULT': 'success', 'MACOS_CORE_RECEIPT': '{}',
                  'MACOS_CORE_ARTIFACT_ID': '7001', 'MACOS_CORE_ARTIFACT_SHA256': 'a' * 64}
        cases = ({'MACOS_CORE_RESULT': status} for status in
                 ('failure', 'skipped', 'cancelled', 'timed_out', 'neutral', '', 'SUCCESS'))
        for case in [*cases, *(dict.fromkeys([key], '') for key in list(inputs)[1:]), {}]:
            with self.subTest(case=case):
                result = subprocess.run(['bash', '-c', self.first['run']], timeout=5, cwd=self.root,
                                        env={'PATH': self.env['PATH'], **inputs, **case},
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, int(bool(case)))
                if case:
                    self.assertIn('did not finish successfully with bound evidence', result.stdout)
        self.assertFalse((self.root / '.git').exists())

    def test_exact_shallow_merge_receipt_and_distinct_release_artifacts(self):
        receipt = self.core_receipt()
        source = json.loads(receipt)['source']
        self.assertEqual((source['base'], source['head'], source['checkout'], source['tree']),
                         (self.base, self.head, self.merge, self.tree))
        self.assertEqual((source['os'], source['architecture']), ('Darwin', 'arm64'))
        self.assertEqual(self.run_binding('release-check', MACOS_CORE_RECEIPT=receipt), ('', ''))
        _, final = self.run_binding(
            'release-complete', MACOS_CORE_RECEIPT=receipt,
            MACOS_NATIVE_ARTIFACT_ID='7002', MACOS_NATIVE_ARTIFACT_SHA256='b' * 64,
            MACOS_DEVELOPMENT_ARTIFACT_ID='7003', MACOS_DEVELOPMENT_ARTIFACT_SHA256='c' * 64,
        )
        summary = json.loads(final)
        self.assertEqual(summary['source'], source)
        self.assertEqual([summary[key]['id'] for key in ('core_evidence', 'native_evidence', 'development')],
                         ['7001', '7002', '7003'])

    def test_cross_attempt_run_checkout_workflow_and_upload_refuse(self):
        receipt = self.core_receipt()
        cases = {
            'previous attempt': {'GITHUB_RUN_ATTEMPT': '3'},
            'different run': {'GITHUB_RUN_ID': '44792308595'},
            'same-tree PR head': {'GITHUB_SHA': self.head},
            'workflow SHA': {'GITHUB_WORKFLOW_SHA': self.head},
            'workflow ref': {'GITHUB_WORKFLOW_REF': self.env['GITHUB_WORKFLOW_REF'].replace('/merge', '/head')},
            'different core ID': {'MACOS_CORE_ARTIFACT_ID': '7009'},
            'different core digest': {'MACOS_CORE_ARTIFACT_SHA256': 'b' * 64},
            'noncanonical upload ID': {'MACOS_CORE_ARTIFACT_ID': '07001'},
        }
        for label, env in cases.items():
            with self.subTest(label=label), self.assertRaises(SystemExit):
                self.run_binding('release-check', MACOS_CORE_RECEIPT=receipt, **env)

    def test_wrong_event_parents_types_and_noncanonical_receipts_refuse(self):
        receipt = self.core_receipt()
        wrong_parents = deepcopy(self.payload)
        wrong_parents['pull_request']['base']['sha'] = self.head
        with self.assertRaisesRegex(SystemExit, 'parents differ'):
            self.run_binding('release-check', payload=wrong_parents, MACOS_CORE_RECEIPT=receipt)
        source_values = {
            'pr_number': (33.0, True),
            'tree': ('d' * 40,),
            'merge_ref': ('refs/pull/33/head',),
            'architecture': ('x86_64',),
        }
        for field, values in source_values.items():
            for value in values:
                with self.subTest(field=field, value=value):
                    forged = json.loads(receipt)
                    forged['source'][field] = value
                    with self.assertRaisesRegex(SystemExit, 'proof differs'):
                        self.run_binding('release-check', MACOS_CORE_RECEIPT=encoded(forged))
        duplicate = '{"contract":"agentbaton.macos-core-ci.v1",' + receipt[1:]
        for raw in ('', ' ' + receipt, duplicate, 'x' * 4097):
            with self.assertRaises(SystemExit):
                self.run_binding('release-check', MACOS_CORE_RECEIPT=raw)

    def test_failed_upstream_gate_changed_source_and_artifact_alias_refuse(self):
        start, _ = self.run_binding('core-start')
        context = start.removeprefix('source_context=')
        for result in ('failure', 'skipped', 'cancelled', ''):
            changed = self.outcomes.replace('core_native=success', f'core_native={result}')
            with self.subTest(result=result), self.assertRaisesRegex(SystemExit, 'gate did not pass'):
                self.run_binding('core-complete', MACOS_CORE_START_CONTEXT=context,
                                 MACOS_CORE_GATE_OUTCOMES=changed)
        with self.assertRaisesRegex(SystemExit, 'Core source changed'):
            self.run_binding('core-complete', MACOS_CORE_START_CONTEXT=context,
                             MACOS_CORE_GATE_OUTCOMES=self.outcomes, GITHUB_RUN_ATTEMPT='3')
        receipt = self.core_receipt()
        with self.assertRaisesRegex(SystemExit, 'distinct artifacts'):
            self.run_binding('release-complete', MACOS_CORE_RECEIPT=receipt,
                             MACOS_NATIVE_ARTIFACT_ID='7001', MACOS_NATIVE_ARTIFACT_SHA256='b' * 64,
                             MACOS_DEVELOPMENT_ARTIFACT_ID='7003', MACOS_DEVELOPMENT_ARTIFACT_SHA256='c' * 64)

    def test_wrong_host_and_tracked_mutation_refuse(self):
        for host in (SimpleNamespace(sysname='Darwin', machine='x86_64'),
                     SimpleNamespace(sysname='Linux', machine='aarch64')):
            with self.subTest(host=host), self.assertRaisesRegex(SystemExit, 'requires Apple Silicon'):
                self.run_binding('core-start', uname=host)
        path = self.checkout / 'README'
        content = path.read_bytes()
        try:
            path.write_bytes(b'tampered\n')
            with self.assertRaisesRegex(SystemExit, 'not the clean event SHA'):
                self.run_binding('core-start')
        finally:
            path.write_bytes(content)


class MacosPackageGraphTests(unittest.TestCase):
    def test_previous_workflow_and_historical_assertions_use_the_pinned_bytes(self):
        actual = current_workflow_text()
        restored = restore_package_partitioned_workflow(actual)
        self.assertEqual(digest(actual), PACKAGE_SHA256)
        self.assertEqual(digest(restored), PARTITION_SHA256)
        self.assertEqual(restore_partitioned_workflow(actual), restore_partitioned_workflow(restored))
        self.assertEqual(digest(restore_partitioned_workflow(actual)), PREVIOUS_SHA256)
        environment = load((WORKFLOWS / 'environment-preflight.yml').read_bytes().decode())
        self.assertEqual(assert_action_inventory(load(actual), environment, partitioned=True, package_partitioned=True), 20)
        self.assertEqual(assert_action_inventory(load(restored), environment, partitioned=True), 18)
        for changed in (actual + '# changed\n', actual.replace('timeout-minutes: 30', 'timeout-minutes: 31', 1)):
            with self.assertRaisesRegex(ValueError, 'Unknown complete macOS packaging workflow'):
                restore_package_partitioned_workflow(changed)

    def test_packaging_graph_refuses_skipped_checks_wrong_sources_and_shared_builds(self):
        actual = load(current_workflow_text())
        validate_package_graph(actual)
        cases = {
            'conditional required job': lambda native, package: package.update({'if': '${{ success() }}'}),
            'checks missing Native result': lambda native, package: package.update(needs=['desktop']),
            'checks wrong name or runner': lambda native, package: package.update(name=native['name']),
            'allows longer Native stage': lambda native, package: native.update({'timeout-minutes': '60'}),
            'allows timeouts': lambda native, package: package.update({'continue-on-error': 'true'}),
            'checkout precedes refusal': lambda native, package: package['steps'].reverse(),
            'hard-coded Native result': lambda native, package: package['steps'][0]['env'].update(MACOS_NATIVE_RESULT='success'),
            'misroutes Native artifact': lambda native, package: native['outputs'].update(native_evidence_id='7002'),
            'misroutes attempt receipt': lambda native, package: package['steps'][2]['env'].update(MACOS_NATIVE_RECEIPT='{}'),
            'skips Native proof': lambda native, package: native['steps'][-1].update({'if': 'false'}),
            'skips fresh schema18': lambda native, package: native['steps'][7].update({'if': 'false'}),
            'ignores failed fresh schema18': lambda native, package: native['steps'][7].update({'continue-on-error': 'true'}),
            'rewrites original fresh schema18 command': lambda native, package: native['steps'][7].update(run='exit 0'),
            'skips required audit': lambda native, package: package['steps'][8].update({'if': 'false'}),
            'reuses cross-job executable': lambda native, package: package['steps'][6].update(run='cp old-binary new-binary'),
        }
        for label, mutate in cases.items():
            with self.subTest(label=label):
                changed = deepcopy(actual)
                mutate(changed['jobs']['desktop_release'], changed['jobs']['desktop_package'])
                with self.assertRaises(ValueError):
                    validate_package_graph(changed)
        changed = deepcopy(actual)
        changed['jobs']['desktop_package']['steps'][11]['with']['path'] = 'artifacts/**'
        environment = load((WORKFLOWS / 'environment-preflight.yml').read_bytes().decode())
        with self.assertRaisesRegex(ValueError, 'Product Actions'):
            assert_action_inventory(changed, environment, partitioned=True, package_partitioned=True)


class MacosPackageReceiptTests(MacosReceiptFixture, unittest.TestCase):
    workflow_input = staticmethod(current_workflow_text)

    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.package = load(cls.workflow_input())['jobs']['desktop_package']['steps']

    def setUp(self):
        self.native_artifact = {'MACOS_NATIVE_ARTIFACT_ID': '7002', 'MACOS_NATIVE_ARTIFACT_SHA256': 'b' * 64}
        self.development = {'MACOS_DEVELOPMENT_ARTIFACT_ID': '7003', 'MACOS_DEVELOPMENT_ARTIFACT_SHA256': 'c' * 64}
        self.proof = self.checkout / 'artifacts/native-smoke/integration-execution.json'
        self.proof.parent.mkdir(parents=True, exist_ok=True)
        self.addCleanup(self.proof.unlink, missing_ok=True)
        self.integration = {
            'contract': 'agentbaton.integration-native-ci.v1',
            'status': 'driver_and_projection_passed',
            'cases': {case: {'driver_exit': 0, 'projection_exit': 0, 'complete': True}
                      for case in ('first-page', 'last-page')},
            'private_history_published': False,
            'visual_review': 'not_established',
            'runtime_acceptance': False,
        }
        self.proof.write_text(json.dumps(self.integration, indent=2))
        self.native_outcomes = ' '.join(f'{gate}=success' for gate in NATIVE_GATE_IDS)

    def native_inputs(self):
        core = self.core_receipt()
        started, _ = self.run_binding('native-start', MACOS_CORE_RECEIPT=core)
        return {'MACOS_CORE_RECEIPT': core, **self.native_artifact,
                'MACOS_NATIVE_START_CONTEXT': started.removeprefix('source_context='),
                'MACOS_NATIVE_GATE_OUTCOMES': self.native_outcomes}

    def package_inputs(self):
        native = self.native_inputs()
        published, record = self.run_binding('native-complete', **native)
        self.assertEqual(published, 'source_receipt=' + record)
        self.assertEqual(json.loads(record)['integration_cases'], self.integration['cases'])
        return {'MACOS_CORE_RECEIPT': native['MACOS_CORE_RECEIPT'],
                'MACOS_NATIVE_RECEIPT': record, **self.native_artifact}

    def test_required_job_rejects_unsuccessful_dependencies_and_missing_uploads_before_checkout(self):
        guard = self.package[0]
        self.assertEqual(guard['name'], PACKAGE_DEPENDENCY_GUARD)
        inputs = {
            'MACOS_CORE_RESULT': 'success', 'MACOS_NATIVE_RESULT': 'success',
            'MACOS_CORE_RECEIPT': '{}', 'MACOS_CORE_ARTIFACT_ID': '7001', 'MACOS_CORE_ARTIFACT_SHA256': 'a' * 64,
            'MACOS_NATIVE_RECEIPT': '{}', **self.native_artifact,
        }
        negatives = [{key: status} for key in ('MACOS_CORE_RESULT', 'MACOS_NATIVE_RESULT')
                     for status in ('failure', 'skipped', 'cancelled', 'timed_out', 'neutral', '', 'SUCCESS')]
        negatives.extend({key: ''} for key in inputs if not key.endswith('_RESULT'))
        for changes in ({}, *negatives):
            with self.subTest(changes=changes):
                attempt = subprocess.run(['bash', '-c', guard['run']], timeout=5, cwd=self.root,
                                         env={'PATH': self.env['PATH'], **inputs, **changes},
                                         capture_output=True, text=True)
                self.assertEqual(attempt.returncode, int(bool(changes)))
                if changes:
                    self.assertIn('did not finish successfully with bound evidence', attempt.stdout)
        self.assertFalse((self.root / '.git').exists())

    def test_receipts_bind_both_native_cases_and_three_uploads_to_same_shallow_merge(self):
        package = self.package_inputs()
        self.assertEqual(self.run_binding('package-check', **package), ('', ''))
        published, payload = self.run_binding('release-complete', **package, **self.development)
        self.assertEqual(published, '')
        receipt = json.loads(payload)
        self.assertEqual(receipt['source'], json.loads(package['MACOS_NATIVE_RECEIPT'])['source'])
        self.assertEqual(receipt['source'], json.loads(package['MACOS_CORE_RECEIPT'])['source'])
        self.assertEqual([receipt[role]['id'] for role in ('core_evidence', 'native_evidence', 'development')],
                         ['7001', '7002', '7003'])

    def test_manual_dispatch_keeps_its_source_identity(self):
        dispatch = {'GITHUB_EVENT_NAME': 'workflow_dispatch', 'GITHUB_REF': 'refs/heads/main',
                    'GITHUB_WORKFLOW_REF': self.env['GITHUB_WORKFLOW_REF'].replace('refs/pull/33/merge', 'refs/heads/main')}
        payload = {'repository': self.payload['repository']}
        context, _ = self.run_binding('core-start', payload=payload, **dispatch)
        _, core = self.run_binding('core-complete', payload=payload, **dispatch,
                                   MACOS_CORE_START_CONTEXT=context.removeprefix('source_context='),
                                   MACOS_CORE_GATE_OUTCOMES=self.outcomes)
        started, _ = self.run_binding('native-start', payload=payload, **dispatch, MACOS_CORE_RECEIPT=core)
        _, native = self.run_binding('native-complete', payload=payload, **dispatch, **self.native_artifact,
                                     MACOS_CORE_RECEIPT=core,
                                     MACOS_NATIVE_START_CONTEXT=started.removeprefix('source_context='),
                                     MACOS_NATIVE_GATE_OUTCOMES=self.native_outcomes)
        self.assertEqual(self.run_binding('package-check', payload=payload, **dispatch, **self.native_artifact,
                                          MACOS_CORE_RECEIPT=core, MACOS_NATIVE_RECEIPT=native), ('', ''))

    def test_native_completion_rejects_failed_missing_or_forged_projection(self):
        native = self.native_inputs()
        for gate in NATIVE_GATE_IDS:
            for state in ('failure', 'skipped', 'cancelled', ''):
                outcomes = self.native_outcomes.replace(f'{gate}=success', f'{gate}={state}')
                with self.subTest(gate=gate, state=state), self.assertRaisesRegex(SystemExit, 'Native gate did not pass'):
                    self.run_binding('native-complete', **{**native, 'MACOS_NATIVE_GATE_OUTCOMES': outcomes})
        self.proof.unlink()
        with self.assertRaises(FileNotFoundError):
            self.run_binding('native-complete', **native)
        for case in ('first-page', 'last-page'):
            for key, bad in (('driver_exit', 1), ('projection_exit', False), ('complete', 1)):
                forged = deepcopy(self.integration)
                forged['cases'][case][key] = bad
                self.proof.write_text(json.dumps(forged))
                with self.subTest(case=case, key=key), self.assertRaisesRegex(SystemExit, 'Both Native Integration'):
                    self.run_binding('native-complete', **native)
        self.proof.write_text('x' * 8193)
        with self.assertRaisesRegex(SystemExit, 'oversized Native Integration proof'):
            self.run_binding('native-complete', **native)
        encoded_report = encoded(self.integration)
        for duplicate in ('{"status":"failed",' + encoded_report[1:],
                          encoded_report.replace('"driver_exit":0', '"driver_exit":1,"driver_exit":0', 1)):
            self.proof.write_text(duplicate)
            with self.assertRaisesRegex(SystemExit, 'Duplicate key in Native Integration proof'):
                self.run_binding('native-complete', **native)

    def test_packaging_rejects_stale_attempt_or_workflow_and_swapped_artifacts(self):
        package = self.package_inputs()
        changes = (
            {'GITHUB_RUN_ATTEMPT': '3'},
            {'GITHUB_RUN_ID': '44792307597'},
            {'GITHUB_WORKFLOW_SHA': self.head},
            {'GITHUB_WORKFLOW_REF': self.env['GITHUB_WORKFLOW_REF'].replace('/merge', '/head')},
            {'GITHUB_SHA': self.head},
            {'MACOS_NATIVE_ARTIFACT_ID': '7001'},
            {'MACOS_NATIVE_ARTIFACT_ID': '7008'},
            {'MACOS_NATIVE_ARTIFACT_SHA256': 'd' * 64},
            {'MACOS_NATIVE_ARTIFACT_SHA256': ''},
            {'MACOS_NATIVE_RECEIPT': package['MACOS_CORE_RECEIPT']},
            {'MACOS_NATIVE_RECEIPT': ' ' + package['MACOS_NATIVE_RECEIPT']},
        )
        for changed in changes:
            with self.subTest(changed=changed), self.assertRaises(SystemExit):
                self.run_binding('package-check', **{**package, **changed})
        for role in ('7001', '7002'):
            with self.subTest(role=role), self.assertRaisesRegex(SystemExit, 'distinct artifacts'):
                self.run_binding('release-complete', **package, **{**self.development, 'MACOS_DEVELOPMENT_ARTIFACT_ID': role})

    def test_native_receipt_alone_cannot_substitute_stale_fields_or_one_case(self):
        package = self.package_inputs()
        changes = (
            ('source', 'run_attempt', '1'), ('source', 'tree', 'd' * 40), ('source', 'head', self.base),
            ('source', 'base', self.head), ('source', 'checkout', self.head),
            ('core_evidence', 'sha256', 'b' * 64), ('native_evidence', 'id', '7001'),
            ('gates', 'native_webview', 'skipped'), ('integration_cases', 'first-page', None),
        )
        for section, key, bad in changes:
            proof = json.loads(package['MACOS_NATIVE_RECEIPT'])
            proof[section][key] = bad
            with self.subTest(section=section, key=key), self.assertRaisesRegex(SystemExit, 'Native proof differs'):
                self.run_binding('package-check', **{**package, 'MACOS_NATIVE_RECEIPT': encoded(proof)})


if __name__ == '__main__':
    unittest.main()
