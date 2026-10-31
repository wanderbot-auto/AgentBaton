"""Synthetic files and mocked EnvBuilder only; never create or execute a venv."""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

from tests.infrastructure import test_schema17_migration_workflow as W

ROOT = Path(__file__).resolve().parents[2]

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value

P = module('_private_browser_python', ROOT / 'scripts/prepare-browser-runtime.py')
D = module('_private_python_original_driver', ROOT / 'apps/desktop/tests/integration-real-ipc.py')
HEAD = 'a' * 40
IMAGE = b'synthetic interpreter image; never execute'
NAMES = {'python', 'python3', 'python3.' + str(sys.version_info.minor)}


@contextlib.contextmanager
def fixture(change=None):
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary).resolve(); root.chmod(0o700)
        (root / 'artifacts').mkdir(mode=0o755)
        source = root / ('python3.' + str(sys.version_info.minor))
        source.write_bytes(IMAGE); source.chmod(0o775)
        def create(destination):
            output = Path(destination); binary = output / 'bin'; binary.mkdir(mode=0o755)
            (output / 'pyvenv.cfg').write_text('home = retained base standard library\n')
            for name in NAMES:
                path = binary / name; path.write_bytes(IMAGE); path.chmod(0o775)
            (binary / 'pip').write_text('synthetic pip script; never execute')
            if change is not None: change(root, source, output)
        builder = types.SimpleNamespace(create=mock.Mock(side_effect=create))
        with contextlib.chdir(root), mock.patch.object(P.sys, '_base_executable', str(source)), \
            mock.patch.object(P.venv, 'EnvBuilder', return_value=builder) as factory, \
            mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('No process')), \
            mock.patch.object(subprocess, 'run', side_effect=AssertionError('No process')), \
            mock.patch.object(sqlite3, 'connect', side_effect=AssertionError('No SQL')):
            yield root, source, root / 'artifacts/browser-venv', factory, builder


def driver_args(root):
    for name in ('batond','baton','chromium'):
        path=root/name;path.write_bytes(b'synthetic-'+name.encode());path.chmod(0o500)
    frontend=root/'frontend';frontend.mkdir();(frontend/'index.html').write_text('synthetic')
    candidate=root/'candidate.json';candidate.write_text(json.dumps({'schema':17,'git_head':HEAD,'dirty':False,
        'binaries':{name:hashlib.sha256((root/name).read_bytes()).hexdigest()for name in ('batond','baton')}}))
    build=root/'build.json';build.write_text(json.dumps({'source':HEAD,'exit':0,
        'command':['npm','--prefix','apps/desktop','run','build'],'source_head_and_tracked_unchanged':True}))
    return types.SimpleNamespace(expected_source_head=HEAD,batond=root/'batond',candidate_manifest=candidate,
        frontend_build_record=build,frontend=frontend,chromium=root/'chromium')


