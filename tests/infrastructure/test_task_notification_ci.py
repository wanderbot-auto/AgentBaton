"""Synthetic CI/projector controls only; no browser/Core/SQL/network execution."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import struct
import tempfile
import types
import unittest
from unittest import mock
import zlib

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/product.yml'
STEP = 'Retain finite task notification evidence without private runtime data'
HEAD, SECRET = 'a' * 40, 'PRIVATE_TOKEN_DATABASE_PATH_CANARY'


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


N = load('_task_notification_ci_pure', 'scripts/record-task-notification-ci-evidence.py')
F = load('_notification_ci_original_fixture', 'tests/infrastructure/test_integration_browser_ci_evidence.py')
W = load('_notification_workflow_inverse', 'tests/infrastructure/test_schema17_migration_workflow.py')


def source_pin():
    raw = (ROOT / N.PROBE).read_bytes()
    return {'source': HEAD, 'relative_path': N.PROBE, 'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}


def seed(root):
    private = F.seed(root)
    normal = private / 'driver/normal/report.json'
    base = json.loads(normal.read_text())
    base['http_method_counts'].update({'system.snapshot': 20, 'sessions.list': 10, 'events.subscribe': 10})
    F.write(normal, base)
    common = {'name': '', 'status': 'passed'}
    checks = [dict(common, name=N.CHECKS[0], actual_reply_sha256='b' * 64, task_revision=3, acceptance_sequence=9, mapped_event_count=2),
        dict(common, name=N.CHECKS[1], notices=1, naturally_observed_events=2, synthetic_repeated_events=0),
        dict(common, name=N.CHECKS[2], inspected_references=['event-one', 'event-two'], original_event_id='event-two',
            original_event_sequence=9, original_payload_matched=True),
        dict(common, name=N.CHECKS[3], same_task=True, modal_count=1, shortcuts=4, escape_background_restored=True),
        dict(common, name=N.CHECKS[4], ui_reload=True, manual_evidence=True, os_notification_delivery_tested=False),
        dict(common, name=N.CHECKS[5], bridge_read_counts={'system.snapshot': 2, 'sessions.list': 2, 'events.subscribe': 2},
            private_read_counts={'system.snapshot': 2, 'sessions.list': 2, 'events.subscribe': 2})]
    report = {'contract': 'agentbaton.task-notifications-real-ipc.v1', 'status': 'passed', 'flow_complete': True,
        'dimension': N.DIMENSION, 'not_claimed': N.NOT_CLAIMED, 'source': HEAD, 'probe_source': source_pin(),
        'checks': checks, 'screenshots': [], 'cleanup_observed': True,
        'observations': {'task_id': 'task-' + SECRET, 'acceptance_ref': 'acceptance-' + SECRET,
            'source_event_count': 2, 'inspected_event_count': 2, 'actual_reply_sha256': 'b' * 64, 'fixed_deadline_seconds': 15}}
    path = private / 'driver/normal/notification-probe'
    for name in N.NAMES:
        mobile = 'mobile' in name
        raw = F.IMAGES['mobile' if mobile else 'desktop']
        row = {'name': name, 'sha256': N.raw_sha(raw), 'bytes': len(raw),
            'viewport': {'width': 390 if mobile else 1440, 'height': 844 if mobile else 1100},
            'focus_box_before_capture_observation': {'x': 20, 'y': 100, 'width': 100, 'height': 20},
            'pixel_synchronized': False, 'dimension': 'Linux Chromium only'}
        report['screenshots'].append(row)
        F.write(path / (name + '.png'), raw)
    F.write(path / 'report.json', report)
    F.write(path / 'traceback.txt', SECRET.encode())
    return private, path


class NotificationWorkflowInverse(unittest.TestCase):
    def test_exact_inverse_restores_entire_prior_workflow_and_all_historical_pins(self):
        text = WORKFLOW.read_text()
        restored = W.strip_task_notification_ci_layer(text)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), '2571ca962ee7eca8ef0b679b86bd939187d69427d83fce3d5ca18c885fc18d23')
        W.verify_only_six_historical_target_changes(text)
        self.assertEqual(W.BASELINE_PRODUCT_SHA256, '4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d')
        self.assertEqual(W.BROWSER_CI_BASE_SHA256, '599074c95afb9324470bc859f11added4b5232a877ba6e0486001e1bebf015fc')
        self.assertEqual(text.count("'--notification-probe',"), 1)
        self.assertEqual(text.count('apps/desktop/tests/integration-real-ipc.py\', \'--run\', \'--case\', \'all\''), 1)

    def test_unknown_permission_owner_memory_toolchain_job_writes_and_other_changes_refuse(self):
        text = WORKFLOW.read_text()
        for old, new in [('contents: read', 'contents: write'), ('timeout-minutes: 20', 'timeout-minutes: 21'),
            ('AGENTBATON_OWNER_FIXTURE_DAEMON:', 'AGENTBATON_OWNER_FAKE_DAEMON:'), ('134217728', '268435456'),
            ('set -euo pipefail', 'export RUSTFLAGS=-Copt-level=3\n          set -euo pipefail'),
            ("'--notification-probe',", "'--notification-probe', '--approve-all',"),
            ('TASK_NOTIFICATION_DRIVER_OUTCOME:', 'UNTRUSTED_OWNER_AUTHORITY:'),
            ("'--driver-step-outcome', outcome,", "'--driver-step-outcome', 'success',"),
            ('runs-on: ubuntu-24.04', 'runs-on: self-hosted'), ('retention-days: 14', 'retention-days: 15')]:
            self.assertIn(old, text)
            with self.subTest(old=old), self.assertRaises(ValueError): W.verify_only_six_historical_target_changes(text.replace(old, new, 1))

    def test_exact_step_cannot_move_duplicate_disappear_or_hide_an_extra_job(self):
        text = WORKFLOW.read_text(); headers = list(re.finditer(r'^      - .*\n', text, re.M))
        start, end = headers[29].start(), headers[30].start(); block = text[start:end]
        for changed in [text[:start] + text[end:], text[:start] + block + block + text[end:],
            text[:start] + block.replace('if: always()', 'if: success()') + text[end:],
            text[:start] + block.replace('0o600', '0o644') + text[end:],
            text[:start] + block.replace('\n', '\r\n') + text[end:],
            text[:start] + '  unknown_job:\n' + block + text[end:]]:
            with self.subTest(), self.assertRaises(ValueError): W.strip_task_notification_ci_layer(changed)
        without = text[:start] + text[end:]
        moved = without.replace('      - name: Retain finite integration browser evidence without private runtime data\n',
            block + '      - name: Retain finite integration browser evidence without private runtime data\n', 1)
        with self.assertRaises(ValueError): W.strip_task_notification_ci_layer(moved)

    def program(self):
        text = WORKFLOW.read_text(); start = text.index('      - name: ' + STEP + '\n')
        block = text[start:text.index('      - uses:', start)]
        source = block.split("          python3 - <<'TASK_NOTIFICATION_EVIDENCE'\n", 1)[1].rsplit('          TASK_NOTIFICATION_EVIDENCE\n', 1)[0]
        source = '\n'.join(line[10:] if line.startswith('          ') else line for line in source.splitlines())
        code = {'__name__': '_notification_projection_ci_contract'}
        exec(compile(source, '<fixed-notification-ci>', 'exec'), code)
        return code

    def test_new_step_invokes_only_fixed_projector_and_never_overrides_failure_outcome(self):
        for outcome, exit_code, expected in [('success', 0, 0), ('success', 1, 1), ('failure', 0, 1), ('unobserved', 0, 1), ('success', False, 1)]:
            with self.subTest(outcome=outcome, exit_code=exit_code), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); (root / 'artifacts/ui-browser').mkdir(parents=True)
                previous = Path.cwd()
                try:
                    os.chdir(root)
                    calls = []
                    code = self.program()
                    def run(command, **kwargs):
                        calls.append((command, kwargs)); return types.SimpleNamespace(returncode=exit_code)
                    code['subprocess'] = types.SimpleNamespace(check_output=lambda *a, **k: HEAD + '\n', run=run, STDOUT=-2)
                    with mock.patch.dict(os.environ, {'RUNNER_TEMP': str(root), 'TASK_NOTIFICATION_DRIVER_OUTCOME': outcome}):
                        self.assertEqual(code['project_task_notifications'](), expected)
                    self.assertEqual(len(calls), 1)
                    command, kwargs = calls[0]
                    self.assertEqual(command[1], 'scripts/record-task-notification-ci-evidence.py')
                    self.assertEqual(command[-2:], ['--driver-step-outcome', outcome])
                    self.assertIn(str(root / 'artifacts/ui-browser/task-notifications'), command)
                    self.assertIs(kwargs['check'], False)
                finally:
                    os.chdir(previous)


class FiniteNotificationProjector(unittest.TestCase):
    def project(self, private, output, outcome='success'):
        with mock.patch.object(N, 'expected_probe', return_value=source_pin()):
            return N.main(['--private', str(private), '--output', str(output), '--expected-source-head', HEAD,
                '--driver-step-outcome', outcome])

    def test_complete_original_pair_and_probe_publish_only_four_original_pngs_and_finite_summary(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private, probe = seed(root); output = root / 'public'
            self.assertEqual(self.project(private, output), 0)
            self.assertEqual({path.name for path in output.iterdir()}, N.PUBLIC_NAMES | {'summary.json'})
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual((summary['checks'], summary['naturally_observed_events'], summary['task_notices']), (6, 2, 1))
            self.assertEqual(summary['probe_source_sha256'], source_pin()['sha256'])
            self.assertIs(summary['cleanup']['joined'], True)
            self.assertIs(summary['screenshots_published'], True)
            self.assertEqual(summary['visual_review'], 'not_performed_by_projector')
            for name in N.PUBLIC_NAMES:
                self.assertEqual((output / name).read_bytes(), (probe / name).read_bytes())
            self.assertNotIn(SECRET, json.dumps(summary)); self.assertNotIn(str(private), json.dumps(summary))
            self.assertNotIn('inspected_references', summary)
            self.assertEqual(N.E.PUBLIC_NAMES, F.E.PUBLIC_NAMES)
            self.assertEqual(len(N.E.PUBLIC_NAMES), 12)

    def test_unknown_fields_paths_false_cleanup_source_or_partial_success_never_publish(self):
        changes = [lambda r: r.update(raw_database=SECRET), lambda r: r.update(status='observed_pending_cleanup'),
            lambda r: r.update(cleanup_observed=1), lambda r: r.update(flow_complete=False),
            lambda r: r.update(source='f' * 40), lambda r: r['probe_source'].update(sha256='0' * 64),
            lambda r: r['probe_source'].update(relative_path='/private/' + SECRET),
            lambda r: r['checks'].pop(), lambda r: r['checks'][0].update(payload=SECRET),
            lambda r: r['checks'][2].update(original_event_id='../' + SECRET),
            lambda r: r['checks'][5]['bridge_read_counts'].update({'sessions.send': 1}),
            lambda r: r['checks'][5]['private_read_counts'].update({'unknown.read': 1}),
            lambda r: r['checks'][5]['bridge_read_counts'].update({'events.subscribe': 100001}),
            lambda r: r['checks'][5]['bridge_read_counts'].update({'events.subscribe': True}),
            lambda r: r['checks'][1].update(synthetic_repeated_events=100),
            lambda r: r['checks'][3].update(modal_count=True),
            lambda r: r['observations'].update(acceptance_payload=SECRET),
            lambda r: r['screenshots'].pop(), lambda r: r['screenshots'][0].update(name='../' + SECRET),
            lambda r: r['screenshots'][0].update(private_payload=SECRET),
            lambda r: r['screenshots'][0]['viewport'].update(width=390),
            lambda r: r['screenshots'][0]['focus_box_before_capture_observation'].update(x=float('nan'))]
        for change in changes:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root); path = probe / 'report.json'
                report = json.loads(path.read_text()); change(report)
                path.write_text(json.dumps(report))  # Deliberately permit NaN bytes for rejection.
                output = root / 'public'
                self.assertEqual(self.project(private, output), 1)
                self.assertEqual({p.name for p in output.iterdir()}, {'summary.json'})
                self.assertNotIn(SECRET, (output / 'summary.json').read_text())

    def test_original_failure_case_schema_candidate_build_and_cleanup_remain_required(self):
        for variant in ('parent-failure', 'missing-old-png', 'schema', 'candidate', 'build', 'normal-cleanup'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root)
                if variant == 'missing-old-png':
                    (private / 'driver/command-failure/terminal-state-mobile.png').unlink()
                else:
                    path = private / {'parent-failure': 'driver/command-failure/report.json', 'schema': 'schema-probe.json',
                        'candidate': 'candidate.json', 'build': 'frontend-build-record.json', 'normal-cleanup': 'driver/normal/report.json'}[variant]
                    value = json.loads(path.read_text())
                    if variant == 'parent-failure': value['status'] = 'failed_or_interrupted'
                    elif variant == 'schema': value['database_schema_version'] = 16
                    elif variant == 'candidate': value['git_head'] = 'f' * 40
                    elif variant == 'build': value['source'] = 'f' * 40
                    else: value['cleanup']['daemon_joined']['joined'] = False
                    F.write(path, value)
                self.assertEqual(self.project(private, root / 'public'), 1)
                self.assertEqual({p.name for p in (root / 'public').iterdir()}, {'summary.json'})

    def test_failed_unobserved_or_skipped_driver_cannot_reuse_complete_old_pass(self):
        for outcome in ('failure', 'skipped', 'cancelled', 'unobserved'):
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root)
                with mock.patch.object(N, 'collect', side_effect=AssertionError('Must not read stale success')):
                    self.assertEqual(self.project(private, root / 'public', outcome), 1)
                self.assertEqual(json.loads((root / 'public/summary.json').read_text())['failure_kind'], 'driver_step_not_successful')

    def test_private_file_aliases_metadata_and_corrupt_pngs_refuse(self):
        for variant in ('report-mode', 'report-symlink', 'report-hardlink', 'directory-symlink', 'png-hardlink', 'png-crc', 'png-text'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root)
                report = probe / 'report.json'; png = probe / 'accepted-task-desktop.png'
                if variant == 'report-mode': report.chmod(0o644)
                elif variant == 'report-symlink':
                    raw = report.read_bytes(); report.unlink(); F.write(root / 'outside', raw); report.symlink_to(root / 'outside')
                elif variant == 'report-hardlink': os.link(report, root / 'alias')
                elif variant == 'directory-symlink':
                    probe.rename(root / 'outside'); probe.symlink_to(root / 'outside', target_is_directory=True)
                elif variant == 'png-hardlink': os.link(png, root / 'alias')
                else:
                    raw = png.read_bytes()
                    if variant == 'png-crc': raw = raw[:-1] + bytes([raw[-1] ^ 1])
                    else:
                        data = b'tEXt' + SECRET.encode()
                        raw = raw[:-12] + struct.pack('>I', len(SECRET)) + data + struct.pack('>I', zlib.crc32(data) & 0xffffffff) + raw[-12:]
                    png.write_bytes(raw)
                    value = json.loads(report.read_text()); value['screenshots'][0].update(sha256=N.raw_sha(raw), bytes=len(raw)); F.write(report, value)
                self.assertEqual(self.project(private, root / 'public'), 1)
                self.assertEqual({p.name for p in (root / 'public').iterdir()}, {'summary.json'})

    def test_exclusive_output_and_pinned_source_object_reject_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private, probe = seed(root); output = root / 'public'
            output.mkdir(); F.write(output / 'keep', SECRET.encode())
            self.assertEqual(self.project(private, output), 1)
            self.assertEqual((output / 'keep').read_text(), SECRET)
        raw = (ROOT / N.PROBE).read_bytes()
        with mock.patch.object(N.subprocess, 'check_output', side_effect=[b'', raw]):
            self.assertEqual(N.expected_probe(HEAD), source_pin())
        for values in ([b'M modified'], [b'', b'changed']):
            with mock.patch.object(N.subprocess, 'check_output', side_effect=values), self.assertRaises(N.Refused): N.expected_probe(HEAD)

    def test_normal_source_or_read_counts_cannot_detach_after_complete_parent_collection(self):
        for variant in ('tools', 'assets', 'runtime', 'reads'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root)
                original_collect = N.E.collect
                def changed(*args):
                    result = original_collect(*args)
                    path = private / 'driver/normal/report.json'; value = json.loads(path.read_text())
                    if variant == 'tools': value['inputs']['tools']['batond']['sha256'] = '0' * 64
                    elif variant == 'assets': value['inputs']['assets']['index.html']['sha256'] = '0' * 64
                    elif variant == 'runtime': value['inputs']['runtime_tools']['python']['sha256'] = '0' * 64
                    else: value['http_method_counts']['system.snapshot'] += 1
                    F.write(path, value)
                    return result
                with mock.patch.object(N.E, 'collect', side_effect=changed):
                    self.assertEqual(self.project(private, root / 'public'), 1)
                self.assertEqual({path.name for path in (root / 'public').iterdir()}, {'summary.json'})

    def test_probe_directory_report_or_png_cannot_change_after_validation(self):
        for variant in ('directory', 'report', 'png'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, probe = seed(root)
                if variant == 'directory':
                    original_collect = N.E.collect
                    def changed(*args):
                        result = original_collect(*args)
                        probe.rename(root / 'original-probe')
                        probe.mkdir(mode=0o700)
                        return result
                    context = mock.patch.object(N.E, 'collect', side_effect=changed)
                elif variant == 'report':
                    original_summary = N.report_summary
                    def changed(*args):
                        result = original_summary(*args)
                        path = probe / 'report.json'; path.write_bytes(path.read_bytes() + b' ')
                        return result
                    context = mock.patch.object(N, 'report_summary', side_effect=changed)
                else:
                    original_png = N.P.png_bytes
                    def changed(raw, row, name):
                        result = original_png(raw, row, name)
                        if name == 'accepted-task-desktop.png':
                            path = probe / name; path.write_bytes(path.read_bytes() + b' ')
                        return result
                    context = mock.patch.object(N.P, 'png_bytes', side_effect=changed)
                with context:
                    self.assertEqual(self.project(private, root / 'public'), 1)
                self.assertEqual({path.name for path in (root / 'public').iterdir()}, {'summary.json'})


if __name__ == '__main__':
    unittest.main()
