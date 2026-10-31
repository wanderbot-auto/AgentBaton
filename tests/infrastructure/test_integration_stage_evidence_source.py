"""Source/inverse checks only; these do not execute Rust or SQLite."""
import hashlib
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'crates/core/tests/fixtures/integration_runtime.rs'
HOST = ROOT / 'crates/core/tests/integration_ipc.rs'
MODULE = ROOT / 'crates/core/tests/fixtures/integration_stage_evidence.rs'
RUST_TEST = ROOT / 'crates/core/tests/fixtures/integration_stage_evidence_tests.rs'


def remove_once(text, fragment):
    if text.count(fragment) != 1:
        raise AssertionError('inverse fragment must occur exactly once')
    return text.replace(fragment, '', 1)


def fixture_inverse(text):
    text = remove_once(text, '#[path = "integration_stage_evidence.rs"]\nmod stage_evidence;\n')
    text = remove_once(text, '        stage_evidence::finish(\n            &self.server.runtime,\n            self.server.join_outcome,\n            self.task["id"].as_str(),\n            std::thread::panicking(),\n        );\n')
    fragment = '    let reply = server.request(frame);\n    stage_evidence::observe_reply(&server.runtime, method, &reply);\n    result(reply)\n'
    assert text.count(fragment) == 1
    return text.replace(fragment, '    result(server.request(frame))\n', 1)


def host_inverse(text):
    text = remove_once(text, '    join_outcome: Option<ActorJoinOutcome>,\n')
    text = remove_once(text, '// Test-host observation only; this does not certify worker/process stop.\n#[derive(Clone, Copy, PartialEq, Eq)]\nenum ActorJoinOutcome {\n    Ok,\n    Error,\n    Panic,\n}\n')
    text = remove_once(text, '            join_outcome: None,\n')
    return remove_once(text, '            self.join_outcome = Some(match &outcome {\n                Ok(Ok(())) => ActorJoinOutcome::Ok,\n                Ok(Err(_)) => ActorJoinOutcome::Error,\n                Err(_) => ActorJoinOutcome::Panic,\n            });\n')


def fields(text, name):
    match = re.search(r'\bstruct\s+' + name + r'\s*\{', text)
    assert match, name
    start = match.end()
    end = text.index('}', start)
    return set(re.findall(r'^\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:', text[start:end], re.M))


class IntegrationStageEvidenceSourceTests(unittest.TestCase):
    def test_whole_fixture_inverse_exact_e874(self):
        self.assertEqual(hashlib.sha256(fixture_inverse(FIXTURE.read_text()).encode()).hexdigest(), '149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06')

    def test_whole_host_inverse_exact_e874(self):
        self.assertEqual(hashlib.sha256(host_inverse(HOST.read_text()).encode()).hexdigest(), '90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb')

    def test_inverse_rejects_unknown_region_and_missing_fragment(self):
        for path, inverse in [(FIXTURE, fixture_inverse), (HOST, host_inverse)]:
            original = path.read_text()
            changed = original + '// unreviewed-region\n'
            self.assertNotEqual(hashlib.sha256(inverse(changed).encode()).digest(), hashlib.sha256(inverse(original).encode()).digest())
            with self.assertRaises(AssertionError):
                inverse(inverse(original))

    def test_attempt_shape_matches_real_production_record(self):
        production = (ROOT / 'crates/core/src/integration_budget.rs').read_text()
        self.assertEqual(fields(MODULE.read_text(), 'Attempt'), fields(production, 'IntegrationAttemptRecord'))
        self.assertEqual(len(fields(production, 'IntegrationAttemptRecord')), 14)
        for actual in ['view: IntegrationAttemptView', 'binding: ExecutionBinding', 'let run: IntegrationRun']:
            self.assertIn(actual, MODULE.read_text())

    def test_preparation_io_shape_matches_real_production(self):
        production = (ROOT / 'crates/workspaces/src/adoption.rs').read_text()
        self.assertEqual(fields(MODULE.read_text(), 'PreparationIo'), fields(production, 'ControlledPreparationIo'))

    def test_event_envelope_matches_actual_stored_shape(self):
        production = (ROOT / 'crates/core/src/storage.rs').read_text()
        self.assertEqual(fields(MODULE.read_text(), 'Event'), fields(production, 'Event'))
        self.assertIn('e.sequence == 0', MODULE.read_text())  # original records::event stores zero inside document.

    def test_selected_payload_shapes_match_original_event_construction(self):
        production = (ROOT / 'crates/core/src/integration_settlement.rs').read_text()
        for kind, raw in [('integration.stage_unknown', 'UnknownEvent'), ('integration.preparation_io_observed', 'PreparationEvent')]:
            tail = production.split('"' + kind + '"', 1)[1].split(')?;', 1)[0]
            self.assertEqual(fields(MODULE.read_text(), raw), set(re.findall(r'"([a-z_]+)"\s*:', tail)))

    def test_read_adapter_has_no_write_recovery_or_rpc(self):
        source = MODULE.read_text()
        collector = source.split('fn collect(', 1)[1]
        for forbidden in ['Store::', 'Daemon::', 'rpc(', 'request(', 'CREATE ', 'INSERT ', 'UPDATE ', 'DELETE ', 'migrate', 'recover']:
            self.assertNotIn(forbidden, collector)
        for required in ['SQLITE_OPEN_READ_ONLY', 'SQLITE_OPEN_URI', 'mode=ro&immutable=1', 'PRAGMA query_only=ON; BEGIN;', 'ROLLBACK;', 'Duration::ZERO', 'no_journal(runtime)?']:
            self.assertIn(required, source)

    def test_query_reverse_bindings_and_limits_are_present(self):
        source = MODULE.read_text()
        for required in ["id=?1 OR json_extract(document,'$.id')=?1 LIMIT 2", "attempt_id=?1 OR json_extract(document,'$.view.attempt_id')=?1 LIMIT 2", "integration_id=?1 OR json_extract(document,'$.integration_id')=?1", "json_extract(document,'$.payload.attempt_id') IN", 'LIMIT 33', 'LIMIT 65', 'MAX_ROWS: usize = 32', 'MAX_EVENTS: usize = 64', 'MAX_OUTPUT: usize = 16_384', 'MAX_INPUT: usize = 262_144']:
            self.assertIn(required, source)

    def test_read_gate_and_public_scope_are_explicit(self):
        source = MODULE.read_text()
        gate = source.split('fn diagnostic(', 1)[1].split('// Reject duplicate', 1)[0]
        self.assertLess(gate.index('actor != Some(ActorJoinOutcome::Ok)'), gate.index('read(&b)'))
        for required in ['post_owned_shutdown_not_failure_instant', 'process_stop_proven', 'measurement_not_authority', 'unobserved_actor_join', 'unobserved_run_binding', 'unobserved_records', 'INTEGRATION_STAGE_FAILURE ']:
            self.assertIn(required, source)
        self.assertIn('values.contains_key(&k)', source)
        self.assertRegex(source, r'shape\(\s*&value')

    def test_rust_controls_cover_required_negative_dimensions(self):
        source = RUST_TEST.read_text()
        for required in ['must not read', 'PRIVATE_CANARY_DO_NOT_PUBLISH', 'other-attempt', 'foreign', '33', '65', 'worker_thread_joined', 'git_stdout_observed_bytes', 'source_commit', 'unrecognized', 'server.finish()', 'result.is_ok()']:
            self.assertIn(required, source)
        self.assertEqual(source.count('#[test]'), 9)


if __name__ == '__main__':
    unittest.main()
