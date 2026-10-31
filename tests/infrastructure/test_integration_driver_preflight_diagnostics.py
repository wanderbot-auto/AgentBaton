"""Synthetic preflight/strict publication controls; no actual runtime or SQL."""
import ast
import builtins
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / 'apps/desktop/tests/integration-real-ipc.py'
PROJECTOR = ROOT / 'scripts/record-integration-browser-ci-evidence.py'
FIXTURES = ROOT / 'tests/infrastructure/fixtures/integration_driver_preflight_diagnostics'
HEAD, SECRET = 'a' * 40, 'PRIVATE_TOKEN_PATH_REASON_CANARY'


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    sys.modules[name] = value
    spec.loader.exec_module(value)
    return value


D = module('_preflight_driver_controls', DRIVER)
E = module('_preflight_projection_controls', PROJECTOR)
UNAVAILABLE = {'status': 'unavailable', 'failure_kind': 'missing_or_invalid_driver_preflight_diagnostic'}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def write(path, value):
    path.write_bytes(value if type(value) is bytes else json.dumps(value, allow_nan=False).encode())
    path.chmod(0o600)


def argv(root, *, run=True):
    for name in ('batond', 'baton', 'chromium', 'python'):
        p = root / name; p.write_bytes(('synthetic-' + name).encode()); p.chmod(0o500)
    frontend = root / 'frontend'; frontend.mkdir(); (frontend / 'index.html').write_text('synthetic frontend')
    manifest = root / 'candidate.json'
    write(manifest, {'schema': 17, 'git_head': HEAD, 'dirty': False,
        'binaries': {k: sha((root / k).read_bytes()) for k in ('batond', 'baton')}})
    build = root / 'build.json'
    write(build, {'source': HEAD, 'exit': 0, 'command': ['npm', '--prefix', 'apps/desktop', 'run', 'build'],
        'source_head_and_tracked_unchanged': True})
    return ['integration-real-ipc.py', '--batond', str(root / 'batond'), '--candidate-manifest', str(manifest),
        '--frontend', str(frontend), '--frontend-build-record', str(build), '--expected-source-head', HEAD,
        '--chromium', str(root / 'chromium'), '--output', str(root / 'output'), *(['--run'] if run else [])]


def fixture_main(root, args, *, stage=None, error=None, import_failure=False, run_result=True):
    original_mark, original_import = D.driver_preflight_stage, builtins.__import__
    def mark(value, current):
        original_mark(value, current)
        if current == stage:
            raise error if error is not None else ValueError(SECRET)
    def importing(name, *a, **kw):
        if name == 'playwright.sync_api':
            if import_failure:
                raise ImportError(SECRET)
            return types.SimpleNamespace(sync_playwright=lambda: None)
        return original_import(name, *a, **kw)
    # The real preflight runs against synthetic regular files. Only the Git
    # source probe and dependency import are replaced; no process/SQL may run.
    with mock.patch.object(sys, 'argv', args), mock.patch.object(sys, 'executable', str(root / 'python')), \
        mock.patch.object(D, 'driver_preflight_stage', side_effect=mark), \
        mock.patch.object(D, 'source_guard', return_value={'head': HEAD, 'tracked_clean': True,
            'driver_sha256': sha(DRIVER.read_bytes())}), \
        mock.patch.object(builtins, '__import__', side_effect=importing), \
        mock.patch.object(D, 'run_case', return_value=run_result) as cases, \
        mock.patch.object(sqlite3, 'connect', side_effect=AssertionError('No SQL in controls')), \
        mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('No runtime in controls')), \
        mock.patch.object(subprocess, 'run', side_effect=AssertionError('No process in controls')):
        result = D.main()
    return result, cases.call_count


def seed(root, stage='source_guard', exception='refusal', run=True):
    private = root / 'private'; private.mkdir(mode=0o700)
    driver = private / 'driver'; driver.mkdir(mode=0o700)
    checks = dict.fromkeys(D.DRIVER_RUNTIME_CHECKS)
    if D.DRIVER_PREFLIGHT_STAGES.index(stage) >= D.DRIVER_PREFLIGHT_STAGES.index('runtime_owner_mode_guard'):
        checks = dict.fromkeys(checks, True)
    value = {'version': 1, 'declared_source': HEAD, 'driver_sha256': E.expected_driver(),
        'stage': stage, 'exception_kind': exception, 'run_requested': run,
        'runtime_case_entered': False, 'runtime_checks': checks}
    write(private / 'status.json', {'source': HEAD, 'driver_exit': 2, 'probe_passed': True})
    write(driver / 'preflight-diagnostic.json', value)
    write(driver / 'report.json', {'token': SECRET, 'traceback': SECRET})
    return private, value


