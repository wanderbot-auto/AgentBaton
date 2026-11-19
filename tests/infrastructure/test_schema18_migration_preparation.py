"""Finite provenance/FS/archive and local missing-object Git controls; no network/compiler/product."""
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('build18_controls', ROOT / 'scripts/prepare-schema18-migration-fixture.py')
H = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(H)


def synthetic_build():
    files = {'Cargo.lock': H.LOCK_SHA256, 'Cargo.toml': 'a' * 64}
    files.update({'crates/synthetic' + str(i) + '.rs': 'b' * 64 for i in range(242)})
    record = dict(contract=H.CONTRACT, status='compiled', schema=18, platform=sys.platform,
        architecture=platform.machine(), builder_sha256=H.sha(Path(H.__file__).read_bytes()),
        helpers_sha256=H.HELPERS.copy(),
        source=dict(commit=H.SOURCE, tree=H.TREE, repository=H.REPOSITORY,
                    archive_sha256=H.ARCHIVE_SHA256, files_sha256=files, compiled_source_clean=True,
                    directory='/never-executed/source'),
        build=dict(argv=['/never-executed/cargo', *H.BUILD_ARGS], features=[], default_features=True,
                   exit_code=0, lockfile_sha256=H.LOCK_SHA256, group_quiescent_before_reap=True,
                   cwd='/never-executed/source', target_dir='/never-executed/target',
                   owned_identity=dict(pid=4321001, pgid=4321001),
                   cargo_config_paths_checked=H.cargo_config_paths(Path('/never-executed/source')),
                   cargo_config_absent_before=True, cargo_config_absent_after=True),
        binaries={name: dict(sha256='c' * 64, bytes=100, mode='0o500', nlink=1,
                            architecture=platform.machine()) for name in ('batond', 'baton')},
        toolchain={name: dict(path='/never-executed/' + name, sha256='d' * 64,
                             version=name + ' 1.91.1') for name in ('cargo', 'rustc')})
    return record


def archive(entries):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w') as output:
        for name, kind in entries:
            member = tarfile.TarInfo(name)
            member.type = kind
            member.mode = 0o644
            data = b'only synthetic bytes'
            if kind == tarfile.REGTYPE:
                member.size = len(data)
            else:
                member.linkname = 'Cargo.toml'
            output.addfile(member, io.BytesIO(data) if kind == tarfile.REGTYPE else None)
    return buffer.getvalue()


