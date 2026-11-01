"""Actual workflow AST with tiny files and stub subprocesses; no product execution."""
import ast
import contextlib
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import textwrap
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SECRET = 'PRIVATE_PATH_CREDENTIAL_HISTORY_CANARY'


def workflow_function():
    lines = (ROOT / '.github/workflows/product.yml').read_text().splitlines(True)
    start = next(i for i, line in enumerate(lines) if line == '          def run_task_workspace_evidence():\n')
    end = next(i for i in range(start + 1, len(lines)) if lines[i] == '          run_task_workspace_evidence()\n')
    return ast.parse(textwrap.dedent(''.join(lines[start:end]))).body[0]


class LaunchDiagnostics(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.runtime = self.root / 'runtime'; self.runtime.mkdir(mode=0o700)
        self.tools = self.root / 'target/debug'; self.tools.mkdir(parents=True)
        self.payload = {'batond': b'generated tiny daemon, never executed', 'baton': b'generated tiny CLI, never executed'}
        for name, raw in self.payload.items():
            (self.tools / name).write_bytes(raw); (self.tools / name).chmod(0o700)
        self.candidate = {'git_head': 'a' * 40, 'binaries': {k: hashlib.sha256(v).hexdigest() for k, v in self.payload.items()}}
        self.calls = []
        self.namespace = dict(pathlib=__import__('pathlib'), hashlib=hashlib, json=json,
            subprocess=subprocess, sys=sys, candidate=self.candidate, chromium=SECRET)
        tree = ast.Module(body=[workflow_function()], type_ignores=[])
        exec(compile(ast.fix_missing_locations(tree), '<actual-workflow-function>', 'exec'), self.namespace)
        self.fn = self.namespace['run_task_workspace_evidence']
        helper = next(node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef) and node.name == 'diagnostic_json')
        helper_tree = ast.Module(body=[helper], type_ignores=[])
        exec(compile(ast.fix_missing_locations(helper_tree), '<actual-workflow-diagnostic>', 'exec'), self.namespace)
        self.serialize = self.namespace['diagnostic_json']

    def tearDown(self):
        self.temp.cleanup()

    def execute(self, driver=0, projection=0, launch_error=None, projection_error=None, fstat=None, open_hook=None, printer=None):
        def run(argv, **kwargs):
            role = 'driver' if argv[1].endswith('task-workspace-binding-real-ipc.py') else 'projection'
            self.calls.append((role, argv, kwargs))
            error = launch_error if role == 'driver' else projection_error
            if error is not None:
                raise error
            return SimpleNamespace(returncode=driver if role == 'driver' else projection)
        out = io.StringIO(); failure = None
        with contextlib.ExitStack() as stack:
            stack.enter_context(contextlib.chdir(self.root))
            stack.enter_context(mock.patch.dict(os.environ, RUNNER_TEMP=str(self.runtime)))
            stack.enter_context(mock.patch.object(subprocess, 'run', side_effect=run))
            stack.enter_context(contextlib.redirect_stdout(out))
            if fstat is not None: stack.enter_context(mock.patch.object(os, 'fstat', side_effect=fstat))
            if open_hook is not None: stack.enter_context(mock.patch.object(os, 'open', side_effect=open_hook))
            if printer is not None: self.namespace['print'] = printer
            try:
                self.fn()
            except Exception as error:
                failure = error
            finally:
                self.namespace.pop('print', None)
        lines = out.getvalue().splitlines()
        self.assertLessEqual(len(lines), 1)
        self.assertNotIn(SECRET, out.getvalue())
        self.assertNotIn(str(self.root), out.getvalue())
        return failure, json.loads(lines[0]) if lines else None

    def test_success_keeps_owned_private_copy_and_exact_two_launches(self):
        error, row = self.execute()
        self.assertIsNone(error)
        self.assertEqual([c[0] for c in self.calls], ['driver', 'projection'])
        self.assertEqual((row['driver_stage'], row['projection_stage']), ('driver_completed', 'projection_completed'))
        self.assertEqual((row['driver_exit'], row['projection_exit']), (0, 0))
        self.assertLessEqual(len(self.serialize(row).encode()), 4096)
        for name, raw in self.payload.items():
            copied = self.runtime / 'task-workspace-browser-private/tools' / name
            self.assertEqual(copied.read_bytes(), raw)
            self.assertEqual(stat.S_IMODE(copied.stat().st_mode), 0o500)
            self.assertTrue(row['tools'][name]['identity_unchanged'])
            self.assertTrue(row['tools'][name]['digest_matches'])
        self.assertIn('--run', self.calls[0][1])
        self.assertEqual(self.calls[0][2]['check'], False)

    def metadata_override(self, **changes):
        real = os.fstat; inode = (self.tools / 'batond').stat().st_ino
        def changed(fd):
            value = real(fd)
            if value.st_ino != inode: return value
            fields = {key: getattr(value, key) for key in dir(value) if key.startswith('st_')}
            fields.update(changes)
            return SimpleNamespace(**fields)
        return changed

    def test_original_128MiB_limit_refuses_before_driver_with_finite_size(self):
        error, row = self.execute(fstat=self.metadata_override(st_size=134217729))
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'tool_metadata')
        self.assertEqual(row['tools']['batond']['bytes'], 134217729)
        self.assertIsNone(row['driver_exit'])
        self.assertEqual([c[0] for c in self.calls], ['projection'])

    def test_wrong_owner_refuses_before_driver_without_exposing_uid(self):
        error, row = self.execute(fstat=self.metadata_override(st_uid=os.getuid() + 1))
        self.assertIsInstance(error, RuntimeError)
        self.assertFalse(row['tools']['batond']['owner_matches'])
        self.assertEqual(row['driver_failure_stage'], 'tool_metadata')
        self.assertNotIn('uid', self.serialize(row))

    def test_group_world_writable_tool_still_refuses(self):
        (self.tools / 'batond').chmod(0o722)
        error, row = self.execute()
        self.assertIsInstance(error, RuntimeError)
        self.assertTrue(row['tools']['batond']['writable'])
        self.assertEqual(row['driver_failure_stage'], 'tool_metadata')

    def test_nonregular_tool_metadata_still_refuses(self):
        error, row = self.execute(fstat=self.metadata_override(st_mode=stat.S_IFIFO | 0o700))
        self.assertIsInstance(error, RuntimeError)
        self.assertIs(row['tools']['batond']['regular'], False)
        self.assertEqual(row['driver_failure_stage'], 'tool_metadata')

    def test_copy_destination_open_failure_stays_before_driver(self):
        real = os.open
        def changed(path, flags, *args, **kwargs):
            if Path(path).name == 'batond' and flags & os.O_CREAT: raise OSError(SECRET)
            return real(path, flags, *args, **kwargs)
        error, row = self.execute(open_hook=changed)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'tool_copy')
        self.assertEqual([c[0] for c in self.calls], ['projection'])

    def test_source_symlink_remains_refused_at_open(self):
        source = self.tools / 'batond'; source.rename(self.tools / 'original')
        source.symlink_to('original')
        error, row = self.execute()
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'tool_open')
        self.assertIsNone(row['tools']['batond'])

    def test_digest_mismatch_still_refuses_at_postcopy(self):
        self.candidate['binaries']['batond'] = '0' * 64
        error, row = self.execute()
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'tool_postcopy')
        self.assertIs(row['tools']['batond']['digest_matches'], False)
        self.assertIs(row['tools']['batond']['identity_unchanged'], True)

    def test_changed_identity_still_refuses_at_postcopy(self):
        real = os.fstat; inode = (self.tools / 'batond').stat().st_ino; reads = []
        def changed(fd):
            value = real(fd)
            if value.st_ino != inode: return value
            reads.append(1)
            if len(reads) == 1: return value
            fields = {key: getattr(value, key) for key in dir(value) if key.startswith('st_')}
            fields['st_mtime_ns'] += 1
            return SimpleNamespace(**fields)
        error, row = self.execute(fstat=changed)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'tool_postcopy')
        self.assertIs(row['tools']['batond']['identity_unchanged'], False)

    def test_preflight_like_exit_without_report_distinguished_from_copy_failure(self):
        error, row = self.execute(driver=1, projection=2)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual((row['driver_stage'], row['driver_exit']), ('driver_completed', 1))
        self.assertEqual((row['projection_stage'], row['projection_exit']), ('projection_completed', 2))
        self.assertIsNone(row['driver_failure_stage'])

    def test_driver_launch_oserror_keeps_null_exit_and_projection_attempt(self):
        error, row = self.execute(launch_error=OSError(SECRET))
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'driver_launch')
        self.assertEqual(row['driver_failure_kind'], 'os_error')
        self.assertIsNone(row['driver_exit'])
        self.assertEqual([c[0] for c in self.calls], ['driver', 'projection'])

    def test_driver_log_failure_still_attempts_projection(self):
        real = os.open
        def changed(path, *args, **kwargs):
            if Path(path).name == 'driver.log': raise OSError(SECRET)
            return real(path, *args, **kwargs)
        error, row = self.execute(open_hook=changed)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_stage'], 'driver_log_open')
        self.assertIsNone(row['driver_exit'])
        self.assertEqual([c[0] for c in self.calls], ['projection'])

    def test_driver_subprocesserror_is_finite_and_not_success(self):
        error, row = self.execute(launch_error=subprocess.TimeoutExpired(SECRET, 1, output=SECRET))
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_failure_kind'], 'subprocess_error')
        self.assertIsNone(row['driver_exit'])

    def test_projection_launch_failure_preserves_actual_driver_exit(self):
        error, row = self.execute(projection_error=OSError(SECRET))
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['driver_exit'], 0)
        self.assertIsNone(row['projection_exit'])
        self.assertEqual(row['projection_failure_stage'], 'projection_launch')

    def test_projection_log_failure_is_finite_without_launch(self):
        real = os.open
        def changed(path, *args, **kwargs):
            if Path(path).name == 'projection.log': raise OSError(SECRET)
            return real(path, *args, **kwargs)
        error, row = self.execute(open_hook=changed)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(row['projection_failure_stage'], 'projection_log_open')
        self.assertEqual([c[0] for c in self.calls], ['driver'])

    def test_private_directory_reuse_preserves_original_refusal(self):
        (self.runtime / 'task-workspace-browser-private').mkdir(mode=0o700)
        error, row = self.execute()
        self.assertIsInstance(error, FileExistsError)
        self.assertEqual(row['driver_failure_stage'], 'private_directory')
        self.assertEqual(self.calls, [])

    def test_diagnostic_exception_never_masks_original_failure(self):
        def refused(*args, **kwargs): raise RuntimeError(SECRET)
        error, row = self.execute(driver=1, printer=refused)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(str(error), 'Task workspace driver or finite evidence projection failed')
        self.assertIsNone(row)

    def test_diagnostic_exception_cannot_create_success(self):
        def refused(*args, **kwargs): raise OSError(SECRET)
        error, row = self.execute(printer=refused)
        self.assertIsInstance(error, RuntimeError)
        self.assertIsNone(row)

    def test_closed_diagnostic_schema_unknown_and_untyped_values_refused(self):
        _, good = self.execute()
        mutations = [lambda r: r.update(path=SECRET), lambda r: r.update(driver_stage=SECRET),
            lambda r: r.update(tool_role=SECRET), lambda r: r.update(driver_failure_kind=SECRET),
            lambda r: r.update(driver_failure_stage=SECRET), lambda r: r.update(contract=SECRET),
            lambda r: r.update(driver_exit=False), lambda r: r.update(projection_exit=0.0),
            lambda r: r.update(driver_exit=2147483648), lambda r: r['tools']['batond'].update(bytes=True),
            lambda r: r['tools']['batond'].update(bytes=-1), lambda r: r['tools']['batond'].update(bytes=9223372036854775808),
            lambda r: r['tools']['batond'].update(mode=0o10000),
            lambda r: r['tools']['batond'].update(owner_matches=1), lambda r: r['tools']['batond'].update(digest_matches=0),
            lambda r: r['tools']['batond'].update(inode=SECRET), lambda r: r['tools'].update(unknown=None)]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                value = copy.deepcopy(good); mutate(value)
                with self.assertRaises(ValueError) as error: self.serialize(value)
                self.assertNotIn(SECRET, str(error.exception))

    def test_boolean_driver_exit_never_counts_as_actual_zero(self):
        error, row = self.execute(driver=False)
        self.assertIsInstance(error, RuntimeError)
        self.assertIsNone(row)


if __name__ == '__main__':
    unittest.main()
