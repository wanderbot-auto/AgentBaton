"""Strict old Goal Graph source and historical schema consumers."""
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import types
import unittest
from unittest.mock import Mock, patch

from tests.infrastructure import host19_goal_graph_source_fixture as F


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / F.DRIVER_PATH


class OriginalGraphSource(unittest.TestCase):
    def test_exact_bytes_restore_and_legacy_controls_replay(self):
        current = SOURCE.read_bytes()
        self.assertEqual(hashlib.sha256(current).hexdigest(), F.CURRENT_SHA256)
        schema19 = F.restore_goal_graph_before_ui_simplification(F.DRIVER_PATH, current)
        self.assertEqual(hashlib.sha256(schema19).hexdigest(),
            '550256ce728cbc26c5873237e09888195cfe01726b697d46d5e2b881e46b4d18')
        original = F.restore_goal_graph_before_schema19(F.DRIVER_PATH, current)
        self.assertEqual(F.restore_goal_graph_before_schema19(F.DRIVER_PATH, schema19), original)
        self.assertEqual(hashlib.sha256(original).hexdigest(),
            '5c07f3fadcad286dffba98021f5d83c16eb999261d5887c506e830f5b52ca6cb')
        historical = types.ModuleType('_historical_goal_graph18')
        historical.__file__ = str(SOURCE)
        with patch.dict(sys.modules, {historical.__name__: historical}):
            exec(compile(original, str(SOURCE), 'exec'), historical.__dict__)
        static = historical.driver_self_check()
        self.assertEqual((static['status'], static['count']), ('passed', 33))
        self.assertTrue(any(row['name'].endswith('access 19') for row in static['checks']))

        binary = Mock()
        binary.read_bytes.return_value = b'frozen inert schema 18 fixture bytes'
        manifest = {'schema': 18, 'source_head': 'a' * 40,
            'binaries': {'batond': historical.sha(binary.read_bytes.return_value)}}
        self.assertEqual(historical.verify_candidate(binary, json.dumps(manifest).encode()), manifest)
        with self.assertRaises(AssertionError):
            historical.verify_candidate(binary, json.dumps({**manifest, 'schema': 19}).encode())
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.execute('PRAGMA user_version=18')
        for table in (*historical.FACTORY_RECORD_TABLES.values(), *historical.INTEGRATION_RECORD_TABLES.values()):
            db.execute(f'CREATE TABLE "{table}" (id INTEGER)')
        self.assertEqual(historical.current_factory_counts(db), dict.fromkeys(historical.FACTORY_RECORD_TABLES, 0))
        self.assertEqual(historical.current_integration_counts(db), dict.fromkeys(historical.INTEGRATION_RECORD_TABLES, 0))
        db.execute('PRAGMA user_version=19')
        with self.assertRaises(AssertionError):
            historical.current_factory_counts(db)

    def test_original_partial_foreign_and_tampered_input_refused(self):
        raw = SOURCE.read_bytes()
        original = F.restore_goal_graph_before_schema19(F.DRIVER_PATH, raw)
        for path, value in (
            ('foreign.py', raw), (F.DRIVER_PATH, raw.decode()), (F.DRIVER_PATH, original),
            (F.DRIVER_PATH, raw + b'# outside reviewed edits\n'),
            (F.DRIVER_PATH, raw.replace(b'GRAPH_SCHEMA = 19', b'GRAPH_SCHEMA = 20', 1)),
        ):
            with self.subTest(path=path, kind=type(value)), self.assertRaises(ValueError):
                F.restore_goal_graph_before_schema19(path, value)

    def test_tampered_span_and_target_digest_refuse(self):
        raw = SOURCE.read_bytes()
        start, current, original = F.EDITS[0]
        for edits in (
            ((start + 1, current, original), *F.EDITS[1:]),
            ((start, current, original + b'# outside original\n'), *F.EDITS[1:]),
        ):
            with patch.object(F, 'EDITS', edits), self.assertRaises(ValueError):
                F.restore_goal_graph_before_schema19(F.DRIVER_PATH, raw)

    def test_ui_layer_first_requires_exact_current_source_and_pinned_target(self):
        raw = SOURCE.read_bytes()
        schema19 = F.restore_goal_graph_before_ui_simplification(F.DRIVER_PATH, raw)
        for value in (schema19, raw + b'# outside reviewed selectors\n',
                      raw.replace(b'goal-graph-evidence', b'goal-graph-evidencE', 1)):
            self.assertNotEqual(value, raw)
            with self.subTest(value_digest=hashlib.sha256(value).hexdigest()), self.assertRaises(ValueError):
                F.restore_goal_graph_before_ui_simplification(F.DRIVER_PATH, value)

        start, current, original = F.UI_SIMPLIFICATION_EDITS[0]
        for edits in (
            ((start + 1, current, original), *F.UI_SIMPLIFICATION_EDITS[1:]),
            ((start, current, original + b'# outside canonical schema19\n'), *F.UI_SIMPLIFICATION_EDITS[1:]),
        ):
            with patch.object(F, 'UI_SIMPLIFICATION_EDITS', edits), self.assertRaises(ValueError):
                F.restore_goal_graph_before_schema19(F.DRIVER_PATH, raw)
