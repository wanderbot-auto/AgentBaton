"""Pure finite projection controls; no product or model execution."""
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('ancestor_admission_projection', ROOT / 'scripts/record-factory-rework-ci-evidence.py')
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)
SECRET = 'PRIVATE_CANARY_PATH_TOKEN_ID'


def sample(**changes):
    out = dict(guard_kind='runtime', run_revision=13, plan_revision=2,
        relevant_count=4, active_count=0, attempt_cap=5, parallel_cap=1,
        runtime_accounted_count=4, output_accounted_count=4,
        settled_actual_runtime_ms=15589, unsettled_reserved_runtime_ms=0,
        requested_runtime_ms=6000, runtime_cap_ms=20000,
        settled_actual_output_bytes=62, unsettled_reserved_output_bytes=0,
        requested_output_bytes=1024, output_cap_bytes=8192, aggregation_complete=True)
    out.update(changes)
    return out


def count_sample(guard='attempts'):
    return sample(guard_kind=guard, relevant_count=5, active_count=1,
        runtime_accounted_count=0, output_accounted_count=0,
        settled_actual_runtime_ms=None, unsettled_reserved_runtime_ms=None,
        settled_actual_output_bytes=None, unsettled_reserved_output_bytes=None,
        aggregation_complete=False)


class AncestorAdmissionProjection(unittest.TestCase):
    def event(self, detail=None, reason='RESOURCE_LIMIT', **extra):
        payload = dict(reason_code=reason, automatic_retry=False, task_accepted=False, **extra)
        if detail is not None:
            payload['budget_admission'] = detail
        return P.event_fields(dict(kind='goal.run_blocked', revision=14, payload=payload), SECRET, 2)

    def test_original_guard_numbers_are_not_post_shutdown_remaining_budget(self):
        result = self.event(sample(), message=SECRET)
        self.assertEqual(result['budget_admission']['guard_kind'], 'runtime')
        self.assertEqual(result['budget_admission']['settled_actual_runtime_ms'], 15589)
        self.assertEqual(result['budget_admission']['requested_runtime_ms'], 6000)
        self.assertEqual(result['budget_admission']['runtime_cap_ms'], 20000)
        self.assertEqual(result['budget_admission']['snapshot_time'], 'original_ancestor_reservation_guard')
        self.assertTrue(result['budget_admission']['not_an_authority_or_stop_proof'])
        self.assertFalse(result['automatic_retry'])
        self.assertFalse(result['task_accepted'])
        self.assertNotIn(SECRET, json.dumps(result))


    def test_original_guard_is_bound_to_the_blocked_event_revision_and_plan(self):
        payload = dict(reason_code='RESOURCE_LIMIT', automatic_retry=False,
            task_accepted=False, budget_admission=sample())
        for revision, plan in [(13, 2), (15, 2), (14, 1)]:
            with self.subTest(revision=revision, plan=plan):
                with self.assertRaises(P.Invalid):
                    P.event_fields(dict(kind='goal.run_blocked', revision=revision,
                        payload=payload), SECRET, plan)

    def test_old_blocked_event_shape_has_no_fabricated_zero_diagnostic(self):
        self.assertEqual(self.event(), dict(kind='goal.run_blocked', task_accepted=False,
            reason_code='RESOURCE_LIMIT', automatic_retry=False))

    def test_attempt_and_parallel_short_circuit_leave_aggregates_unknown(self):
        for guard, detail in [('attempts', count_sample()), ('parallel', count_sample('parallel') | {'attempt_cap': 6})]:
            with self.subTest(guard=guard):
                result = self.event(detail)['budget_admission']
                self.assertEqual(result['guard_kind'], guard)
                self.assertFalse(result['aggregation_complete'])
                self.assertIsNone(result['settled_actual_runtime_ms'])
                self.assertIsNone(result['unsettled_reserved_output_bytes'])

    def test_output_refusal_cannot_relabel_prior_runtime_failure(self):
        detail = sample(guard_kind='output', settled_actual_runtime_ms=13000,
            settled_actual_output_bytes=8000)
        self.assertEqual(self.event(detail)['budget_admission']['guard_kind'], 'output')
        with self.assertRaises(P.Invalid):
            self.event(detail | {'settled_actual_runtime_ms': 15589})

    def test_overflow_is_partial_and_never_reconstructed_as_zero(self):
        maximum = (1 << 64) - 1
        for detail in [sample(guard_kind='runtime_overflow', runtime_accounted_count=2,
                output_accounted_count=1, settled_actual_runtime_ms=maximum,
                unsettled_reserved_runtime_ms=None, aggregation_complete=False),
                sample(guard_kind='output_overflow', runtime_accounted_count=2,
                output_accounted_count=2, settled_actual_output_bytes=None,
                unsettled_reserved_output_bytes=maximum, aggregation_complete=False)]:
            with self.subTest(guard=detail['guard_kind']):
                result = self.event(detail)['budget_admission']
                self.assertFalse(result['aggregation_complete'])
                self.assertEqual(result['runtime_accounted_count'], 2)
        with self.assertRaises(P.Invalid):
            self.event(sample(settled_actual_runtime_ms=maximum))

    def test_each_missing_or_unknown_key_refuses_projection(self):
        for key in sample():
            with self.subTest(missing=key):
                detail = sample(); del detail[key]
                with self.assertRaises(P.Invalid):
                    self.event(detail)
        with self.assertRaises(P.Invalid):
            self.event(sample() | {'private_path': SECRET})

    def test_scalar_types_ranges_and_closed_guard_vocabulary(self):
        for key in sample():
            with self.subTest(key=key):
                detail = sample(); detail[key] = SECRET
                with self.assertRaises(P.Invalid):
                    self.event(detail)
        for changes in [dict(run_revision=True), dict(plan_revision=0),
                dict(relevant_count=100001), dict(active_count=5),
                dict(runtime_accounted_count=5), dict(output_accounted_count=5),
                dict(requested_runtime_ms=-1), dict(runtime_cap_ms=1 << 64),
                dict(settled_actual_output_bytes=True), dict(aggregation_complete=1),
                dict(guard_kind='private-other-guard')]:
            with self.subTest(changes=changes):
                with self.assertRaises(P.Invalid):
                    self.event(sample(**changes))

    def test_non_resource_errors_and_false_guard_claims_refuse(self):
        with self.assertRaises(P.Invalid):
            self.event(sample(), reason='INPUT_CHANGED')
        for detail in [sample(settled_actual_runtime_ms=10000),
                sample(aggregation_complete=False), count_sample() | {'relevant_count': 4},
                count_sample('parallel') | {'attempt_cap': 6, 'active_count': 0},
                count_sample() | {'settled_actual_runtime_ms': 0}]:
            with self.subTest(detail=detail):
                with self.assertRaises(P.Invalid):
                    self.event(detail)


