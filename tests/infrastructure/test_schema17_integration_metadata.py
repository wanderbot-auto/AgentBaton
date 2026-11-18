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
    def database(self, schema=18):
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
            for bad in ['missing', 'nonzero', 'view']:
                db = self.database()
                if bad in ['missing', 'view']:
                    db.execute('DROP TABLE ' + table)
                    if bad == 'view': db.execute('CREATE VIEW '+table+' AS SELECT 0 AS id WHERE 0')
                else: db.execute('INSERT INTO ' + table + ' VALUES(1)')
                with self.subTest(table=table, bad=bad), self.assertRaises((sqlite3.OperationalError, AssertionError)):
                    G.current_factory_counts(db)

    def test_old_and_future_real_sqlite_versions_never_become_current(self):
        for schema in [15, 16, 17, 19]:
            with self.subTest(schema=schema), self.assertRaises(AssertionError):
                G.current_factory_counts(self.database(schema))


    def test_graph_schema19_retains_exact_factory_and_integration_counts(self):
        db = self.database(19)
        counts = G.graph_factory_counts(db)
        self.assertEqual(counts, dict.fromkeys(G.FACTORY_RECORD_TABLES, 0))
        db.execute('INSERT INTO factory_child_goal_runs VALUES(1)')
        self.assertEqual(G.graph_factory_counts(db)['child_goal_runs'], 1)
        for verify in (G.current_factory_counts, G.current_integration_counts):
            with self.assertRaises(AssertionError):
                verify(db)
        for schema in (17, 18, 20):
            with self.subTest(schema=schema), self.assertRaises(G.BoundaryRefusal):
                G.graph_factory_counts(self.database(schema))

    def test_graph_schema19_keeps_existing_row_and_table_boundary(self):
        for table in G.INTEGRATION_RECORD_TABLES.values():
            for condition in ('nonzero', 'missing', 'view'):
                db = self.database(19)
                if condition == 'nonzero':
                    db.execute(f'INSERT INTO "{table}" VALUES(1)')
                else:
                    db.execute(f'DROP TABLE "{table}"')
                    if condition == 'view':
                        db.execute(f'CREATE VIEW "{table}" AS SELECT 0 AS id WHERE 0')
                with self.subTest(table=table, condition=condition), self.assertRaises(AssertionError):
                    G.graph_factory_counts(db)
        db = self.database(19)
        db.execute('DROP TABLE factory_child_goal_runs')
        with self.assertRaises(sqlite3.OperationalError):
            G.graph_factory_counts(db)


if __name__ == '__main__':
    unittest.main()
