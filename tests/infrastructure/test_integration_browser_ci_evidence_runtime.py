"""Synthetic failed-run projection controls; no browser/Core/Provider execution."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_integration_runtime_projection_fixtures',
    ROOT / 'tests/infrastructure/test_integration_browser_ci_evidence.py')
F = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F)
E, HEAD, SECRET = F.E, F.HEAD, F.SECRET


RUNTIME_LAYER_MARKER = '# Closed observations from a failed real runtime report. These never satisfy\n'
RUNTIME_LAYER_SHA256 = 'a7d93f3c628a51212e7712c79a710388e9d27ea63da1ce05960304503f6e48c9'
RUNTIME_BASE_PROJECTOR_SHA256 = '06d5222af1d9901173f0ed7ff291895e6fe2049151459a8143d6ec73a3fc9315'
RUNTIME_MAIN_LINE = "            summary['runtime_failure'] = runtime_failure_diagnostic(args.private, args.expected_source_head)\n"


def strip_runtime_failure_projection(text):
    """Undo only the complete reviewed failed-runtime layer, then match all base bytes."""
    if text.count(RUNTIME_LAYER_MARKER) != 1 or text.count(RUNTIME_MAIN_LINE) != 1:
        raise ValueError('Runtime projection layer count changed')
    start = text.index(RUNTIME_LAYER_MARKER)
    end = text.index('def publish(', start)
    block = text[start:end]
    neighbor = "            summary['driver_preflight'] = driver_preflight_diagnostic(args.private, args.expected_source_head)\n"
    if (hashlib.sha256(block.encode()).hexdigest() != RUNTIME_LAYER_SHA256
            or text.count(neighbor + RUNTIME_MAIN_LINE + '        publish(args.output, summary, payloads)\n') != 1):
        raise ValueError('Runtime projection exact block or main neighbors changed')
    restored = text[:start] + text[end:]
    restored = restored.replace(RUNTIME_MAIN_LINE, '', 1)
    if hashlib.sha256(restored.encode()).hexdigest() != RUNTIME_BASE_PROJECTOR_SHA256:
        raise ValueError('Runtime inverse refuses changed or moved base bytes')
    return restored


def failed_seed(root):
    private = F.seed(root)
    F.write(private / 'status.json', {'source': HEAD, 'driver_exit': 1, 'probe_passed': True})
    q = private / 'driver/normal/report.json'
    value = json.loads(q.read_text())
    value.update(status='failed_or_interrupted', checks=value['checks'][:3], screenshots=value['screenshots'][:2],
        failure={'stage': 'real_owner_acceptance_and_watcher', 'kind': 'Refusal'},
        fixture_retained_privately=True, http_faults=[{'private_text': SECRET}])
    value['cleanup']['fixture_removed'] = False
    F.write(q, value)
    return private


class RuntimeFailureProjection(unittest.TestCase):
    def diagnostic(self, private):
        return E.runtime_failure_diagnostic(private, HEAD)

    def mutate(self, private, name, change):
        path = private / name
        value = json.loads(path.read_text())
        change(value)
        F.write(path, value)

    def unavailable(self, private):
        value = self.diagnostic(private)
        self.assertEqual(value, {'status': 'unavailable',
            'failure_kind': 'missing_or_invalid_runtime_failure_diagnostic'})
        self.assertNotIn(SECRET, json.dumps(value))
        return value

    def project(self, private, output):
        return E.main(['--private', str(private), '--output', str(output),
            '--expected-source-head', HEAD, '--driver-step-outcome', 'failure'])

    def test_failed_runtime_is_finite_observation_with_original_failure_and_no_images(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = failed_seed(root); output = root / 'public'
            self.assertEqual(self.project(private, output), 1)
            self.assertEqual({p.name for p in output.iterdir()}, {'summary.json'})
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual(summary['status'], 'failed')
            self.assertEqual(summary['failure_kind'], 'driver_step_not_successful')
            value = summary['runtime_failure']
            self.assertEqual(value['status'], 'observed')
            self.assertEqual(value['driver_exit'], 1)
            self.assertIs(value['observation_only'], True)
            self.assertIs(value['runtime_acceptance'], False)
            normal, sibling = value['cases']
            self.assertEqual(normal['phase'], 'real_owner_acceptance_and_watcher')
            self.assertEqual(normal['exception_kind'], 'Refusal')
            self.assertEqual(normal['completed_checks'], 3)
            self.assertEqual(normal['last_completed_check'], E.CHECKS[2])
            self.assertIs(normal['cleanup_observations']['fixture_removed'], False)
            self.assertEqual(normal['cleanup_observations']['daemon_joined'],
                {'exit': 0, 'joined': True, 'forced_kill': False})
            self.assertEqual(sibling['report_status'], 'passed')
            self.assertIsNone(sibling['phase'])
            raw = (output / 'summary.json').read_text()
            for text in [SECRET, str(private), 'traceback', 'private_text', 'http_faults']:
                self.assertNotIn(text, json.dumps(value))
            self.assertNotIn(SECRET, raw)

    def test_original_success_collect_and_publication_unchanged(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = F.seed(root)
            expected, images = E.collect(private, HEAD)
            self.assertEqual(E.main(['--private', str(private), '--output', str(root / 'public'),
                '--expected-source-head', HEAD, '--driver-step-outcome', 'success']), 0)
            self.assertEqual(json.loads((root / 'public/summary.json').read_text()), expected)
            self.assertNotIn('runtime_failure', expected)
            for name, raw in images.items(): self.assertEqual((root / 'public' / name).read_bytes(), raw)

    def test_both_cases_can_fail_and_none_of_their_screenshots_is_opened(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = failed_seed(root)
            def fail(value):
                value.update(status='failed_or_interrupted', checks=[],
                    failure={'stage': 'startup', 'kind': 'TimeoutError'}, cleanup={})
            self.mutate(private, 'driver/command-failure/report.json', fail)
            # Reading even one PNG is an error in the failure projection.
            original = E.P.regular_bytes
            def read(descriptor, name, *args, **kwargs):
                self.assertFalse(name.endswith('.png'))
                return original(descriptor, name, *args, **kwargs)
            with mock.patch.object(E.P, 'regular_bytes', side_effect=read): value = self.diagnostic(private)
            self.assertEqual(value['status'], 'observed')
            sibling = value['cases'][1]
            self.assertEqual(sibling['completed_checks'], 0)
            self.assertIsNone(sibling['last_completed_check'])
            self.assertTrue(all(v is None for v in sibling['cleanup_observations'].values()))

    def test_missing_or_error_cleanup_never_claims_success_and_nonzero_is_retained(self):
        for variant in ['missing', 'false', 'error', 'nonzero', 'forced', 'unjoined']:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                def change(value):
                    cleanup = value['cleanup']
                    if variant == 'missing': cleanup.pop('browser_closed')
                    elif variant == 'false': cleanup['browser_closed'] = False
                    elif variant == 'error': cleanup['browser_closed'] = {'error': 'Error'}
                    elif variant == 'nonzero': cleanup['daemon_joined']['exit'] = 1
                    elif variant == 'forced': cleanup['daemon_joined']['forced_kill'] = True
                    else: cleanup['daemon_joined']['joined'] = False
                self.mutate(private, 'driver/normal/report.json', change)
                value = self.diagnostic(private); self.assertEqual(value['status'], 'observed')
                observation = value['cases'][0]['cleanup_observations']
                expected = {'missing': None, 'false': False, 'error': {'exception_kind': 'Error'}}
                if variant in expected: self.assertEqual(observation['browser_closed'], expected[variant])
                else: self.assertEqual(observation['daemon_joined'],
                    {'exit': 1 if variant == 'nonzero' else 0, 'joined': variant != 'unjoined',
                     'forced_kill': variant == 'forced'})
                self.assertIs(value['runtime_acceptance'], False)

    def test_unknown_failure_or_report_shape_cannot_publish_private_strings(self):
        mutations = [lambda v: v['failure'].update(stage=SECRET), lambda v: v['failure'].update(kind=SECRET),
            lambda v: v['failure'].update(message=SECRET), lambda v: v.update(token=SECRET),
            lambda v: v.update(status=SECRET), lambda v: v.update(case=SECRET),
            lambda v: v.update(dimension=SECRET), lambda v: v.update(not_claimed=[]),
            lambda v: v['cleanup'].update(browser_closed={'error': SECRET}),
            lambda v: v['cleanup'].update(browser_closed=SECRET),
            lambda v: v['cleanup'].update(private_field=SECRET),
            lambda v: v['cleanup']['daemon_joined'].update(joined='true'),
            lambda v: v['cleanup']['daemon_joined'].update(exit=False)]
        for index, change in enumerate(mutations):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                self.mutate(private, 'driver/normal/report.json', change)
                self.unavailable(private)

    def test_partial_checks_must_be_original_complete_ordered_fixed_prefix(self):
        for variant in ['unknown', 'skip', 'repeat', 'extra', 'false', 'privatefact', 'oversizedfact']:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                def change(value):
                    rows = value['checks']
                    if variant == 'unknown': rows[0]['name'] = SECRET
                    elif variant == 'skip': rows.pop(0)
                    elif variant == 'repeat': rows.append(copy.deepcopy(rows[-1]))
                    elif variant == 'extra': rows[0]['private'] = SECRET
                    elif variant == 'false': rows[0]['status'] = False
                    elif variant == 'privatefact': rows[2]['proposal_sha256'] = SECRET
                    else: rows[2]['proposal_bytes'] = 1048577
                self.mutate(private, 'driver/normal/report.json', change)
                self.unavailable(private)

    def test_every_source_tool_and_pairing_binding_remains_mandatory(self):
        cases = [lambda v: v.update(source='b' * 40),
            lambda v: v.update(driver_sha256='0' * 64),
            lambda v: v['source_guard'].update(head='b' * 40),
            lambda v: v['source_guard'].update(tracked_clean=False),
            lambda v: v.update(manifest_sha256='0' * 64),
            lambda v: v.update(build_record_sha256='0' * 64),
            lambda v: v['tools']['batond'].update(sha256='0' * 64),
            lambda v: v['tools']['baton'].update(mode=0o755),
            lambda v: v['tools']['baton'].update(uid=os.geteuid() + 1),
            lambda v: v['tools']['baton'].update(links=2),
            lambda v: v['runtime_tools']['python'].update(mode=0o777),
            lambda v: v['assets'].update({'../' + SECRET: F.tool('a')})]
        for index, change in enumerate(cases):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                self.mutate(private, 'driver/normal/report.json', lambda v: change(v['inputs']))
                self.unavailable(private)

    def test_manifest_build_probe_and_typed_original_exit_are_not_bypassed(self):
        cases = [('status.json', 'source', 'b' * 40), ('status.json', 'probe_passed', False),
            *[('status.json', 'driver_exit', bad) for bad in (0, 2, None, True, False, 1.0, -1)],
            ('candidate.json', 'schema', 16), ('candidate.json', 'schema', True),
            ('candidate.json', 'git_head', 'b' * 40), ('candidate.json', 'dirty', True),
            ('candidate.json', 'tracked_clean', False), ('candidate.json', 'compiled_core_source_head', 'b' * 40),
            ('frontend-build-record.json', 'command', ['npm', 'build']),
            ('frontend-build-record.json', 'exit', True), ('frontend-build-record.json', 'source', 'b' * 40),
            ('frontend-build-record.json', 'source_head_and_tracked_unchanged', False),
            ('schema-probe.json', 'source', 'b' * 40), ('schema-probe.json', 'api_version', True),
            ('schema-probe.json', 'database_schema_version', 16), ('schema-probe.json', 'joined', False),
            ('schema-probe.json', 'daemon_exit', 1)]
        for filename, key, bad in cases:
            with self.subTest(file=filename, key=key, bad=bad), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                self.mutate(private, filename, lambda v: v.update({key: bad}))
                self.unavailable(private)

    def test_successful_sibling_must_match_sources_and_all_original_report_guards(self):
        changes = [lambda v: v['inputs']['runtime_tools']['python'].update(sha256='a' * 64),
            lambda v: v['checks'].pop(), lambda v: v['cleanup']['daemon_joined'].update(joined=False),
            lambda v: v.update(failure={'kind': SECRET}), lambda v: v['http_method_counts'].update({'integrations.start': 1})]
        for index, change in enumerate(changes):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                self.mutate(private, 'driver/command-failure/report.json', change)
                self.unavailable(private)

    def test_no_failed_case_is_not_a_runtime_failure_observation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = F.seed(root)
            F.write(private / 'status.json', {'source': HEAD, 'driver_exit': 1, 'probe_passed': True})
            self.unavailable(private)

    def test_private_descriptors_modes_symlinks_hardlinks_fifo_missing_and_size_are_strict(self):
        for variant in ['root-mode', 'driver-mode', 'case-mode', 'report-mode', 'report-symlink',
                'report-hardlink', 'report-fifo', 'directory-symlink', 'ancestor-symlink',
                'missing-report', 'missing-candidate', 'oversized', 'relative', 'dotdot']:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root); q = private / 'driver/normal/report.json'
                if variant == 'root-mode': private.chmod(0o755)
                elif variant == 'driver-mode': (private / 'driver').chmod(0o755)
                elif variant == 'case-mode': q.parent.chmod(0o755)
                elif variant == 'report-mode': q.chmod(0o644)
                elif variant == 'report-symlink':
                    raw = q.read_bytes(); q.unlink(); F.write(root / 'outside', raw); q.symlink_to(root / 'outside')
                elif variant == 'report-hardlink': os.link(q, root / 'linked')
                elif variant == 'report-fifo': q.unlink(); os.mkfifo(q, 0o600)
                elif variant == 'directory-symlink':
                    target = root / 'original'; q.parent.rename(target); q.parent.symlink_to(target, target_is_directory=True)
                elif variant == 'ancestor-symlink':
                    alias = root / 'alias'; alias.symlink_to(private, target_is_directory=True); private = alias
                elif variant == 'missing-report': q.unlink()
                elif variant == 'missing-candidate': (private / 'candidate.json').unlink()
                elif variant == 'oversized': F.write(q, b' ' * (E.MAX_REPORT + 1))
                elif variant == 'relative': private = Path('private')
                else: private = private / '..' / 'private'
                self.unavailable(private)

    def test_malformed_json_duplicate_keys_nonfinite_and_shape_are_unavailable(self):
        for raw in [b'[]', b'null', b'{"status":1,"status":2}', b'{"status":NaN}', b'{"status":Infinity}',
                b'{"nested":' * 1100 + b'null' + b'}' * 1100]:
            with self.subTest(raw=raw[:20]), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root)
                F.write(private / 'driver/normal/report.json', raw)
                self.unavailable(private)
        for filename in ['candidate.json', 'frontend-build-record.json', 'schema-probe.json']:
            with self.subTest(file=filename), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root); F.write(private / filename, [])
                self.unavailable(private)

    def test_later_case_cannot_hide_an_earlier_report_or_boundary_change(self):
        for variant in ['report', 'status', 'candidate', 'probe', 'case-directory', 'driver-directory']:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private = failed_seed(root); original = E.runtime_report_observation
                def changed(report, case, *args):
                    value = original(report, case, *args)
                    if case == 'command-failure':
                        if variant in ('case-directory', 'driver-directory'):
                            q = private / 'driver' / 'normal' if variant == 'case-directory' else private / 'driver'
                            q.rename(q.with_name(q.name + '-saved')); q.mkdir(mode=0o700)
                        else:
                            name = {'report': 'driver/normal/report.json', 'status': 'status.json',
                                'candidate': 'candidate.json', 'probe': 'schema-probe.json'}[variant]
                            q = private / name; q.write_bytes(q.read_bytes() + b' ')
                    return value
                with mock.patch.object(E, 'runtime_report_observation', side_effect=changed): self.unavailable(private)


class ExactRuntimeInverse(unittest.TestCase):
    def test_whole_projector_inverse_preserves_all_preexisting_guards(self):
        text = (ROOT / 'scripts/record-integration-browser-ci-evidence.py').read_text()
        restored = strip_runtime_failure_projection(text)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), RUNTIME_BASE_PROJECTOR_SHA256)
        self.assertNotIn(RUNTIME_LAYER_MARKER, restored)
        self.assertNotIn(RUNTIME_MAIN_LINE, restored)

    def test_unknown_inner_boundary_reordered_removed_duplicate_or_outside_edits_refuse(self):
        text = (ROOT / 'scripts/record-integration-browser-ci-evidence.py').read_text()
        start = text.index(RUNTIME_LAYER_MARKER); end = text.index('def publish(', start)
        block = text[start:end]
        mutations = [text.replace("status['driver_exit'] == 1", "status['driver_exit'] == 0", 1),
            text[:start] + block + block + text[end:], text[:start] + text[end:],
            text.replace(RUNTIME_MAIN_LINE, '', 1), text.replace(RUNTIME_MAIN_LINE, RUNTIME_MAIN_LINE * 2, 1),
            text.replace(block, block.replace('\n', '\r\n'), 1),
            text.replace(RUNTIME_LAYER_MARKER, '# Changed boundary\n', 1),
            text[:start] + block + '# Unreviewed between layer and publish\n' + text[end:],
            text.replace('MAX_REPORT, MAX_STATUS', 'MAX_REPORT, PRIVATE_UNKNOWN_STATUS', 1),
            text.replace('            summary[\'driver_preflight\']', '            summary[\'unknown_preflight\']', 1)]
        without = text[:start] + text[end:]
        mutations.append(without.replace('def input_summary(', block + 'def input_summary(', 1))
        moved_line = text.replace(RUNTIME_MAIN_LINE, '', 1)
        mutations.append(moved_line.replace('        publish(args.output, summary, payloads)\n',
            '        publish(args.output, summary, payloads)\n' + RUNTIME_MAIN_LINE, 1))
        for index, changed in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises(ValueError): strip_runtime_failure_projection(changed)


if __name__ == '__main__':
    unittest.main()
