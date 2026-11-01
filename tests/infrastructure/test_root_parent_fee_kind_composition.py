"""Pure source-only negative controls for one reviewed Root17/Parent merge."""
import ast
import hashlib
from pathlib import Path
import unittest
from unittest import mock
import rework_fee_kind_composition_fixture as F

ROOT = Path(__file__).resolve().parents[2]
PROJECTOR = ROOT / 'scripts/record-factory-rework-ci-evidence.py'
ORIGINAL_ROOT_ATTEMPT_GUARD_SHA256 = '672b583b689741a1f5b363a09ff0fd7982e07897058910d326215661c5f0b065'
PARENT_FEE_TEST_SHA256 = '9f5df0be17e7fa48a22453aa1d33c0d308bb2ac6b942918b2c51b66c5e23f4a1'


class RootParentFeeSourceComposition(unittest.TestCase):
    def test_exact_full_root_source_restores_and_parent_helper_stays_exact(self):
        text = PROJECTOR.read_text()
        self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), F.AFTER_SHA256)
        original = F.strip_root_fee_kind_layer(text)
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(), F.BEFORE_SHA256)
        self.assertEqual(hashlib.sha256(F.FEE_HELPER.encode()).hexdigest(), F.FEE_HELPER_SHA256)
        self.assertEqual(F.strip_root_fee_kind_layer(original), original)
        self.assertIn('SCHEMA = 17', original)
        self.assertIn('def attempt_budget_observations(', original)
        self.assertIn('def settled_session_stop(', original)
        self.assertIn('conn, subject, prior_revision, prior[0][0], event)', original)

    def test_each_existing_function_body_is_byte_exact_except_five_budget_insertions(self):
        text = PROJECTOR.read_text(); original = F.strip_root_fee_kind_layer(text)
        def functions(source):
            return {node.name: ast.get_source_segment(source, node) for node in ast.parse(source).body
                    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))}
        old, new = functions(original), functions(text)
        for name, source in old.items():
            changed = new[name]
            if name == 'budget_usage':
                for insertion in F.FEE_INSERTIONS:
                    self.assertEqual(changed.count(insertion), 1)
                    changed = changed.replace(insertion, '', 1)
            self.assertEqual(changed, source, name)
        self.assertEqual(len(new) - len(old), 6)

    def test_unknown_outer_region_schema_budget_fees_and_output_cap_tamper_refuse(self):
        text = PROJECTOR.read_text()
        mutations = [text+'# foreign\n', text.replace('SCHEMA = 17','SCHEMA = 16',1),
                     text.replace('attempt not in seen','True',1),
                     text.replace("totals['settled_count'] += 1", "totals['settled_count'] += 0",1),
                     text.replace('MAX_PUBLIC = ', 'MAX_PUBLIC = 2 * ',1)]
        for changed in mutations:
            self.assertNotEqual(changed, text)
            with self.assertRaises(AssertionError): F.strip_root_fee_kind_layer(changed)

    def test_missing_duplicate_moved_changed_parent_helper_refuse(self):
        text = PROJECTOR.read_text(); block = F.FEE_HELPER
        for changed in [text.replace(block,'',1), text+block,
                        text.replace(block,'',1)+block,
                        text.replace(block,block.replace('MAX_FEE_GROUPS = 8','MAX_FEE_GROUPS = 80',1),1)]:
            with self.assertRaises(AssertionError): F.strip_root_fee_kind_layer(changed)

    def test_every_single_missing_duplicate_moved_or_modified_insertion_refuses(self):
        text = PROJECTOR.read_text()
        for fragment in F.FEE_INSERTIONS:
            for changed in [text.replace(fragment,'',1),text+fragment,
                            text.replace(fragment,'',1)+fragment,text.replace(fragment,fragment+'    # foreign\n',1)]:
                with self.subTest(fragment=fragment,changed_sha256=hashlib.sha256(changed.encode()).hexdigest()):
                    with self.assertRaises(AssertionError): F.strip_root_fee_kind_layer(changed)

    def test_unknown_types_foreign_source_and_changed_descriptor_refuse(self):
        text = PROJECTOR.read_text()
        for changed in [None, b'bytes', 17, False, '', text.replace('\n','\r\n')]:
            with self.assertRaises(AssertionError): F.strip_root_fee_kind_layer(changed)
        for field,value in [('FEE_HELPER', F.FEE_HELPER+'# extra\n'),('FEE_HELPER_SHA256','0'*64),
                            ('FEE_INSERTIONS',F.FEE_INSERTIONS[:-1])]:
            with mock.patch.object(F,field,value),self.assertRaises(AssertionError): F.strip_root_fee_kind_layer(text)

    def test_original_attempt_guard_full_file_and_parent_fee_tests_restore_exact(self):
        path = ROOT / 'tests/infrastructure/test_factory_rework_attempt_budget.py'
        text = path.read_text()
        for fragment in ['import rework_fee_kind_composition_fixture as fee_composition\n',
                         '    text = fee_composition.strip_root_fee_kind_layer(text)\n',
                         '        current=fee_composition.strip_root_fee_kind_layer(current)\n']:
            self.assertEqual(text.count(fragment),1);text=text.replace(fragment,'',1)
        self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), ORIGINAL_ROOT_ATTEMPT_GUARD_SHA256)
        path = ROOT / 'tests/infrastructure/test_factory_rework_fee_kinds.py'
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), PARENT_FEE_TEST_SHA256)


if __name__ == '__main__':
    unittest.main()
