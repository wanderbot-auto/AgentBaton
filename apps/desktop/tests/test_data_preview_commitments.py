#!/usr/bin/env python3
"""Actual readonly journal reader against an inert two-column SQLite fixture.

This proves JSON-event schema/query behavior, not a Core migration, real worker
join, cancellation or browser result. No daemon or product process is started.
"""
import ast
import copy
import hashlib
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

SOURCE = Path(__file__).with_name('data-workspace-real-ipc.py').read_text()
TREE = ast.parse(SOURCE)


class PreviewCommitmentTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='preview-journal-pure-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.db = self.root / 'baton.sqlite3'
        with sqlite3.connect(self.db) as store:
            # Exact events table column layout from Core storage migration1;
            # deliberately not a counterfeit schema15 database or migration.
            store.execute('CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL)')
        self.terminal = {'id': 'preview-fixture', 'state': 'cancelled', 'revision': 3, 'snapshot': None, 'error': {'code': 'CANCEL_REQUESTED'}}
        self.events = [
            {'sequence': 0, 'aggregate_id': 'preview-fixture', 'revision': 2, 'kind': 'data_workspace.preview_cancel_requested', 'payload': {'confirmed_stop': False}},
            {'sequence': 0, 'aggregate_id': 'preview-fixture', 'revision': 3, 'kind': 'data_workspace.preview_finished', 'payload': {'state': 'cancelled', 'copy_authorized': False}},
        ]
        scope = dict(sqlite3=sqlite3, json=json, runtime=self.root)
        unique = next(n for n in TREE.body if isinstance(n, ast.FunctionDef) and n.name == 'unique_object')
        reader = next(n for n in ast.walk(TREE) if isinstance(n, ast.FunctionDef) and n.name == 'preview_cancel_commitment')
        exec(compile(ast.Module(body=[unique, reader], type_ignores=[]), '<actual-readonly-journal-reader>', 'exec'), scope)
        self.read = scope['preview_cancel_commitment']
        self.write(self.events)

    def write(self, events):
        with sqlite3.connect(self.db) as store:
            store.execute('DELETE FROM events')
            store.execute("DELETE FROM sqlite_sequence WHERE name='events'")
            store.executemany('INSERT INTO events(document) VALUES(?)', [(e if isinstance(e, str) else json.dumps(e),) for e in events])

    def test_actual_two_column_schema_reads_json_fields_and_sql_sequences(self):
        result = self.read(self.terminal)
        self.assertEqual((result['cancel_requested_sequence'], result['finished_sequence'], result['terminal_revision']), (1, 2, 3))
        self.assertEqual(result['stage'], 'gated_before_scan')
        self.assertFalse(result['mid_read_cancellation_claimed'])
        with sqlite3.connect(self.db) as store:
            self.assertEqual([row[1] for row in store.execute('PRAGMA table_info(events)')], ['sequence', 'document'])

    def test_original_relational_column_query_reproduces_failure(self):
        with sqlite3.connect(self.db) as store, self.assertRaisesRegex(sqlite3.OperationalError, 'no such column: revision'):
            store.execute("SELECT sequence,revision,kind,payload FROM events WHERE aggregate_id=?", ['preview-fixture']).fetchall()

    def test_read_is_readonly_and_does_not_modify_journal(self):
        before = hashlib.sha256(self.db.read_bytes()).hexdigest()
        self.read(self.terminal)
        self.assertEqual(hashlib.sha256(self.db.read_bytes()).hexdigest(), before)

    def test_full_u64_revision_domain_is_not_rounded_through_sqlite_json_numbers(self):
        self.events[0]['revision'] = 18_446_744_073_709_551_614
        self.events[1]['revision'] = self.terminal['revision'] = 18_446_744_073_709_551_615
        self.write(self.events)
        self.assertEqual(self.read(self.terminal)['terminal_revision'], 18_446_744_073_709_551_615)

    def test_missing_or_duplicate_commitments_cannot_establish_joined_terminal(self):
        for events in [[], self.events[:1], self.events + self.events[1:]]:
            with self.subTest(count=len(events)):
                self.write(events)
                with self.assertRaises(AssertionError):
                    self.read(self.terminal)

    def test_reversed_or_foreign_event_lineage_is_rejected(self):
        for mode in ['reversed', 'foreign-aggregate', 'foreign-kind']:
            events = copy.deepcopy(self.events)
            if mode == 'reversed':
                events.reverse()
            elif mode == 'foreign-aggregate':
                events[1]['aggregate_id'] = 'foreign-preview'
            else:
                events[1]['kind'] = 'session.stopped'
            with self.subTest(mode=mode):
                self.write(events)
                with self.assertRaises(AssertionError):
                    self.read(self.terminal)

    def test_malformed_boolean_null_string_or_overflow_event_revision_is_rejected(self):
        for value in [True, False, None, '3', 0, -1, 18_446_744_073_709_551_616]:
            events = copy.deepcopy(self.events)
            events[1]['revision'] = value
            self.write(events)
            with self.subTest(value=value), self.assertRaises(AssertionError):
                self.read(self.terminal)

    def test_equal_backward_or_noncurrent_revision_cannot_establish_commitment(self):
        for revision in [1, 2, 4]:
            events = copy.deepcopy(self.events)
            events[1]['revision'] = revision
            self.write(events)
            with self.subTest(revision=revision), self.assertRaises(AssertionError):
                self.read(self.terminal)

    def test_stop_and_no_copy_flags_must_be_actual_false_booleans(self):
        for index, key, values in [(0, 'confirmed_stop', [True, 0, None]), (1, 'copy_authorized', [True, 0, None])]:
            for value in values:
                events = copy.deepcopy(self.events)
                events[index]['payload'][key] = value
                self.write(events)
                with self.subTest(index=index, value=value), self.assertRaises(AssertionError):
                    self.read(self.terminal)

    def test_current_terminal_must_be_cancelled_no_snapshot_and_cancel_specific(self):
        for patch in [{'state': 'verified'}, {'state': 'outcome_unknown'}, {'snapshot': {}}, {'error': {'code': 'DEADLINE_EXCEEDED'}}, {'error': None}, {'revision': 4}]:
            with self.subTest(patch=patch), self.assertRaises(AssertionError):
                self.read({**self.terminal, **patch})

    def test_malformed_or_duplicate_json_document_never_becomes_missing_zero(self):
        duplicate = json.dumps(self.events[1])[:-1] + ', "revision": 3}'
        for document in ['{invalid-json}', duplicate]:
            self.write([self.events[0], document])
            with self.subTest(document_kind='malformed' if document.startswith('{invalid') else 'duplicate'):
                with self.assertRaises((sqlite3.OperationalError, ValueError)):
                    self.read(self.terminal)


if __name__ == '__main__':
    unittest.main()
