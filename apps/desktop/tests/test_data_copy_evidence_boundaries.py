#!/usr/bin/env python3
"""Copy evidence boundaries using inert SQLite/private filesystem only; no product run."""
import ast
import copy
import errno
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('copy_evidence_inert_fixture', Path(__file__).with_name('test_data_copy_evidence.py'))
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


class CopyEvidenceBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = BASE.CopyReceiptSQLiteTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.scope = self.fixture.scope
        self.scope.update(Path=Path, errno=errno)
        nodes = [n for n in BASE.BASE.TREE.body if isinstance(n, ast.FunctionDef) and n.name in {'copy_evidence_destination_absent', 'copy_evidence_persisted_revisions'}]
        exec(compile(ast.Module(body=nodes, type_ignores=[]), '<actual-copy-evidence-boundaries>', 'exec'), self.scope)

    def receipt(self, terminal=None):
        return self.scope['completion_receipt'](terminal or self.fixture.creation)

    def observe(self, terminal=None):
        terminal = terminal or self.fixture.creation
        receipt = self.receipt(terminal)
        observed = self.scope['copy_evidence_cancel_commitment'](terminal, receipt, False)
        self.scope['copy_evidence_persisted_revisions'](self.fixture.runtime, terminal, receipt)
        return observed

    def write(self, creation):
        self.fixture.write(creation, self.fixture.finished(creation))

    def test_invalid_consistent_terminal_event_and_row_revisions_are_rejected(self):
        for value in [0, -1, 18_446_744_073_709_551_616, True, 3.0, '3', None]:
            terminal = {**self.fixture.creation, 'revision': value}
            self.write(terminal)
            with self.subTest(value=value), self.assertRaises(AssertionError):
                self.observe(terminal)

    def test_positive_full_u64_domain_is_preserved_exactly(self):
        for value in [1, 3, 18_446_744_073_709_551_615]:
            terminal = {**self.fixture.creation, 'revision': value}
            self.write(terminal)
            with self.subTest(value=value):
                self.assertEqual(self.observe(terminal)['terminal_revision'], value)

    def test_stored_float_or_boolean_is_not_a_typed_equal_integer(self):
        for stored_value, integer in [(3.0, 3), (True, 1)]:
            terminal = {**self.fixture.creation, 'revision': integer}
            stored = {**terminal, 'revision': stored_value}
            self.fixture.write(stored, self.fixture.finished(terminal))
            receipt = self.receipt(terminal)  # Old equality alone permits these.
            self.scope['copy_evidence_cancel_commitment'](terminal, receipt, False)
            with self.subTest(stored=stored_value), self.assertRaisesRegex(AssertionError, 'persisted copy evidence revision'):
                self.scope['copy_evidence_persisted_revisions'](self.fixture.runtime, terminal, receipt)

    def test_duplicate_raw_creation_or_event_revision_is_refused(self):
        for table, key in [('data_workspace_creations', self.fixture.creation['id']), ('events', 101)]:
            self.write(self.fixture.creation)
            document = self.fixture.creation if table == 'data_workspace_creations' else self.fixture.event
            duplicate = json.dumps(document)[:-1] + ', "revision": 3}'
            column = 'id' if table == 'data_workspace_creations' else 'sequence'
            with BASE.sqlite3.connect(self.fixture.db) as store:
                store.execute('UPDATE ' + table + ' SET document=? WHERE ' + column + '=?', [duplicate, key])
            receipt = self.receipt()
            with self.subTest(table=table), self.assertRaises(ValueError):
                self.scope['copy_evidence_persisted_revisions'](self.fixture.runtime, self.fixture.creation, receipt)

    def actual_gate_branch(self, destination):
        terminal = {**self.fixture.creation, 'destination': str(destination)}
        self.write(terminal)
        receipt = self.receipt(terminal)
        node = next(n for n in ast.walk(BASE.BASE.TREE) if isinstance(n, ast.If) and any(isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'copy_evidence_cancel_commitment' for c in ast.walk(n)))
        self.scope.update(copy_evidence_gate={}, requested=True, terminal=terminal, completion=receipt,
                          retained_destination=destination.exists(), runtime=self.fixture.runtime, report={'observations': {}})
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-main-copy-evidence-branch>', 'exec'), self.scope)
        return self.scope['report']['observations']

    def test_actual_main_refuses_dangling_entry_even_when_exists_is_false(self):
        destination = self.fixture.runtime / 'dangling-destination'
        destination.symlink_to(self.fixture.runtime / 'missing-target')
        self.assertFalse(destination.exists())
        self.assertTrue(destination.is_symlink())
        with self.assertRaisesRegex(AssertionError, 'unexpectedly wrote'):
            self.actual_gate_branch(destination)
        self.assertNotIn('copy_evidence_cancel_commitment', self.scope['report']['observations'])

    def test_actual_main_accepts_real_enoent_with_correlated_readonly_rows(self):
        destination = self.fixture.runtime / 'absent-destination'
        result = self.actual_gate_branch(destination)
        self.assertTrue(result['copy_evidence_cancel_commitment']['original_copy_finished_matched'])
        self.assertFalse(destination.exists())

    def test_any_existing_file_directory_or_link_is_not_absence(self):
        file = self.fixture.runtime / 'entry-file'
        file.write_text('owned fixture')
        folder = self.fixture.runtime / 'entry-dir'
        folder.mkdir()
        link = self.fixture.runtime / 'entry-link'
        link.symlink_to(file)
        for destination in [file, folder, link]:
            with self.subTest(kind=destination.name):
                self.assertFalse(self.scope['copy_evidence_destination_absent'](destination))

    def test_enotdir_is_unknown_not_absence(self):
        file = self.fixture.runtime / 'not-a-directory'
        file.write_text('owned fixture')
        with self.assertRaises(NotADirectoryError):
            self.scope['copy_evidence_destination_absent'](file / 'child')

    def test_permission_and_nonenoent_missing_errors_do_not_prove_absence(self):
        for failure in [PermissionError(errno.EACCES, 'fixture denial'), FileNotFoundError(), FileNotFoundError(errno.EACCES, 'not ENOENT')]:
            with self.subTest(error=type(failure).__name__), patch.object(Path, 'lstat', side_effect=failure), self.assertRaises(OSError):
                self.scope['copy_evidence_destination_absent'](self.fixture.runtime / 'unknown')

    def test_missing_or_changed_persisted_row_never_becomes_default_revision(self):
        receipt = self.receipt()
        with BASE.sqlite3.connect(self.fixture.db) as store:
            store.execute('DELETE FROM data_workspace_creations')
        with self.assertRaisesRegex(AssertionError, 'Missing persisted'):
            self.scope['copy_evidence_persisted_revisions'](self.fixture.runtime, self.fixture.creation, receipt)
        self.write(self.fixture.creation)
        changed = {**self.fixture.creation, 'revision': 4}
        with BASE.sqlite3.connect(self.fixture.db) as store:
            store.execute('UPDATE data_workspace_creations SET document=?', [json.dumps(changed)])
        with self.assertRaisesRegex(AssertionError, 'revision mismatch'):
            self.scope['copy_evidence_persisted_revisions'](self.fixture.runtime, self.fixture.creation, receipt)


if __name__ == '__main__':
    unittest.main()