# Exercise the actual existing post-owned-shutdown collector with generated
# SQLite event rows, not only the field helper. The original 80 tests are not
# copied, inherited or altered by these three controls.
LEGACY_SPEC = importlib.util.spec_from_file_location('ancestor_admission_sql_fixture', ROOT / 'tests/infrastructure/test_factory_rework_ci_evidence.py')
T = importlib.util.module_from_spec(LEGACY_SPEC)
LEGACY_SPEC.loader.exec_module(T)


class AncestorAdmissionSqlProjection(unittest.TestCase):
    setUp = T.EventProjection.setUp
    add = T.EventProjection.add
    collected = T.EventProjection.collected
    events = T.EventProjection.events

    def test_same_original_guard_reaches_existing_retained_event_projection(self):
        self.add(payload=dict(reason_code='RESOURCE_LIMIT', automatic_retry=False,
            task_accepted=False, budget_admission=sample(run_revision=8)))
        result = self.events()
        self.assertEqual(result['observed_run_revision'], 9)
        self.assertEqual(result['events'][0]['budget_admission']['run_revision'], 8)
        self.assertEqual(result['events'][0]['budget_admission']['runtime_cap_ms'], 20000)
        self.assertEqual(result['events'][0]['budget_admission']['snapshot_time'],
            'original_ancestor_reservation_guard')

    def test_malformed_or_wrong_revision_optional_detail_fails_closed(self):
        for detail in [sample(run_revision=7), sample(run_revision=8, plan_revision=1),
                sample(run_revision=8) | {'private_path': SECRET}]:
            with self.subTest(detail=detail):
                self.f.sql('DELETE FROM events')
                self.add(payload=dict(reason_code='RESOURCE_LIMIT', automatic_retry=False,
                    task_accepted=False, budget_admission=detail))
                self.assertEqual(self.collected()['cases'][0]['status'], 'invalid')

    def test_missing_guard_stays_absent_after_sqlite_projection(self):
        self.add()
        self.assertNotIn('budget_admission', self.events()['events'][0])


if __name__ == '__main__':
    unittest.main()
