"""Pure source composition controls. No Rust/SQLite/product execution."""
import hashlib
import unittest
import test_integration_stage_evidence_source as original


class IntegrationPreparationPhaseInverseTests(unittest.TestCase):
    def test_exact_whole_sources_restore_prior_pins_and_original_rust_count(self):
        settlement = (original.ROOT / 'crates/core/src/integration_settlement.rs').read_text()
        tests = original.RUST_TEST.read_text()
        restored_settlement = original.settlement_before_phase(settlement)
        restored_tests = original.rust_controls_before_phase(tests)
        self.assertEqual(hashlib.sha256(restored_settlement.encode()).hexdigest(), original.BEFORE_PHASE_SETTLEMENT_SHA256)
        self.assertEqual(hashlib.sha256(restored_tests.encode()).hexdigest(), original.BEFORE_PHASE_RUST_TEST_SHA256)
        self.assertEqual(restored_tests.count('#[test]'), 10)
        self.assertEqual(tests.count('#[test]'), 13)
        self.assertEqual(tests[:tests.index(original.PHASE_TEST_MARKER)], restored_tests)
        for kind, raw in [('integration.stage_unknown', 'UnknownEvent'),
                          ('integration.preparation_io_observed', 'PreparationEvent')]:
            tail = restored_settlement.split('"' + kind + '"', 1)[1].split(')?;', 1)[0]
            self.assertEqual(original.fields(original.MODULE.read_text(), raw),
                             set(original.re.findall(r'"([a-z_]+)"\s*:', tail)))

    def test_changed_original_fields_clocks_fees_or_outside_region_are_rejected(self):
        text = (original.ROOT / 'crates/core/src/integration_settlement.rs').read_text()
        mutations = [text + '// unknown-outside-region\n',
                     text.replace('"whole_filesystem_output":null', '"whole_filesystem_output":0', 1),
                     text.replace('ticket.queued_at.elapsed()', 'ticket.deadline.elapsed()', 1),
                     text.replace('reservation_refunded":false', 'reservation_refunded":true', 1)]
        for changed in mutations:
            self.assertNotEqual(changed, text)
            with self.assertRaises(AssertionError):
                original.settlement_before_phase(changed)

    def test_changed_missing_duplicate_or_moved_alias_is_rejected(self):
        text = (original.ROOT / 'crates/core/src/integration_settlement.rs').read_text()
        block = original.PHASE_PAYLOAD_ALIAS
        self.assertEqual(text.count(block), 1)
        mutations = [text.replace(block, '', 1), text + block,
                     text.replace(block, block.replace('let mut payload = payload;', 'let mut payload = json!({});'), 1),
                     text.replace(block, '', 1) + block,
                     text.replace('                &payload,\n', '                &payload,\n                &payload,\n', 1)]
        for changed in mutations:
            with self.assertRaises(AssertionError):
                original.settlement_before_phase(changed)

    def test_changed_original_or_new_rust_assertions_are_rejected(self):
        text = original.RUST_TEST.read_text()
        marker = text.index(original.PHASE_TEST_MARKER)
        old, new = text[:marker], text[marker:]
        mutations = [old.replace('PRIVATE_CANARY_DO_NOT_PUBLISH', 'not-a-canary', 1) + new,
                     old + new.replace('assert_eq!(observed, base);', 'assert!(true);', 1),
                     old + new.replace('MAX_OUTPUT', 'MAX_INPUT', 1),
                     text + '// unrelated-new-region\n']
        for changed in mutations:
            self.assertNotEqual(changed, text)
            with self.assertRaises(AssertionError):
                original.rust_controls_before_phase(changed)

    def test_missing_duplicate_or_moved_phase_test_suffix_is_rejected(self):
        text = original.RUST_TEST.read_text()
        marker = text.index(original.PHASE_TEST_MARKER)
        old, suffix = text[:marker], text[marker:]
        for changed in [old, text + suffix, suffix + old,
                        old + suffix.replace(original.PHASE_TEST_MARKER, '\nmod unknown {', 1)]:
            with self.assertRaises(AssertionError):
                original.rust_controls_before_phase(changed)

    def test_inverse_is_not_a_generic_source_sanitizer(self):
        settlement = (original.ROOT / 'crates/core/src/integration_settlement.rs').read_text()
        tests = original.RUST_TEST.read_text()
        for inverse, text, foreign in [(original.settlement_before_phase, settlement, tests),
                                       (original.rust_controls_before_phase, tests, settlement)]:
            with self.assertRaises(AssertionError):
                inverse(foreign)
            with self.assertRaises(AssertionError):
                inverse(inverse(text))


if __name__ == '__main__':
    unittest.main()
