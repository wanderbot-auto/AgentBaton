"""Synthetic owned metadata/PNG/FD controls, never actual Core/browser evidence."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import socket
import stat
import struct
import tempfile
import unittest
from unittest import mock
import zlib

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('task_workspace_projection', ROOT / 'scripts/record-task-workspace-ci-evidence.py')
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)
HEAD = 'a' * 40
HARNESS = 'b' * 64
SECRET = 'PRIVATE_SENTINEL_TASK_RPC_TOKEN_PATH_STDERR'


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)


def image(width, height):
    # Real blank RGBA PNG bytes generated here, not a browser screenshot.
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(bytes((width * 4 + 1) * height))) + chunk(b'IEND', b''))


IMAGES = {'desktop': image(1440, 1100), 'mobile': image(390, 844)}


def report():
    tools = {name: {'bytes': 123, 'mode': 0o500, 'sha256': 'c' * 64} for name in ('baton', 'batond')}
    assets = {'index.html': {'bytes': 99, 'mode': 0o644, 'sha256': 'd' * 64}}
    value = {'status': 'passed', 'flow_complete': True, 'harness_sha256': HARNESS,
             'inputs': {'expected_source_head': HEAD, 'compiled_core_source_head': HEAD,
                        'frontend_build_head': HEAD, 'manifest_sha256': 'e' * 64,
                        'frontend_build_record_sha256': 'f' * 64, 'tools': tools, 'frontend_assets': assets},
             'checks': [{'id': name, 'status': 'passed', 'name': SECRET, 'task_id': SECRET} for name in P.CASES],
             'scenarios': {name: 'passed' for name in P.CASES}, 'http_errors': [], 'page_errors': [],
             'cleanup': {**{name: None for name in P.NULL_CLEANUP}, 'complete': True,
                         'daemon_stop': {'pid': 999999, 'wait_exitcode': 0, 'popen_reaped': True},
                         'http_thread': {'thread_alive_after_join': False},
                         'tools_after': copy.deepcopy(tools), 'assets_after': copy.deepcopy(assets)},
             'no_execution': {'schema': 16, 'no_execution_or_grants': True,
                              'table_counts': {name: 0 for name in P.TABLES}},
             'screenshots': [], 'http': [{'token': SECRET}], 'error': {'private': SECRET},
             'runtime': {'path': SECRET}, 'raw_report': SECRET}
    next(row for row in value['checks'] if row['id'] == 'WSI-B05').update(execution_authorized=False, confirmed_selection_steps=4, completed_task_get_responses=4)
    next(row for row in value['checks'] if row['id'] == 'WSI-B06').update(execution_authorized=False, prepare_requests=2, response_drops=1,
        parent_refresh_recovery=True, same_component_retained=True, fresh_task_read=True, fresh_scope_lists=True,
        locked_own_workspace=True, old_tasks_unchanged=True, old_workspaces_unchanged=True, source_unchanged=True, exact_replay_no_rows=True, no_execution_or_grants=True)
    for name in P.PNG_NAMES:
        kind = 'mobile' if 'mobile' in name else 'desktop'
        value['screenshots'].append({'name': name, 'sha256': P.sha(IMAGES[kind]),
                                     'viewport': {'width': 390 if kind == 'mobile' else 1440,
                                                  'height': 844 if kind == 'mobile' else 1100},
                                     'visual_review': SECRET})
    return value


class TaskWorkspaceProjectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='ab-task-projection-', dir='/tmp')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.private = self.root / 'private'
        self.private.mkdir(mode=0o700)
        self.number = 0

    def write(self, value=None):
        value = report() if value is None else value
        target = self.private / 'report.json'
        target.write_text(json.dumps(value)); target.chmod(0o600)
        for name in P.PNG_NAMES:
            path = self.private / name
            path.write_bytes(IMAGES['mobile' if 'mobile' in name else 'desktop']); path.chmod(0o600)
        (self.private / 'daemon.log').write_text(SECRET)
        (self.private / 'unlisted.png').write_bytes(SECRET.encode())
        return value

    def run_projection(self, private=None, head=HEAD):
        self.number += 1
        output = self.root / ('public-' + str(self.number))
        stream = io.StringIO()
        with mock.patch.object(P, 'expected_harness', return_value=HARNESS), contextlib.redirect_stdout(stream), contextlib.redirect_stderr(stream):
            code = P.main(['--private', str(private or self.private), '--output', str(output), '--expected-source-head', head])
        text = stream.getvalue()
        self.assertNotIn(SECRET, text)
        summary = json.loads((output / 'summary.json').read_bytes()) if (output / 'summary.json').exists() else None
        return code, summary, output

    def assert_no_images(self, output):
        if output.exists():
            self.assertFalse(any(path.suffix == '.png' for path in output.iterdir()))

    def test_success_is_finite_six_cases_eight_images_without_private_payload(self):
        original = self.write(); raw = (self.private / 'report.json').read_bytes()
        code, value, output = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(value['status'], 'passed')
        self.assertEqual(set(p.name for p in output.iterdir()), {'summary.json', *P.PNG_NAMES})
        self.assertEqual(value['report_sha256'], P.sha(raw))
        self.assertEqual(value['cases'], [{'id': name, 'status': 'passed'} for name in P.CASES])
        self.assertIs(value['no_execution']['driver_asserted_no_execution'], True)
        self.assertIs(value['private_history_published'], False)
        self.assertNotIn(SECRET, json.dumps(value)); self.assertNotIn('pid', json.dumps(value))
        self.assertEqual((self.private / 'report.json').read_bytes(), raw)
        self.assertEqual(original['http'][0]['token'], SECRET)
        for path in output.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            if path.suffix == '.png': self.assertEqual(path.read_bytes(), (self.private / path.name).read_bytes())

    def test_failed_valid_projection_returns_zero_but_never_pass_or_images(self):
        value = report(); value.update(status='failed', flow_complete=False, failure={'message': SECRET})
        value.pop('no_execution'); value['cleanup'] = {}
        self.write(value)
        code, projected, output = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(projected['status'], 'failed')
        self.assertFalse(projected['no_execution']['driver_asserted_no_execution'])
        self.assert_no_images(output); self.assertNotIn(SECRET, json.dumps(projected))

    def test_missing_private_or_report_is_pending_nonzero(self):
        for private in [self.private, self.root / 'absent-private']:
            code, value, output = self.run_projection(private=private)
            self.assertEqual(code, 2); self.assertEqual(value['status'], 'pending'); self.assert_no_images(output)

    def test_source_alias_and_harness_mismatch_is_fixed_invalid(self):
        for field in ['expected_source_head', 'compiled_core_source_head', 'frontend_build_head']:
            value = report(); value['inputs'][field] = '0' * 40; self.write(value)
            code, result, out = self.run_projection(); self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid'); self.assert_no_images(out)
        value = report(); value['harness_sha256'] = '0' * 64; self.write(value)
        self.assertEqual(self.run_projection()[0], 1)
        self.write(); self.assertEqual(self.run_projection(head=SECRET)[0], 1)

    def test_expected_harness_is_the_bounded_current_checkout_source_digest(self):
        path = ROOT / 'apps/desktop/tests/task-workspace-binding-real-ipc.py'
        self.assertEqual(P.expected_harness(), hashlib.sha256(path.read_bytes()).hexdigest())

    def test_checks_unknown_duplicate_missing_and_scenario_disagreement_refuse(self):
        for mode in ['unknown', 'duplicate', 'scenario', 'missing']:
            value = report()
            if mode == 'unknown': value['checks'][0]['id'] = SECRET
            if mode == 'duplicate': value['checks'][0] = value['checks'][1]
            if mode == 'scenario': value['scenarios']['WSI-B01'] = 'NOT_RUN'
            if mode == 'missing': value['scenarios'].pop('WSI-B01')
            self.write(value); code, result, out = self.run_projection()
            self.assertEqual(code, 1); self.assert_no_images(out); self.assertNotIn(SECRET, json.dumps(result))

    def test_incomplete_known_case_stays_not_run_no_images(self):
        value = report(); value['checks'].pop(0); value['scenarios']['WSI-B01'] = 'NOT_RUN'
        self.write(value); code, result, out = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['cases'][0], {'id': 'WSI-B01', 'status': 'NOT_RUN'}); self.assert_no_images(out)

    def test_b05_does_not_coerce_boolean_or_numeric_execution_proof(self):
        for key, bad in [('execution_authorized', 0), ('execution_authorized', True), ('confirmed_selection_steps', 4.0), ('completed_task_get_responses', True), ('completed_task_get_responses', 3)]:
            value = report(); next(row for row in value['checks'] if row['id'] == 'WSI-B05')[key] = bad; self.write(value)
            code, result, out = self.run_projection(); self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)

    def test_historical_five_cases_are_incomplete_or_invalid_never_new_success(self):
        value = report(); value['checks'] = [r for r in value['checks'] if r['id'] != 'WSI-B06']
        value['scenarios']['WSI-B06'] = 'NOT_RUN'; self.write(value)
        code, result, out = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)
        value['scenarios'].pop('WSI-B06'); self.write(value)
        self.assertEqual(self.run_projection()[0], 1)

    def test_b06_requires_strict_real_recovery_proof_fields_not_truthy_or_old_b05(self):
        fields = ['parent_refresh_recovery', 'same_component_retained', 'fresh_task_read', 'fresh_scope_lists',
                  'locked_own_workspace', 'old_tasks_unchanged', 'old_workspaces_unchanged', 'source_unchanged', 'exact_replay_no_rows', 'no_execution_or_grants']
        for field, invalid in [(key, 1) for key in fields] + [('execution_authorized', 0),
                                ('prepare_requests', 2.0), ('prepare_requests', 1), ('response_drops', True), ('response_drops', 0)]:
            with self.subTest(field=field, invalid=invalid):
                value = report(); next(r for r in value['checks'] if r['id'] == 'WSI-B06')[field] = invalid
                self.write(value); code, result, out = self.run_projection()
                self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)
        value = report(); row = next(r for r in value['checks'] if r['id'] == 'WSI-B06')
        row.pop('same_component_retained'); self.write(value)
        code, result, out = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)

    def test_b06_requires_explicit_old_worktree_fs_proof_not_only_old_task_records(self):
        for missing, value in [(False, False), (False, 1), (True, None)]:
            report_value = report()
            proof = next(row for row in report_value['checks'] if row['id'] == 'WSI-B06')
            if missing:
                proof.pop('old_workspaces_unchanged')
            else:
                proof['old_workspaces_unchanged'] = value
            self.write(report_value); code, result, out = self.run_projection()
            self.assertEqual(code, 0); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)

    def test_new_recovery_screenshot_pair_required_and_private_http_fields_not_published(self):
        value = self.write()
        for name in P.PNG_NAMES[-2:]:
            (self.private / name).unlink()
        code, result, out = self.run_projection()
        self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid'); self.assert_no_images(out)
        value['entry_read_proofs'] = {'raw': SECRET, 'key': SECRET}
        self.write(value)
        code, result, out = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'passed')
        self.assertNotIn('entry_read_proofs', result); self.assertNotIn(SECRET, json.dumps(result))

    def test_nullable_cleanup_absence_is_not_success(self):
        for name in P.NULL_CLEANUP:
            value = report(); value['cleanup'].pop(name); self.write(value)
            code, result, out = self.run_projection(); self.assertEqual(code, 0); self.assertFalse(result['cleanup']['complete']); self.assert_no_images(out)

    def test_typed_join_complete_and_input_drift_never_publish_images(self):
        changes = [(('complete',), False), (('daemon_stop', 'popen_reaped'), 1), (('daemon_stop', 'wait_exitcode'), False), (('http_thread', 'thread_alive_after_join'), 0), (('tools_after', 'baton', 'sha256'), '0' * 64)]
        for fields, bad in changes:
            value = report(); target = value['cleanup']
            for key in fields[:-1]: target = target[key]
            target[fields[-1]] = bad; self.write(value)
            _, result, out = self.run_projection(); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)
        value = report(); value['cleanup']['complete'] = 1; self.write(value)
        self.assertEqual(self.run_projection()[0], 1)

    def test_all_table_counts_exact_typed_bounded_and_zero_execution_domains(self):
        for name in P.TABLES:
            if name in P.NONZERO_ALLOWED: continue
            value = report(); value['no_execution']['table_counts'][name] = 1; self.write(value)
            code, result, out = self.run_projection(); self.assertEqual(code, 0); self.assertFalse(result['no_execution']['driver_asserted_no_execution']); self.assert_no_images(out)
        for mode in ['missing', 'extra', 'bool', 'float', 'negative', 'oversize']:
            value = report(); counts = value['no_execution']['table_counts']
            if mode == 'missing': counts.pop('sessions')
            elif mode == 'extra': counts[SECRET] = 0
            else: counts['sessions'] = {'bool': False, 'float': 0.0, 'negative': -1, 'oversize': 10001}[mode]
            self.write(value); code, _, out = self.run_projection(); self.assertEqual(code, 1); self.assert_no_images(out)

    def test_schema_old_future_and_float_do_not_coerce(self):
        for schema in [15, 17, 16.0, True, '16']:
            value = report(); value['no_execution']['schema'] = schema; self.write(value)
            self.assertEqual(self.run_projection()[0], 1)

    def test_errors_and_failure_presence_never_copy_arbitrary_diagnostics(self):
        for field, data in [('http_errors', [SECRET]), ('page_errors', [{'token': SECRET}]), ('failure', {}), ('flow_complete', 1)]:
            value = report(); value[field] = data; self.write(value)
            _, result, out = self.run_projection(); self.assertEqual(result['status'], 'failed'); self.assert_no_images(out)
            self.assertNotIn(SECRET, json.dumps(result))

    def test_json_duplicate_nonfinite_overflow_invalid_utf8_and_size_refuse(self):
        self.write(); path = self.private / 'report.json'
        for raw in [b'{"status":"passed","status":"failed"}', b'{"ignored":{"x":1,"x":2}}', b'{"ignored":NaN}', b'{"ignored":Infinity}', b'{"ignored":1e999}', b'\xff', b'', b' ' * (P.MAX_REPORT + 1)]:
            path.write_bytes(raw); code, result, out = self.run_projection()
            self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid'); self.assert_no_images(out)

    def test_report_symlink_hardlink_mode_fifo_directory_and_socket_refuse(self):
        for mode in ['symlink', 'hardlink', 'mode', 'fifo', 'directory', 'socket']:
            self.write(); path = self.private / 'report.json'; source = self.private / 'raw-input'
            if source.exists(): source.unlink()
            owner = None
            try:
                if mode == 'mode': path.chmod(0o644)
                elif mode == 'hardlink': os.link(path, source)
                else:
                    path.rename(source)
                    if mode == 'symlink': path.symlink_to(source)
                    elif mode == 'fifo': os.mkfifo(path, 0o600)
                    elif mode == 'directory': path.mkdir()
                    else: owner = socket.socket(socket.AF_UNIX); owner.bind(str(path))
                self.assertEqual(self.run_projection()[0], 1)
            finally:
                if owner: owner.close()
                if path.is_dir(): path.rmdir()
                elif path.exists() or path.is_symlink(): path.unlink()
                if source.exists(): source.unlink()

    def test_wrong_uid_refuses_without_exporting_file(self):
        self.write()
        with mock.patch.object(P.os, 'geteuid', return_value=os.geteuid() + 1):
            code, _, out = self.run_projection()
        self.assertEqual(code, 1); self.assert_no_images(out)

    def test_private_root_wrong_mode_and_symlink_ancestor_are_refused(self):
        self.write(); self.private.chmod(0o755)
        self.assertEqual(self.run_projection()[0], 1); self.private.chmod(0o700)
        alias = self.root / 'alias'; alias.symlink_to(self.root, target_is_directory=True)
        self.assertEqual(self.run_projection(private=alias / 'private')[0], 1)

    def test_mid_read_file_mutation_rejected_by_fd_witness(self):
        self.write(); target = self.private / 'report.json'; original = P.os.fdopen
        @contextlib.contextmanager
        def changing(fd, mode):
            with original(fd, mode) as stream:
                if mode != 'rb': yield stream; return
                class Changed:
                    def fileno(self): return stream.fileno()
                    def read(self, size):
                        value = stream.read(size); target.write_bytes(b'changed'); return value
                yield Changed()
        with mock.patch.object(P.os, 'fdopen', side_effect=changing):
            self.assertEqual(self.run_projection()[0], 1)

    def test_root_replacement_after_pinned_read_is_refused(self):
        self.write(); original = P.regular_bytes; changed = False
        def replacing(*args, **kwargs):
            nonlocal changed
            raw = original(*args, **kwargs)
            if not changed:
                changed = True; self.private.rename(self.root / 'old-private'); self.private.mkdir(mode=0o700)
            return raw
        with mock.patch.object(P, 'regular_bytes', side_effect=replacing):
            code, _, out = self.run_projection()
        self.assertEqual(code, 1); self.assert_no_images(out)

    def test_png_mutation_after_read_and_before_publish_refused(self):
        self.write(); original = P.screenshots
        def mutated(*args, **kwargs):
            value = original(*args, **kwargs)
            target = self.private / P.PNG_NAMES[0]; target.write_bytes(target.read_bytes())
            return value
        with mock.patch.object(P, 'screenshots', side_effect=mutated):
            self.assertEqual(self.run_projection()[0], 1)

    def test_screenshot_set_is_exact_no_duplicate_traversal_missing_or_failure_image(self):
        for mode in ['duplicate', 'missing', 'traversal', 'failure']:
            value = report()
            if mode == 'duplicate': value['screenshots'][0] = value['screenshots'][1]
            if mode == 'missing': value['screenshots'].pop()
            if mode == 'traversal': value['screenshots'][0]['name'] = '../' + SECRET
            if mode == 'failure': value['screenshots'][0]['name'] = 'failure-desktop.png'
            self.write(value); code, _, out = self.run_projection(); self.assertEqual(code, 1); self.assert_no_images(out)

    def test_png_hash_header_chunk_crc_trailing_bytes_and_text_payload_refuse(self):
        for mode in ['hash', 'header', 'crc', 'trailing', 'text']:
            value = self.write(); path = self.private / P.PNG_NAMES[0]; raw = path.read_bytes()
            if mode == 'hash': raw += b'bad'
            if mode == 'header': raw = b'not-PNG' + raw[7:]
            if mode == 'crc': raw = raw[:29] + b'\x00' * 4 + raw[33:]
            if mode == 'trailing': raw += SECRET.encode()
            if mode == 'text': raw = raw[:33] + chunk(b'tEXt', SECRET.encode()) + raw[33:]
            path.write_bytes(raw)
            if mode != 'hash':
                value['screenshots'][0]['sha256'] = P.sha(raw)
                (self.private / 'report.json').write_text(json.dumps(value))
            code, _, out = self.run_projection(); self.assertEqual(code, 1); self.assert_no_images(out)

    def test_png_ancillary_canary_and_raw_idat_rejected_via_main(self):
        secret = b'PRIVATE_STDERR_CANARY_NOT_SCREENSHOT_METADATA'
        for mode in ['gAMA-text', 'raw-IDAT']:
            with self.subTest(mode=mode):
                value = self.write(); path = self.private / P.PNG_NAMES[0]; original = path.read_bytes()
                if mode == 'gAMA-text':
                    raw = original[:33] + chunk(b'gAMA', secret) + original[33:]
                else:
                    raw = original[:33] + chunk(b'IDAT', secret) + chunk(b'IEND', b'')
                path.write_bytes(raw); value['screenshots'][0]['sha256'] = P.sha(raw)
                (self.private / 'report.json').write_text(json.dumps(value))
                code, result, out = self.run_projection()
                self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid')
                self.assert_no_images(out)
                self.assertNotIn(secret, (out / 'summary.json').read_bytes())

    def test_png_zlib_dimensions_filters_eof_and_metadata_fail_closed_via_main(self):
        rows = bytes((1440 * 4 + 1) * 1100)
        encoded = zlib.compress(rows)
        for mode in ['short', 'long', 'bad-filter', 'truncated', 'trailing', 'concatenated', 'metadata', 'interlaced']:
            with self.subTest(mode=mode):
                value = self.write(); path = self.private / P.PNG_NAMES[0]
                header = path.read_bytes()[:33]
                if mode == 'short': data = zlib.compress(rows[:-1])
                elif mode == 'long': data = zlib.compress(rows + b'\0')
                elif mode == 'bad-filter':
                    invalid = bytearray(rows); invalid[(1440 * 4 + 1) * 63] = 5
                    data = zlib.compress(invalid)
                elif mode == 'truncated': data = encoded[:-1]
                elif mode == 'trailing': data = encoded + SECRET.encode()
                elif mode == 'concatenated': data = encoded + zlib.compress(b'')
                else: data = encoded
                if mode == 'interlaced':
                    header = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1440, 1100, 8, 6, 0, 0, 1))
                # Legitimate ancillary metadata is deliberately unsupported,
                # not silently stripped or passed as opaque public payload.
                ancillary = chunk(b'gAMA', struct.pack('>I', 45455)) if mode == 'metadata' else b''
                raw = header + ancillary + chunk(b'IDAT', data) + chunk(b'IEND', b'')
                path.write_bytes(raw); value['screenshots'][0]['sha256'] = P.sha(raw)
                (self.private / 'report.json').write_text(json.dumps(value))
                code, result, out = self.run_projection()
                self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid'); self.assert_no_images(out)

    def test_png_rgb_multiple_idat_and_legal_filter_values_preserve_original_hash(self):
        value = self.write(); path = self.private / P.PNG_NAMES[0]
        row_bytes = 1440 * 3 + 1
        pixels = bytearray(row_bytes * 1100)
        for row in range(1100): pixels[row * row_bytes] = row % 5
        encoded = zlib.compress(pixels)
        raw = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1440, 1100, 8, 2, 0, 0, 0))
               + b''.join(chunk(b'IDAT', encoded[n:n + 7]) for n in range(0, len(encoded), 7)) + chunk(b'IEND', b''))
        path.write_bytes(raw); value['screenshots'][0]['sha256'] = P.sha(raw)
        (self.private / 'report.json').write_text(json.dumps(value))
        code, result, out = self.run_projection()
        self.assertEqual(code, 0); self.assertEqual(result['status'], 'passed')
        self.assertEqual((out / path.name).read_bytes(), raw)
        self.assertEqual(result['screenshots'][0]['sha256'], P.sha(raw))

    def test_png_decoded_budget_refuses_before_inflater_creation(self):
        self.write()
        with mock.patch.object(P, 'MAX_PNG_DECODED', 10), mock.patch.object(P.zlib, 'decompressobj') as inflate:
            code, result, out = self.run_projection()
        self.assertEqual(code, 1); self.assertEqual(result['status'], 'invalid'); self.assert_no_images(out)
        inflate.assert_not_called()

    def test_png_private_mode_link_and_byte_caps_refuse(self):
        for mode in ['permission', 'link', 'cap', 'total']:
            self.write(); path = self.private / P.PNG_NAMES[0]
            if mode == 'permission': path.chmod(0o644)
            if mode == 'link': os.link(path, self.private / 'image-alias')
            with mock.patch.object(P, 'MAX_PNG' if mode == 'cap' else 'MAX_PNG_TOTAL', 10 if mode in ('cap', 'total') else P.MAX_PNG_TOTAL):
                code, _, out = self.run_projection()
            self.assertEqual(code, 1); self.assert_no_images(out)
            alias = self.private / 'image-alias'
            if alias.exists(): alias.unlink()

    def test_failed_report_never_opens_even_a_listed_png(self):
        value = report(); value['status'] = 'failed'; self.write(value)
        original = P.regular_bytes
        def report_only(fd, name, *args, **kwargs):
            self.assertEqual(name, 'report.json'); return original(fd, name, *args, **kwargs)
        with mock.patch.object(P, 'regular_bytes', side_effect=report_only):
            self.assertEqual(self.run_projection()[0], 0)

    def test_output_overlap_and_existing_output_never_overwrite_private_or_unknown(self):
        self.write(); output = self.private / 'public'; stream = io.StringIO()
        with contextlib.redirect_stdout(stream):
            self.assertEqual(P.main(['--private', str(self.private), '--output', str(output), '--expected-source-head', HEAD]), 1)
        self.assertFalse(output.exists())
        output = self.root / 'existing'; output.mkdir(); sentinel = output / 'summary.json'; sentinel.write_text(SECRET)
        with mock.patch.object(P, 'expected_harness', return_value=HARNESS), contextlib.redirect_stdout(stream):
            self.assertEqual(P.main(['--private', str(self.private), '--output', str(output), '--expected-source-head', HEAD]), 1)
        self.assertEqual(sentinel.read_text(), SECRET)

    def test_actual_pretty_serialized_bytes_and_newline_define_one_budget(self):
        value = {'bounded': ['x' * 30 for _ in range(4)]}
        raw = (json.dumps(value, indent=2, allow_nan=False) + '\n').encode()
        self.assertGreater(len(raw), len(json.dumps(value, separators=(',', ':')).encode()))
        with mock.patch.object(P, 'MAX_PUBLIC_SUMMARY', len(raw)):
            self.assertEqual(P.serialized_summary(value), raw)
        with mock.patch.object(P, 'MAX_PUBLIC_SUMMARY', len(raw) - 1), self.assertRaises(P.Refused): P.serialized_summary(value)
        for value in [float('nan'), float('inf')]:
            with self.assertRaises(ValueError): P.serialized_summary({'value': value})

    def test_cli_unknown_missing_and_help_never_echo_input_or_exception(self):
        for argv in [[], ['--unknown-' + SECRET], ['--private', SECRET], ['--help', SECRET]]:
            out = io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out): self.assertEqual(P.main(argv), 1)
            self.assertNotIn(SECRET, out.getvalue()); self.assertIn('projection_arguments_invalid', out.getvalue())
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out): self.assertEqual(P.main(['--help']), 0)
        self.assertEqual(out.getvalue(), 'usage: record-task-workspace-ci-evidence.py --private DIR --output DIR --expected-source-head HEAD\n')

    def test_partial_write_and_late_summary_failure_leave_no_images(self):
        self.write()
        with mock.patch.object(P.os, 'fsync', side_effect=OSError(SECRET)):
            code, _, out = self.run_projection()
        self.assertEqual(code, 1); self.assertEqual(list(out.iterdir()), [])
        original = P.write_new
        def fail_summary(fd, name, raw):
            if name == 'summary.json': raise OSError(SECRET)
            return original(fd, name, raw)
        with mock.patch.object(P, 'write_new', side_effect=fail_summary): code, _, out = self.run_projection()
        self.assertEqual(code, 1); self.assertEqual(list(out.iterdir()), [])

    def test_publication_parent_symlink_never_writes_unknown_target(self):
        self.write(); alias = self.root / 'public-alias'; target = self.root / 'public-target'; target.mkdir(); alias.symlink_to(target, target_is_directory=True)
        out = io.StringIO()
        with mock.patch.object(P, 'expected_harness', return_value=HARNESS), contextlib.redirect_stdout(out):
            self.assertEqual(P.main(['--private', str(self.private), '--output', str(alias / 'not-created'), '--expected-source-head', HEAD]), 1)
        self.assertEqual(list(target.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