class ActualPreflightStageControls(unittest.TestCase):
    def test_argument_parse_refusal_remains_before_any_diagnostic_or_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); args = argv(root)
            with mock.patch.object(sys, 'argv', args + ['--unknown-control']), \
                mock.patch.object(sys, 'stderr', io.StringIO()), mock.patch.object(D, 'run_case') as cases:
                with self.assertRaises(SystemExit) as error:
                    D.main()
            self.assertEqual(error.exception.code, 2); cases.assert_not_called()
            self.assertFalse((root / 'output').exists())

    def test_original_success_values_and_actual_pin_order_are_identical(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); argv(root)
            args = types.SimpleNamespace(expected_source_head=HEAD, batond=root / 'batond',
                candidate_manifest=root / 'candidate.json', frontend=root / 'frontend',
                frontend_build_record=root / 'build.json', chromium=root / 'chromium', run=True)
            pin = D.pinned_file; calls = []
            def observed(path, *a):
                calls.append((path, a))
                return pin(path, *a)
            with mock.patch.object(sys, 'executable', str(root / 'python')), \
                mock.patch.object(D, 'source_guard', return_value={'head': HEAD}), \
                mock.patch.object(D, 'pinned_file', side_effect=observed):
                namespace = dict(D.__dict__)
                exec(compile((FIXTURES / 'original_preflight.py.txt').read_text(), '<exact-original-preflight>', 'exec'), namespace)
                original = namespace['preflight'](copy.deepcopy(args)); old_calls = list(calls); calls.clear()
                actual = D.preflight(copy.deepcopy(args), D.new_driver_preflight_diagnostic(args))
                self.assertTrue(D.exact(original, actual))
                self.assertEqual(calls, old_calls)

    def test_all_preflight_stages_keep_exit2_original_private_report_and_no_runtime(self):
        for stage in D.DRIVER_PREFLIGHT_STAGES[:-2]:
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); args = argv(root)
                result, calls = fixture_main(root, args, stage=stage)
                self.assertEqual((result, calls), (2, 0))
                output = root / 'output'
                original = json.loads((output / 'report.json').read_bytes())
                self.assertEqual(original, {'status': 'preflight_refused', 'actual_runtime': 'NOT_RUN',
                    'kind': 'ValueError', 'no_process_token_or_listener': True})
                value = json.loads((output / 'preflight-diagnostic.json').read_bytes())
                self.assertEqual((value['stage'], value['exception_kind']), (stage, 'value_error'))
                self.assertIs(value['runtime_case_entered'], False)
                self.assertNotIn(SECRET, json.dumps(value)); self.assertNotIn(str(root), json.dumps(value))
                self.assertEqual((output / 'preflight-diagnostic.json').stat().st_mode & 0o777, 0o600)

    def test_missing_run_and_dependency_import_are_separate_and_neither_enters_cases(self):
        for run in (False, True):
            with self.subTest(run=run), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); args = argv(root, run=run)
                result, calls = fixture_main(root, args, import_failure=True)
                self.assertEqual((result, calls), (2, 0))
                value = json.loads((root / 'output/preflight-diagnostic.json').read_bytes())
                self.assertEqual(value['stage'], 'playwright_import' if run else 'run_required')
                self.assertEqual(value['exception_kind'], 'import_error' if run else None)
                self.assertEqual(value['run_requested'], run)
                self.assertTrue(all(v is True for v in value['runtime_checks'].values()))
                self.assertNotIn(SECRET, json.dumps(value))

    def test_real_runtime_pin_mode_guard_is_not_relaxed_and_facts_are_booleans(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); args = argv(root); (root / 'python').chmod(0o722)
            result, calls = fixture_main(root, args)
            self.assertEqual((result, calls), (2, 0))
            value = json.loads((root / 'output/preflight-diagnostic.json').read_bytes())
            self.assertEqual((value['stage'], value['exception_kind']), ('runtime_owner_mode_guard', 'refusal'))
            self.assertIs(value['runtime_checks']['python_mode_allowed'], False)
            self.assertTrue(all(type(v) is bool for v in value['runtime_checks'].values()))

    def test_closed_exception_categories_never_copy_class_names_messages_or_paths(self):
        for error, tag in [(D.Refusal(SECRET), 'refusal'), (ImportError(SECRET), 'import_error'),
            (OSError(SECRET), 'os_error'), (ValueError(SECRET), 'value_error'), (KeyError(SECRET), 'key_error'),
            (TypeError(SECRET), 'type_error'), (AttributeError(SECRET), 'attribute_error'),
            (KeyboardInterrupt(SECRET), 'interrupted'), (SystemExit(SECRET), 'system_exit'),
            (RuntimeError(SECRET), 'other_exception'),
            (type(SECRET, (RuntimeError,), {})(SECRET), 'other_exception')]:
            with self.subTest(tag=tag), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); args = argv(root)
                result, calls = fixture_main(root, args, stage='candidate_decode', error=error)
                self.assertEqual((result, calls), (2, 0))
                raw = (root / 'output/preflight-diagnostic.json').read_text()
                self.assertEqual(json.loads(raw)['exception_kind'], tag); self.assertNotIn(SECRET, raw)

    def test_runtime_results_never_create_a_no_runtime_preflight_record(self):
        for passed in (False, True):
            with self.subTest(passed=passed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); args = argv(root)
                result, calls = fixture_main(root, args, run_result=passed)
                self.assertEqual(result, 0 if passed else 1); self.assertEqual(calls, 2)
                self.assertFalse((root / 'output/preflight-diagnostic.json').exists())

    def test_diagnostic_io_failure_does_not_change_original_preflight_exit_or_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); args = argv(root)
            original = D.private_json
            def refusing(path, value):
                if path.name == 'preflight-diagnostic.json':
                    raise OSError(SECRET)
                return original(path, value)
            with mock.patch.object(D, 'private_json', side_effect=refusing):
                self.assertEqual(fixture_main(root, args, stage='candidate_decode'), (2, 0))
            self.assertTrue((root / 'output/report.json').is_file())
            self.assertFalse((root / 'output/preflight-diagnostic.json').exists())

    def test_private_diagnostic_symlink_collision_never_overwrites_or_changes_exit2(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); args = argv(root); outside = root / 'outside'
            outside.write_text(SECRET); original = D.private_json
            def collision(path, value):
                result = original(path, value)
                if path.name == 'report.json':
                    (path.parent / 'preflight-diagnostic.json').symlink_to(outside)
                return result
            with mock.patch.object(D, 'private_json', side_effect=collision):
                self.assertEqual(fixture_main(root, args, stage='candidate_decode'), (2, 0))
            self.assertEqual(outside.read_text(), SECRET)
            self.assertEqual(json.loads((root / 'output/report.json').read_bytes())['status'], 'preflight_refused')