class PrivateCopies(unittest.TestCase):
    def test_only_fresh_copied_owned_images_are_hardened_and_original_layout_retained(self):
        with fixture()as(root,source,output,factory,builder):
            original=P.file_pin(source.stat());original_hash=hashlib.sha256(source.read_bytes()).hexdigest()
            result=P.prepare(Path('artifacts/browser-venv'))
            factory.assert_called_once_with(symlinks=False,with_pip=True)
            builder.create.assert_called_once_with(str(output))
            self.assertEqual(result,{'prepared_copied_interpreters':3,'mode':'0500','shared_source_unchanged':True})
            self.assertEqual(stat.S_IMODE(output.stat().st_mode),0o700)
            for name in NAMES:
                path=output/'bin'/name
                self.assertFalse(path.is_symlink());self.assertEqual(path.stat().st_nlink,1)
                self.assertEqual(stat.S_IMODE(path.stat().st_mode),0o500)
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),original_hash)
                self.assertNotEqual(path.stat().st_ino,source.stat().st_ino)
            self.assertEqual(P.file_pin(source.stat()),original)
            self.assertEqual((output/'pyvenv.cfg').read_text(),'home = retained base standard library\n')
            self.assertEqual((output/'bin/pip').read_text(),'synthetic pip script; never execute')

    def test_original_driver_still_refuses_wide_source_and_accepts_prepared_copy(self):
        with fixture()as(root,source,output,_,__):
            args=driver_args(root)
            with mock.patch.object(D,'source_guard',return_value={'head':HEAD,'tracked_clean':True}), \
                mock.patch.object(D.sys,'executable',str(source)):
                with self.assertRaises(D.Refusal):D.preflight(args)
            P.prepare(output)
            with mock.patch.object(D,'source_guard',return_value={'head':HEAD,'tracked_clean':True}), \
                mock.patch.object(D.sys,'executable',str(output/'bin/python')):
                attribution=D.preflight(args)[3]
                self.assertEqual(attribution['runtime_tools']['python']['mode'],0o500)
            self.assertEqual(stat.S_IMODE(source.stat().st_mode),0o775)

    def test_symlink_hardlink_empty_changed_extra_and_directory_targets_refuse_before_any_chmod(self):
        for kind in ('symlink','hardlink-source','hardlink-copy','empty','wrong-bytes','extra','directory'):
            def change(root,source,output):
                path=output/'bin/python'
                if kind=='extra':(output/'bin/python3.99').write_bytes(IMAGE);return
                path.unlink()
                if kind=='symlink':path.symlink_to(source)
                elif kind=='hardlink-source':os.link(source,path)
                elif kind=='hardlink-copy':os.link(output/'bin/python3',path)
                elif kind=='directory':path.mkdir()
                else:path.write_bytes(b''if kind=='empty'else b'changed')
            with self.subTest(kind=kind),fixture(change)as(_,source,output,__,___), \
                mock.patch.object(P.os,'fchmod',wraps=P.os.fchmod)as chmod:
                with self.assertRaises((ValueError,OSError)):P.prepare(output)
                chmod.assert_not_called();self.assertEqual(stat.S_IMODE(source.stat().st_mode),0o775)

    def test_unowned_private_interpreter_refuses_before_any_chmod(self):
        private_inodes=set()
        def identify_copies(root,source,output):
            private_inodes.update((p.stat().st_dev,p.stat().st_ino)for p in (output/'bin').iterdir()if p.name in NAMES)
        with fixture(identify_copies)as(_,__,output,___,____):
            original=P.os.fstat
            def unowned(fd):
                value=original(fd)
                if (value.st_dev,value.st_ino)in private_inodes:
                    values={name:getattr(value,name)for name in dir(value)if name.startswith('st_')}
                    values['st_uid']=os.geteuid()+1;return types.SimpleNamespace(**values)
                return value
            with mock.patch.object(P.os,'fstat',side_effect=unowned),mock.patch.object(P.os,'fchmod')as chmod:
                with self.assertRaises(ValueError):P.prepare(output)
                chmod.assert_not_called()

    def test_root_escape_existing_output_or_aliased_parent_refuse_without_builder_or_chmod(self):
        for kind in ('escape','absolute-escape','existing','parent-alias'):
            with self.subTest(kind=kind),fixture()as(root,_,output,__,builder),mock.patch.object(P.os,'fchmod')as chmod:
                if kind=='escape':argument=Path('../escape')
                elif kind=='absolute-escape':argument=root.parent/'escape'
                elif kind=='existing':output.mkdir();argument=output
                else:
                    (root/'alias').symlink_to(root/'artifacts',target_is_directory=True)
                    argument=root/'alias/browser-venv'
                with self.assertRaises((ValueError,OSError)):P.prepare(argument)
                builder.create.assert_not_called();chmod.assert_not_called()

    def test_private_root_replacement_refuses_and_never_chmods_outside(self):
        def replace(root,source,output):
            outside=root/'outside';outside.mkdir();(outside/'python').write_bytes(IMAGE)
            output.rename(root/'retained-private');output.symlink_to(outside,target_is_directory=True)
        with fixture(replace)as(root,_,output,__,___),mock.patch.object(P.os,'fchmod')as chmod:
            with self.assertRaises(ValueError):P.prepare(output)
            chmod.assert_not_called();self.assertEqual((root/'outside/python').read_bytes(),IMAGE)

    def test_named_copy_replacement_after_hash_refuses_before_any_chmod(self):
        with fixture()as(_,__,output,___,____):
            original=P.image;count=0
            def replace(fd,owned=False):
                nonlocal count
                result=original(fd,owned);count+=1
                if count==4:
                    path=output/'bin/python';path.rename(output/'bin/retained-copy')
                    path.write_bytes(IMAGE);path.chmod(0o775)
                return result
            with mock.patch.object(P,'image',side_effect=replace),mock.patch.object(P.os,'fchmod')as chmod:
                with self.assertRaises(ValueError):P.prepare(output)
                chmod.assert_not_called()

    def test_original_source_replacement_during_creation_refuses_before_any_chmod(self):
        def replace(root,source,output):
            source.rename(root/'retained-source');source.write_bytes(IMAGE);source.chmod(0o775)
        with fixture(replace)as(_,__,output,___,____),mock.patch.object(P.os,'fchmod')as chmod:
            with self.assertRaises(ValueError):P.prepare(output)
            chmod.assert_not_called()


class ExactWorkflowLayer(unittest.TestCase):
    def test_single_command_inverse_restores_entire_e874_workflow_and_all_old_pins(self):
        text=(ROOT/'.github/workflows/product.yml').read_text()
        old=W.strip_private_browser_python(text)
        self.assertEqual(hashlib.sha256(old.encode()).hexdigest(),'6d82c3f8799925f89b8a13e395356066995680c7d131d10345c2d6e64fd0bc3d')
        self.assertEqual(old,text.replace(W.PRIVATE_BROWSER_PYTHON_AFTER,W.PRIVATE_BROWSER_PYTHON_BEFORE,1))
        W.verify_only_six_historical_target_changes(text)
        W.verify_only_six_historical_target_changes(old)

    def test_unknown_command_extra_step_changed_guard_or_outside_edit_cannot_be_hidden(self):
        text=(ROOT/'.github/workflows/product.yml').read_text()
        for before,after in [(W.PRIVATE_BROWSER_PYTHON_AFTER,W.PRIVATE_BROWSER_PYTHON_BEFORE+'          echo unknown\n'),
            ('--output artifacts/browser-venv','--output /global/python'),
            ('--require-hashes','--no-deps'),('contents: read','contents: write'),
            ('timeout-minutes: 20','timeout-minutes: 21'),('134217728','268435456')]:
            self.assertIn(before,text)
            with self.subTest(before=before),self.assertRaises(ValueError):
                W.verify_only_six_historical_target_changes(text.replace(before,after,1))
        with self.assertRaises(ValueError):
            W.verify_only_six_historical_target_changes(text.replace(W.PRIVATE_BROWSER_PYTHON_AFTER,W.PRIVATE_BROWSER_PYTHON_AFTER*2,1))


if __name__=='__main__':unittest.main()
