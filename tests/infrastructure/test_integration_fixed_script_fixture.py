"""Keep the old whole-source observers strict across the fixed-script fixture."""
import copy
import hashlib
from pathlib import Path
import unittest
from unittest import mock
import integration_fixed_script_fixture as F
import test_integration_ipc_diagnostics as D

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / D.RUNTIME

class FixedScriptFixtureInverseTests(unittest.TestCase):
    def test_exact_whole_source_inverse_preserves_all_original_regions(self):
        text = FIXTURE.read_text()
        self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), F.FIXED_SCRIPT_INPUT_SHA256)
        restored = F.restore_before_fixed_scripts(text)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), F.FIXED_SCRIPT_RESTORED_SHA256)
        self.assertEqual(F.restore_before_fixed_scripts(restored), restored)
        self.assertNotIn('FIXED_VALIDATOR_SCRIPTS', restored)
        self.assertIn('root.join("fixed-sh")', restored)

    def test_unknown_regions_and_original_budgets_still_fail_whole_source_guard(self):
        text = FIXTURE.read_text()
        for changed in [text+'// unexpected source\n', text.replace('reservation(4000, 2048)', 'reservation(4001, 2048)', 1), text.replace('Duration::from_secs(5)', 'Duration::from_secs(6)', 1)]:
            self.assertNotEqual(text, changed)
            with self.subTest(source=hashlib.sha256(changed.encode()).hexdigest()), self.assertRaises(AssertionError):
                D.strip_post_join_layer(D.RUNTIME, changed)

    def test_missing_or_changed_approved_script_anchor_is_not_normalized(self):
        text = FIXTURE.read_text()
        for after,before in F._FIXED_SCRIPT_EDITS:
            self.assertTrue(after)
            with self.subTest(anchor=hashlib.sha256(after.encode()).hexdigest()), self.assertRaises(AssertionError):
                F.restore_before_fixed_scripts(text.replace(after, after[:len(after)//2]+'/*mutated*/'+after[len(after)//2:], 1))

    def test_tampered_inverse_cannot_restore_original_digest(self):
        text=FIXTURE.read_text()
        bad=copy.deepcopy(F._FIXED_SCRIPT_EDITS)
        bad[0][1]+='// unexpected restored fragment\n'
        with mock.patch.object(F, '_FIXED_SCRIPT_EDITS', bad):
            self.assertNotEqual(hashlib.sha256(F.restore_before_fixed_scripts(text).encode()).hexdigest(),F.FIXED_SCRIPT_RESTORED_SHA256)
            with self.assertRaises(AssertionError):D.strip_post_join_layer(D.RUNTIME,text)

if __name__ == '__main__':
    unittest.main()