class FinitePreflightProjectionControls(unittest.TestCase):
    def test_every_closed_stage_projects_without_raw_private_report_and_stays_failed(self):
        for stage in D.DRIVER_PREFLIGHT_STAGES:
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                exception = None if stage == 'run_required' else 'import_error' if stage == 'playwright_import' else 'refusal'
                private, _ = seed(root, stage, exception, stage != 'run_required')
                self.assertEqual(E.main(['--private', str(private), '--output', str(root / 'public'),
                    '--expected-source-head', HEAD, '--driver-step-outcome', 'failure']), 1)
                self.assertEqual({p.name for p in (root / 'public').iterdir()}, {'summary.json'})
                raw = (root / 'public/summary.json').read_text(); value = json.loads(raw)
                self.assertEqual(value['driver_preflight']['stage'], stage)
                self.assertEqual(value['status'], 'failed'); self.assertIs(value['screenshots_published'], False)
                self.assertNotIn(SECRET, raw); self.assertNotIn(str(private), raw)

    def test_unknown_extra_wrong_types_source_hash_outcome_or_inconsistent_checks_are_unavailable(self):
        changes = [lambda v: v.update(stage=SECRET), lambda v: v.update(exception_kind=SECRET),
            lambda v: v.update(token=SECRET), lambda v: v.update(version=True),
            lambda v: v.update(declared_source='f' * 40), lambda v: v.update(driver_sha256='f' * 64),
            lambda v: v.update(runtime_case_entered=0), lambda v: v.update(runtime_case_entered=True),
            lambda v: v.update(run_requested=1), lambda v: v.update(exception_kind=None),
            lambda v: v['runtime_checks'].update(token=SECRET),
            lambda v: v['runtime_checks'].update(python_owner_allowed=1),
            lambda v: v['runtime_checks'].update(python_owner_allowed=False),
            lambda v: v.update(stage='runtime_python_pin')]
        for change in changes:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, value = seed(root)
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD)['status'], 'observed')
                change(value); write(private / 'driver/preflight-diagnostic.json', value)
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD), UNAVAILABLE)
        for stage, exception, run in [('run_required', 'refusal', False), ('run_required', None, True),
            ('playwright_import', 'value_error', True), ('playwright_import', 'import_error', False)]:
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                private, _ = seed(Path(temporary).resolve(), stage, exception, run)
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD), UNAVAILABLE)

    def test_status_exit_bool_other_exit_or_probe_not_passed_never_claims_no_runtime(self):
        for patch in [{'driver_exit': True}, {'driver_exit': 0}, {'driver_exit': 1},
            {'driver_exit': None}, {'probe_passed': False}, {'source': 'f' * 40}, {'extra': SECRET}]:
            with self.subTest(patch=patch), tempfile.TemporaryDirectory() as temporary:
                private, _ = seed(Path(temporary).resolve())
                status = json.loads((private / 'status.json').read_bytes()); status.update(patch)
                write(private / 'status.json', status)
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD), UNAVAILABLE)

    def test_missing_duplicate_symlink_hardlink_fifo_mode_oversize_or_alias_directory_refuse(self):
        for kind in ('missing', 'duplicate', 'symlink', 'hardlink', 'fifo', 'mode', 'oversize', 'alias'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); private, _ = seed(root); path = private / 'driver/preflight-diagnostic.json'
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD)['status'], 'observed')
                if kind == 'missing': path.unlink()
                elif kind == 'duplicate':
                    path.write_bytes(path.read_bytes().replace(b'"version": 1', b'"version": 1,"version": 1', 1))
                elif kind == 'symlink':
                    outside = root / 'outside'; path.rename(outside); path.symlink_to(outside)
                elif kind == 'hardlink': os.link(path, root / 'alias')
                elif kind == 'fifo': path.unlink(); os.mkfifo(path, 0o600)
                elif kind == 'mode': path.chmod(0o644)
                elif kind == 'oversize': path.write_bytes(b' ' * 4097)
                else:
                    (private / 'driver').rename(private / 'actual-driver')
                    (private / 'driver').symlink_to(private / 'actual-driver', target_is_directory=True)
                self.assertEqual(E.driver_preflight_diagnostic(private, HEAD), UNAVAILABLE)

    def test_status_or_diagnostic_changed_after_read_is_not_publishable(self):
        for name in ('status.json', 'preflight-diagnostic.json'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                private, _ = seed(Path(temporary).resolve()); original = E.P.regular_bytes
                def changing(fd, current, *a, **kw):
                    raw = original(fd, current, *a, **kw)
                    if current == name:
                        path = private / current if current == 'status.json' else private / 'driver' / current
                        path.write_bytes(raw + b' ')
                    return raw
                with mock.patch.object(E.P, 'regular_bytes', side_effect=changing):
                    self.assertEqual(E.driver_preflight_diagnostic(private, HEAD), UNAVAILABLE)


class ExactOriginalSourceControls(unittest.TestCase):
    def test_closed_stage_exception_and_check_names_match_independent_projector(self):
        self.assertEqual(D.DRIVER_PREFLIGHT_STAGES, E.DRIVER_PREFLIGHT_STAGES)
        self.assertEqual(D.DRIVER_PREFLIGHT_EXCEPTIONS, E.DRIVER_PREFLIGHT_EXCEPTIONS)
        self.assertEqual(set(D.DRIVER_RUNTIME_CHECKS), E.DRIVER_RUNTIME_CHECKS)

    def restore_driver(self, text):
        start, end = text.index('DRIVER_PREFLIGHT_STAGES = '), text.index('def preflight(')
        self.assertEqual(sha(text[start:end].encode()), '51cc52c0a3c2c42e03288a1933f3f7091c1fbaa3a0b8e1c6015d9f992f3bab99')
        text = text[:start] + text[end:]
        for name, new_hash, old_hash in [('preflight', '4dfa6e2de32ef3c7694c3106aff8cee89913ef03fb1ab359b3f39d3777c1d909',
            'a829d3b9185b174ac137a972f77a17ef4eb918acc52603593b256d74af2f7de2'),
            ('main', 'ab5a5c5e683102bcea96122510010565c4ae17bb6d9ab121dc4209edb561bf26',
            'd2b170c7bdb5e22983ff006070887169a2c8fdcc48b329d0a2fb2938af54a7fe')]:
            node = next(n for n in ast.parse(text).body if isinstance(n, ast.FunctionDef) and n.name == name)
            changed = ast.get_source_segment(text, node); original = (FIXTURES / ('original_' + name + '.py.txt')).read_bytes()
            self.assertEqual(sha(changed.encode()), new_hash); self.assertEqual(sha(original), old_hash)
            self.assertEqual(text.count(changed), 1); text = text.replace(changed, original.decode(), 1)
        return text

    def test_full_original_driver_and_projector_restore_exactly_no_region_ignored(self):
        self.assertEqual(sha(self.restore_driver(DRIVER.read_text()).encode()),
            '1e34768c2d79314163080300e741ed3a98a9ba64752f0b38152852b15028bd1e')
        text = PROJECTOR.read_text(); start, end = text.index('DRIVER_PREFLIGHT_STAGES = '), text.index('def publish(')
        self.assertEqual(sha(text[start:end].encode()), '4911cd73dde7031b417fa35ae14d2eddd902d58eb780860254170ca65f43eac1')
        text = text[:start] + text[end:]
        addition = "            summary['driver_preflight'] = driver_preflight_diagnostic(args.private, args.expected_source_head)\n"
        self.assertEqual(text.count(addition), 1); text = text.replace(addition, '', 1)
        self.assertEqual(sha(text.encode()), '0f1de4bc3e2ff73bd88764069d3b5b345a3c8af5b2c4ff36259ad5c85e10a167')

    def test_all_original_guard_expressions_and_pin_call_arguments_are_unchanged(self):
        before = ast.parse((FIXTURES / 'original_preflight.py.txt').read_text()).body[0]
        after = next(n for n in ast.parse(DRIVER.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == 'preflight')
        def calls(node, name):
            return [ast.dump(n, include_attributes=False) for n in ast.walk(node)
                if isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id == name]
        self.assertEqual(calls(before, 'require'), calls(after, 'require'))
        # Dict values originally evaluated Chromium then Python; the split
        # statements retain those exact actual pin calls and that order.
        self.assertCountEqual(calls(before, 'pinned_file'), calls(after, 'pinned_file'))
        self.assertEqual(calls(before, 'source_guard'), calls(after, 'source_guard'))

    def test_inverse_never_hides_guard_size_source_or_unknown_outside_changes(self):
        source = DRIVER.read_text()
        original = '1e34768c2d79314163080300e741ed3a98a9ba64752f0b38152852b15028bd1e'
        self.assertEqual(sha(self.restore_driver(source).encode()), original)
        changes = [('MAX_CHROMIUM_IMAGE = 536870912', 'MAX_CHROMIUM_IMAGE = 1073741824'),
            ('def pinned_file(path, limit=268435456)', 'def pinned_file(path, limit=536870912)'),
            ("driver_preflight_stage(diagnostic, 'source_guard')", "driver_preflight_stage(diagnostic, 'frontend_assets')"),
            ("'--run', action='store_true'", "'--run', action='store_false'"),
            ("MAX_BODY, MAX_REPLY, MAX_REVIEW = 270336, 4194304, 1048576", "MAX_BODY, MAX_REPLY, MAX_REVIEW = 270336, 4194304, 2097152")]
        for old, new in changes:
            self.assertIn(old, source)
            with self.subTest(old=old), self.assertRaises(AssertionError):
                self.assertEqual(sha(self.restore_driver(source.replace(old, new, 1)).encode()), original)


if __name__ == '__main__':
    unittest.main()
