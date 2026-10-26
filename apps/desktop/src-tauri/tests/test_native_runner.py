"""Test external native evidence and fatal/timeout cleanup; no GUI is simulated."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('native_run', ROOT / 'scripts/native-run.py')
native_run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native_run)
GOOD = {
    'status': 'passed',
    'first_webview': dict(domMounted=True, realComposerSubmitted=True, realTaskRendered=True, taskId='fixture', idempotent=True, invalidInputRejected=True, journal=True),
    'reopened_webview': dict(domMounted=True, taskId='fixture', restartPersisted=True, draftCancelled=True, stopEvidence=True),
    'window_close_daemon_survived': True, 'second_writer_rejected': True,
    'owned_daemon_restarted': True, 'owned_daemon_cleanup': True,
}


class NativeRunnerTests(unittest.TestCase):
    def run_fixture(self, code, timeout=3, stale=False):
        with tempfile.TemporaryDirectory(prefix='native-runner-test-') as temporary:
            root = Path(temporary)
            path = root / 'native-smoke.json'
            write = f'from pathlib import Path;Path({str(path)!r}).write_text({json.dumps(GOOD)!r});'
            code = code.replace('WRITE_GOOD;', write)
            if stale:
                path.write_text(json.dumps(GOOD))
            status = native_run.run([sys.executable, '-c', code], root, timeout)
            report = json.loads((root / 'native-runner.json').read_text())
            if stale:
                self.assertTrue(list((root / 'previous').glob('*/native-smoke.json')))
            return status, report, json.loads(path.read_text()) if path.exists() else None

    def test_success_requires_current_native_evidence_after_group_cleanup(self):
        status, report, _ = self.run_fixture('WRITE_GOOD;pass')
        self.assertEqual(status, 0)
        self.assertEqual(report['status'], 'passed')
        self.assertTrue(report['owned_group_cleanup'])

    def test_exit_zero_without_native_report_is_failure(self):
        status, report, _ = self.run_fixture('pass')
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], 0)
        self.assertEqual(report['status'], 'failed')
        self.assertIn('not produced', report['evidence_error'])

    def test_stale_passing_report_is_archived_and_cannot_pass(self):
        status, report, _ = self.run_fixture('pass', stale=True)
        self.assertEqual(status, 1)
        self.assertEqual(report['status'], 'failed')

    def test_malformed_report_section_has_structured_failure(self):
        malformed = dict(GOOD, first_webview=[])
        self.assertIn('lacks required', native_run.native_report_error(malformed))

    def test_nonzero_downgrades_even_a_passing_native_report(self):
        status, report, native = self.run_fixture('WRITE_GOOD;raise SystemExit(7)')
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], 7)
        self.assertEqual(native['status'], 'failed')

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux subreaper evidence; macOS lane remains separate')
    def test_fatal_signal_reaps_adopted_owned_descendant(self):
        status, report, _ = self.run_fixture("import os,signal,subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(20)']);os.kill(os.getpid(),signal.SIGKILL)")
        self.assertEqual(status, 1)
        self.assertEqual(report['exit_code'], -9)
        self.assertGreaterEqual(report['adopted_owned_group_children_reaped'], 1)

    def test_timeout_remains_bounded_when_child_ignores_term(self):
        status, report, _ = self.run_fixture('import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);time.sleep(20)', timeout=1)
        self.assertEqual(status, 1)
        self.assertTrue(report['timeout'])
        self.assertEqual(report['exit_code'], -9)


if __name__ == '__main__':
    unittest.main()
