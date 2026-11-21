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
    CHECKOUT, DEPENDENCY_GUARD, PARTITION_SHA256, PREFLIGHT_SHA256, PREVIOUS_SHA256,
    UPSTREAM_GATE_IDS, assert_action_inventory, digest, restore_partitioned_workflow,
    validate_partition_graph,
)


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / '.github/workflows'
MARKER = 'MACOS_SOURCE_BINDING'


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def workflow_text():
    return (WORKFLOWS / 'product.yml').read_bytes().decode()


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


class MacosPartitionReceiptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        raw = workflow_text()
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


if __name__ == '__main__':
    unittest.main()
