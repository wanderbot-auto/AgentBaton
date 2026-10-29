"""Pure metadata/archive negatives; none are real Core/build/migration evidence."""
import copy
import io
import json
from pathlib import Path
import tempfile
import tarfile
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[5]
builder=types.ModuleType('reviewed_source_builder')
builder.__file__=str(ROOT/'scripts/prepare-legacy-migration-fixture.py')
exec(compile(Path(builder.__file__).read_bytes(),builder.__file__,'exec'),builder.__dict__)
generator=types.ModuleType('reviewed_fixture_generator')
generator.__file__=str(Path(__file__).with_name('generate.py'))
exec(compile(Path(generator.__file__).read_bytes(),generator.__file__,'exec'),generator.__dict__)


def synthetic_metadata():
    # Deliberately synthetic finite contract input, never emitted as actual.
    h='a'*64
    return {'contract':builder.CONTRACT,'status':'passed','schema':14,'platform':'linux','builder_sha256':h,
      'source':{'commit':builder.SOURCE,'tree':builder.TREE,'repository':builder.PUBLIC_REPOSITORY,
        'compiled_source_clean':True,'archive_sha256':h,'files_sha256':{n:h for n in ['Cargo.toml','Cargo.lock','crates/core/Cargo.toml','apps/daemon/Cargo.toml','apps/cli/Cargo.toml']}},
      'build':{'exit_code':0,'packages':['batond','baton'],'features':[],'default_features':True,'lockfile_sha256':h,
        'argv':['/explicit/cargo','build','--locked','-j','2','--bin','batond','--bin','baton']},
      'toolchain':{'cargo':'explicit synthetic cargo metadata','rustc':'explicit synthetic rustc metadata'},
      'probe':{'schema':14,'query_only':True,'foreign_key_errors':[],'counts':{n:0 for n in builder.TABLES},
        'snapshot_api_version':1,'snapshot_sequence':0,'cli_exit':0,'sigterm_sent':True,
        'daemon_wait_exit':0,'forced_kill':False,'identity_gone_after_join':True},
      'binaries':{n:{'sha256':h,'bytes':1024,'mode':'0o500','nlink':1} for n in ['batond','baton']}}


