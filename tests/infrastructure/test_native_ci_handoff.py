"""Synthetic images and real filesystem negatives, never execute product tools."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('native_ci_handoff_tested', ROOT / 'scripts/native-ci-handoff.py')
H = importlib.util.module_from_spec(spec)
spec.loader.exec_module(H)
IDENTITY = {'repository': 'wanderbot-auto/AgentBaton', 'run': '100', 'attempt': '2',
            'pr_head': 'a' * 40, 'test_merge': 'b' * 40, 'tree': 'c' * 40}


class Handoff(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.addCleanup(self.temp.cleanup)
        self.patch = mock.patch.object(H, 'ROOT', self.root)
        self.patch.start(); self.addCleanup(self.patch.stop)
        self.ident = mock.patch.object(H, 'identity', return_value=IDENTITY.copy())
        self.ident.start(); self.addCleanup(self.ident.stop)
        self.arm = mock.patch.object(H, 'arm_host')
        self.arm.start(); self.addCleanup(self.arm.stop)
        self.out = self.root / 'output'
        self.environ = mock.patch.dict(os.environ, {'GITHUB_OUTPUT': str(self.out), 'NATIVE_CI_CANCELLED': 'false'})
        self.environ.start(); self.addCleanup(self.environ.stop)
        self.image = struct.pack('<IIIIIIII', 0xfeedfacf, 0x0100000c, 0, 2, 0, 0, 0, 0) + b'synthetic-never-executed'
        for path in H.TOOLS.values():
            p = self.root / path; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(self.image)
        for path in ('artifacts/native-smoke/legacy-migration/summary.json', 'artifacts/native-smoke/factory-rework-progress.json'):
            p = self.root / path; p.parent.mkdir(parents=True, exist_ok=True); p.write_text('{"finite":"synthetic"}')
        self.directory = self.root / 'handoff'
        H.pack(self.directory)
        self.expected = H.digest(H.read(self.directory / 'manifest.json'))

    def rewrite(self, name, change):
        p = self.directory / name; value = json.loads(p.read_text()); change(value); p.write_text(json.dumps(value))
        if name == 'manifest.json': self.expected = H.digest(H.read(p))

    def test_fixed_files_actual_arm_hashes_current_identity_and_fresh_restore(self):
        record = H.validate_manifest(self.directory, self.expected)
        self.assertEqual(set(record['tools']), set(H.TOOLS))
        for path in H.TOOLS.values(): (self.root / path).unlink()
        H.restore(self.directory, self.expected)
        self.assertEqual(H.tools(), record['tools'])
        with self.assertRaises(ValueError): H.restore(self.directory, self.expected)
        self.assertEqual(len(list(self.directory.iterdir())), 6)

    def test_replayed_run_attempt_source_tree_and_extra_manifest_fields_rejected(self):
        original = (self.directory / 'manifest.json').read_bytes()
        for key, value in [('run', '99'), ('attempt', '1'), ('pr_head', 'd' * 40), ('test_merge', 'e' * 40), ('tree', 'f' * 40)]:
            self.rewrite('manifest.json', lambda v: v['identity'].__setitem__(key, value))
            with self.subTest(key=key), self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
            (self.directory / 'manifest.json').write_bytes(original)
        self.rewrite('manifest.json', lambda v: v.__setitem__('auth', 'forbidden'))
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)

    def test_unknown_profile_arch_hash_or_file_bytes_fail_closed(self):
        original = (self.directory / 'manifest.json').read_bytes()
        for change in [lambda v: v['tools'].__setitem__('smoke-baton', v['tools']['debug-baton']),
                       lambda v: v['tools']['debug-baton'].__setitem__('architecture', 'x86_64'),
                       lambda v: v['tools']['debug-baton'].__setitem__('sha256', '0' * 64),
                       lambda v: v.__setitem__('stage', 'native-default')]:
            self.rewrite('manifest.json', change)
            with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
            (self.directory / 'manifest.json').write_bytes(original)
        self.expected = H.digest(original)
        (self.directory / 'debug-baton').write_bytes(self.image + b'changed')
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)

    def test_symlinks_parent_hardlinks_fifo_extra_path_and_sparse_oversize_rejected(self):
        p = self.directory / 'debug-baton'; raw = p.read_bytes(); p.unlink()
        target = self.root / 'source'; target.write_bytes(raw); p.symlink_to(target)
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
        p.unlink(); os.link(target, p)
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
        p.unlink(); os.mkfifo(p)
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
        p.unlink(); p.write_bytes(raw)
        extra = self.directory / 'baton.sqlite3'; extra.write_text('never transferred')
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
        extra.unlink()
        with p.open('wb') as stream: stream.truncate(H.TOOL_LIMIT + 1)
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, self.expected)
        alias = self.root / 'alias'; alias.symlink_to(self.directory, target_is_directory=True)
        with self.assertRaises(ValueError): H.validate_manifest(alias, self.expected)

    def test_duplicate_json_nonfinite_malformed_and_wrong_macho_refused(self):
        p = self.root / 'json'
        for raw in ('{"x":1,"x":2}', '{"x":NaN}', '{'):
            p.write_text(raw)
            with self.assertRaises(ValueError): H.load(p)
        p.write_bytes(b'\xcf\xfa\xed\xfe' + b'\0' * 28)
        with self.assertRaises(ValueError): H.binary(p)
        with self.assertRaises(ValueError): H.validate_manifest(self.directory, '0' * 64)

    def seed_receipt(self):
        receipt = self.root / 'receipt'; receipt.mkdir()
        value = {'contract': H.CONTRACT, 'identity': IDENTITY.copy(), 'host': 'darwin-arm64',
                 'stage': 'native-default', 'producer_manifest_sha256': self.expected,
                 'actual_tools': H.tools(), 'actual_probe': True,
                 'default_app': H.binary(self.root / 'target/release/baton'), 'profile': 'release', 'features': [],
                 'package_sha256': 'd' * 64, 'package_bytes': 100,
                 'native_success': True, 'default_build_success': True,
                 'release_audit_success': True, 'package_success': True, 'development_upload_success': True}
        H.write(receipt / 'native.json', value)
        needs = {'desktop': {'result': 'success', 'outputs': {'artifact_id': '123', 'manifest_sha256': self.expected}},
                 'native': {'result': 'success', 'outputs': {'artifact_id': '124', 'receipt_sha256': H.digest(H.read(receipt / 'native.json'))}}}
        return receipt, value, needs

    def run_gate(self, receipt, needs):
        with mock.patch.dict(os.environ, NATIVE_CI_NEEDS=json.dumps(needs)):
            H.aggregate(self.directory, self.expected, receipt, needs['native']['outputs']['receipt_sha256'])

    def test_aggregate_requires_literal_success_for_both_current_run_arm_stages(self):
        receipt, value, needs = self.seed_receipt()
        self.run_gate(receipt, needs)
        for stage in needs:
            for result in ('skipped', 'cancelled', 'failure', 'neutral', 'unknown', ''):
                changed = copy.deepcopy(needs); changed[stage]['result'] = result
                with self.subTest(stage=stage, result=result), self.assertRaises(ValueError): self.run_gate(receipt, changed)
        with mock.patch.dict(os.environ, NATIVE_CI_CANCELLED='true'), self.assertRaises(ValueError): self.run_gate(receipt, needs)

    def test_aggregate_rejects_missing_audit_wrong_profile_smoke_features_or_replayed_receipt(self):
        receipt, original, needs = self.seed_receipt()
        for key, value in [('release_audit_success', False), ('development_upload_success', False),
                           ('profile', 'debug'), ('features', ['native-smoke']), ('host', 'linux-arm64'),
                           ('actual_probe', 1), ('package_bytes', True), ('producer_manifest_sha256', '0' * 64)]:
            changed = copy.deepcopy(original); changed[key] = value
            (receipt / 'native.json').write_text(json.dumps(changed))
            needs['native']['outputs']['receipt_sha256'] = H.digest(H.read(receipt / 'native.json'))
            with self.subTest(key=key), self.assertRaises(ValueError): self.run_gate(receipt, needs)
        changed = copy.deepcopy(original); del changed['release_audit_success']
        (receipt / 'native.json').write_text(json.dumps(changed))
        needs['native']['outputs']['receipt_sha256'] = H.digest(H.read(receipt / 'native.json'))
        with self.assertRaises(ValueError): self.run_gate(receipt, needs)

    def actual_fixture(self):
        actual = self.root / 'actual'; actual.mkdir()
        record = {'contract': H.CONTRACT, 'identity': IDENTITY.copy(), 'host': 'darwin-arm64',
                  'tools': H.tools(), 'empty_schema': 18, 'debug_and_release_probe': True,
                  'sigterm_joined': True, 'producer_manifest_sha256': self.expected,
                  'build': {'debug': ['cargo', 'build', '--locked', '--bin', 'batond', '--bin', 'baton'],
                            'release': 'verified_current_source_producer_handoff', 'features': []}}
        H.write(actual / 'actual.json', record)
        for path in ('apps/desktop/src-tauri/target/debug/batond',
                     'apps/desktop/src-tauri/binaries/baton-aarch64-apple-darwin',
                     'apps/desktop/src-tauri/binaries/batond-aarch64-apple-darwin'):
            p = self.root / path; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(self.image)
        return actual

    def test_actual_check_refuses_rebuilt_bytes_native_sibling_sidecar_or_probe_replay(self):
        actual = self.actual_fixture(); H.check_actual(actual)
        for path in ('target/debug/baton', 'apps/desktop/src-tauri/target/debug/batond',
                     'apps/desktop/src-tauri/binaries/baton-aarch64-apple-darwin'):
            p = self.root / path; p.write_bytes(self.image + b'different-reconstruction')
            with self.subTest(path=path), self.assertRaises(ValueError): H.check_actual(actual)
            p.write_bytes(self.image)
        p = actual / 'actual.json'; value = json.loads(p.read_text()); value['identity']['attempt'] = '1'; p.write_text(json.dumps(value))
        with self.assertRaises(ValueError): H.check_actual(actual)

    def package_fixture(self):
        p = self.root / H.APP; p.parent.mkdir(parents=True); p.write_bytes(self.image)
        dev = self.root / 'artifacts/development'; dev.mkdir()
        name = 'AgentBaton-arm64-' + IDENTITY['test_merge'] + '-development.dmg'
        (dev / name).write_bytes(b'synthetic-package-never-mounted')
        (dev / 'SHA256SUMS').write_text(H.digest((dev / name).read_bytes()) + '  ./' + name + '\n')
        (dev / 'build-record.txt').write_text('commit=' + IDENTITY['test_merge'] + '\narchitecture=arm64\n')
        (dev / 'DEVELOPMENT_ACCEPTANCE.md').write_text('synthetic acceptance source')
        return dev, name

    def test_complete_reads_real_default_image_package_and_checksums_before_receipt(self):
        actual = self.actual_fixture(); dev, name = self.package_fixture(); receipt = self.root / 'complete'
        H.complete(self.directory, self.expected, actual, receipt)
        value = H.load(receipt / 'native.json')
        self.assertEqual(value['default_app'], H.binary(self.root / H.APP))
        self.assertEqual(value['package_sha256'], H.digest((dev / name).read_bytes()))
        self.assertTrue(value['release_audit_success'])

    def test_complete_refuses_package_corruption_missing_audit_image_extra_files_and_cancel(self):
        actual = self.actual_fixture(); dev, name = self.package_fixture(); receipt = self.root / 'never'
        sums = dev / 'SHA256SUMS'; original = sums.read_bytes(); sums.write_text('0' * 64 + '  ./' + name + '\n')
        with self.assertRaises(ValueError): H.complete(self.directory, self.expected, actual, receipt)
        self.assertFalse(receipt.exists()); sums.write_bytes(original)
        p = self.root / H.APP; image = p.read_bytes(); p.unlink()
        with self.assertRaises(OSError): H.complete(self.directory, self.expected, actual, receipt)
        p.write_bytes(image)
        extra = dev / 'test-control-app'; extra.write_bytes(b'forbidden')
        with self.assertRaises(ValueError): H.complete(self.directory, self.expected, actual, receipt)
        extra.unlink()
        with mock.patch.dict(os.environ, NATIVE_CI_CANCELLED='true'), self.assertRaises(ValueError):
            H.complete(self.directory, self.expected, actual, receipt)
        self.assertFalse(receipt.exists())

    def test_probe_records_fresh_debug_reconstruction_and_rejects_changed_bytes_or_failed_probe(self):
        # Stub only the existing empty-runtime probe. No fixture executable runs.
        helper = type('Probe', (), {})()
        def probe(tools, evidence):
            return {'schema': 18, 'snapshot_api_version': 1, 'sigterm_sent': True,
                    'forced_kill': False, 'original_daemon_SIGTERM_wait_exit': 0}
        helper.probe = probe
        source = H.importlib.util.spec_from_file_location('probe_for_test', ROOT / 'scripts/prepare-factory-test-tools.py')
        source.loader = type('Loader', (), {'exec_module': lambda *args: None})()
        # The real helper import is replaced, never product/process behavior.
        with mock.patch.object(H.importlib.util, 'spec_from_file_location', return_value=source), \
                mock.patch.object(H.importlib.util, 'module_from_spec', return_value=helper):
            rebuilt = self.root / 'target/debug/baton'; rebuilt.write_bytes(self.image + b'fresh-source-build')
            H.probe_actual(self.root / 'probed', self.expected)
            value = H.load(self.root / 'probed/actual.json')
            self.assertNotEqual(value['tools']['debug-baton'], H.load(self.directory / 'manifest.json')['tools']['debug-baton'])
            helper.probe = lambda *args: {'schema': 18, 'snapshot_api_version': 1, 'sigterm_sent': False,
                                         'forced_kill': True, 'original_daemon_SIGTERM_wait_exit': -9}
            with self.assertRaises(ValueError): H.probe_actual(self.root / 'bad-probe', self.expected)
            self.assertFalse((self.root / 'bad-probe/actual.json').exists())
            def changing(tools, evidence):
                rebuilt.write_bytes(rebuilt.read_bytes() + b'changed-mid-probe')
                return probe(tools, evidence)
            helper.probe = changing
            with self.assertRaises(ValueError): H.probe_actual(self.root / 'changed-probe', self.expected)
            self.assertFalse((self.root / 'changed-probe/actual.json').exists())


if __name__ == '__main__': unittest.main()
