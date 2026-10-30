"""Actual SQLite count-query controls on inert metadata, not Core migration proof."""
import importlib.util
from pathlib import Path
import sqlite3
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'apps/desktop/tests/goal-graph-real-ipc.py'
spec = importlib.util.spec_from_file_location('_schema17_metadata_guard', SOURCE)
G = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = G
spec.loader.exec_module(G)


class IntegrationMetadataControls(unittest.TestCase):
    def database(self, schema=17):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.execute(f'PRAGMA user_version={schema}')
        # Only count(*) is exercised; no production columns/rows are fabricated.
        for table in (*G.FACTORY_RECORD_TABLES.values(), *G.INTEGRATION_RECORD_TABLES.values()):
            db.execute('CREATE TABLE "' + table + '"(id INTEGER)')
        return db

    def test_current_real_sqlite_queries_require_separate_four_zero_tables(self):
        db = self.database()
        self.assertEqual(G.current_integration_counts(db), {'policies': 0, 'runs': 0, 'attempts': 0, 'ref_fences': 0})
        self.assertEqual(len(G.current_factory_counts(db)), 12)
        db.execute('INSERT INTO factory_child_goal_runs VALUES(1)')
        self.assertEqual(G.current_factory_counts(db)['child_goal_runs'], 1)

    def test_each_missing_or_nonzero_domain_refuses_without_factory_shape_changes(self):
        for table in G.INTEGRATION_RECORD_TABLES.values():
            for bad in ['missing', 'nonzero']:
                db = self.database()
                if bad == 'missing': db.execute('DROP TABLE ' + table)
                else: db.execute('INSERT INTO ' + table + ' VALUES(1)')
                with self.subTest(table=table, bad=bad), self.assertRaises((sqlite3.OperationalError, AssertionError)):
                    G.current_factory_counts(db)

    def test_old_and_future_real_sqlite_versions_never_become_current(self):
        for schema in [15, 16, 18]:
            with self.subTest(schema=schema), self.assertRaises(AssertionError):
                G.current_factory_counts(self.database(schema))


if __name__ == '__main__':
    unittest.main()
