"""Exact fixture source/inverse checks; no Git, Rust, SQLite or process run."""
import hashlib
import json
from pathlib import Path
import unittest
from unittest import mock

import integration_drift_ref_preparation_fixture as F

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / F.PATH


def function(text, name):
    start = text.index('fn ' + name + '(')
    end = text.index('\n}\n', start) + 3
    return text[start:end]


class DriftPreparationSource(unittest.TestCase):
    def current_and_before(self):
        current = SOURCE.read_text()
        return current, F.restore_before_drift_ref_preparation(current)

    def test_whole_runtime_and_existing_observers_restore_exact_original(self):
        current, before = self.current_and_before()
        self.assertEqual(hashlib.sha256(current.encode()).hexdigest(), F.CURRENT_SCHEMA18_SHA256)
        self.assertEqual(hashlib.sha256(before.encode()).hexdigest(), F.RESTORED_SHA256)
        self.assertEqual(F.restore_before_drift_ref_preparation(before), before)
        from integration_fixed_script_fixture import restore_before_fixed_scripts, FIXED_SCRIPT_RESTORED_SHA256
        self.assertEqual(hashlib.sha256(restore_before_fixed_scripts(current).encode()).hexdigest(), FIXED_SCRIPT_RESTORED_SHA256)

    def test_old_host_git_gate_positive_withdrawal_and_source_image_whole_bytes_unchanged(self):
        current, before = self.current_and_before()
        for name in ('reprepare_host_git', 'advance_owned_test_target', 'source_image',
                     'three_real_rounds_revalidate_each_merged_candidate_after_consecutive_descendant_advances',
                     'assert_withdrawal_during_actual_target_drift'):
            self.assertEqual(function(current, name), function(before, name), name)
        start, end = 'struct ReprepareGate {', '\nimpl RuntimeFixture {\n    fn reprepare_command_definition'
        self.assertEqual(current.split(start, 1)[1].split(end, 1)[0],
                         before.split(start, 1)[1].split(end, 1)[0])

    def test_fixed_prepared_chain_and_source_target_proofs_precede_approval_and_start(self):
        current, _ = self.current_and_before()
        body = function(current, 'assert_real_drift_refusal')
        ordered = ['let before_source = source_image(&f.source())',
                   'let before_project = source_image(&f.project_root)',
                   'let prepared = prepare_drift_refs(',
                   'source_image(&f.source()) == before_source',
                   'source_image(&f.project_root) == before_project',
                   'let policy = if matches!', '"integrations.start"']
        positions = [body.index(text) for text in ordered]
        self.assertEqual(positions, sorted(positions))
        prepare = function(current, 'prepare_drift_refs')
        self.assertEqual(prepare.count('DriftHostStep::ConfirmUnchangedTarget'), 2)
        self.assertIn('let mut expected = f.initial_target.clone()', prepare)
        self.assertIn('expected = next.next.clone()', prepare)
        self.assertIn('assert!((1..=3).contains(&rounds))', prepare)
        self.assertIn('assert!(!non_descendant || rounds == 1)', prepare)
        for name in ('prepare_drift_ref', 'prepare_drift_refs'):
            block = function(current, name)
            self.assertNotIn('"update-ref"', block)
            self.assertNotIn('rpc(', block)
        self.assertIn('"commit-tree", &tree, "-m", "owned unrelated replacement"',
                      function(current, 'prepare_drift_ref'))

    def test_live_checks_and_unchanged_completion_tail_keep_all_refusal_assertions(self):
        current, before = self.current_and_before()
        after = function(current, 'assert_real_drift_refusal')
        old = function(before, 'assert_real_drift_refusal')
        self.assertEqual(after.split('    let done = observe(', 1)[1], old.split('    let done = observe(', 1)[1])
        for token in ('assert_eq!(live["run"]["active_stage"], "validator")',
                      'assert_eq!(live["run"]["round"], round)',
                      'assert_eq!(attempt["kind"], "validator")',
                      'assert_eq!(attempt["settled"], false)',
                      'live["run"]["expected_target_head"]', 'heads[(round - 1) as usize]'):
            self.assertIn(token, after)
            self.assertLess(after.index(token), after.index('let next = apply_prepared_drift_ref'))
        self.assertIn('["rev-list", "--count", heads.last().unwrap()]', after)
        self.assertIn('"1"', after)
        self.assertIn('"reservation_refunded"], false', after)
        self.assertIn('"integration.cas_observed"', after)

    def test_marker_deadline_cas_exact_confirmation_and_release_never_reset(self):
        current, _ = self.current_and_before()
        apply = function(current, 'apply_prepared_drift_ref')
        for token in ('prepared.expected == expected', 'prepared.round, round',
                      'prepared.non_descendant, non_descendant', '&prepared.tree',
                      '"update-ref"', '"--no-deref"', '&prepared.next', '&prepared.expected',
                      'DriftHostStep::ConfirmChangedTarget', '["rev-parse", TARGET]'):
            self.assertIn(token, apply)
        self.assertNotIn('Instant::now()', apply)
        self.assertNotIn('setup(', apply)
        body = function(current, 'assert_real_drift_refusal')
        ordered = ['gate.next_owned_validator()', 'DriftHostClock::marker(round, deadline)',
                   'let next = apply_prepared_drift_ref(', 'heads.push(next)', 'gate.release(deadline)']
        positions = [body.index(t) for t in ordered]
        self.assertEqual(positions, sorted(positions))
        self.assertIn('started: deadline.checked_sub(Duration::from_millis(500)).unwrap()', current)
        self.assertNotIn('Duration::from_millis(501)', current)

    def test_finite_step_enums_diagnostic_has_only_closed_clock_fields(self):
        current, _ = self.current_and_before()
        start = current.index('enum DriftHostStep {')
        end = current.index('\n}', start)
        steps = [x.strip().rstrip(',') for x in current[start:end].splitlines()[1:] if x.strip()]
        self.assertEqual(steps, ['ConfirmUnchangedTarget', 'ReadExpectedTree', 'CreatePreparedObject',
                         'VerifyPreparedAncestry', 'ReadLiveValidator', 'TargetCompareAndSwap',
                         'ConfirmChangedTarget', 'ReleaseValidator'])
        record = current.split('    fn record(&self, step: DriftHostStep, boundary: DriftHostBoundary)', 1)[1].split('    fn git(', 1)[0]
        self.assertIn('original_elapsed_ms={}', record)
        self.assertIn('original_remaining_ms={}', record)
        self.assertIn('saturating_duration_since', record)
        for forbidden in ('args', 'payload', 'path', 'token', 'policy', 'definition'):
            self.assertNotIn(forbidden, record)

    def test_any_budget_deadline_old_assert_or_cleanup_change_fails_exact_whole_source(self):
        raw = SOURCE.read_text()
        for token, changed in [('"wall_time_ms":500', '"wall_time_ms":501'),
                               ('"cleanup_time_ms":500', '"cleanup_time_ms":501'),
                               ('Duration::from_millis(500)', 'Duration::from_millis(501)'),
                               ('"ttl_ms":30_000', '"ttl_ms":30_001'),
                               ('Duration::from_secs(5)', 'Duration::from_secs(6)'),
                               ('reservation(2000,1024)', 'reservation(2001,1024)'),
                               ('gate.release(deadline)', 'gate.release(Instant::now())'),
                               ('assert_eq!(attempt["settled"], false)', 'assert_eq!(attempt["settled"], true)')]:
            self.assertTrue(token in raw, f'Missing exact budget token: {token}')
            with self.subTest(token=token), self.assertRaises(AssertionError):
                F.restore_before_drift_ref_preparation(raw.replace(token, changed, 1))

    def test_duplicate_moved_removed_helper_and_unrelated_source_are_rejected(self):
        raw = SOURCE.read_text()
        helper = F.EDITS[0][0]
        for bad in (raw + '\n', helper + raw, raw.replace(helper, '', 1) + helper,
                    raw.replace('let value = reprepare_host_git(f, args, self.deadline);',
                                'let value = reprepare_host_git(f, args, Instant::now());', 1)):
            self.assertNotEqual(raw, bad)
            with self.assertRaises(AssertionError): F.restore_before_drift_ref_preparation(bad)

    def test_inverse_metadata_and_old_fragment_cannot_be_changed_or_omitted(self):
        raw = F.FIXTURE.read_bytes()
        for bad in (raw + b'\n', b'{}', raw.replace(b'500', b'501', 1)):
            with self.assertRaises(AssertionError): F.load_inverse(bad)
        current = SOURCE.read_text()
        edits = [list(x) for x in F.EDITS]
        edits[-1][1] += '// hidden change\n'
        with mock.patch.object(F, 'EDITS', edits), self.assertRaises(AssertionError):
            F.restore_before_drift_ref_preparation(current)


if __name__ == '__main__':
    unittest.main()
