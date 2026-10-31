#!/usr/bin/env python3
"""Finite CI projection for actual integration browser cases, never raw history.

Only a complete successful driver/probe/report/cleanup pair publishes twelve
validated original PNGs. Missing/invalid/failed evidence emits a finite failure
summary and nonzero exit. This is neither OS Native proof nor visual review.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import stat
import sys

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location('_integration_png_boundary', ROOT / 'scripts/record-task-workspace-ci-evidence.py')
P = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(P)
Refused = P.Refused
CONTRACT = 'agentbaton.integration-browser-ci.v1'
CASES = ('normal', 'command-failure')
NAMES = ('task-pending-before-acceptance', 'permission-before-task-acceptance',
    'terminal-state-desktop', 'terminal-receipt-desktop', 'terminal-state-mobile', 'terminal-receipt-mobile')
PUBLIC_NAMES = frozenset(case + '-' + name + '.png' for case in CASES for name in NAMES)
MAX_REPORT, MAX_STATUS, MAX_PNG, MAX_TOTAL = 2097152, 65536, 16777216, 67108864
DIMENSION = 'Linux built React/Chromium + finite TEST bridge + actual Core17 Unix IPC + generated ACP + private trusted test host'
NOT_CLAIMED = ['production Tauri Native formatter', 'OS dialog', 'macOS', 'live Provider/model',
    'OS sandbox', 'whole-filesystem output quota', 'complete failure matrix', 'whole product completion']
CHECKS = ('real managed Task/Graph before Task acceptance; visible new5s draft; optional original Command selected',
    'actual Core RESOURCE_LIMIT for oversized default output; no target/fence/policy/run/attempt authority',
    'complete real pending proposal and consumed receipt matched outside HTTP before TaskAccepted',
    'actual watcher fresh merged Command/owned stop/join/shared fees/source preservation and exact local ref result',
    'actual built component displays separate Task/integration result and real receipt; desktop/mobile screenshots')
PRIVATE_FACTS = {'proposal_sha256', 'snapshot_sha256', 'proposal_bytes', 'snapshot_bytes',
    'whole_response_bytes', 'whole_response_sha256'}
TERMINAL_FACTS = {'state', 'stages', 'command_exit', 'cas_attempts', 'source_and_index_unchanged',
    'original_command_unchanged', 'actual_owned_reap_and_join', 'shared_runtime_ms',
    'shared_output_bytes', 'tokens', 'cost_microunits'}
INPUT_KEYS = {'source', 'tools', 'assets', 'manifest_sha256', 'build_record_sha256', 'driver_sha256', 'source_guard', 'runtime_tools'}
HTTP_READS = {'system.snapshot', 'projects.list', 'connections.list', 'sessions.list', 'tasks.get',
    'goal_scopes.list', 'events.subscribe', 'integration_context.get', 'integration_targets.list',
    'integration_policies.get', 'integration_policies.list', 'integrations.get', 'integrations.list'}
CLEANUP_BOOLS = {'pending_reviews_stopped', 'browser_closed', 'playwright_stopped', 'http_shutdown',
    'http_thread_joined', 'private_transcript_saved', 'tools_assets_unchanged', 'fixture_removed'}


def require(value):
    if not value:
        raise Refused('Finite integration browser evidence refused')


def number(value, low=0, high=9007199254740991):
    require(type(value) is int and low <= value <= high)
    return value


def sha(value, width=64):
    require(type(value) is str and re.fullmatch('[a-f0-9]{' + str(width) + '}', value) is not None)
    return value


def raw_sha(raw):
    return hashlib.sha256(raw).hexdigest()


def tool(value, maximum):
    require(type(value) is dict and set(value) == {'sha256', 'bytes', 'mode', 'device', 'inode', 'uid', 'links'})
    number(value['bytes'], 1, maximum); sha(value['sha256']); number(value['mode'], 0, 0o7777)
    require(value['mode'] & 0o022 == 0 and number(value['uid']) in (0, os.geteuid()))
    number(value['links'], 1)
    for key in ('device', 'inode'):
        require(type(value[key]) is str and re.fullmatch('0|[1-9][0-9]*', value[key]) is not None)
    return value['sha256']


def expected_driver():
    fd = P.directory(ROOT / 'apps/desktop/tests')
    try:
        return raw_sha(P.regular_bytes(fd, 'integration-real-ipc.py', 1048576, private=False))
    finally:
        os.close(fd)


def input_summary(value, expected, manifest_hash, build_hash, harness):
    require(type(value) is dict and set(value) == INPUT_KEYS and value['source'] == expected
        and value['manifest_sha256'] == manifest_hash and value['build_record_sha256'] == build_hash
        and value['driver_sha256'] == harness)
    guard = value['source_guard']
    require(type(guard) is dict and set(guard) == {'head', 'tracked_clean', 'driver_sha256'}
        and guard['head'] == expected and guard['tracked_clean'] is True and guard['driver_sha256'] == harness)
    require(type(value['tools']) is dict and set(value['tools']) == {'batond', 'baton'})
    tools = {k: tool(v, 268435456) for k, v in value['tools'].items()}
    require(all(v['mode'] == 0o500 and v['links'] == 1 and v['uid'] == os.geteuid() for v in value['tools'].values()))
    require(type(value['runtime_tools']) is dict and set(value['runtime_tools']) == {'python', 'chromium'})
    runtime = {k: tool(v, 536870912 if k == 'chromium' else 268435456) for k, v in value['runtime_tools'].items()}
    assets = value['assets']; require(type(assets) is dict and 'index.html' in assets and 0 < len(assets) <= 512)
    total = 0
    for key, record in assets.items():
        require(type(key) is str and re.fullmatch('[A-Za-z0-9._/-]{1,256}', key) is not None
            and not key.startswith('/') and '..' not in key.split('/'))
        tool(record, 16777216); total += record['bytes']
    require(total <= 67108864)
    return {'source': expected, 'driver_sha256': harness, 'manifest_sha256': manifest_hash,
        'build_record_sha256': build_hash, 'tools_sha256': tools, 'runtime_tools_sha256': runtime,
        'asset_count': len(assets), 'assets_sha256': raw_sha(json.dumps(assets, sort_keys=True, separators=(',', ':')).encode())}


def cleanup(value):
    require(type(value) is dict and set(value) == CLEANUP_BOOLS | {'daemon_joined'})
    require(all(value[k] is True for k in CLEANUP_BOOLS))
    daemon = value['daemon_joined']
    require(type(daemon) is dict and set(daemon) == {'exit', 'joined', 'forced_kill'}
        and type(daemon['exit']) is int and daemon['exit'] == 0
        and daemon['joined'] is True and daemon['forced_kill'] is False)
    return {'owned_daemon_exit': 0, 'joined': True, 'http_handlers_joined': True,
        'browser_closed': True, 'source_tools_assets_unchanged': True, 'fixture_removed': True}


def check_summary(report, case):
    expected_names = CHECKS if case == 'normal' else (CHECKS[0], *CHECKS[2:])
    checks = report['checks']; require(type(checks) is list and len(checks) == len(expected_names))
    require([c.get('name') if type(c) is dict else None for c in checks] == list(expected_names))
    terminal = None
    for row in checks:
        require(row.get('status') == 'passed')
        name = row['name']
        fields = PRIVATE_FACTS if name == CHECKS[2] else TERMINAL_FACTS if name == CHECKS[3] else set()
        require(set(row) == {'name', 'status'} | fields)
        if fields == PRIVATE_FACTS:
            for key in ('proposal_sha256', 'snapshot_sha256', 'whole_response_sha256'): sha(row[key])
            for key in ('proposal_bytes', 'snapshot_bytes', 'whole_response_bytes'): number(row[key], 1, 1048576)
        if fields == TERMINAL_FACTS:
            failing = case == 'command-failure'
            require(row['state'] == ('failed' if failing else 'confirmed_local_ref')
                and type(row['stages']) is int and row['stages'] == (2 if failing else 3)
                and type(row['cas_attempts']) is int and row['cas_attempts'] == (0 if failing else 1)
                and type(row['command_exit']) is int and row['command_exit'] == (7 if failing else 0))
            require(all(row[k] is True for k in ('source_and_index_unchanged', 'original_command_unchanged', 'actual_owned_reap_and_join'))
                and row['tokens'] is None and row['cost_microunits'] is None)
            number(row['shared_runtime_ms'], 1, 20000); number(row['shared_output_bytes'], 1, 8192)
            terminal = {k: row[k] for k in ('state', 'stages', 'cas_attempts', 'command_exit',
                'shared_runtime_ms', 'shared_output_bytes', 'tokens', 'cost_microunits')}
    require(terminal is not None)
    methods = report['http_method_counts']
    require(type(methods) is dict and set(methods) <= HTTP_READS | {'integration_policies.propose'})
    require(methods.get('integration_policies.propose') == (2 if case == 'normal' else 1))
    for count in methods.values(): number(count, 1, 100000)
    number(report['readonly_reply_aborts'], 0, 100000)
    return {'checks': len(checks), 'terminal': terminal, 'automatic_watcher_without_start': True}


def report_summary(report, case, expected, manifest_hash, build_hash, harness):
    require(type(report) is dict and set(report) == {'case', 'status', 'dimension', 'not_claimed', 'checks',
        'screenshots', 'cleanup', 'inputs', 'browser_version', 'http_method_counts', 'readonly_reply_aborts'})
    require(report['status'] == 'passed' and report['case'] == ('command_exit7' if case == 'command-failure' else 'normal')
        and report['dimension'] == DIMENSION and report['not_claimed'] == NOT_CLAIMED)
    require(type(report['browser_version']) is str and re.fullmatch('[0-9.]{1,128}', report['browser_version']) is not None)
    source = input_summary(report['inputs'], expected, manifest_hash, build_hash, harness)
    return {'case': case, 'status': 'passed', **check_summary(report, case), 'cleanup': cleanup(report['cleanup'])}, source


def screenshots(fd, report, case, witnesses):
    items = report['screenshots']; require(type(items) is list and len(items) == 6)
    require([r.get('name') if type(r) is dict else None for r in items] == list(NAMES))
    result, metadata, total = {}, [], 0
    for row in items:
        require(set(row) == {'name', 'sha256', 'bytes', 'viewport', 'focus_box_before_capture_observation',
            'focus_viewport_asserted_before_capture', 'pixel_synchronized', 'dimension'})
        require(row['focus_viewport_asserted_before_capture'] is True and row['pixel_synchronized'] is False
            and row['dimension'] == 'Linux Chromium only')
        box = row['focus_box_before_capture_observation']
        require(type(box) is dict and set(box) == {'x', 'y', 'width', 'height'})
        for key, value in box.items():
            require(type(value) in (int, float) and math.isfinite(value) and abs(value) <= 10000000
                and (value > 0 if key in ('width', 'height') else True))
        name = row['name'] + '.png'
        raw = P.regular_bytes(fd, name, MAX_PNG, witnesses=witnesses)
        require(number(row['bytes'], 1, MAX_PNG) == len(raw))
        total += len(raw); require(total <= MAX_TOTAL)
        image = P.png_bytes(raw, row, name)
        require(image['height'] == row['viewport']['height'])  # No full-page substitute.
        public_name = case + '-' + name
        require(public_name in PUBLIC_NAMES)
        image['name'] = public_name
        metadata.append(image); result[public_name] = raw
    return result, metadata, total


def failure(expected, reason):
    return {'contract': CONTRACT, 'status': 'failed', 'source': expected,
        'failure_kind': reason, 'private_history_published': False, 'screenshots_published': False,
        'visual_review': 'not_performed_by_projector', 'pixel_synchronized': False,
        'not_claimed': NOT_CLAIMED}


def collect(private, expected):
    sha(expected, 40)
    fd = P.directory(private, private=True)
    witnesses, opened = [], []
    try:
        status_raw = P.regular_bytes(fd, 'status.json', MAX_STATUS, witnesses=witnesses)
        status = P.decode(status_raw)
        require(type(status) is dict and set(status) == {'source', 'driver_exit', 'probe_passed'}
            and status['source'] == expected and type(status['driver_exit']) is int
            and status['driver_exit'] == 0 and status['probe_passed'] is True)
        manifest_raw = P.regular_bytes(fd, 'candidate.json', MAX_REPORT, witnesses=witnesses)
        build_raw = P.regular_bytes(fd, 'frontend-build-record.json', MAX_REPORT, witnesses=witnesses)
        probe_raw = P.regular_bytes(fd, 'schema-probe.json', MAX_STATUS, witnesses=witnesses)
        manifest, build, probe = P.decode(manifest_raw), P.decode(build_raw), P.decode(probe_raw)
        require(type(manifest.get('schema')) is int and manifest['schema'] == 17 and manifest['git_head'] == expected
            and manifest.get('dirty') is False and manifest.get('tracked_clean', True) is True
            and set(manifest['binaries']) == {'batond', 'baton'}
            and all(manifest.get(k, expected) == expected for k in ('source_head', 'compiled_core_source_head')))
        require(build['source'] == expected and type(build['exit']) is int and build['exit'] == 0
            and build['source_head_and_tracked_unchanged'] is True
            and build.get('dirty', False) is False and build.get('tracked_clean', True) is True
            and build['command'] == ['npm', '--prefix', 'apps/desktop', 'run', 'build'])
        require(type(probe) is dict and set(probe) == {'source', 'api_version', 'database_schema_version', 'daemon_exit', 'joined'}
            and probe['source'] == expected and type(probe['api_version']) is int and probe['api_version'] == 1
            and type(probe['database_schema_version']) is int and probe['database_schema_version'] == 17
            and type(probe['daemon_exit']) is int and probe['daemon_exit'] == 0 and probe['joined'] is True)
        summaries, sources, payloads, images, total = [], [], {}, [], 0
        case_witnesses = []
        harness = expected_driver()
        driver_fd = P.directory(Path(private) / 'driver', private=True); opened.append(driver_fd)
        for case in CASES:
            path = Path(private) / 'driver' / case
            case_fd = P.directory(path, private=True); opened.append(case_fd)
            local_witness = []
            report_raw = P.regular_bytes(case_fd, 'report.json', MAX_REPORT, witnesses=local_witness)
            report = P.decode(report_raw)
            summary, source = report_summary(report, case, expected, raw_sha(manifest_raw), raw_sha(build_raw), harness)
            require(source['tools_sha256'] == manifest['binaries'])
            data, pngs, size = screenshots(case_fd, report, case, local_witness)
            case_witnesses.append((path, case_fd, local_witness))
            total += size; require(total <= MAX_TOTAL)
            summaries.append(summary); sources.append(source); payloads.update(data); images.extend(pngs)
            P.same_directory(path, case_fd)
            for name, original_pin in local_witness:
                require(P.pin(os.stat(name, dir_fd=case_fd, follow_symlinks=False)) == original_pin)
        require(sources[0] == sources[1] and len(images) == len(payloads) == 12
            and sum(s['checks'] for s in summaries) == 9)
        for path, case_fd, local_witness in case_witnesses:
            P.same_directory(path, case_fd)
            for name, original_pin in local_witness:
                require(P.pin(os.stat(name, dir_fd=case_fd, follow_symlinks=False)) == original_pin)
        P.same_directory(Path(private) / 'driver', driver_fd)
        P.same_directory(private, fd)
        for name, original_pin in witnesses:
            require(P.pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == original_pin)
        return {'contract': CONTRACT, 'status': 'passed', **sources[0], 'cases': summaries,
            'checks': 9, 'actual_schema_probe': {'api_version': 1, 'database_schema_version': 17, 'joined': True},
            'private_history_published': False, 'screenshots_published': True, 'screenshots': images,
            'visual_review': 'not_performed_by_projector', 'pixel_synchronized': False,
            'not_claimed': NOT_CLAIMED}, payloads
    finally:
        for child in reversed(opened): os.close(child)
        os.close(fd)


SETUP_STAGES = ('tracked_source_guard', 'paired_record_read', 'driver_import', 'paired_record_decode',
    'paired_metadata_guard', 'private_records', 'private_tools', 'copy_batond', 'copy_baton',
    'schema_probe', 'chromium_discovery', 'chromium_pin', 'driver_log_open', 'driver_run', 'driver_returned')
SETUP_EXCEPTIONS = ('import_error', 'os_error', 'value_error', 'key_error', 'type_error',
    'attribute_error', 'other_exception')


def setup_diagnostic(private, expected):
    """Strict independent finite setup projection; never read logs/runtime/DB."""
    unavailable = {'status': 'unavailable', 'failure_kind': 'missing_or_invalid_setup_diagnostic'}
    try:
        fd = P.directory(private, private=True)
        try:
            witnesses = []
            value = P.decode(P.regular_bytes(fd, 'setup-diagnostic.json', 4096, witnesses=witnesses))
            status = P.decode(P.regular_bytes(fd, 'status.json', MAX_STATUS, witnesses=witnesses))
            require(type(value) is dict and set(value) == {'source', 'stage', 'exception_kind',
                'browser_discovered', 'browser_pinned', 'traceback_bytes'} and value['source'] == expected)
            require(type(status) is dict and set(status) == {'source', 'driver_exit', 'probe_passed'}
                and status['source'] == expected and type(status['probe_passed']) is bool)
            stage, exception = value['stage'], value['exception_kind']
            require(type(stage) is str and stage in SETUP_STAGES
                and (exception is None or type(exception) is str and exception in SETUP_EXCEPTIONS))
            require(all(type(value[key]) is bool for key in ('browser_discovered', 'browser_pinned')))
            position = SETUP_STAGES.index(stage)
            require(status['probe_passed'] is (position > SETUP_STAGES.index('schema_probe'))
                and value['browser_discovered'] is (position > SETUP_STAGES.index('chromium_discovery'))
                and value['browser_pinned'] is (position > SETUP_STAGES.index('chromium_pin')))
            trace_size = number(value['traceback_bytes'], 0, 16384)
            exit_code = status['driver_exit']
            if stage == 'driver_returned':
                require(exception is None and type(exit_code) is int and -255 <= exit_code <= 255 and trace_size == 0)
            else:
                require(exception in SETUP_EXCEPTIONS and exit_code is None and trace_size > 0)
            P.same_directory(private, fd)
            for name, original_pin in witnesses:
                require(P.pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == original_pin)
            return {'status': 'observed', 'stage': stage, 'exception_kind': exception,
                'probe_passed': status['probe_passed'], 'browser_discovered': value['browser_discovered'],
                'browser_pinned': value['browser_pinned'], 'driver_exit': exit_code, 'traceback_bytes': trace_size}
        finally:
            os.close(fd)
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        return unavailable


DRIVER_PREFLIGHT_STAGES = ('expected_source_head', 'core_paths', 'core_tool_pins',
    'core_executable_guard', 'core_owner_mode_guard', 'candidate_read', 'candidate_size_guard',
    'candidate_decode', 'candidate_schema_guard', 'candidate_source_guard', 'candidate_clean_guard',
    'candidate_tool_hash_guard', 'frontend_record_read', 'frontend_record_size_guard',
    'frontend_record_decode', 'frontend_record_guard', 'frontend_resolve', 'chromium_resolve',
    'runtime_chromium_pin', 'runtime_python_pin', 'runtime_owner_mode_guard', 'source_guard',
    'frontend_assets', 'run_required', 'playwright_import')
DRIVER_PREFLIGHT_EXCEPTIONS = ('refusal', 'import_error', 'os_error', 'value_error', 'key_error',
    'type_error', 'attribute_error', 'interrupted', 'system_exit', 'other_exception')
DRIVER_RUNTIME_CHECKS = frozenset(('chromium_owner_allowed', 'chromium_mode_allowed',
    'python_owner_allowed', 'python_mode_allowed'))


def driver_preflight_diagnostic(private, expected):
    """Closed exit2 observation, not source-guard success or external authority."""
    unavailable = {'status': 'unavailable', 'failure_kind': 'missing_or_invalid_driver_preflight_diagnostic'}
    root_fd = driver_fd = None
    try:
        sha(expected, 40)
        root_fd = P.directory(private, private=True)
        root_witnesses, driver_witnesses = [], []
        status = P.decode(P.regular_bytes(root_fd, 'status.json', MAX_STATUS, witnesses=root_witnesses))
        require(type(status) is dict and set(status) == {'source', 'driver_exit', 'probe_passed'}
            and status['source'] == expected and type(status['driver_exit']) is int
            and status['driver_exit'] == 2 and status['probe_passed'] is True)
        path = Path(private) / 'driver'
        driver_fd = P.directory(path, private=True)
        value = P.decode(P.regular_bytes(driver_fd, 'preflight-diagnostic.json', 4096, witnesses=driver_witnesses))
        require(type(value) is dict and set(value) == {'version', 'declared_source', 'driver_sha256',
            'stage', 'exception_kind', 'run_requested', 'runtime_case_entered', 'runtime_checks'})
        require(type(value['version']) is int and value['version'] == 1
            and value['declared_source'] == expected and value['driver_sha256'] == expected_driver()
            and type(value['run_requested']) is bool and value['runtime_case_entered'] is False)
        stage, exception = value['stage'], value['exception_kind']
        require(type(stage) is str and stage in DRIVER_PREFLIGHT_STAGES)
        checks = value['runtime_checks']
        require(type(checks) is dict and set(checks) == DRIVER_RUNTIME_CHECKS
            and (all(v is None for v in checks.values()) or all(type(v) is bool for v in checks.values())))
        position, owner_guard = DRIVER_PREFLIGHT_STAGES.index(stage), DRIVER_PREFLIGHT_STAGES.index('runtime_owner_mode_guard')
        if position < owner_guard:
            require(all(v is None for v in checks.values()))
        elif position > owner_guard:
            require(all(v is True for v in checks.values()))
        if stage == 'run_required':
            require(exception is None and value['run_requested'] is False)
            outcome = 'run_required'
        elif stage == 'playwright_import':
            require(exception == 'import_error' and value['run_requested'] is True)
            outcome = 'dependency_import_failed'
        else:
            require(type(exception) is str and exception in DRIVER_PREFLIGHT_EXCEPTIONS)
            outcome = 'preflight_refused'
        for location, fd, witnesses in ((private, root_fd, root_witnesses), (path, driver_fd, driver_witnesses)):
            P.same_directory(location, fd)
            for name, original_pin in witnesses:
                require(P.pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == original_pin)
        return {'status': 'observed', 'declared_source': expected, 'driver_sha256': value['driver_sha256'],
            'stage': stage, 'exception_kind': exception, 'outcome': outcome,
            'run_requested': value['run_requested'], 'runtime_case_entered': False,
            'runtime_checks': checks}
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        return unavailable
    finally:
        for fd in (driver_fd, root_fd):
            if fd is not None:
                os.close(fd)


def publish(output, summary, payloads):
    require(Path(output).is_absolute() and '..' not in Path(output).parts)
    parent = P.directory(Path(output).parent)
    try:
        os.mkdir(Path(output).name, 0o700, dir_fd=parent)
    finally:
        os.close(parent)
    fd = P.directory(output, private=True)
    written = {}
    try:
        P.same_directory(output, fd)
        for name, raw in payloads.items():
            require(name in PUBLIC_NAMES)
            written[name] = P.write_new(fd, name, raw)
        raw = (json.dumps(summary, indent=2, allow_nan=False) + '\n').encode()
        require(len(raw) <= 131072)
        written['summary.json'] = P.write_new(fd, 'summary.json', raw)
        P.same_directory(output, fd)
    except BaseException:
        for name, stamp in written.items():
            try:
                if P.pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == stamp:
                    os.unlink(name, dir_fd=fd)
            except OSError: pass
        raise
    finally:
        os.close(fd)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--private', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expected-source-head', required=True)
    parser.add_argument('--driver-step-outcome', choices=('success', 'failure', 'skipped', 'cancelled', 'unobserved'), required=True)
    args = parser.parse_args(argv)
    try:
        sha(args.expected_source_head, 40)
        try:
            if args.driver_step_outcome != 'success':
                summary, payloads = failure(args.expected_source_head, 'driver_step_not_successful'), {}
            else:
                summary, payloads = collect(args.private, args.expected_source_head)
        except FileNotFoundError:
            summary, payloads = failure(args.expected_source_head, 'missing_private_evidence'), {}
        except (ValueError, OSError, KeyError, TypeError, OverflowError):
            summary, payloads = failure(args.expected_source_head, 'invalid_failed_or_incomplete_private_evidence'), {}
        if summary['status'] != 'passed':
            summary['setup_diagnostic'] = setup_diagnostic(args.private, args.expected_source_head)
            summary['driver_preflight'] = driver_preflight_diagnostic(args.private, args.expected_source_head)
        publish(args.output, summary, payloads)
        return 0 if summary['status'] == 'passed' else 1
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        print('Finite integration browser projection refused', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
