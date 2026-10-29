"""Pure boundary/fail-closed tests; no Node, Playwright, subprocess or Core runtime."""
import copy
import ctypes
from unittest.mock import patch
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

PATH = Path(__file__).with_name('task-workspace-binding-real-ipc.py')
SPEC = importlib.util.spec_from_file_location('task_workspace_browser', PATH)
h = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(h)


class FixedBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.pin = {'canonical_root': '/private/generated/source', 'device': '9007199254740993',
                    'inode': '18446744073709551615', 'common_dir': '/private/generated/source/.git', 'head': 'a'*40}
        self.tasks = [{'id': 'task-one', 'revision': 1}, {'id': 'task-two', 'revision': 1}]
        self.b = h.Boundary({'id': 'project-one', 'revision': 1}, self.tasks, self.pin)
        self.p = {'task_id': 'task-one', 'state': 'needs_git_workspace', 'task_revision': 1,
                  'spec_revision': 1, 'project_revision': 1, 'preparation_key': 'code-prep:'+'b'*64,
                  'execution_authorized': False, 'creates_workspace': False}
        self.b.observe('workspaces.resolve', {'task_id': 'task-one'}, {'result': self.p})
        self.args = {'selection': {'task_id': 'task-one', 'expected_task_revision': 1,
                     'expected_spec_revision': 1, 'expected_project_revision': 1, 'host_id': 'local',
                     'expected_source_pin': self.pin, 'preparation_key': self.p['preparation_key']},
                     'idempotencyKey': 'original-key'}

    def refused(self, command, args):
        before = copy.deepcopy(self.b.writes)
        with self.assertRaises((ValueError, PermissionError, KeyError, TypeError)):
            self.b.route(command, args)
        self.assertEqual(self.b.writes, before)

    def test_exact_write_maps_six_fields_and_original_envelope_cas(self):
        method, body, key, rev = self.b.route('workspaces_prepare_task_git', self.args)
        self.assertEqual(method, 'workspaces.prepare_task_git')
        self.assertEqual(set(body), h.SELECTION_KEYS - {'expected_task_revision'})
        self.assertEqual((key, rev), ('original-key', 1))
        self.assertIs(type(body['expected_source_pin']['device']), str)
        self.assertEqual(body['expected_source_pin']['inode'], '18446744073709551615')

    def test_same_key_exact_historical_body_recovers_but_upgraded_body_does_not(self):
        first = self.b.route('workspaces_prepare_task_git', self.args)
        self.b.tasks['task-one'] = {'id': 'task-one', 'revision': 7}
        self.assertEqual(first, self.b.route('workspaces_prepare_task_git', copy.deepcopy(self.args)))
        for field, val in [('expected_task_revision', 7), ('expected_spec_revision', 2),
                           ('expected_project_revision', 2), ('host_id', 'remote')]:
            a = copy.deepcopy(self.args)
            a['selection'][field] = val
            self.refused('workspaces_prepare_task_git', a)
        a = copy.deepcopy(self.args)
        a['idempotencyKey'] = 'new-key'
        self.refused('workspaces_prepare_task_git', a)

    def test_unknown_methods_never_dispatch_owner_agent_or_generic_operations(self):
        for command in ('approvals_finalize', 'approvals_propose', 'approvals_decline', 'tasks_create',
                        'projects_import', 'sessions_create', 'workspaces_create', 'generic_rpc',
                        'factory_tool', 'dispatch', 'system.snapshot'):
            with self.subTest(command=command):
                self.refused(command, {})

    def test_write_extra_authority_destination_and_envelope_fields_refused(self):
        for field in ('grant', 'path', 'destination', 'method', 'params', 'expectedRevision'):
            for where in ('outer', 'selection'):
                a = copy.deepcopy(self.args)
                (a if where == 'outer' else a['selection'])[field] = 'untrusted'
                self.refused('workspaces_prepare_task_git', a)

    def test_foreign_task_unobserved_task_and_wrong_pin_refused(self):
        for task in ('foreign', 'task-two'):
            a = copy.deepcopy(self.args)
            a['selection']['task_id'] = task
            self.refused('workspaces_prepare_task_git', a)
        for field, val in [('device', 9007199254740993), ('inode', '1'), ('head', 'f'*40),
                           ('canonical_root', '/other'), ('common_dir', '/other/.git')]:
            a = copy.deepcopy(self.args)
            a['selection']['expected_source_pin'][field] = val
            self.refused('workspaces_prepare_task_git', a)

    def test_safe_integer_types_and_original_observation_required(self):
        for field in ('expected_task_revision', 'expected_spec_revision', 'expected_project_revision'):
            for val in (True, 1.0, 0, -1, 9007199254740992):
                a = copy.deepcopy(self.args)
                a['selection'][field] = val
                self.refused('workspaces_prepare_task_git', a)
        self.b.resolved.clear()
        self.refused('workspaces_prepare_task_git', self.args)

    def test_keys_require_nonempty_bounded_exact_string(self):
        for key in (None, True, '', 'k'*257, 'line\n', '\u00e9'*128):
            a = copy.deepcopy(self.args)
            a['idempotencyKey'] = key
            self.refused('workspaces_prepare_task_git', a)

    def test_fixed_read_requires_scope_and_never_write_envelope(self):
        self.assertEqual(self.b.route('tasks_get', {'taskId': 'task-two'}),
                         ('tasks.get', {'task_id': 'task-two'}, None, None))
        for args in ({'taskId': 'foreign'}, {'taskId': 'task-one', 'key': 'write'}, {}):
            self.refused('tasks_get', args)
        for command in h.READS:
            self.assertEqual(self.b.route(command, {})[0], h.READS[command])
            self.refused(command, {'method': 'write'})

    def test_resolver_strict_four_fields_local_original_task_project_cas(self):
        good = {'selection': {'task_id': 'task-one', 'expected_task_revision': 1,
                              'expected_project_revision': 1, 'host_id': 'local'}}
        self.assertEqual(self.b.route('workspaces_resolve', good)[0], 'workspaces.resolve')
        for field, val in [('host_id', 'remote'), ('expected_project_revision', 2),
                           ('expected_task_revision', 2), ('task_id', 'foreign'), ('path', '/tmp')]:
            a = copy.deepcopy(good)
            a['selection'][field] = val
            self.refused('workspaces_resolve', a)

    def test_reads_cannot_claim_creation_or_authority(self):
        for field in ('creates_workspace', 'execution_authorized'):
            p = copy.deepcopy(self.p)
            p[field] = True
            with self.assertRaises(ValueError):
                self.b.observe('workspaces.resolve', {'task_id': 'task-one'}, {'result': p})

    def test_origin_host_contenttype_and_json_bounds_are_independent(self):
        origin = 'http://127.0.0.1:4321'
        headers = {'Origin': origin, 'Host': '127.0.0.1:4321', 'Content-Type': 'application/json'}
        raw = b'{"command":"tasks_get","args":{"taskId":"task-one"}}'
        self.assertEqual(h.parse_http(raw, headers, origin)[0], 'tasks_get')
        for field, val in [('Origin', 'http://evil.invalid'), ('Host', 'localhost:4321'),
                           ('Content-Type', 'text/plain')]:
            bad = dict(headers)
            bad[field] = val
            with self.assertRaises(ValueError):
                h.parse_http(raw, bad, origin)
        for bad in (b'', b' '* (h.MAX_BODY+1), b'{"command":"x","command":"y"}',
                    b'{"command":"tasks_get","token":"bad"}', b'{"command":"x","args":{"n":NaN}}'):
            with self.assertRaises(ValueError):
                h.parse_http(bad, headers, origin)

    def test_late_handler_browser_cleanup_failure_never_keeps_passed(self):
        good = {'flow_complete': True, 'http_errors': [], 'page_errors': [], 'cleanup': {'complete': True},
                'checks': [{'id': f'WSI-B0{i}', 'status': 'passed'} for i in range(1, 5)]}
        self.assertEqual(h.seal_status(good), 0)
        for change in ({'http_errors': [{'stage': 'late_writer_failure'}]},
                       {'failure': {'stage': 'browser_close'}}, {'cleanup': {'complete': False}},
                       {'flow_complete': False}, {'checks': []}, {'page_errors': [{'kind': 'pageerror'}]}):
            r = copy.deepcopy(good)
            r.update(change)
            self.assertEqual(h.seal_status(r), 1)
            self.assertEqual(r['status'], 'failed')

    def test_strict_equality_is_not_python_bool_int_coercion(self):
        self.assertFalse(h.exact({'i': 1}, {'i': True}))
        self.assertFalse(h.exact({'i': '1'}, {'i': 1}))
        self.assertTrue(h.exact(self.pin, copy.deepcopy(self.pin)))

    def test_private_writer_exclusive_mode_and_no_rewrite(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / 'safe.json'
            h.private_json(path, {'status': 'NOT_RUN'})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(json.loads(path.read_text())['status'], 'NOT_RUN')
            with self.assertRaises(FileExistsError):
                h.private_json(path, {'status': 'passed'})


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.binary = root / 'batond'
        self.cli = root / 'baton'
        for path in (self.binary, self.cli):
            path.write_bytes(b'Pure metadata fixture; never executable code')
            path.chmod(0o500)
        self.front = root / 'dist'
        self.front.mkdir()
        (self.front / 'index.html').write_text('<div>Metadata only, never served</div>')
        self.manifest = root / 'manifest.json'
        self.build = root / 'build.json'
        self.data = {'schema': 16, 'git_head': h.BASE,
                     'binaries': {'batond': h.file_digest(self.binary)['sha256'],
                                  'baton': h.file_digest(self.cli)['sha256']}}
        self.build_data = {'source': h.BASE, 'exit': 0, 'source_head_and_tracked_unchanged': True,
                           'command': ['npm', '--prefix', 'apps/desktop', 'run', 'build']}
        self.args = SimpleNamespace(batond=self.binary, candidate_manifest=self.manifest,
                                    frontend=self.front, frontend_build_record=self.build)
        self.save()

    def save(self):
        self.manifest.write_text(json.dumps(self.data))
        self.build.write_text(json.dumps(self.build_data))

    def test_source_tool_and_frontend_build_are_separate_bound_inputs(self):
        binary, frontend, result = h.preflight(self.args)
        self.assertEqual(binary, self.binary)
        self.assertEqual(frontend, self.front)
        self.assertEqual(result['compiled_core_source_head'], h.BASE)
        self.assertEqual(result['frontend_build_head'], h.BASE)
        self.assertEqual(result['frontend_assets']['index.html']['sha256'], h.sha((self.front / 'index.html').read_bytes()))

    def test_schema_float_bool_old_future_and_unmatched_source_are_refused(self):
        for schema in (16.0, True, 15, 17):
            self.data['schema'] = schema
            self.save()
            with self.assertRaises(ValueError):
                h.preflight(self.args)
        self.data['schema'] = 16
        self.data['git_head'] = 'a'*40
        self.save()
        with self.assertRaises(ValueError):
            h.preflight(self.args)

    def test_wrong_binary_and_failed_or_foreign_build_record_refused(self):
        self.data['binaries']['batond'] = 'a'*64
        self.save()
        with self.assertRaises(ValueError):
            h.preflight(self.args)
        self.data['binaries']['batond'] = h.file_digest(self.binary)['sha256']
        for field, value in [('source', 'f'*40), ('exit', 1), ('exit', False),
                             ('source_head_and_tracked_unchanged', False)]:
            old = self.build_data[field]
            self.build_data[field] = value
            self.save()
            with self.assertRaises(ValueError):
                h.preflight(self.args)
            self.build_data[field] = old

    def test_symlink_frontend_asset_refused_without_following(self):
        (self.front / 'external').symlink_to(self.manifest)
        with self.assertRaises(ValueError):
            h.preflight(self.args)


class ManagedFixturePathTests(unittest.TestCase):
    """Actual self-owned temp FS guards; not actual worktrees/Core receipts."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.runtime = self.root / 'runtime'
        self.runtime.mkdir(mode=0o700)
        self.managed = self.runtime / 'workspaces'
        self.managed.mkdir(mode=0o700)
        self.work = self.managed / 'workspace-one'
        self.work.mkdir(mode=0o700)
        (self.work / 'input.txt').write_bytes(b'committed source\n')
        (self.work / '.git').write_bytes(b'gitfile metadata only; no Git execution')
        self.external = self.root / 'surrogate-external'
        self.external.mkdir(mode=0o700)
        (self.external / 'preserve').write_bytes(b'outside surrogate must not change')
        self.pin = {'canonical_root': str(self.root / 'source'), 'device': '1', 'inode': '2',
                    'common_dir': str(self.root / 'source/.git'), 'head': 'a'*40}
        info = self.work.stat()
        self.binding = {'state': 'ready', 'receipt': {'task_id': 'task-one', 'project_id': 'project-one',
                         'host_id': 'local', 'source_pin': self.pin, 'workspace_id': self.work.name,
                         'managed_root': str(self.managed)},
                        'managed_pin': {'canonical_root': str(self.work), 'device': str(info.st_dev),
                                        'inode': str(info.st_ino), 'common_dir': self.pin['common_dir'], 'head': 'a'*40}}
        self.guard = h.ManagedFixturePaths(self.runtime, 'project-one', self.pin)

    def unchanged(self):
        self.assertEqual((self.external / 'preserve').read_bytes(), b'outside surrogate must not change')
        self.assertFalse((self.external / 'own-output.txt').exists())
        self.assertFalse(self.external.with_name(self.external.name + '-retained-fixture').exists())
        self.assertEqual((self.work / 'input.txt').read_bytes(), b'committed source\n')

    def reject_binding(self, bad):
        for action in (self.guard.workspace, self.guard.committed_input,
                       self.guard.write_output, self.guard.replace_for_drift):
            with self.assertRaises((ValueError, FileNotFoundError, NotADirectoryError)):
                action(bad, 'task-one')
            self.unchanged()

    def test_same_core_shaped_external_path_cannot_authorize_any_read_write_or_rename(self):
        bad = copy.deepcopy(self.binding)
        info = self.external.stat()
        bad['managed_pin'].update(canonical_root=str(self.external), device=str(info.st_dev), inode=str(info.st_ino))
        self.reject_binding(bad)
        bad['receipt']['managed_root'] = str(self.external.parent)
        bad['receipt']['workspace_id'] = self.external.name
        self.reject_binding(bad)

    def test_wrong_task_project_source_host_or_workspace_component_refuses(self):
        for field, value in [('task_id', 'foreign'), ('project_id', 'foreign'), ('host_id', 'remote'),
                             ('workspace_id', '../surrogate-external'), ('workspace_id', '.'),
                             ('managed_root', str(self.external))]:
            bad = copy.deepcopy(self.binding)
            bad['receipt'][field] = value
            self.reject_binding(bad)
        bad = copy.deepcopy(self.binding)
        bad['receipt']['source_pin']['head'] = 'b'*40
        self.reject_binding(bad)

    def test_pin_requires_exact_canonical_string_identity(self):
        for field, value in [('device', self.work.stat().st_dev), ('inode', self.work.stat().st_ino),
                             ('inode', '0'+str(self.work.stat().st_ino)), ('inode', '1'),
                             ('common_dir', '/foreign/.git')]:
            bad = copy.deepcopy(self.binding)
            bad['managed_pin'][field] = value
            self.reject_binding(bad)

    def test_symlink_workspace_root_never_follows_external_target(self):
        retained = self.work.with_name('original-private')
        self.work.rename(retained)
        self.work.symlink_to(self.external, target_is_directory=True)
        for action in (self.guard.workspace, self.guard.committed_input, self.guard.write_output, self.guard.replace_for_drift):
            with self.assertRaises(ValueError):
                action(self.binding, 'task-one')
        self.assertEqual((self.external / 'preserve').read_bytes(), b'outside surrogate must not change')
        self.assertFalse((self.external / 'own-output.txt').exists())
        self.assertEqual((retained / 'input.txt').read_bytes(), b'committed source\n')

    def test_symlink_or_replaced_managed_parent_is_rejected_before_leaf_access(self):
        self.managed.rename(self.runtime / 'original-managed')
        self.managed.symlink_to(self.external, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.guard.workspace(self.binding, 'task-one')
        self.assertFalse((self.external / 'own-output.txt').exists())
        self.assertEqual((self.external / 'preserve').read_bytes(), b'outside surrogate must not change')

    def test_exclusive_output_and_actual_atomic_private_drift_positive(self):
        self.guard.committed_input(self.binding, 'task-one')
        self.assertTrue(self.guard.output_absent(self.binding, 'task-one'))
        self.guard.write_output(self.binding, 'task-one')
        self.assertFalse(self.guard.output_absent(self.binding, 'task-one'))
        with self.assertRaises(FileExistsError):
            self.guard.write_output(self.binding, 'task-one')
        self.assertEqual((self.work / 'own-output.txt').read_bytes(), b'only Task one')
        retained = self.guard.replace_for_drift(self.binding, 'task-one')
        self.assertEqual(retained.parent, self.managed)
        self.assertEqual((retained / 'input.txt').read_bytes(), b'committed source\n')
        self.assertEqual(list(self.work.iterdir()), [])
        with self.assertRaises(ValueError):
            self.guard.workspace(self.binding, 'task-one')
        self.assertEqual((self.external / 'preserve').read_bytes(), b'outside surrogate must not change')

    def test_existing_retained_directory_and_symlink_refused_without_overwrite(self):
        retained = self.work.with_name(self.work.name + '-retained-fixture')
        retained.mkdir()
        (retained / 'preserve').write_bytes(b'existing retained')
        with self.assertRaises(ValueError):
            self.guard.replace_for_drift(self.binding, 'task-one')
        self.assertEqual((retained / 'preserve').read_bytes(), b'existing retained')
        self.unchanged()
        (retained / 'preserve').unlink()
        retained.rmdir()
        retained.symlink_to(self.external, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.guard.replace_for_drift(self.binding, 'task-one')
        self.unchanged()

    def test_retained_created_after_precheck_is_still_atomically_not_overwritten(self):
        retained = self.work.with_name(self.work.name + '-retained-fixture')
        real = ctypes.CDLL(None, use_errno=True).renameat2
        real.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        real.restype = ctypes.c_int
        class InjectConcurrentDestination:
            def __call__(_, *args):
                retained.mkdir()
                (retained / 'preserve').write_bytes(b'late retained')
                return real(*args)
        with patch.object(h.ctypes, 'CDLL', return_value=SimpleNamespace(renameat2=InjectConcurrentDestination())):
            with self.assertRaises(OSError):
                self.guard.replace_for_drift(self.binding, 'task-one')
        self.assertEqual((retained / 'preserve').read_bytes(), b'late retained')
        self.unchanged()

    def test_managed_input_symlink_and_preexisting_output_symlink_are_refused(self):
        (self.work / 'input.txt').unlink()
        (self.work / 'input.txt').symlink_to(self.external / 'preserve')
        with self.assertRaises(OSError):
            self.guard.committed_input(self.binding, 'task-one')
        (self.work / 'own-output.txt').symlink_to(self.external / 'preserve')
        with self.assertRaises(FileExistsError):
            self.guard.write_output(self.binding, 'task-one')
        self.assertEqual((self.external / 'preserve').read_bytes(), b'outside surrogate must not change')


class ExpectedSourceHeadTests(unittest.TestCase):
    # Reuse only the inert filesystem fixture setup, not duplicate its test cases.
    def setUp(self):
        PreflightTests.setUp(self)

    save = PreflightTests.save

    def test_default_absent_attribute_preserves_original840_selection(self):
        self.assertFalse(hasattr(self.args, 'expected_source_head'))
        result = h.preflight(self.args)[2]
        self.assertEqual(result['expected_source_head'], h.BASE)
        self.assertEqual(result['compiled_core_source_head'], h.BASE)
        self.assertEqual(result['frontend_build_head'], h.BASE)

    def test_explicit_same_source_and_equal_aliases_accepted(self):
        self.args.expected_source_head = h.BASE
        self.data.update(source_head=h.BASE, compiled_core_source_head=h.BASE, tracked_clean=True, dirty=False)
        self.build_data.update(tracked_clean=True, dirty=False)
        self.save()
        self.assertEqual(h.preflight(self.args)[2]['expected_source_head'], h.BASE)

    def test_explicit_new_source_requires_both_actual_records_and_is_recorded(self):
        expected = '0123456789abcdef0123456789abcdef01234567'
        self.args.expected_source_head = expected
        self.data['git_head'] = expected
        self.build_data['source'] = expected
        self.save()
        result = h.preflight(self.args)[2]
        self.assertEqual(result['expected_source_head'], expected)
        self.assertEqual(result['compiled_core_source_head'], expected)
        self.assertEqual(result['frontend_build_head'], expected)
        self.assertEqual(h.BASE, '840334e2507c7b2b39e89917b491e75c1ca3de18')

    def test_expected_head_requires_exact_lowercase40_not_prefix_or_typed_coercion(self):
        for expected in (None, True, 1, 1.0, '', 'a'*39, 'a'*41, 'A'*40,
                         'a'*40+'\n', 'a'*40+'\r', 'a'*40+'\u2028', 'a'*40+'\u2029'):
            self.args.expected_source_head = expected
            with self.subTest(expected=repr(expected)):
                with self.assertRaises(ValueError):
                    h.preflight(self.args)

    def test_default_or_explicit_cannot_mix_core_and_frontend_heads(self):
        for core, frontend, expected in ((h.BASE, 'b'*40, h.BASE), ('b'*40, h.BASE, h.BASE),
                                         (h.BASE, h.BASE, 'b'*40), ('b'*40, h.BASE, 'b'*40)):
            self.data['git_head'] = core
            self.build_data['source'] = frontend
            self.args.expected_source_head = expected
            self.save()
            with self.assertRaises(ValueError):
                h.preflight(self.args)

    def test_missing_source_conflicting_or_invalid_alias_is_refused(self):
        self.args.expected_source_head = h.BASE
        for alias in ('b'*40, None, True, h.BASE+'\n'):
            self.data['source_head'] = alias
            self.save()
            with self.assertRaises(ValueError):
                h.preflight(self.args)
        self.data.pop('source_head')
        self.data.pop('git_head')
        self.save()
        with self.assertRaises(ValueError):
            h.preflight(self.args)
        self.data['compiled_core_source_head'] = h.BASE
        self.save()
        self.assertEqual(h.preflight(self.args)[2]['compiled_core_source_head'], h.BASE)
        self.data['git_head'] = h.BASE
        self.data['compiled_core_source_head'] = 'b'*40
        self.save()
        with self.assertRaises(ValueError):
            h.preflight(self.args)

    def test_dirty_and_untyped_cleanliness_claims_are_not_accepted(self):
        self.args.expected_source_head = h.BASE
        for target in (self.data, self.build_data):
            for field, value in (('dirty', True), ('dirty', 0), ('dirty', None),
                                  ('tracked_clean', False), ('tracked_clean', 1), ('tracked_clean', None)):
                target[field] = value
                self.save()
                with self.subTest(field=field, value=value):
                    with self.assertRaises(ValueError):
                        h.preflight(self.args)
                target.pop(field)

    def test_frontend_must_keep_actual_success_unchanged_and_exact_command_record(self):
        self.args.expected_source_head = h.BASE
        for field, value in (('source', None), ('exit', False), ('exit', 0.0), ('exit', 1),
                              ('source_head_and_tracked_unchanged', False),
                              ('source_head_and_tracked_unchanged', 1), ('command', [])):
            old = self.build_data[field]
            self.build_data[field] = value
            self.save()
            with self.assertRaises(ValueError):
                h.preflight(self.args)
            self.build_data[field] = old
        self.build.unlink()
        with self.assertRaises(FileNotFoundError):
            h.preflight(self.args)


    def test_real_cli_parser_default_and_explicit_preflight_never_launch_runtime(self):
        for i, head in enumerate((h.BASE, 'd'*40)):
            self.data['git_head'] = head
            self.build_data['source'] = head
            self.save()
            output = self.binary.parent / f'preflight-only-{i}'
            argv = ['harness', '--batond', str(self.binary), '--candidate-manifest', str(self.manifest),
                    '--frontend', str(self.front), '--frontend-build-record', str(self.build),
                    '--output', str(output)]
            if i:
                argv += ['--expected-source-head', head]
            with patch('sys.argv', argv), patch.object(h, 'run', side_effect=AssertionError('Runtime forbidden in this pure control')) as runtime:
                self.assertEqual(h.main(), 2)
                runtime.assert_not_called()
            record = json.loads((output / 'preflight.json').read_text())
            self.assertEqual(record['status'], 'NOT_RUN')
            self.assertEqual(record['inputs']['expected_source_head'], head)
            self.assertEqual(record['inputs']['compiled_core_source_head'], head)
            self.assertEqual(record['inputs']['frontend_build_head'], head)


if __name__ == '__main__':
    unittest.main()
