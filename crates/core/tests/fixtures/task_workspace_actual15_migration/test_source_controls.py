"""Pure provenance/owned-file controls, not an actual15 generator run."""
import ast
import copy
import importlib.util
import json
import os
from pathlib import Path
import platform
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

REPO=Path(__file__).resolve().parents[5]
spec=importlib.util.spec_from_file_location('schema15_source',REPO/'scripts/prepare-schema15-migration-fixture.py')
H=importlib.util.module_from_spec(spec);spec.loader.exec_module(H)

def compiled():
    return {'contract':H.CONTRACT,'status':'compiled','schema':15,'platform':sys.platform,'architecture':platform.machine(),
            'source':{'commit':H.SOURCE,'tree':H.TREE,'repository':H.REPOSITORY,'compiled_source_clean':True,'archive_sha256':'b'*64,'files_sha256':{'Cargo.toml':'c'*64,'Cargo.lock':'a'*64}},
            'build':{'argv':['/inert/cargo','build','--offline','--locked','-j','2','--bin','batond','--bin','baton'],'exit_code':0,'default_features':True,'features':[],'lockfile_sha256':'a'*64},
            'toolchain':{'cargo':'inert metadata, no build','rustc':'inert metadata, no compiler'},'binaries':{'batond':{},'baton':{}}}

