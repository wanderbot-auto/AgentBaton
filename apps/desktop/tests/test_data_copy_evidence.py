#!/usr/bin/env python3
"""Private copy gate and inert SQLite receipt contracts, not Core/worker/browser proof."""
import ast
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import stat
import tempfile
import types
import unittest

SPEC = importlib.util.spec_from_file_location('preview_private_fixture_for_copy', Path(__file__).with_name('test_data_preview_evidence.py'))
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


def copy_scope():
    scope = BASE.extracted_scope()
    functions = {'prepare_copy_evidence', 'copy_evidence_ready', 'copy_evidence_cancel_commitment', 'wait_preview_evidence_ready', 'copy_completion', 'integer'}
    nodes = [n for n in BASE.TREE.body if isinstance(n, ast.FunctionDef) and n.name in functions or isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id.startswith('COPY_EVIDENCE_') for t in n.targets)]
    exec(compile(ast.Module(body=nodes, type_ignores=[]), '<actual-copy-evidence-helpers>', 'exec'), scope)
    return scope


class CopyPrivateEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.preview = BASE.PreviewEvidenceTests()
        self.preview.setUp()
        self.addCleanup(self.preview.doCleanups)
        self.scope = copy_scope()
        self.source = self.preview.root / 'copy-source'
        self.source.mkdir(mode=0o700)
        with (self.source / 'large.bin').open('wb') as stream:
            stream.truncate(32 * 1024 * 1024)
        pin = (self.source / 'large.bin').stat()
        self.original = {'large.bin': {'bytes': pin.st_size, 'mtime_ns': pin.st_mtime_ns, 'sha256': 'a' * 64}}
        self.gate = self.scope['prepare_copy_evidence'](self.preview.runtime, self.source, self.original)
        self.marker = self.preview.runtime / self.scope['COPY_EVIDENCE_READY']
        self.ready = {'schema_version': 1, 'gate': 'agentbaton_copy_evidence_gate_v1', 'stage': 'before_copy',
                      'nonce': self.gate['nonce'], 'descriptor_sha256': self.gate['descriptor_sha256'],
                      'creation_id': 'actual-copy', 'project_id': 'actual-project'}

    def write(self, value):
        raw = value if isinstance(value, bytes) else json.dumps(value).encode()
        fd = os.open(self.marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw)
        return raw

    def read(self):
        return self.scope['copy_evidence_ready'](self.preview.runtime, self.gate, 'actual-copy', 'actual-project')

    def test_copy_descriptor_is_separate_and_keeps_exact_original_bounds(self):
        preview_config = self.preview.runtime / self.scope['PREVIEW_EVIDENCE_CONFIG']
        before = preview_config.read_bytes()
        config = self.preview.runtime / self.scope['COPY_EVIDENCE_CONFIG']
        raw = config.read_bytes()
        value = json.loads(raw)
        self.assertEqual(set(value), {'schema_version', 'nonce', 'source', 'files', 'limits'})
        self.assertEqual(value['source'], {'canonical_root': str(self.source), 'device': str(self.source.stat().st_dev), 'inode': str(self.source.stat().st_ino)})
        self.assertEqual(value['files'], [{'artifact_id': 'file-1', 'relative_path': 'large.bin', 'max_bytes': 67_108_864, 'expected_sha256': 'a' * 64}])
        self.assertEqual(value['limits'], {'max_file_bytes': 67_108_864, 'max_total_bytes': 67_108_864, 'max_files': 4})
        self.assertEqual(stat.S_IMODE(config.stat().st_mode), 0o600)
        self.assertEqual(config.stat().st_nlink, 1)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), self.gate['descriptor_sha256'])
        self.assertEqual(preview_config.read_bytes(), before)
        with self.assertRaises(FileExistsError):
            self.scope['prepare_copy_evidence'](self.preview.runtime, self.source, self.original)
        self.assertEqual(config.read_bytes(), raw)

    def test_copy_ready_exact_creation_binding_is_not_a_preview_marker(self):
        raw = self.write(self.ready)
        result = self.read()
        self.assertEqual(result['stage'], 'before_copy')
        self.assertTrue(result['nonce_creation_project_matched'])
        self.assertEqual(result['marker_sha256'], hashlib.sha256(raw).hexdigest())
        self.assertNotIn('nonce', result)
        self.assertFalse((self.preview.runtime / self.scope['PREVIEW_EVIDENCE_READY']).exists())

    def test_foreign_copy_identity_digest_nonce_or_preview_shape_is_refused(self):
        controls = [self.preview.ready]  # Even an otherwise valid preview proof cannot release copy.
        for key, value in [('creation_id', 'foreign-copy'), ('project_id', 'foreign-project'), ('nonce', self.preview.gate['nonce']),
                           ('descriptor_sha256', self.preview.gate['descriptor_sha256']), ('stage', 'before_scan'),
                           ('gate', 'agentbaton_preview_evidence_gate_v1'), ('schema_version', True), ('extra', False)]:
            controls.append({**self.ready, key: value})
        for value in controls:
            self.write(value)
            with self.subTest(shape=set(value)), self.assertRaises(AssertionError):
                self.read()
            self.marker.unlink()

    def test_missing_and_partial_copy_marker_are_not_feature_proof(self):
        self.assertIsNone(self.read())
        raw = json.dumps(self.ready).encode()
        fd = os.open(self.marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            os.write(fd, raw[:len(raw) // 2])
            self.assertIsNone(self.read())
            os.write(fd, raw[len(raw) // 2:])
            os.fsync(fd)
            self.assertTrue(self.read()['nonce_creation_project_matched'])
        finally:
            os.close(fd)

    def test_copy_private_file_mode_links_nonregular_and_cap_are_enforced(self):
        target = self.preview.root / 'private-target'
        target.write_text(json.dumps(self.ready))
        target.chmod(0o600)
        self.marker.symlink_to(target)
        with self.assertRaises(OSError):
            self.read()
        self.marker.unlink()
        os.link(target, self.marker)
        with self.assertRaises(AssertionError):
            self.read()
        self.marker.unlink()
        self.write(self.ready)
        self.marker.chmod(0o644)
        with self.assertRaises(AssertionError):
            self.read()
        self.marker.unlink()
        os.mkfifo(self.marker, 0o600)
        with self.assertRaises(AssertionError):
            self.read()
        self.marker.unlink()
        self.write(b'x' * 8193)
        with self.assertRaises(AssertionError):
            self.read()

    def test_default_mode_does_not_prepare_or_read_copy_gate(self):
        startup = next(n for n in ast.walk(BASE.TREE) if isinstance(n, ast.If) and any(isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'prepare_copy_evidence' for c in ast.walk(n)))
        observing = next(n for n in ast.walk(BASE.TREE) if isinstance(n, ast.If) and any(isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'copy_evidence_ready' for c in ast.walk(n)))

        def forbidden(*args):
            self.fail('Default mode touched private copy gate')

        scope = dict(args=types.SimpleNamespace(preview_evidence=False), copy_evidence_gate=None,
                     prepare_preview_evidence=forbidden, prepare_copy_evidence=forbidden, copy_evidence_ready=forbidden)
        for node in [startup, observing]:
            exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-default-copy-off>', 'exec'), scope)

    def test_actual_copy_marker_cannot_be_accepted_after_absolute_deadline(self):
        self.write(self.ready)
        now = {'value': 0.0}
        clock = types.SimpleNamespace(monotonic=lambda: now['value'], monotonic_ns=lambda: int(now['value'] * 1e9), sleep=lambda _: None)
        self.scope['time'] = clock

        def read():
            result = self.read()
            now['value'] = 16.0
            return result

        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.scope['wait_preview_evidence_ready'](read, lambda value: value is not None, seconds=15, pump=lambda: None)

    def test_missing_copy_feature_marker_times_out_without_fallback(self):
        ticks = iter([0.0, 0.0, 0.0, 16.0])
        self.scope['time'] = types.SimpleNamespace(monotonic=lambda: next(ticks), sleep=lambda _: None)
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.scope['wait_preview_evidence_ready'](self.read, lambda value: value is not None, seconds=15, pump=lambda: None)


class CopyReceiptSQLiteTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='copy-receipt-pure-')
        self.addCleanup(self.directory.cleanup)
        self.runtime = Path(self.directory.name)
        self.db = self.runtime / 'baton.sqlite3'
        with sqlite3.connect(self.db) as store:
            # Actual table column layouts; inert read fixture, not schema15 or migration proof.
            store.executescript('''CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL);
CREATE TABLE data_workspace_creations(id TEXT PRIMARY KEY,preview_id TEXT NOT NULL UNIQUE REFERENCES data_workspace_previews(id),project_id TEXT NOT NULL REFERENCES projects(id),workspace_id TEXT NOT NULL UNIQUE REFERENCES managed_workspaces(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),state TEXT NOT NULL,document TEXT NOT NULL);
CREATE TABLE data_workspace_receipts(creation_id TEXT PRIMARY KEY REFERENCES data_workspace_creations(id),owner_sequence INTEGER NOT NULL,prepared_sequence INTEGER NOT NULL,finished_sequence INTEGER);''')
        self.scope = copy_scope()
        self.scope.update(sqlite3=sqlite3, runtime=self.runtime)
        node = next(n for n in ast.walk(BASE.TREE) if isinstance(n, ast.FunctionDef) and n.name == 'completion_receipt')
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-copy-completion-receipt>', 'exec'), self.scope)
        self.creation = {'id': 'copy-fixture', 'preview_id': 'preview-fixture', 'project_id': 'project-fixture', 'workspace_id': 'workspace-fixture',
                         'approval_id': 'real-owner-fixture-reference', 'revision': 3, 'state': 'cancelled', 'dataset_revision': 'd' * 64,
                         'contract': None, 'observation': None, 'cancel_requested': True, 'io_completion': 'worker_joined',
                         'error': {'code': 'CANCEL_REQUESTED', 'side_effect_state': 'confirmed'}}
        self.event = self.finished(self.creation)
        self.write(self.creation, self.event)

    def finished(self, creation):
        return {'sequence': 0, 'event_id': 'event-fixture', 'aggregate_id': creation['id'], 'revision': creation['revision'],
                'kind': 'data_workspace.copy_finished', 'occurred_at': '2026-10-29T00:00:00Z', 'source': 'data_workspace_core',
                'payload': {'state': creation['state'], 'workspace_id': creation['workspace_id'], 'cancel_requested': creation['cancel_requested'],
                            'automatic_retry': False, 'source_write': False, 'approval_id': creation['approval_id'],
                            'dataset_revision': creation['dataset_revision'], 'contract_digest': (creation.get('contract') or {}).get('contract_digest'),
                            'io_completion': creation['io_completion']}}

    def write(self, creation, event, sequence=101):
        with sqlite3.connect(self.db) as store:
            for table in ['data_workspace_receipts', 'data_workspace_creations', 'events']:
                store.execute('DELETE FROM ' + table)
            store.execute('INSERT INTO data_workspace_creations VALUES(?,?,?,?,?,?,?)', [creation['id'], creation['preview_id'], creation['project_id'], creation['workspace_id'], creation['approval_id'], creation['state'], json.dumps(creation)])
            if event is not None:
                store.execute('INSERT INTO events(sequence,document) VALUES(?,?)', [sequence, json.dumps(event)])
            store.execute('INSERT INTO data_workspace_receipts VALUES(?,?,?,?)', [creation['id'], 99, 100, sequence if event is not None else None])

    def observe(self, creation=None, destination_exists=False):
        creation = creation or self.creation
        receipt = self.scope['completion_receipt'](creation)
        return self.scope['copy_evidence_cancel_commitment'](creation, receipt, destination_exists)

    def test_original_real_shape_receipt_reads_event_document_then_before_copy_projection(self):
        before = hashlib.sha256(self.db.read_bytes()).hexdigest()
        result = self.observe()
        self.assertEqual(result['stage'], 'gated_before_copy')
        self.assertEqual(result['finished_sequence'], 101)
        self.assertTrue(result['original_copy_finished_matched'])
        self.assertFalse(result['mid_copy_cancellation_claimed'])
        self.assertFalse(result['partial_copy_claimed'])
        self.assertEqual(hashlib.sha256(self.db.read_bytes()).hexdigest(), before)
        with sqlite3.connect(self.db) as store:
            self.assertEqual([x[1] for x in store.execute('PRAGMA table_info(events)')], ['sequence', 'document'])

    def test_wrong_finished_source_identity_revision_payload_or_join_is_refused(self):
        for mode in ['source', 'kind', 'aggregate_id', 'revision', 'approval_id', 'workspace_id', 'dataset_revision', 'io_completion', 'automatic_retry', 'source_write']:
            event = copy.deepcopy(self.event)
            if mode in ['source', 'kind', 'aggregate_id']:
                event[mode] = 'foreign'
            elif mode == 'revision':
                event['revision'] = 4
            elif mode in ['automatic_retry', 'source_write']:
                event['payload'][mode] = True
            else:
                event['payload'][mode] = 'foreign'
            self.write(self.creation, event)
            with self.subTest(mode=mode), self.assertRaises(AssertionError):
                self.observe()

    def test_no_join_or_absent_finished_pointer_cannot_pass(self):
        for completion in [None, 'interrupted_no_join_receipt', 'not_scheduled']:
            creation = {**self.creation, 'io_completion': completion}
            self.write(creation, self.finished(creation))
            with self.subTest(completion=completion), self.assertRaises(AssertionError):
                self.observe(creation)
        self.write(self.creation, None)
        with self.assertRaises(AssertionError):
            self.observe()

    def test_ready_race_remains_ready_even_with_join_and_is_not_gate_success(self):
        creation = {**self.creation, 'state': 'ready', 'cancel_requested': False, 'error': None, 'contract': {'contract_digest': 'c' * 64}}
        self.write(creation, self.finished(creation))
        receipt = self.scope['completion_receipt'](creation)
        self.assertTrue(receipt['owned_copy_worker_join_observed'])
        self.assertFalse(receipt['cancelled_owned_worker_join_observed'])
        with self.assertRaisesRegex(AssertionError, 'not confirmed'):
            self.scope['copy_evidence_cancel_commitment'](creation, receipt, True)

    def test_unknown_without_finished_receipt_is_retained_not_promoted(self):
        creation = {**self.creation, 'state': 'outcome_unknown', 'io_completion': 'interrupted_no_join_receipt'}
        self.write(creation, None)
        receipt = self.scope['completion_receipt'](creation)
        self.assertIsNone(receipt['finished_event'])
        with self.assertRaises(AssertionError):
            self.scope['copy_evidence_cancel_commitment'](creation, receipt, False)

    def test_partial_copy_join_is_not_presented_as_before_copy(self):
        creation = {**self.creation, 'observation': {'state': 'failed', 'error': 'cancel_requested', 'copied_files': 1}}
        self.write(creation, self.finished(creation))
        with self.assertRaisesRegex(AssertionError, 'unexpectedly wrote'):
            self.observe(creation, destination_exists=True)

    def test_before_copy_absence_must_be_explicit_not_unknown_or_numeric(self):
        for exists in [None, 0, '', {}]:
            with self.subTest(exists=exists), self.assertRaises(AssertionError):
                self.observe(destination_exists=exists)

    def test_tampered_actual_creation_document_is_not_reconciled_as_same_copy(self):
        different = {**self.creation, 'approval_id': 'foreign-owner'}
        self.write(different, self.finished(different))
        with self.assertRaises(AssertionError):
            self.observe(self.creation)


if __name__ == '__main__':
    unittest.main()
