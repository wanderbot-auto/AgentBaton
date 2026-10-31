#!/usr/bin/env python3
"""Actual private marker/helpers with a controlled clock; no product scheduling claim."""
import ast
import importlib.util
import os
from pathlib import Path
import unittest

HELPER_SPEC = importlib.util.spec_from_file_location('private_marker_fixture', Path(__file__).with_name('test_data_preview_evidence.py'))
BASE = importlib.util.module_from_spec(HELPER_SPEC)
HELPER_SPEC.loader.exec_module(BASE)


class ControlledClock:
    def __init__(self):
        self.now = 0.0

    def monotonic(self):
        return self.now

    def monotonic_ns(self):
        return int(self.now * 1_000_000_000)

    def sleep(self, seconds):
        self.now += seconds


class PreviewEvidenceDeadlineTests(unittest.TestCase):
    def setUp(self):
        self.fixture = BASE.PreviewEvidenceTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.clock = ControlledClock()
        scope = self.fixture.scope
        scope['time'] = self.clock
        node = next(n for n in BASE.TREE.body if isinstance(n, ast.FunctionDef) and n.name == 'wait_preview_evidence_ready')
        exec(compile(ast.Module(body=[node], type_ignores=[]), '<actual-absolute-ready-wait>', 'exec'), scope)
        self.wait = scope['wait_preview_evidence_ready']
        self.reads = 0

    def read(self, elapsed):
        self.reads += 1
        result = self.fixture.read()  # Secure actual file reader, not a fabricated ready DTO.
        self.clock.now += elapsed
        return result

    def observe(self, read, pump=lambda: None, predicate=lambda value: value is not None):
        return self.wait(read, predicate, seconds=15, pump=pump)

    def test_actual_ready_before_deadline_is_accepted(self):
        self.fixture.write(self.fixture.ready)
        result = self.observe(lambda: self.read(14.0))
        self.assertTrue(result['nonce_preview_project_matched'])
        self.assertEqual(self.reads, 1)
        self.assertEqual(self.clock.now, 14.0)

    def test_actual_valid_marker_arriving_after_slow_read_is_rejected(self):
        self.fixture.write(self.fixture.ready)
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(16.0))
        self.assertEqual(self.reads, 1)
        self.assertEqual(self.clock.now, 16.0)

    def test_marker_at_exact_deadline_is_not_accepted(self):
        self.fixture.write(self.fixture.ready)
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(15.0))
        self.assertEqual(self.reads, 1)

    def test_slow_partial_read_cannot_reset_observation_budget(self):
        self.fixture.write(b'{"schema_version":')
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(16.0))
        self.assertEqual(self.reads, 1)

    def test_slow_browser_pump_prevents_read_after_deadline(self):
        self.fixture.write(self.fixture.ready)

        def pump():
            self.clock.now = 16.0

        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(0), pump=pump)
        self.assertEqual(self.reads, 0)

    def test_already_expired_loop_does_not_pump_or_read(self):
        ticks = iter([0.0, 15.0])
        self.clock.monotonic = lambda: next(ticks)
        pumps = []
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(0), pump=lambda: pumps.append('unexpected'))
        self.assertEqual(pumps, [])
        self.assertEqual(self.reads, 0)

    def test_missing_and_partial_marker_share_one_deadline(self):
        def read():
            if self.reads == 1:
                self.fixture.write(b'{"schema_version":')
            return self.read(8.0)

        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(read)
        self.assertEqual(self.reads, 2)
        self.assertGreater(self.clock.now, 15.0)

    def test_missing_partial_and_completed_writer_can_finish_within_same_budget(self):
        raw = BASE.json.dumps(self.fixture.ready).encode()
        held = []

        def read():
            if self.reads == 1:
                fd = os.open(self.fixture.marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                held.append(fd)
                os.write(fd, raw[:len(raw) // 2])
            if self.reads == 2:
                os.write(held[0], raw[len(raw) // 2:])
                os.fsync(held[0])
            return self.read(4.0)

        try:
            self.assertTrue(self.observe(read)['nonce_preview_project_matched'])
            self.assertEqual(self.reads, 3)
            self.assertLess(self.clock.now, 15.0)
        finally:
            for fd in held:
                os.close(fd)

    def test_complete_wrong_nonce_fails_immediately_without_retry(self):
        self.fixture.write({**self.fixture.ready, 'nonce': 'b' * 32})

        def read():
            self.reads += 1
            return self.fixture.read()

        with self.assertRaisesRegex(AssertionError, 'descriptor mismatch'):
            self.observe(read)
        self.assertEqual(self.reads, 1)
        self.assertEqual(self.clock.now, 0.0)

    def test_result_cannot_be_accepted_after_predicate_crosses_deadline(self):
        self.fixture.write(self.fixture.ready)

        def predicate(value):
            self.clock.now = 16.0
            return value is not None

        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.observe(lambda: self.read(1.0), predicate=predicate)
        self.assertEqual(self.reads, 1)

    def test_observer_rejects_increased_or_foreign_budget(self):
        for seconds in [16, 30, 15.0, True]:
            with self.subTest(seconds=seconds), self.assertRaisesRegex(AssertionError, 'fixed at 15'):
                self.wait(lambda: self.read(0), lambda value: value is not None, seconds=seconds, pump=lambda: None)
        self.assertEqual(self.reads, 0)


if __name__ == '__main__':
    unittest.main()