class SourceControls(unittest.TestCase):
    def test_compiled_record_is_build_only_and_exact_source(self):
        H.validate_compiled(compiled())
        for key,value in [('schema',14),('schema',15.0),('status','passed')]:
            m=compiled();m[key]=value
            with self.subTest(key=key,value=value),self.assertRaises(ValueError):H.validate_compiled(m)
        for field,value in [('exit_code',False),('exit_code',1),('default_features',False),('features',['unexpected'])]:
            m=compiled();m['build'][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):H.validate_compiled(m)
        m=compiled();m['source']['commit']='f'*40
        with self.assertRaises(ValueError):H.validate_compiled(m)

    def test_build_argv_cannot_lose_lock_or_add_features_or_relative_compiler(self):
        for change in [lambda a:a.__setitem__(0,'cargo'),lambda a:a.remove('--locked'),lambda a:a.extend(['--features','preview-evidence'])]:
            m=compiled();change(m['build']['argv'])
            with self.assertRaises(ValueError):H.validate_compiled(m)

    def test_strict_json_types_and_duplicate_keys(self):
        self.assertTrue(H.strict_equal({'x':[15,False]},{'x':[15,False]}))
        for value in [{'x':[15.0,False]},{'x':[15,0]}]:self.assertFalse(H.strict_equal({'x':[15,False]},value))
        with self.assertRaises(ValueError):json.loads('{"schema":15,"schema":15}',object_pairs_hook=H.unique)

    def test_private_regular_read_rejects_links_mode_and_oversize(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);p=root/'owned';p.write_bytes(b'{}');p.chmod(0o600)
            self.assertEqual(H.read_regular(p,100),b'{}')
            (root/'symlink').symlink_to(p)
            with self.assertRaises(OSError):H.read_regular(root/'symlink',100)
            os.link(p,root/'hardlink')
            with self.assertRaises(ValueError):H.read_regular(p,100)
            (root/'hardlink').unlink();p.chmod(0o644)
            with self.assertRaises(ValueError):H.read_regular(p,100)
            p.chmod(0o600)
            with self.assertRaises(ValueError):H.read_regular(p,1)

    def native_header(self):
        if sys.platform=='darwin':return H.struct.pack('<II',0xfeedfacf,{'arm64':0x0100000c,'x86_64':0x01000007}[platform.machine()])+b'INERT'
        raw=bytearray(32);raw[:6]=b'\x7fELF\x02\x01';raw[18:20]={'x86_64':62,'aarch64':183}[platform.machine()].to_bytes(2,'little');return bytes(raw)

    def test_native_header_and_hardlink_controls_are_not_execution_proof(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp).resolve();root.chmod(0o700);raw=self.native_header()
            for name in ['batond','baton']:(root/name).write_bytes(raw);(root/name).chmod(0o500)
            records=H.tool_records(root);self.assertEqual(records['batond']['sha256'],H.sha(raw))
            self.assertEqual(records['baton']['inode'],str((root/'baton').stat().st_ino))
            os.link(root/'baton',root/'extra-link')
            with self.assertRaises(ValueError):H.tool_records(root)
        with self.assertRaises(ValueError):H.binary_metadata(b'not an executable')

    def test_same_bytes_path_replacement_is_not_a_stable_frozen_inode(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp).resolve();root.chmod(0o700);raw=self.native_header()
            for name in ['batond','baton']:(root/name).write_bytes(raw);(root/name).chmod(0o500)
            original=H.read_regular
            def changed(path,*args):
                value=original(path,*args);replacement=root/'replacement';replacement.write_bytes(raw);replacement.chmod(0o500);os.replace(replacement,path);return value
            with mock.patch.object(H,'read_regular',side_effect=changed),self.assertRaises(ValueError):H.tool_records(root)

    def test_fifo_read_has_a_bounded_owned_refusal_helper(self):
        with tempfile.TemporaryDirectory()as tmp:
            path=Path(tmp)/'fifo';os.mkfifo(path,0o600)
            code="import runpy,sys; m=runpy.run_path(sys.argv[1],run_name='pure_schema15');\ntry:m['read_regular'](sys.argv[2],100)\nexcept (OSError,ValueError):print('refused')\nelse:raise SystemExit(3)"
            child=subprocess.run([sys.executable,'-B','-c',code,str(REPO/'scripts/prepare-schema15-migration-fixture.py'),str(path)],capture_output=True,timeout=2)
            self.assertEqual(child.returncode,0);self.assertEqual(child.stdout,b'refused\n');self.assertEqual(child.stderr,b'')

    def test_failed_private_publication_never_exposes_a_passed_final_file(self):
        common,_=H.common()
        for operation in ['fchmod','replace']:
            with self.subTest(operation=operation),tempfile.TemporaryDirectory()as tmp:
                path=Path(tmp)/'manifest.json'
                with mock.patch.object(common.os,operation,side_effect=OSError('inert fault')),self.assertRaises(OSError):common.publish_private_json(path,{'status':'passed'})
                self.assertFalse(path.exists());self.assertTrue(path.with_name('manifest.json.pending').exists())

    def test_sqlite_backup_cookie_differs_while_raw_rows_ddl_and_user_version_survive(self):
        with sqlite3.connect(':memory:')as source,sqlite3.connect(':memory:')as destination:
            source.executescript("CREATE TABLE first(raw BLOB);CREATE TABLE second(document TEXT);INSERT INTO first(rowid,raw) VALUES(31,x'00ff');INSERT INTO second VALUES('raw UTF8 雪');PRAGMA user_version=15;")
            source.backup(destination)
            self.assertNotEqual(source.execute('PRAGMA schema_version').fetchone(),destination.execute('PRAGMA schema_version').fetchone())
            for query in ['SELECT * FROM sqlite_schema ORDER BY type,name','SELECT rowid,* FROM first','SELECT rowid,* FROM second','PRAGMA user_version']:
                self.assertEqual(source.execute(query).fetchall(),destination.execute(query).fetchall())

    def test_generator_import_does_not_spawn_and_only_actual_rpc_can_create_history(self):
        path=Path(__file__).with_name('generate.py');source=path.read_text();tree=ast.parse(source)
        calls={node.args[0].value for node in ast.walk(tree)if isinstance(node,ast.Call)and isinstance(node.func,ast.Name)and node.func.id=='call'and node.args and isinstance(node.args[0],ast.Constant)}
        self.assertEqual(calls,{'system.snapshot','projects.import','tasks.create','tasks.cancel'})
        self.assertNotIn('PRAGMA user_version=',source);self.assertNotIn('INSERT INTO',source);self.assertNotIn('UPDATE tasks',source)
        module=importlib.util.module_from_spec(importlib.util.spec_from_file_location('inert_actual15_generator',path))
        with mock.patch('subprocess.Popen',side_effect=AssertionError('no Actor on import')):module.__spec__.loader.exec_module(module)

    def test_rust_consumer_retains_failures_and_compares_private_rows_without_debug(self):
        source=(REPO/'crates/core/tests/task_workspace_actual15_migration.rs').read_text()
        self.assertEqual(source.count('#[test]'),5);self.assertNotIn('#[ignore',source)
        self.assertIn('AGENTBATON_ACTUAL_SCHEMA15_FIXTURE',source);self.assertIn('root.keep()',source)
        self.assertIn('SELECT rowid,*',source);self.assertIn('code_workspace_binding',source)
        self.assertIn('#[derive(PartialEq)]',source);self.assertNotIn('derive(Debug',source)
        self.assertNotIn('assert_eq!(before.rows',source);self.assertIn('VERSION_UNSUPPORTED',source)

if __name__=='__main__':unittest.main(verbosity=2)
