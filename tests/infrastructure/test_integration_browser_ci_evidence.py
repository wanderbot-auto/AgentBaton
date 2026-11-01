"""Synthetic finite projector tests; no actual browser/Core/Provider execution."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import struct
import tempfile
import unittest
from unittest import mock
import zlib

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_integration_browser_projection_pure', ROOT / 'scripts/record-integration-browser-ci-evidence.py')
E = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(E)
HEAD, SECRET = 'a' * 40, 'PRIVATE_TOKEN_PROPOSAL_DATABASE_ABSOLUTE_PATH_CANARY'


def image(width, height):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    # Generated black pixels, not a screenshot or visual acceptance.
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)) + chunk(
        b'IDAT', zlib.compress((b'\0' + b'\0' * (width * 3)) * height)) + chunk(b'IEND', b'')


IMAGES = {'desktop': image(1440, 1100), 'mobile': image(390, 844)}


def write(path, value):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_bytes(value if type(value) is bytes else json.dumps(value, allow_nan=False).encode())
    path.chmod(0o600)


def tool(h, size=100, mode=0o500):
    return {'sha256': h * 64, 'bytes': size, 'mode': mode, 'device': '1', 'inode': '2', 'uid': os.geteuid(), 'links': 1}


def report(case, manifest_raw, build_raw):
    harness = E.expected_driver()
    inputs = {'source': HEAD, 'manifest_sha256': E.raw_sha(manifest_raw), 'build_record_sha256': E.raw_sha(build_raw),
        'driver_sha256': harness, 'source_guard': {'head': HEAD, 'tracked_clean': True, 'driver_sha256': harness},
        'tools': {'batond': tool('b'), 'baton': tool('c')}, 'runtime_tools': {
            'chromium': tool('d', 326880088, 0o755), 'python': tool('e', 30894944, 0o755)},
        'assets': {'index.html': tool('f', 100, 0o644)}}
    checks = []
    for name in E.CHECKS if case == 'normal' else (E.CHECKS[0], *E.CHECKS[2:]):
        row = {'name': name, 'status': 'passed'}
        if name == E.CHECKS[2]:
            row.update(proposal_sha256='a'*64, snapshot_sha256='b'*64, whole_response_sha256='c'*64,
                proposal_bytes=2000, snapshot_bytes=1800, whole_response_bytes=2300)
        if name == E.CHECKS[3]:
            failed = case == 'command-failure'
            row.update(state='failed' if failed else 'confirmed_local_ref', stages=2 if failed else 3,
                command_exit=7 if failed else 0, cas_attempts=0 if failed else 1, source_and_index_unchanged=True,
                original_command_unchanged=True, actual_owned_reap_and_join=True,
                shared_runtime_ms=3000, shared_output_bytes=50 if failed else 120, tokens=None, cost_microunits=None)
        checks.append(row)
    shots = []
    for name in E.NAMES:
        mobile = 'mobile' in name
        raw = IMAGES['mobile' if mobile else 'desktop']
        shots.append({'name': name, 'sha256': E.raw_sha(raw), 'bytes': len(raw),
            'viewport': {'width': 390 if mobile else 1440, 'height': 844 if mobile else 1100},
            'focus_box_before_capture_observation': {'x': 20, 'y': 1170, 'width': 100, 'height': 20},
            'focus_viewport_asserted_before_capture': True, 'pixel_synchronized': False, 'dimension': 'Linux Chromium only'})
    return {'case': 'normal' if case == 'normal' else 'command_exit7', 'status': 'passed', 'dimension': E.DIMENSION,
        'not_claimed': E.NOT_CLAIMED, 'checks': checks, 'screenshots': shots, 'inputs': inputs,
        'browser_version': '151.0.0.0', 'http_method_counts': {'system.snapshot': 8,
            'integration_policies.propose': 2 if case == 'normal' else 1}, 'readonly_reply_aborts': 0,
        'cleanup': dict.fromkeys(E.CLEANUP_BOOLS, True) | {'daemon_joined': {'exit': 0, 'joined': True, 'forced_kill': False}}}


def seed(root):
    private = root / 'private'; private.mkdir(mode=0o700)
    # mkdir(parents=True) applies mode only to the leaf directory. Create this
    # intermediate private boundary explicitly, independently of the CI umask.
    (private / 'driver').mkdir(mode=0o700)
    manifest = {'schema': 17, 'git_head': HEAD, 'dirty': False, 'binaries': {'batond': 'b'*64, 'baton': 'c'*64}}
    build = {'source': HEAD, 'command': ['npm', '--prefix', 'apps/desktop', 'run', 'build'],
        'exit': 0, 'source_head_and_tracked_unchanged': True}
    manifest_raw, build_raw = json.dumps(manifest).encode(), json.dumps(build).encode()
    write(private / 'candidate.json', manifest_raw); write(private / 'frontend-build-record.json', build_raw)
    write(private / 'status.json', {'source': HEAD, 'driver_exit': 0, 'probe_passed': True})
    write(private / 'schema-probe.json', {'source': HEAD, 'api_version': 1, 'database_schema_version': 17,
        'daemon_exit': 0, 'joined': True})
    for case in E.CASES:
        data = report(case, manifest_raw, build_raw)
        write(private / 'driver' / case / 'report.json', data)
        for row in data['screenshots']:
            write(private / 'driver' / case / (row['name'] + '.png'), IMAGES['mobile' if 'mobile' in row['name'] else 'desktop'])
        write(private / 'driver' / case / 'private' / 'raw-canary.json', {'token': SECRET, 'database': SECRET})
    return private


class ProjectionControls(unittest.TestCase):
    def project(self, private, output, outcome='success'):
        return E.main(['--private', str(private), '--output', str(output), '--expected-source-head', HEAD,
            '--driver-step-outcome', outcome])

    def test_complete_pair_copies_only_exact_twelve_original_pngs_and_finite_summary(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = seed(root); output = root/'public'
            self.assertEqual(self.project(private, output), 0)
            self.assertEqual({p.name for p in output.iterdir()}, E.PUBLIC_NAMES | {'summary.json'})
            value = json.loads((output/'summary.json').read_text())
            self.assertEqual(value['checks'], 9); self.assertEqual([v['checks'] for v in value['cases']], [5, 4])
            self.assertEqual([v['terminal']['stages'] for v in value['cases']], [3, 2])
            self.assertEqual([v['terminal']['cas_attempts'] for v in value['cases']], [1, 0])
            self.assertIs(value['pixel_synchronized'], False)
            self.assertEqual(value['visual_review'], 'not_performed_by_projector')
            self.assertNotIn(SECRET, json.dumps(value)); self.assertNotIn(str(private), json.dumps(value))
            for case in E.CASES:
                for name in E.NAMES:
                    self.assertEqual((output/(case+'-'+name+'.png')).read_bytes(),
                        (private/'driver'/case/(name+'.png')).read_bytes())
            self.assertTrue(all(stat.S_IMODE(p.stat().st_mode) == 0o600 for p in output.iterdir()))

    def test_private_driver_fixture_and_complete_projection_under_both_umasks(self):
        for mask in (0o022, 0o077):
            with self.subTest(umask=oct(mask)), tempfile.TemporaryDirectory() as temporary:
                previous = os.umask(mask)
                try:
                    root = Path(temporary).resolve(); private = seed(root); output = root / 'public'
                    for directory in (private, private / 'driver', *(private / 'driver' / case for case in E.CASES)):
                        self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
                    self.assertEqual(self.project(private, output), 0)
                    self.assertEqual({p.name for p in output.iterdir()}, E.PUBLIC_NAMES | {'summary.json'})
                    self.assertEqual(json.loads((output / 'summary.json').read_text())['checks'], 9)
                finally:
                    os.umask(previous)

    def test_failed_skipped_or_unobserved_step_cannot_publish_old_pass(self):
        for outcome in ['failure', 'skipped', 'cancelled', 'unobserved']:
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);output=root/'public'
                self.assertEqual(self.project(private,output,outcome),1)
                self.assertEqual({p.name for p in output.iterdir()},{'summary.json'})
                self.assertEqual(json.loads((output/'summary.json').read_text())['status'],'failed')

    def test_missing_report_has_only_finite_failure_and_nonzero(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();output=root/'public'
            self.assertEqual(self.project(root/'missing',output),1)
            self.assertEqual({p.name for p in output.iterdir()},{'summary.json'})
            self.assertEqual(json.loads((output/'summary.json').read_text())['failure_kind'],'missing_private_evidence')

    def test_driver_exit_null_bool_nonzero_never_becomes_zero(self):
        for bad in [None,False,True,1,-1,0.0]:
            with self.subTest(exit=bad),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root)
                write(private/'status.json',{'source':HEAD,'driver_exit':bad,'probe_passed':True})
                self.assertEqual(self.project(private,root/'public'),1)
                self.assertFalse(any(p.suffix=='.png' for p in (root/'public').iterdir()))

    def test_manifest_or_build_dirty_conflicting_source_alias_never_passes(self):
        for filename,key,bad in [('candidate.json','tracked_clean',False),('candidate.json','source_head','f'*40),
            ('candidate.json','compiled_core_source_head','f'*40),('frontend-build-record.json','dirty',True),
            ('frontend-build-record.json','tracked_clean',False),('frontend-build-record.json','exit',False)]:
            with self.subTest(file=filename,key=key),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);q=private/filename;value=json.loads(q.read_text());value[key]=bad;write(q,value)
                binding='manifest_sha256' if filename=='candidate.json' else 'build_record_sha256'
                for case in E.CASES:
                    r=private/'driver'/case/'report.json';data=json.loads(r.read_text());data['inputs'][binding]=E.raw_sha(q.read_bytes());write(r,data)
                self.assertEqual(self.project(private,root/'public'),1)

    def test_real_schema_probe_and_cleanup_are_strictly_typed(self):
        for key,bad in [('database_schema_version',16),('database_schema_version',18),('database_schema_version',True),
            ('api_version',True),('api_version',17),('joined',False),('daemon_exit',False),('daemon_exit',1),('source','f'*40)]:
            with self.subTest(key=key,bad=bad),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);q=private/'schema-probe.json';v=json.loads(q.read_text());v[key]=bad;write(q,v)
                self.assertEqual(self.project(private,root/'public'),1)

    def test_private_path_mode_symlink_hardlink_and_world_readable_reports_refuse(self):
        for variant in ['root-mode','driver-mode','report-mode','report-symlink','report-hardlink','directory-symlink']:
            with self.subTest(variant=variant),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);q=private/'status.json'
                if variant=='root-mode':private.chmod(0o755)
                elif variant=='driver-mode':(private/'driver').chmod(0o755)
                elif variant=='report-mode':q.chmod(0o644)
                elif variant=='report-symlink':
                    raw=q.read_bytes();q.unlink();write(root/'outside',raw);q.symlink_to(root/'outside')
                elif variant=='report-hardlink':os.link(q,root/'linked')
                else:
                    alias=root/'alias';alias.symlink_to(private,target_is_directory=True);private=alias
                self.assertEqual(self.project(private,root/'public'),1)

    def test_bad_json_duplicate_bool_nan_and_private_canary_never_public(self):
        for raw in [b'{"source":"'+HEAD.encode()+b'","source":"'+SECRET.encode()+b'"}', b'{"source":NaN}',b'{"source":Infinity}',
            json.dumps({'source':HEAD,'driver_exit':0,'probe_passed':True,'secret':SECRET}).encode()]:
            with self.subTest(raw=raw[:20]),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);write(private/'status.json',raw)
                self.assertEqual(self.project(private,root/'public'),1)
                self.assertNotIn(SECRET,(root/'public/summary.json').read_text())

    def test_failed_or_partial_case_never_publishes_images(self):
        for variant in ['status','missing-check','failure-canary','cleanup']:
            with self.subTest(variant=variant),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();private=seed(root);q=private/'driver/normal/report.json';v=json.loads(q.read_text())
                if variant=='status':v['status']='failed_or_interrupted'
                elif variant=='missing-check':v['checks'].pop()
                elif variant=='failure-canary':v['failure']={'message':SECRET}
                else:v['cleanup']['daemon_joined']['joined']=False
                write(q,v);self.assertEqual(self.project(private,root/'public'),1)
                self.assertEqual({p.name for p in (root/'public').iterdir()},{'summary.json'})
                self.assertNotIn(SECRET,(root/'public/summary.json').read_text())

    def test_second_case_cannot_hide_first_report_change_after_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();private=seed(root);real=E.screenshots
            def changed(fd,report,case,witnesses):
                value=real(fd,report,case,witnesses)
                if case=='command-failure':
                    q=private/'driver/normal/report.json';q.write_bytes(q.read_bytes()+b' ')
                return value
            with mock.patch.object(E,'screenshots',side_effect=changed):
                self.assertEqual(self.project(private,root/'public'),1)

    def test_output_reuse_or_alias_never_overwrites_existing_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();private=seed(root);output=root/'public';output.mkdir();write(output/'original',SECRET.encode())
            self.assertEqual(self.project(private,output),1);self.assertEqual((output/'original').read_text(),SECRET)
            alias=root/'alias';alias.symlink_to(output,target_is_directory=True)
            self.assertEqual(self.project(private,alias),1);self.assertEqual((output/'original').read_text(),SECRET)


class ReportGuards(unittest.TestCase):
    def data(self,case='normal'):
        return report(case,b'{}',b'{}')

    def check(self,value,case='normal'):
        return E.report_summary(value,case,HEAD,E.raw_sha(b'{}'),E.raw_sha(b'{}'),E.expected_driver())

    def test_full_source_sha_tool_assets_and_dimension_bindings(self):
        for mutate in [lambda v:v['inputs'].update(source='f'*40),lambda v:v['inputs'].update(driver_sha256='0'*64),
            lambda v:v['inputs']['source_guard'].update(tracked_clean=False),lambda v:v['inputs']['runtime_tools']['chromium'].update(bytes=536870913),
            lambda v:v['inputs']['runtime_tools']['python'].update(bytes=268435457),lambda v:v['inputs']['tools']['batond'].update(mode=0o755),
            lambda v:v['inputs']['tools']['batond'].update(links=2),lambda v:v['inputs']['assets'].update({'/secret':tool('a')}),
            lambda v:v.update(dimension='macOS native success'),lambda v:v.update(not_claimed=[])]:
            v=self.data();mutate(v)
            with self.assertRaises(E.Refused):self.check(v)

    def test_actual_outcome_exact_three_vs_two_stages_and_cas_one_vs_zero(self):
        for case in E.CASES:
            for key,bad in [('stages',True),('stages',4),('cas_attempts',False),('cas_attempts',2),('command_exit',True),
                ('state','outcome_unknown'),('tokens',0),('cost_microunits',0),('actual_owned_reap_and_join',False)]:
                v=self.data(case);row=next(c for c in v['checks'] if c['name']==E.CHECKS[3]);row[key]=bad
                with self.subTest(case=case,key=key,bad=bad),self.assertRaises(E.Refused):self.check(v,case)

    def test_no_http_finalize_or_start_can_be_laundered_as_automatic_watcher(self):
        for method in ['approvals.finalize','integrations.start','rpc','PRIVATE_TOKEN']:
            v=self.data();v['http_method_counts'][method]=1
            with self.assertRaises(E.Refused):self.check(v)

    def test_cleanup_missing_bool_and_forced_exit_never_pass(self):
        for mutate in [lambda v:v.pop('fixture_removed'),lambda v:v.update(http_thread_joined=1),
            lambda v:v['daemon_joined'].update(forced_kill=True),lambda v:v['daemon_joined'].update(exit=False)]:
            value=self.data()['cleanup'];mutate(value)
            with self.assertRaises(E.Refused):E.cleanup(value)

    def test_png_hash_crc_trailing_bytes_ancillary_and_fake_dimensions_fail(self):
        row=self.data()['screenshots'][0];raw=IMAGES['desktop']
        ancillary = struct.pack('>I',4)+b'gAMA'+struct.pack('>I',45455)
        ancillary += struct.pack('>I',zlib.crc32(ancillary[4:])&0xffffffff)
        for data in [raw+b'SECRET',raw[:-1],raw[:40]+bytes([raw[40]^1])+raw[41:],raw[:33]+ancillary+raw[33:]]:
            claimed=copy.deepcopy(row);claimed['sha256']=E.raw_sha(data)
            with self.assertRaises(E.Refused):E.P.png_bytes(data,claimed,'desktop.png')
        wrong=copy.deepcopy(row);wrong['viewport']['width']=True
        with self.assertRaises(E.Refused):E.P.png_bytes(raw,wrong,'desktop.png')
        wrong=copy.deepcopy(row);wrong['sha256']='0'*64
        with self.assertRaises(E.Refused):E.P.png_bytes(raw,wrong,'desktop.png')

    def test_valid_larger_fullpage_cannot_replace_exact_viewport_png(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();private=seed(root);q=private/'driver/normal/report.json';v=json.loads(q.read_text())
            raw=image(1440,1200);v['screenshots'][0]['sha256']=E.raw_sha(raw);v['screenshots'][0]['bytes']=len(raw)
            write(private/'driver/normal'/ (E.NAMES[0]+'.png'),raw);write(q,v)
            self.assertEqual(E.main(['--private',str(private),'--output',str(root/'public'),
                '--expected-source-head',HEAD,'--driver-step-outcome','success']),1)

    def test_before_capture_box_is_not_required_to_match_pixels_or_claim_visual_acceptance(self):
        # A finite pre-capture y outside the image is allowed because layout may
        # change; the explicit false pixel-synchrony claim is mandatory.
        self.assertEqual(self.data()['screenshots'][0]['focus_box_before_capture_observation']['y'],1170)
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();private=seed(root)
            q=private/'driver/normal/report.json';v=json.loads(q.read_text());v['screenshots'][0]['pixel_synchronized']=True;write(q,v)
            self.assertEqual(E.main(['--private',str(private),'--output',str(root/'public'),
                '--expected-source-head',HEAD,'--driver-step-outcome','success']),1)


if __name__=='__main__':
    unittest.main()
