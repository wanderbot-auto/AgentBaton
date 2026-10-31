#!/usr/bin/env python3
"""Finite notification evidence; preserve the original integration collector.

Only six complete real-probe checks and four validated original PNGs can be
published. Raw reports, IDs, payloads, paths and tracebacks remain private.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location('_notification_original_collector', ROOT / 'scripts/record-integration-browser-ci-evidence.py')
E = importlib.util.module_from_spec(_spec); _spec.loader.exec_module(E)
P, Refused = E.P, E.Refused
CONTRACT = 'agentbaton.task-notifications-ci.v1'
PROBE = 'apps/desktop/tests/task_notifications_real_ipc_probe.py'
CHECKS = ('actual accepted Task projection and mapped journal observations',
    'one Task notice for naturally repeated real Task/session events',
    'expanded real source metadata and exact original journal navigation',
    'same Task navigation and modal shortcut isolation',
    'explicit DND persists across UI reload with manual evidence retained',
    'probe bridge and private audit contain only existing bounded reads')
NAMES = ('accepted-task-desktop', 'real-source-desktop', 'original-journal-desktop', 'dnd-mobile')
PUBLIC_NAMES = frozenset(name + '.png' for name in NAMES)
DIMENSION = 'Linux built React/Chromium + existing finite TEST bridge + actual Core17 Unix IPC'
NOT_CLAIMED = ['production Tauri IPC', 'macOS Native', 'live Provider/model', 'OS notifications',
    'background notification delivery', '100 real repeated events', 'whole product completion']
MAX_REPORT, MAX_PNG, MAX_TOTAL = 262144, 16777216, 33554432


def require(value):
    if not value:
        raise Refused('Finite task notification evidence refused')


def number(value, low=0, high=9007199254740991):
    require(type(value) is int and low <= value <= high)
    return value


def identifier(value):
    require(type(value) is str and re.fullmatch('[A-Za-z0-9._:-]{1,256}', value) is not None)
    return value


def sha(value, width=64):
    require(type(value) is str and re.fullmatch('[a-f0-9]{' + str(width) + '}', value) is not None)
    return value


def raw_sha(raw):
    return hashlib.sha256(raw).hexdigest()


def expected_probe(expected):
    sha(expected, 40)
    require(subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=no'], cwd=ROOT) == b'')
    fd = P.directory(ROOT / 'apps/desktop/tests')
    try:
        raw = P.regular_bytes(fd, 'task_notifications_real_ipc_probe.py', 1048576, private=False)
    finally:
        os.close(fd)
    # Retained evidence may be projected later: bind the probe to the declared
    # immutable source object, while requiring the checked-out probe bytes too.
    require(raw == subprocess.check_output(['git', 'show', expected + ':' + PROBE], cwd=ROOT))
    return {'source': expected, 'relative_path': PROBE, 'sha256': raw_sha(raw), 'bytes': len(raw)}


def read_counts(value, parent=None):
    require(type(value) is dict and set(value) <= E.HTTP_READS)
    require(all(method in value for method in ('system.snapshot', 'sessions.list', 'events.subscribe')))
    total = 0
    for method, count in value.items():
        total += number(count, 1, 100000)
        if parent is not None:
            require(count <= parent.get(method, 0))
    require(total <= 100000)
    return dict(value)


def report_summary(report, expected, source_pin, base):
    require(type(report) is dict and set(report) == {'contract', 'status', 'flow_complete', 'dimension',
        'not_claimed', 'source', 'probe_source', 'checks', 'screenshots', 'cleanup_observed', 'observations'})
    require(report['contract'] == 'agentbaton.task-notifications-real-ipc.v1' and report['status'] == 'passed'
        and report['flow_complete'] is True and report['cleanup_observed'] is True
        and report['dimension'] == DIMENSION and report['not_claimed'] == NOT_CLAIMED
        and report['source'] == expected and report['probe_source'] == source_pin)
    # Python bool/int coercion must not make false source-byte metadata equal.
    require(type(report['probe_source']) is dict and set(report['probe_source']) == {'source', 'relative_path', 'sha256', 'bytes'}
        and type(report['probe_source']['bytes']) is int)
    rows = report['checks']; require(type(rows) is list and len(rows) == 6)
    fields = ({'actual_reply_sha256', 'task_revision', 'acceptance_sequence', 'mapped_event_count'},
        {'notices', 'naturally_observed_events', 'synthetic_repeated_events'},
        {'inspected_references', 'original_event_id', 'original_event_sequence', 'original_payload_matched'},
        {'same_task', 'modal_count', 'shortcuts', 'escape_background_restored'},
        {'ui_reload', 'manual_evidence', 'os_notification_delivery_tested'},
        {'bridge_read_counts', 'private_read_counts'})
    for index, row in enumerate(rows):
        require(type(row) is dict and set(row) == {'name', 'status'} | fields[index]
            and row['name'] == CHECKS[index] and row['status'] == 'passed')
    first, group, original, modal, dnd, reads = rows
    witness = sha(first['actual_reply_sha256']); number(first['task_revision'], 1)
    acceptance = number(first['acceptance_sequence'], 1); count = number(first['mapped_event_count'], 2, 500)
    require(number(group['notices'], 1, 1) == 1 and number(group['naturally_observed_events'], 2, 500) == count
        and number(group['synthetic_repeated_events'], 0, 0) == 0)
    refs = original['inspected_references']; require(type(refs) is list and len(refs) == min(10, count))
    require(len({identifier(value) for value in refs}) == len(refs)
        and identifier(original['original_event_id']) in refs
        and number(original['original_event_sequence'], 1) == acceptance
        and original['original_payload_matched'] is True)
    require(modal['same_task'] is True and number(modal['modal_count'], 1, 1) == 1
        and number(modal['shortcuts'], 4, 4) == 4 and modal['escape_background_restored'] is True
        and dnd['ui_reload'] is True and dnd['manual_evidence'] is True
        and dnd['os_notification_delivery_tested'] is False)
    bridge = read_counts(reads['bridge_read_counts'], base['http_method_counts'])
    private_reads = read_counts(reads['private_read_counts'])
    observed = report['observations']
    require(type(observed) is dict and set(observed) == {'task_id', 'acceptance_ref', 'source_event_count',
        'inspected_event_count', 'actual_reply_sha256', 'fixed_deadline_seconds'})
    identifier(observed['task_id']); identifier(observed['acceptance_ref'])
    require(number(observed['source_event_count'], 2, 500) == count
        and number(observed['inspected_event_count'], 1, 10) == len(refs)
        and sha(observed['actual_reply_sha256']) == witness
        and number(observed['fixed_deadline_seconds'], 15, 15) == 15)
    return {'checks': 6, 'task_notices': 1, 'naturally_observed_events': count, 'inspected_event_count': len(refs),
        'synthetic_repeated_events': 0, 'original_event_payload_matched': True, 'same_task_navigation': True,
        'modal_shortcuts': 4, 'dnd_ui_reload': True, 'bridge_read_counts': bridge, 'private_read_counts': private_reads,
        'actual_reply_sha256': witness, 'cleanup': E.cleanup(base['cleanup'])}


def collect(private, expected):
    sha(expected, 40)
    source_pin = expected_probe(expected)
    # Enforce the complete unchanged original collector, including both cases,
    # their original PNGs, source/build/schema metadata, HTTP and owned cleanup.
    parent, _ = E.collect(private, expected)
    base_path = Path(private) / 'driver/normal'
    path = base_path / 'notification-probe'
    root_fd = P.directory(private, private=True)
    base_fd = fd = None
    witnesses, base_witnesses, root_witnesses = [], [], []
    try:
        base_fd = P.directory(base_path, private=True)
        fd = P.directory(path, private=True)
        # Re-read parent witnesses so later changes cannot detach the new probe
        # from the paired source/build and complete original normal report.
        for name, key in (('candidate.json', 'manifest_sha256'), ('frontend-build-record.json', 'build_record_sha256')):
            raw = P.regular_bytes(root_fd, name, E.MAX_REPORT, witnesses=root_witnesses)
            require(raw_sha(raw) == parent[key])
        base_raw = P.regular_bytes(base_fd, 'report.json', E.MAX_REPORT, witnesses=base_witnesses)
        base = P.decode(base_raw)
        E.report_summary(base, 'normal', expected, parent['manifest_sha256'], parent['build_record_sha256'], parent['driver_sha256'])
        raw = P.regular_bytes(fd, 'report.json', MAX_REPORT, witnesses=witnesses)
        report = P.decode(raw)
        facts = report_summary(report, expected, source_pin, base)
        rows = report['screenshots']; require(type(rows) is list and len(rows) == 4)
        payloads, images, total = {}, [], 0
        for index, row in enumerate(rows):
            require(type(row) is dict and set(row) == {'name', 'sha256', 'bytes', 'viewport',
                'focus_box_before_capture_observation', 'pixel_synchronized', 'dimension'}
                and row['name'] == NAMES[index] and row['pixel_synchronized'] is False
                and row['dimension'] == 'Linux Chromium only')
            box = row['focus_box_before_capture_observation']
            require(type(box) is dict and set(box) == {'x', 'y', 'width', 'height'})
            for key, value in box.items():
                require(type(value) in (int, float) and math.isfinite(value) and abs(value) <= 10000000
                    and (value > 0 if key in ('width', 'height') else True))
            name = NAMES[index] + '.png'
            png = P.regular_bytes(fd, name, MAX_PNG, witnesses=witnesses)
            require(number(row['bytes'], 1, MAX_PNG) == len(png))
            total += len(png); require(total <= MAX_TOTAL)
            image = P.png_bytes(png, row, name)  # Reuse the existing full PNG/pixel validator.
            require(image['height'] == row['viewport']['height'])
            images.append(image); payloads[name] = png
        for location, descriptor, seen in ((private, root_fd, root_witnesses), (base_path, base_fd, base_witnesses), (path, fd, witnesses)):
            P.same_directory(location, descriptor)
            for name, original_pin in seen:
                require(P.pin(os.stat(name, dir_fd=descriptor, follow_symlinks=False)) == original_pin)
        require(expected_probe(expected) == source_pin)
        return {'contract': CONTRACT, 'status': 'passed', 'source': expected, **facts,
            'probe_source_sha256': source_pin['sha256'], 'driver_sha256': parent['driver_sha256'],
            'manifest_sha256': parent['manifest_sha256'], 'build_record_sha256': parent['build_record_sha256'],
            'assets_sha256': parent['assets_sha256'], 'probe_report_sha256': raw_sha(raw),
            'screenshots': images, 'screenshots_published': True, 'private_history_published': False,
            'visual_review': 'not_performed_by_projector', 'pixel_synchronized': False, 'not_claimed': NOT_CLAIMED}, payloads
    finally:
        for descriptor in (fd, base_fd, root_fd):
            if descriptor is not None:
                os.close(descriptor)


def failure(expected, reason):
    return {'contract': CONTRACT, 'status': 'failed', 'source': expected, 'failure_kind': reason,
        'screenshots_published': False, 'private_history_published': False,
        'visual_review': 'not_performed_by_projector', 'pixel_synchronized': False, 'not_claimed': NOT_CLAIMED}


def publish(output, summary, payloads):
    # Reuse the original descriptor-safe exclusive writer; its own finite list
    # remains untouched. This module supplies only its separate four names.
    require(set(payloads) <= PUBLIC_NAMES)
    parent = P.directory(Path(output).parent)
    try:
        require(Path(output).is_absolute() and '..' not in Path(output).parts)
        os.mkdir(Path(output).name, 0o700, dir_fd=parent)
    finally:
        os.close(parent)
    fd = P.directory(output, private=True); written = {}
    try:
        P.same_directory(output, fd)
        for name, raw in payloads.items():
            written[name] = P.write_new(fd, name, raw)
        raw = (json.dumps(summary, indent=2, allow_nan=False) + '\n').encode()
        require(len(raw) <= 65536)
        written['summary.json'] = P.write_new(fd, 'summary.json', raw)
        P.same_directory(output, fd)
    except BaseException:
        for name, stamp in written.items():
            try:
                if P.pin(os.stat(name, dir_fd=fd, follow_symlinks=False)) == stamp:
                    os.unlink(name, dir_fd=fd)
            except OSError:
                pass
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
        except (ValueError, OSError, KeyError, TypeError, OverflowError, subprocess.SubprocessError):
            summary, payloads = failure(args.expected_source_head, 'invalid_failed_or_incomplete_private_evidence'), {}
        publish(args.output, summary, payloads)
        return 0 if summary['status'] == 'passed' else 1
    except (ValueError, OSError, KeyError, TypeError, OverflowError):
        print('Finite task notification projection refused', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
