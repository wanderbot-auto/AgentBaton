#!/usr/bin/env python3
"""Finite Task-workspace CI evidence; private driver output is never an upload list.

Exit 0 means a structurally valid projection, including a failed driver report;
it is not a test-pass receipt. Missing reports return 2; invalid input returns 1.
The caller must retain the original driver exit as an independent required gate.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import struct
import zlib

CONTRACT = 'agentbaton.task-workspace-ci.v1'
CASES = ('WSI-B01', 'WSI-B02', 'WSI-B03', 'WSI-B04', 'WSI-B05')
PNG_NAMES = (
    '01-first-task-ready-desktop.png', '02-second-task-exact-recovered-desktop.png',
    '03-drift-invalidates-proof-desktop.png', '04-drift-history-mobile.png',
    '05-task2-bound-execution-desktop.png', '06-task1-bound-execution-mobile.png',
)
TABLES = tuple('''approval_proposals attempts budgets command_validation_policies
command_validation_runs command_workspace_fences connections data_validation_policies
data_validation_runs data_workspace_creations data_workspace_previews data_workspace_receipts
dispatch_intents events execution_grants factory_attempts factory_budget_reservations
factory_child_goal_runs factory_child_plan_heads factory_child_plan_revisions factory_delegations
factory_handoffs factory_runs factory_scoped_handles factory_scopes factory_task_bindings
factory_task_lineage goal_external_action_journal goal_grant_revisions goal_graph_acceptances
goal_graph_candidates goal_graph_capabilities goal_graph_jobs goal_graph_owner_decisions
goal_graph_validations goal_ledgers goal_node_evidence goal_plan_heads goal_plan_proposals
goal_plan_revisions goal_run_nodes goal_run_reservations goal_runs goal_scopes idempotency
managed_workspaces projects session_runtime_snapshots session_turns sessions sqlite_sequence
tasks validation_policies validation_runs workspace_leases'''.split())
NONZERO_ALLOWED = {'events', 'idempotency', 'managed_workspaces', 'projects', 'sqlite_sequence', 'tasks'}
NULL_CLEANUP = ('browser_close', 'playwright_stop', 'http_shutdown',
                'http_close_and_handlers_join', 'log_close', 'temporary_cleanup')
MAX_REPORT = 4 * 1024 * 1024
MAX_PNG = 32 * 1024 * 1024
MAX_PNG_TOTAL = 64 * 1024 * 1024
MAX_PUBLIC_SUMMARY = 128 * 1024


class Refused(ValueError):
    pass


def require(value):
    if not value:
        raise Refused('Finite Task-workspace evidence refused')


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def digest(value, width=64):
    require(type(value) is str and re.fullmatch('[0-9a-f]{' + str(width) + '}', value) is not None)
    return value


def bounded(value, low, high):
    require(type(value) is int and low <= value <= high)
    return value


def unique(pairs):
    out = {}
    for key, value in pairs:
        require(key not in out)
        out[key] = value
    return out


def finite_float(text):
    value = float(text)
    require(math.isfinite(value))
    return value


def decode(raw):
    return json.loads(raw, object_pairs_hook=unique, parse_float=finite_float,
                      parse_constant=lambda _: require(False))


def pin(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def directory(path, private=False):
    """Descriptor-walk every component: callers must supply canonical absolute paths."""
    path = Path(path)
    require(path.is_absolute() and '..' not in path.parts)
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for component in path.parts[1:]:
            next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                              dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_fd
        info = os.fstat(descriptor)
        require(stat.S_ISDIR(info.st_mode))
        if private:
            require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def same_directory(path, descriptor, private=True):
    other = directory(path, private=private)
    try:
        left, right = os.fstat(descriptor), os.fstat(other)
        require((left.st_dev, left.st_ino) == (right.st_dev, right.st_ino))
    finally:
        os.close(other)


def regular_bytes(descriptor, name, maximum, private=True, witnesses=None):
    require(type(name) is str and '/' not in name and name not in ('', '.', '..'))
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=descriptor)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                and before.st_nlink == 1 and 0 < before.st_size <= maximum)
        if private:
            require(stat.S_IMODE(before.st_mode) == 0o600)
        else:
            require(not stat.S_IMODE(before.st_mode) & 0o022)
        raw = stream.read(maximum + 1)
        after = os.fstat(stream.fileno())
        named = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
        require(pin(before) == pin(after) == pin(named) and len(raw) == before.st_size)
        if witnesses is not None:
            witnesses.append((name, pin(named)))
    return raw


def expected_harness():
    path = Path(__file__).absolute().parents[1] / 'apps/desktop/tests/task-workspace-binding-real-ipc.py'
    fd = directory(path.parent)
    try:
        return sha(regular_bytes(fd, path.name, 1024 * 1024, private=False))
    finally:
        os.close(fd)


def files_metadata(value, tools=False):
    require(type(value) is dict and 0 < len(value) <= (2 if tools else 4096))
    if tools:
        require(set(value) == {'baton', 'batond'})
    result = {}
    for name, row in value.items():
        require(type(name) is str and len(name) <= 256 and re.fullmatch('[A-Za-z0-9._/-]+', name)
                and not name.startswith('/') and '..' not in name.split('/'))
        require(type(row) is dict)
        mode = bounded(row.get('mode'), 0, 0o7777)
        if tools:
            require(mode == 0o500)
        result[name] = {'bytes': bounded(row.get('bytes'), 1, 134217728),
                        'mode': mode, 'sha256': digest(row.get('sha256'))}
    return result


def source_summary(report, expected):
    inputs = report.get('inputs')
    require(type(inputs) is dict)
    for key in ('expected_source_head', 'compiled_core_source_head', 'frontend_build_head'):
        require(digest(inputs.get(key), 40) == expected)
    tools = files_metadata(inputs.get('tools'), tools=True)
    assets = files_metadata(inputs.get('frontend_assets'))
    result = {'source_head': expected, 'compiled_core_source_head': expected,
              'frontend_build_head': expected, 'manifest_sha256': digest(inputs.get('manifest_sha256')),
              'frontend_build_record_sha256': digest(inputs.get('frontend_build_record_sha256')),
              'tools_sha256': {name: tools[name]['sha256'] for name in ('baton', 'batond')},
              'frontend_asset_count': len(assets),
              'frontend_assets_sha256': sha(json.dumps(assets, sort_keys=True, separators=(',', ':')).encode())}
    return result, tools, assets


def cleanup_summary(report, tools, assets):
    value = report.get('cleanup')
    require(type(value) is dict)
    if 'complete' in value:
        require(type(value['complete']) is bool)
    # None is the actual success return for these methods; absence is not None.
    nullable = all(key in value and value[key] is None for key in NULL_CLEANUP)
    stop, http = value.get('daemon_stop'), value.get('http_thread')
    stopped = (type(stop) is dict and type(stop.get('wait_exitcode')) is int
               and stop['wait_exitcode'] == 0 and stop.get('popen_reaped') is True)
    joined = type(http) is dict and http.get('thread_alive_after_join') is False
    unchanged = False
    if 'tools_after' in value and 'assets_after' in value:
        unchanged = (files_metadata(value['tools_after'], tools=True) == tools
                     and files_metadata(value['assets_after']) == assets)
    complete = value.get('complete') is True and nullable and stopped and joined and unchanged
    return {'complete': complete, 'daemon_joined': stopped, 'http_joined': joined,
            'nullable_cleanup_complete': nullable, 'input_hashes_unchanged': unchanged}


def execution_summary(report):
    value = report.get('no_execution')
    if value is None:
        return {'driver_asserted_no_execution': False}
    require(type(value) is dict and type(value.get('schema')) is int and value['schema'] == 16
            and type(value.get('no_execution_or_grants')) is bool)
    counts = value.get('table_counts')
    require(type(counts) is dict and set(counts) == set(TABLES))
    for number in counts.values():
        bounded(number, 0, 10000)
    zero = all(counts[name] == 0 for name in TABLES if name not in NONZERO_ALLOWED)
    return {'schema': 16, 'driver_asserted_no_execution': value['no_execution_or_grants'] is True and zero,
            'table_counts': {name: counts[name] for name in TABLES}}


def project(report, report_hash, expected, harness):
    digest(expected, 40)
    require(type(report) is dict and report.get('status') in ('passed', 'failed', 'failed_or_interrupted'))
    require(digest(report.get('harness_sha256')) == harness)
    source, tools, assets = source_summary(report, expected)
    require(type(report.get('checks')) is list and len(report['checks']) <= 5)
    passed = set()
    b05 = False
    for row in report['checks']:
        require(type(row) is dict and type(row.get('id')) is str and row['id'] in CASES
                and row['id'] not in passed and row.get('status') == 'passed')
        passed.add(row['id'])
        if row['id'] == 'WSI-B05':
            b05 = (row.get('execution_authorized') is False
                   and type(row.get('confirmed_selection_steps')) is int and row['confirmed_selection_steps'] == 4
                   and type(row.get('completed_task_get_responses')) is int and 4 <= row['completed_task_get_responses'] <= 2048)
    scenarios = report.get('scenarios')
    require(type(scenarios) is dict and set(scenarios) == set(CASES))
    require(all(type(s) is str and s in ('passed', 'NOT_RUN') for s in scenarios.values()))
    require({key for key in CASES if scenarios[key] == 'passed'} == passed)
    for key in ('http_errors', 'page_errors'):
        require(type(report.get(key)) is list and len(report[key]) <= 2048)
    cleanup = cleanup_summary(report, tools, assets)
    execution = execution_summary(report)
    complete = (report['status'] == 'passed' and report.get('flow_complete') is True
                and 'failure' not in report and passed == set(CASES) and b05
                and report['http_errors'] == [] and report['page_errors'] == []
                and cleanup['complete'] and execution['driver_asserted_no_execution'])
    result = {'contract': CONTRACT, 'metadata_only': True, 'status': 'passed' if complete else 'failed',
              'failure_kind': 'none' if complete else 'driver_failed_or_incomplete',
              **source, 'harness_sha256': harness, 'report_sha256': digest(report_hash),
              'cases': [{'id': key, 'status': 'passed' if key in passed else 'NOT_RUN'} for key in CASES],
              'cleanup': cleanup, 'no_execution': execution, 'screenshots_published': False,
              'private_history_published': False}
    return result


def png_bytes(raw, row, name):
    require(type(row) is dict and digest(row.get('sha256')) == sha(raw))
    viewport = row.get('viewport')
    require(type(viewport) is dict and set(viewport) == {'width', 'height'})
    expected = (390, 844) if 'mobile' in name else (1440, 1100)
    require(type(viewport['width']) is int and type(viewport['height']) is int
            and (viewport['width'], viewport['height']) == expected)
    require(len(raw) >= 45 and raw[:8] == b'\x89PNG\r\n\x1a\n' and raw[8:12] == b'\0\0\0\r' and raw[12:16] == b'IHDR')
    width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', raw[16:29])
    require(width == expected[0] and expected[1] <= height <= 16384 and width * height <= 33554432
            and depth == 8 and color in (2, 6) and compression == filtering == 0 and interlace in (0, 1)
            and zlib.crc32(raw[12:29]) & 0xffffffff == int.from_bytes(raw[29:33], 'big'))
    # Fixed PNG chunk syntax, CRCs and IEND prevent disguised/trailing payloads.
    offset, seen_data, seen_end = 33, False, False
    chunks = 0
    while offset < len(raw):
        require(offset + 12 <= len(raw))
        length = int.from_bytes(raw[offset:offset+4], 'big')
        kind = raw[offset+4:offset+8]
        end = offset + 12 + length
        require(end <= len(raw) and kind in (b'IDAT', b'IEND', b'sRGB', b'gAMA', b'cHRM', b'pHYs'))
        require(zlib.crc32(raw[offset+4:end-4]) & 0xffffffff == int.from_bytes(raw[end-4:end], 'big'))
        if kind == b'IDAT':
            require(not seen_end)
            seen_data = True
        if kind == b'IEND':
            require(length == 0 and seen_data and end == len(raw))
            seen_end = True
        offset = end
        chunks += 1
        require(chunks <= 4096)
    require(seen_end)
    return {'name': name, 'sha256': sha(raw), 'bytes': len(raw), 'width': width, 'height': height}


def screenshots(descriptor, report, witnesses):
    rows = report.get('screenshots')
    require(type(rows) is list and len(rows) == 6)
    by_name = {}
    for row in rows:
        require(type(row) is dict and type(row.get('name')) is str and row['name'] in PNG_NAMES
                and row['name'] not in by_name)
        by_name[row['name']] = row
    require(set(by_name) == set(PNG_NAMES))
    payloads, metadata, total = {}, [], 0
    for name in PNG_NAMES:
        raw = regular_bytes(descriptor, name, MAX_PNG, witnesses=witnesses)
        total += len(raw)
        require(total <= MAX_PNG_TOTAL)
        metadata.append(png_bytes(raw, by_name[name], name))
        payloads[name] = raw
    return payloads, metadata


def write_new(descriptor, name, raw):
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=descriptor)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(os.dup(fd), 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        return pin(os.fstat(fd))
    except BaseException:
        # A failed write must not leave a partial public payload. Only the
        # still-pinned object we created is eligible for removal.
        try:
            if pin(os.stat(name, dir_fd=descriptor, follow_symlinks=False)) == pin(os.fstat(fd)):
                os.unlink(name, dir_fd=descriptor)
        except OSError:
            pass
        raise
    finally:
        os.close(fd)


def serialized_summary(value):
    raw = (json.dumps(value, indent=2, allow_nan=False) + '\n').encode()
    require(len(raw) <= MAX_PUBLIC_SUMMARY)
    return raw


def publish(output, value, payloads):
    """Fresh exclusive output only; never recursively copy a private directory."""
    raw_summary = serialized_summary(value)
    output = Path(output)
    require(output.is_absolute() and output.name not in ('', '.', '..'))
    parent = directory(output.parent)
    try:
        os.mkdir(output.name, 0o700, dir_fd=parent)
        fd = os.open(output.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
    finally:
        os.close(parent)
    written = {}
    try:
        same_directory(output, fd)
        for name, raw in payloads.items():
            require(name in PNG_NAMES)
            written[name] = write_new(fd, name, raw)
        same_directory(output, fd)
        written['summary.json'] = write_new(fd, 'summary.json', raw_summary)
        same_directory(output, fd)
    except BaseException:
        # Remove only our known files, never a replaced/unknown object. Caller
        # must exclusively own this new mode0700 output while publishing.
        for name, expected in written.items():
            try:
                if pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == expected:
                    os.unlink(name, dir_fd=fd)
            except OSError:
                pass
        raise
    finally:
        os.close(fd)


def collect(private, expected):
    digest(expected, 40)
    try:
        fd = directory(private, private=True)
    except FileNotFoundError:
        return {'contract': CONTRACT, 'metadata_only': True, 'status': 'pending',
                'failure_kind': 'report_missing', 'screenshots_published': False,
                'private_history_published': False}, {}, 2
    witnesses = []
    try:
        try:
            raw = regular_bytes(fd, 'report.json', MAX_REPORT, witnesses=witnesses)
        except FileNotFoundError:
            same_directory(private, fd)
            return {'contract': CONTRACT, 'metadata_only': True, 'status': 'pending',
                    'failure_kind': 'report_missing', 'screenshots_published': False,
                    'private_history_published': False}, {}, 2
        report = decode(raw)
        value = project(report, sha(raw), expected, expected_harness())
        payloads = {}
        if value['status'] == 'passed':
            payloads, metadata = screenshots(fd, report, witnesses)
            value.update(screenshots_published=True, screenshots=metadata)
        same_directory(private, fd)
        # Re-open the exact report relative to the pinned root before publishing;
        # no parsing result survives an on-disk replacement/change.
        require(sha(regular_bytes(fd, 'report.json', MAX_REPORT)) == sha(raw))
        for name, expected_pin in witnesses:
            require(pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == expected_pin)
        return value, payloads, 0
    finally:
        os.close(fd)


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Refused('Fixed projection arguments refused')


def main(argv=None):
    import sys
    arguments = list(sys.argv[1:] if argv is None else argv)
    if arguments == ['--help']:
        print('usage: record-task-workspace-ci-evidence.py --private DIR --output DIR --expected-source-head HEAD')
        return 0
    parser = Parser(description=__doc__, add_help=False)
    parser.add_argument('--private', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expected-source-head', required=True)
    try:
        args = parser.parse_args(arguments)
        # No output may be placed inside the original private evidence tree.
        require(args.private.is_absolute() and args.output.is_absolute()
                and '..' not in args.private.parts and '..' not in args.output.parts
                and not args.output.is_relative_to(args.private) and not args.private.is_relative_to(args.output))
    except Refused:
        print('{"status":"invalid","failure_kind":"projection_arguments_invalid"}')
        return 1
    value, payloads, code = None, {}, 1
    try:
        value, payloads, code = collect(args.private, args.expected_source_head)
    except (OSError, ValueError, TypeError, KeyError, RecursionError, OverflowError):
        value = {'contract': CONTRACT, 'metadata_only': True, 'status': 'invalid',
                 'failure_kind': 'private_evidence_invalid', 'screenshots_published': False,
                 'private_history_published': False}
    try:
        publish(args.output, value, payloads)
    except (OSError, ValueError, TypeError, KeyError, RecursionError, OverflowError):
        print(json.dumps({'status': 'invalid', 'failure_kind': 'projection_publication_failed'}))
        return 1
    return code


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except Refused:
        print('{"status":"invalid","failure_kind":"projection_arguments_invalid"}')
        raise SystemExit(1)