class PureSourceBuildContract(unittest.TestCase):
    def test_complete_metadata_validates_without_spawning_any_tool(self):
        with (mock.patch.object(builder.subprocess,'run',side_effect=AssertionError('No subprocess allowed')),
              mock.patch.object(builder.subprocess,'check_output',side_effect=AssertionError('No subprocess allowed'))):
            self.assertEqual(builder.validate_build_manifest(synthetic_metadata()),{'batond':'a'*64,'baton':'a'*64})

    def test_failed_or_unknown_probe_never_becomes_success(self):
        for key,value in [('forced_kill',True),('sigterm_sent',False),('daemon_wait_exit',1),('identity_gone_after_join',False),('schema',15),('cli_exit',1),('query_only',False),('foreign_key_errors',[['orphan']])]:
            with self.subTest(key=key):
                m=synthetic_metadata();m['probe'][key]=value
                with self.assertRaises(ValueError):builder.validate_build_manifest(m)
        m=synthetic_metadata();del m['probe']['counts']['goal_scopes']
        with self.assertRaises(ValueError):builder.validate_build_manifest(m)

    def test_wrong_source_dirty_build_features_or_locks_are_refused(self):
        mutations=[('source','commit','0'*40),('source','tree','0'*40),('source','compiled_source_clean',False),
          ('build','features',['test-control']),('build','exit_code',1),('build','lockfile_sha256','b'*64),('build','default_features',False)]
        for section,key,value in mutations:
            with self.subTest(section=section,key=key):
                m=synthetic_metadata();m[section][key]=value
                with self.assertRaises(ValueError):builder.validate_build_manifest(m)
        m=synthetic_metadata();m['platform']='darwin'
        with self.assertRaises(ValueError):builder.validate_build_manifest(m)

    def test_malformed_or_linked_frozen_pair_is_refused(self):
        for key,value in [('sha256','not-a-sha256'),('mode','0o700'),('nlink',2),('bytes',0)]:
            with self.subTest(key=key):
                m=synthetic_metadata();m['binaries']['baton'][key]=value
                with self.assertRaises(ValueError):builder.validate_build_manifest(m)

    def test_archive_extracts_only_bounded_regular_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            raw=io.BytesIO()
            with tarfile.open(fileobj=raw,mode='w') as t:
                info=tarfile.TarInfo('crates/core/Cargo.toml');info.size=4;t.addfile(info,io.BytesIO(b'test'))
            builder.extract_source(raw.getvalue(),Path(tmp))
            self.assertEqual((Path(tmp)/'crates/core/Cargo.toml').read_bytes(),b'test')

    def test_archive_traversal_links_and_outside_roots_are_refused(self):
        for name,kind in [('../escape',tarfile.REGTYPE),('/escape',tarfile.REGTYPE),('docs/archive.dmg',tarfile.REGTYPE),('crates/escape',tarfile.SYMTYPE),('crates/escape',tarfile.LNKTYPE)]:
            with self.subTest(name=name,kind=kind),tempfile.TemporaryDirectory() as tmp:
                raw=io.BytesIO()
                with tarfile.open(fileobj=raw,mode='w') as t:
                    info=tarfile.TarInfo(name);info.type=kind;info.linkname='/outside';t.addfile(info)
                with self.assertRaises(ValueError):builder.extract_source(raw.getvalue(),Path(tmp))

    def test_cleanup_failure_cannot_publish_passed_fixture_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);frozen=root/'frozen';frozen.mkdir();output=root/'output';output.mkdir()
            for n in ['batond','baton']:(frozen/n).write_bytes(b'pure synthetic metadata tool');(frozen/n).chmod(0o500)
            hashes={n:generator.sha((frozen/n).read_bytes()) for n in ['batond','baton']}
            report={'status':'failed_or_interrupted','dimension':'pure synthetic metadata; no Core/build evidence',
              'tool_metadata_before':{n:{'mode':'0o500','bytes':(frozen/n).stat().st_size} for n in hashes}}
            completed=False
            try:
                with mock.patch.object(generator.shutil,'rmtree',side_effect=OSError('controlled pure cleanup refusal')):
                    with generator.retained_private_runtime(report):pass
                completed=True
            except OSError:pass
            generator.finish_report(report,frozen,hashes,output,completed,False)
            self.assertEqual(json.loads((output/'manifest.json').read_text())['status'],'failed_or_interrupted')
            self.assertTrue(Path(report['private_runtime']).is_dir())
            # Only this empty synthetic unit-test directory, never actual DBs.
            generator.shutil.rmtree(report['private_runtime'])

    def test_final_binary_guard_failure_cannot_leave_passed_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);frozen=root/'frozen';frozen.mkdir();output=root/'output';output.mkdir()
            for n in ['batond','baton']:(frozen/n).write_bytes(b'pure guard control');(frozen/n).chmod(0o500)
            report={'status':'failed_or_interrupted','dimension':'pure guard control only','tool_metadata_before':{}}
            with self.assertRaises(AssertionError):generator.finish_report(report,frozen,{'batond':'0'*64,'baton':'0'*64},output,True,False)
            self.assertEqual(json.loads((output/'manifest.json').read_text())['status'],'failed_or_interrupted')

    def test_permission_or_rename_failure_never_publishes_final_passed_json(self):
        for failing in ['fchmod','replace']:
            with self.subTest(failing=failing),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp);final=root/'source-build-manifest.json'
                with mock.patch.object(builder.os,failing,side_effect=OSError('controlled publication failure')):
                    with self.assertRaises(OSError):builder.publish_private_json(final,{'status':'passed','dimension':'pure control only'})
                self.assertFalse(final.exists());self.assertTrue(final.with_name(final.name+'.pending').is_file())
            with self.subTest(generator=failing),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp);frozen=root/'frozen';frozen.mkdir();output=root/'output';output.mkdir()
                for n in ['batond','baton']:(frozen/n).write_bytes(b'pure metadata');(frozen/n).chmod(0o500)
                hashes={n:generator.sha((frozen/n).read_bytes()) for n in ['batond','baton']}
                report={'status':'failed_or_interrupted','dimension':'pure publication control only','tool_metadata_before':{n:{'mode':'0o500','bytes':(frozen/n).stat().st_size} for n in hashes}}
                with mock.patch.object(generator.os,failing,side_effect=OSError('controlled publication failure')):
                    with self.assertRaises(OSError):generator.finish_report(report,frozen,hashes,output,True,False)
                self.assertFalse((output/'manifest.json').exists());self.assertTrue((output/'manifest.json.pending').is_file())

    def test_unpublished_build_candidate_is_not_a_generator_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);frozen=root/'frozen';frozen.mkdir();pending=frozen/'source-build-manifest.json.pending';pending.write_text('{}')
            argv=['generate.py','--frozen',str(frozen),'--build-manifest',str(pending),'--repository',str(ROOT),'--output',str(root/'output')]
            with mock.patch.object(generator.sys,'argv',argv),mock.patch.object(generator.subprocess,'Popen',side_effect=AssertionError('No Core process allowed')):
                with self.assertRaises(AssertionError):generator.main()
            self.assertFalse((root/'output/manifest.json').exists())


if __name__=='__main__':unittest.main()