class Normal18BuildContracts(unittest.TestCase):
    def test_fixed_source_archive_and_legacy_helpers_are_independent(self):
        self.assertEqual(H.SOURCE, 'a4e03303b54bd5a5544131e95ad39662f8310e29')
        self.assertEqual(H.TREE, 'ddd5bddbeeb8011fa070d53e67f7978deae87214')
        self.assertEqual(H.ARCHIVE_SHA256, '1995df43686db331d61e5d7bdbfdb0146de7eeaa1dd1a784be42597b73f58bb3')
        self.assertEqual(H.SOURCE_FILES_SHA256, '6f064dafeed10bdd60470301262f57472531a935f8f9fc4e849d021ca2412db4')
        self.assertEqual(len(H.SCHEMA_TABLES), 63)
        self.assertNotIn('host_registrations', H.SCHEMA_TABLES)
        self.assertEqual(H.HELPERS['native-run.py'], '205d76e38721760744bd4d889e34ab3d2cf436ebc94583fee40fa17029ffac0a')
        for name, pin in H.HELPERS.items():
            raw = (ROOT / 'scripts' / name).read_bytes()
            if name == 'native-run.py':
                self.assertEqual(H.sha(raw), H.CURRENT_NATIVE_HELPER_SHA256)
                raw = H.helper_bytes(name)
            self.assertEqual(H.sha(raw), pin)

    def test_native_helper_compiles_only_pinned_inverse_and_exact_historical_runner(self):
        with mock.patch.object(H, 'compile', wraps=compile, create=True) as compiler:
            native = H.load_helper('native-run.py')
        self.assertEqual([H.sha(call.args[0]) for call in compiler.call_args_list],
                         [H.NATIVE_WEBVIEW_INVERSE_SHA256, H.HELPERS['native-run.py']])
        self.assertNotIn('READONLY_SCHEMA_VERSION', vars(native))
        self.assertTrue(callable(native.owned_child_exited))
        self.assertTrue(callable(native.owned_group_is_quiescent))

    def test_native_layer_refuses_unknown_downgraded_or_unsafe_sources_before_execution(self):
        current = (ROOT / 'scripts/native-run.py').read_bytes()
        historical = H.helper_bytes('native-run.py')
        inverse = (ROOT / H.NATIVE_WEBVIEW_INVERSE).read_bytes()
        unknown = b'raise AssertionError("unverified helper ran")\n'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            runner_path = root / 'scripts/native-run.py'
            inverse_path = root / H.NATIVE_WEBVIEW_INVERSE
            runner_path.parent.mkdir()
            inverse_path.parent.mkdir(parents=True)
            runner_path.write_bytes(current)
            inverse_path.write_bytes(inverse)
            runner_path.chmod(0o644)
            inverse_path.chmod(0o644)
            with mock.patch.object(H, 'ROOT', root), \
                 mock.patch.object(H, 'compile', side_effect=AssertionError('unverified compilation'), create=True) as compiler:
                for payload in (historical, current + b'\n', unknown):
                    runner_path.write_bytes(payload)
                    with self.subTest(current_sha=H.sha(payload)), self.assertRaisesRegex(ValueError, 'Changed current native helper'):
                        H.load_helper('native-run.py')
                runner_path.write_bytes(current)
                inverse_path.write_bytes(unknown)
                with self.assertRaisesRegex(ValueError, 'Changed immutable native inverse'):
                    H.load_helper('native-run.py')
                inverse_path.write_bytes(inverse)
                inverse_path.chmod(0o664)
                with self.assertRaisesRegex(ValueError, 'Unsafe native helper source'):
                    H.load_helper('native-run.py')
                inverse_path.unlink()
                inverse_path.symlink_to(ROOT / H.NATIVE_WEBVIEW_INVERSE)
                with self.assertRaisesRegex(ValueError, 'Unsafe native helper source'):
                    H.load_helper('native-run.py')
                compiler.assert_not_called()

    def test_native_inverse_output_must_match_the_original_helper_pin(self):
        current = (ROOT / 'scripts/native-run.py').read_bytes()
        inverse = b'def restore_native_webview_before_schema19(path, raw):\n    return raw\n'
        with mock.patch.object(H, 'read_native_layer', side_effect=[current, inverse]), \
             mock.patch.object(H, 'NATIVE_WEBVIEW_INVERSE_SHA256', H.sha(inverse)), \
             mock.patch.object(H, 'compile', wraps=compile, create=True) as compiler:
            with self.assertRaisesRegex(ValueError, 'Changed immutable helper'):
                H.load_helper('native-run.py')
        self.assertEqual([call.args[0] for call in compiler.call_args_list], [inverse])

    def test_finite_synthetic_metadata_positive_and_source_tool_feature_negatives(self):
        # This patches ONLY the synthetic index for finite parser tests. It is
        # never accepted as actual build or fixture evidence.
        original = synthetic_build()
        with mock.patch.object(H, 'SOURCE_FILES_SHA256', H.sha(H.canonical(original['source']['files_sha256']))), \
             mock.patch.object(H.subprocess, 'Popen', side_effect=AssertionError('no process')):
            H.validate_compiled(original)
            cases = [(('schema',), x) for x in [17, 19, True, 18.0, '18', None]] + [
                (('source', 'commit'), 'f' * 40), (('source', 'tree'), 'f' * 40),
                (('source', 'archive_sha256'), 'f' * 64), (('source', 'compiled_source_clean'), 1),
                (('builder_sha256',), 'f' * 64), (('helpers_sha256',), {}),
                (('build', 'features'), ['native-smoke']), (('build', 'default_features'), 1),
                (('build', 'argv'), ['/never/cargo', 'test']), (('build', 'exit_code'), False),
                (('build', 'group_quiescent_before_reap'), False),
                (('build', 'owned_identity'), None), (('build', 'owned_identity'), {'pid':True,'pgid':True}),
                (('build', 'owned_identity'), {'pid':42.0,'pgid':42.0}),
                (('build', 'owned_identity'), {'pid':42,'pgid':43}),
                (('build', 'cargo_config_absent_before'), False),
                (('build', 'cargo_config_absent_after'), False),
                (('build', 'cargo_config_paths_checked'), []),
                (('build', 'lockfile_sha256'), 'e' * 64), (('architecture',), 'unbound')]
            for path, bad in cases:
                value = copy.deepcopy(original)
                target = value
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = bad
                with self.subTest(path=path, bad=bad), self.assertRaises(ValueError):
                    H.validate_compiled(value)
            for mutation in ['path', 'lock', 'extra', 'removed']:
                value = copy.deepcopy(original)
                files = value['source']['files_sha256']
                if mutation == 'path':
                    files['../stolen'] = files.pop('crates/synthetic0.rs')
                elif mutation == 'lock':
                    files['Cargo.lock'] = 'f' * 64
                elif mutation == 'extra':
                    files['crates/unknown.rs'] = 'f' * 64
                else:
                    files.pop('crates/synthetic0.rs')
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    H.validate_compiled(value)

    def test_archive_links_duplicate_traversal_root_type_and_total_bounds(self):
        valid = archive([('Cargo.toml', tarfile.REGTYPE)])
        self.assertEqual(H.archive_files(valid), {'Cargo.toml': H.sha(b'only synthetic bytes')})
        cases = [[('/absolute', tarfile.REGTYPE)], [('../escape', tarfile.REGTYPE)],
                 [('crates/../escape', tarfile.REGTYPE)], [('other/file', tarfile.REGTYPE)],
                 [('Cargo.toml', tarfile.SYMTYPE)], [('Cargo.toml', tarfile.LNKTYPE)],
                 [('Cargo.toml', tarfile.FIFOTYPE)], [('Cargo.toml', tarfile.REGTYPE)] * 2]
        for entries in cases:
            with self.subTest(entries=entries), self.assertRaises(ValueError):
                H.archive_files(archive(entries))
        with tempfile.TemporaryDirectory() as directory, self.assertRaisesRegex(ValueError, 'archive'):
            H.extract_source(valid, Path(directory))
        with self.assertRaises(ValueError):
            H.archive_files(b'')

    def test_duplicate_and_nonfinite_manifest_json_refuse(self):
        for raw in [b'{"schema":18,"schema":18}', b'{"schema":NaN}', b'{"schema":Infinity}']:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                H.parse_json(raw)

    def test_secure_regular_reader_and_source_index_reject_aliases_and_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            path = root / 'Cargo.toml'
            path.write_bytes(b'synthetic')
            path.chmod(0o600)
            self.assertEqual(H.read_regular(path, 32), b'synthetic')
            self.assertEqual(H.source_hashes(root), {'Cargo.toml': H.sha(b'synthetic')})
            alias = root / 'Cargo.lock'
            alias.symlink_to(path)
            with self.assertRaises((ValueError, OSError)):
                H.read_regular(alias, 32)
            with self.assertRaises(ValueError):
                H.source_hashes(root)
            alias.unlink()
            os.link(path, alias)
            with self.assertRaises(ValueError):
                H.read_regular(path, 32)
            with self.assertRaises(ValueError):
                H.source_hashes(root)
            alias.unlink()
            path.chmod(0o644)
            with self.assertRaises(ValueError):
                H.read_regular(path, 32)

    def test_wrong_native_tool_format_cannot_be_frozen(self):
        for raw in [b'', b'test endpoint script', b'\x7fELF' + b'\0' * 32]:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                H.OLD.binary_metadata(raw)

    def test_archive_git_disables_lazy_fetch_every_transport_and_interaction(self):
        with mock.patch.object(H.subprocess, 'check_output', side_effect=subprocess.CalledProcessError(128, 'git')) as command:
            with self.assertRaises(subprocess.CalledProcessError):
                H.git_archive(Path('/never-repository'))
        env = command.call_args.kwargs['env']
        self.assertEqual(env['GIT_NO_LAZY_FETCH'], '1')
        self.assertEqual(env['GIT_ALLOW_PROTOCOL'], '')
        self.assertEqual(env['GIT_TERMINAL_PROMPT'], '0')
        self.assertEqual(env['GIT_PROTOCOL_FROM_USER'], '0')
        self.assertEqual(env['GIT_ASKPASS'], '/usr/bin/false')
        self.assertIn('protocol.allow=never', command.call_args.args[0])

    def test_local_missing_promisor_object_fails_closed_without_transport(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'missing.git'
            subprocess.run(['git', '-c', 'init.templateDir=', 'init', '--bare', '--quiet', str(repo)],
                           env=H.git_environment(), check=True, capture_output=True, timeout=10)
            with (repo / 'config').open('a') as stream:
                stream.write('\n[extensions]\npartialClone = fixture\n[remote "fixture"]\nurl = https://example.invalid/never-contacted\npromisor = true\n')
            with self.assertRaises(subprocess.CalledProcessError):
                H.git_archive(repo)
            self.assertEqual(list((repo / 'objects' / 'pack').iterdir()), [])

    def test_cargo_ancestor_config_and_symlink_refuse_without_global_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve()
            source = parent / 'source'
            source.mkdir()
            H.assert_no_ambient_config(source)
            folder = parent / '.cargo'
            folder.mkdir()
            for name in ('config', 'config.toml'):
                path = folder / name
                path.write_text('[build]\nrustflags = ["--cfg", "test-control"]\n')
                with self.assertRaisesRegex(ValueError, 'Cargo'):
                    H.assert_no_ambient_config(source)
                path.unlink()
                path.symlink_to(parent / 'missing')
                with self.assertRaisesRegex(ValueError, 'Cargo'):
                    H.assert_no_ambient_config(source)
                path.unlink()
            self.assertEqual(H.assert_no_ambient_config(source), H.cargo_config_paths(source))

    def test_identity_sampling_failure_still_reaps_owned_compiler_without_unknown_group_signal(self):
        class Child:
            pid = 4321001
            killed = False
            joined = False
            def kill(self):
                self.killed = True
            def wait(self, timeout=None):
                self.joined = True
                return -9
        child = Child()
        record = {}
        with mock.patch.object(H.subprocess, 'Popen', return_value=child), \
             mock.patch.object(H.COMMON, 'darwin_owned_identity', side_effect=ValueError('identity unavailable')), \
             mock.patch.object(H.COMMON.os, 'killpg', side_effect=AssertionError('unknown group signal')) as group:
            with self.assertRaises(ValueError):
                H.run_build(['/never/cargo'], Path('/never'), {}, io.BytesIO(), record)
        self.assertTrue(child.killed)
        self.assertTrue(child.joined)
        self.assertFalse(record['failed_build_cleanup']['identity_gone_after_join'])
        self.assertTrue(record['failed_build_cleanup']['forced_kill'])
        group.assert_not_called()

    def test_unbound_pair_requires_adjacent_build_manifest_before_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            frozen = Path(directory).resolve()
            with mock.patch.object(H, 'tool_records', side_effect=AssertionError('no tool read')):
                with self.assertRaisesRegex(ValueError, 'manifest'):
                    H.provenance(frozen, None)


if __name__ == '__main__':
    unittest.main()
