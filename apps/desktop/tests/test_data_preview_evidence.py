#!/usr/bin/env python3
"""Private fixture file/control tests only; never starts Core, browser or worker."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import time
import types
import unittest
from unittest.mock import patch
import uuid

SOURCE = Path(__file__).with_name('data-workspace-real-ipc.py').read_text()
TREE = ast.parse(SOURCE)
FUNCTIONS = {'sha', 'unique_object', 'private_runtime_fd', 'private_evidence_file', 'prepare_preview_evidence', 'preview_evidence_ready'}


def extracted_scope():
    scope = dict(os=os, stat=stat, json=json, hashlib=hashlib, re=re, uuid=uuid, time=time)
    definitions = [n for n in TREE.body if isinstance(n, ast.FunctionDef) and n.name in FUNCTIONS or isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id.startswith('PREVIEW_EVIDENCE_') for t in n.targets)]
    exec(compile(ast.Module(body=definitions, type_ignores=[]), '<actual-private-file-helpers>', 'exec'), scope)
    return scope


class PreviewEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='preview-evidence-pure-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.runtime, self.source = self.root / 'runtime', self.root / 'source'
        self.runtime.mkdir(mode=0o700)
        self.source.mkdir(mode=0o700)
        with (self.source / 'large.bin').open('wb') as stream:
            stream.truncate(32 * 1024 * 1024)  # sparse metadata fixture; no product scan
        pin = (self.source / 'large.bin').stat()
        self.original = {'large.bin': {'bytes': pin.st_size, 'mtime_ns': pin.st_mtime_ns, 'sha256': 'a' * 64}}
        self.scope = extracted_scope()
        self.gate = self.scope['prepare_preview_evidence'](self.runtime, self.source, self.original)
        self.marker = self.runtime / self.scope['PREVIEW_EVIDENCE_READY']
        self.ready = {'schema_version': 1, 'gate': 'agentbaton_preview_evidence_gate_v1', 'stage': 'before_scan',
                      'nonce': self.gate['nonce'], 'descriptor_sha256': self.gate['descriptor_sha256'],
                      'preview_id': 'actual-preview', 'project_id': 'actual-project'}

    def write(self, value):
        raw = value if isinstance(value, bytes) else json.dumps(value).encode()
        fd = os.open(self.marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw)
        return raw

    def read(self):
        return self.scope['preview_evidence_ready'](self.runtime, self.gate, 'actual-preview', 'actual-project')

    def test_config_exact_fixture_and_marker_match_are_private_not_public_authority(self):
        config = self.runtime / self.scope['PREVIEW_EVIDENCE_CONFIG']
        raw = config.read_bytes()
        value = json.loads(raw)
        self.assertEqual(set(value), {'schema_version', 'nonce', 'source', 'files', 'limits'})
        self.assertEqual(value['source']['canonical_root'], str(self.source))
        self.assertEqual(value['source']['device'], str(self.source.stat().st_dev))
        self.assertEqual(value['source']['inode'], str(self.source.stat().st_ino))
        self.assertEqual(value['files'], [{'artifact_id': 'file-1', 'relative_path': 'large.bin', 'max_bytes': 67_108_864, 'expected_sha256': 'a' * 64}])
        self.assertEqual(value['limits'], {'max_file_bytes': 67_108_864, 'max_total_bytes': 67_108_864, 'max_files': 4})
        self.assertEqual(stat.S_IMODE(config.stat().st_mode), 0o600)
        self.assertEqual(config.stat().st_nlink, 1)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), self.gate['descriptor_sha256'])
        marker = self.write(self.ready)
        observed = self.read()
        self.assertEqual(observed['stage'], 'before_scan')
        self.assertTrue(observed['nonce_preview_project_matched'])
        self.assertEqual(observed['marker_sha256'], hashlib.sha256(marker).hexdigest())
        self.assertNotIn('nonce', observed)

    def test_configuration_is_create_once_and_never_overwritten(self):
        config = self.runtime / self.scope['PREVIEW_EVIDENCE_CONFIG']
        before = config.read_bytes()
        with self.assertRaises(FileExistsError):
            self.scope['prepare_preview_evidence'](self.runtime, self.source, self.original)
        self.assertEqual(config.read_bytes(), before)

    def test_default_off_does_not_prepare_or_read_even_missing_runtime(self):
        startup = next(n for n in ast.walk(TREE) if isinstance(n, ast.If) and any(isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'prepare_preview_evidence' for c in ast.walk(n)))
        ready = next(n for n in ast.walk(TREE) if isinstance(n, ast.If) and any(isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'preview_evidence_ready' for c in ast.walk(n)))

        def forbidden(*args):
            self.fail('default mode touched private gate')

        namespace = dict(args=types.SimpleNamespace(preview_evidence=False), evidence_gate=None,
                         prepare_preview_evidence=forbidden, preview_evidence_ready=forbidden)
        for node in (startup, ready):
            exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-disabled-gate-branches>', 'exec'), namespace)
        self.assertIsNone(namespace['evidence_gate'])

    def test_missing_feature_marker_is_not_proof(self):
        self.assertIsNone(self.read())

    def test_create_new_partial_writer_must_finish_before_marker_is_proof(self):
        raw = json.dumps(self.ready).encode()
        fd = os.open(self.marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            halfway = len(raw) // 2
            os.write(fd, raw[:halfway])
            self.assertIsNone(self.read())
            os.write(fd, raw[halfway:])
            os.fsync(fd)
            self.assertTrue(self.read()['nonce_preview_project_matched'])
        finally:
            os.close(fd)

    def test_complete_wrong_nonce_preview_project_digest_stage_or_shape_is_rejected(self):
        controls = [('nonce', 'b' * 32), ('preview_id', 'foreign-preview'), ('project_id', 'foreign-project'),
                    ('descriptor_sha256', 'b' * 64), ('stage', 'mid_read'), ('gate', 'foreign-gate'),
                    ('schema_version', 2), ('schema_version', True), ('extra', False), ('preview_id', None)]
        for key, value in controls:
            with self.subTest(key=key, value=value):
                marker = {**self.ready, key: value}
                self.write(marker)
                with self.assertRaises(AssertionError):
                    self.read()
                self.marker.unlink()

    def test_missing_or_duplicate_fields_and_complete_malformed_json_are_rejected(self):
        missing = dict(self.ready)
        del missing['nonce']
        for value in [missing, b'{"schema_version":1,"schema_version":1}', b'{not-json}']:
            self.write(value)
            with self.assertRaises((AssertionError, ValueError)):
                self.read()
            self.marker.unlink()

    def test_wrong_marker_mode_is_rejected(self):
        self.write(self.ready)
        self.marker.chmod(0o644)
        with self.assertRaises(AssertionError):
            self.read()

    def test_marker_symlink_and_multilink_are_rejected(self):
        outside = self.root / 'marker-target'
        outside.write_text(json.dumps(self.ready))
        outside.chmod(0o600)
        self.marker.symlink_to(outside)
        with self.assertRaises(OSError):
            self.read()
        self.marker.unlink()
        os.link(outside, self.marker)
        with self.assertRaises(AssertionError):
            self.read()

    def test_nonregular_marker_is_rejected_without_waiting_for_fifo_writer(self):
        os.mkfifo(self.marker, 0o600)
        with self.assertRaises(AssertionError):
            self.read()

    def test_oversized_marker_is_rejected_without_truncation(self):
        self.write(b'x' * 8193)
        with self.assertRaises(AssertionError):
            self.read()

    def test_wrong_runtime_mode_owner_and_replacement_are_rejected(self):
        self.write(self.ready)
        self.runtime.chmod(0o755)
        with self.assertRaises(AssertionError):
            self.read()
        self.runtime.chmod(0o700)
        with patch.object(os, 'getuid', return_value=os.getuid() + 1):
            with self.assertRaises(AssertionError):
                self.read()
        self.runtime.rename(self.root / 'old-runtime')
        self.runtime.mkdir(mode=0o700)
        with self.assertRaises(AssertionError):
            self.read()

    def test_file_owner_check_rejects_foreign_owner_expectation(self):
        self.write(self.ready)
        fd = os.open(self.marker, os.O_RDONLY)
        try:
            with patch.object(os, 'getuid', return_value=os.getuid() + 1):
                with self.assertRaises(AssertionError):
                    self.scope['private_evidence_file'](fd)
        finally:
            os.close(fd)

    def test_missing_marker_observation_times_out_instead_of_silently_falling_back(self):
        wait = next(n for n in ast.walk(TREE) if isinstance(n, ast.FunctionDef) and n.name == 'wait')
        calls = []
        ticks = iter([0, 8, 16])
        clock = types.SimpleNamespace(monotonic=lambda: next(ticks), sleep=lambda _: None)
        scope = dict(time=clock, pump=lambda: None, safe=str)
        exec(compile(ast.Module(body=[wait], type_ignores=[]), '<actual-driver-wait>', 'exec'), scope)

        def read():
            calls.append('missing real marker')
            return self.read()

        with self.assertRaisesRegex(AssertionError, 'observation deadline'):
            scope['wait'](read, lambda value: value is not None, seconds=15)
        self.assertEqual(len(calls), 2)
        self.assertIn('lambda value: value is not None, seconds=15', SOURCE)


if __name__ == '__main__':
    unittest.main()
