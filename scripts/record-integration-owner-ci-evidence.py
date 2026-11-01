#!/usr/bin/env python3
"""Finite projection of two Native integration reports; no raw fixture upload.

This validates reports, not authority or pixels. The actual driver/NativeParent
and its owned-process result remain separate required evidence.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('_integration_owner_contract', ROOT / 'scripts/native-owner-dialog-smoke.py')
N = importlib.util.module_from_spec(spec)
spec.loader.exec_module(N)


class Refused(ValueError):
    pass


def require(value):
    if not value:
        raise Refused('Integration Native evidence cannot be projected')


def unique(items):
    result = {}
    for key, value in items:
        require(key not in result)
        result[key] = value
    return result


def decode(raw):
    def bad(_):
        raise Refused('Noninteger Native evidence')
    return json.loads(raw, object_pairs_hook=unique, parse_float=bad, parse_constant=bad)


def pin(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns,
            info.st_ctime_ns, info.st_mode, info.st_uid, info.st_nlink)


def open_parent(path):
    """Pin every actual directory component without following path symlinks."""
    require(path.is_absolute() and '..' not in path.parts)
    current = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in path.parts[1:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=current)
            os.close(current)
            current = child
        return current
    except BaseException:
        os.close(current)
        raise


def private_json(path):
    parent = open_parent(path)
    try:
        info = os.fstat(parent)
        require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
        with os.fdopen(fd, 'rb') as stream:
            before = os.fstat(stream.fileno())
            require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                    and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) == 0o600
                    and 0 < before.st_size <= 65536)
            raw = stream.read(65537)
            require(len(raw) == before.st_size and pin(before) == pin(os.fstat(stream.fileno()))
                    and pin(before) == pin(os.stat(path.name, dir_fd=parent, follow_symlinks=False)))
        return decode(raw)
    finally:
        os.close(parent)


def project(cancel, approve, baseline_sequence):
    require(type(baseline_sequence) is int and 0 <= baseline_sequence <= (1 << 64) - 1)
    phases = []
    for phase, report in [('cancel', cancel), ('approve', approve)]:
        require(type(report) is dict and set(report) == {'status', 'dimension', 'provider_model_calls',
                    'automated_fixture_not_human_owner', 'integration_approval_database'})
        require(report['status'] == 'os_button_paths_completed_database_check_passed'
                and report['dimension'] == 'macos_own_process_real_sheet'
                and report['provider_model_calls'] == 'not_run'
                and report['automated_fixture_not_human_owner'] is True)
        group = report['integration_approval_database']
        require(N.integration_database_error(group, phase, baseline_sequence) is None)
        # All keys here are fixed and scalar. Unknown raw/private fields never
        # become a "safe" JSON value through passthrough or stringification.
        phases.append({'phase': phase, 'schema': 17,
            'baseline_sequence': baseline_sequence, 'after_sequence': group['after']['sequence'],
            'policy_count': group['after']['policies'], 'run_count': 0, 'attempt_count': 0, 'ref_fence_count': 0,
            'owner_event_sequence': group['owner_event_sequence'], 'policy_event_sequence': group['policy_event_sequence'],
            'exact_native_record_bindings': True})
    require(N.json_exact(cancel['integration_approval_database']['before'], approve['integration_approval_database']['before']))
    return {'contract': 'agentbaton.integration-owner-ci.v1', 'schema': 17,
            'status': 'valid_native_report_projection', 'origin': 'native_report_asserted',
            'visual_review': 'not_established_by_collector', 'driver_cleanup': 'not_established_by_collector',
            'authority_or_runtime_acceptance': False, 'private_history_published': False,
            'provider_model_calls': 'not_run', 'automated_fixture_not_human_owner': True,
            'phases': phases}


def publish(path, value):
    raw = (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    require(len(raw) <= 131072)
    parent = open_parent(path)
    try:
        info = os.fstat(parent)
        require(info.st_uid == os.geteuid() and not stat.S_IMODE(info.st_mode) & 0o022)
        fd = os.open(path.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=parent)
        with os.fdopen(fd, 'wb') as stream:
            os.fchmod(stream.fileno(), 0o600)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(parent)


class Parser(argparse.ArgumentParser):
    def error(self, _message):
        raise Refused('Invalid finite projection arguments')


def main(argv=None):
    try:
        parser = Parser(description=__doc__)
        parser.add_argument('--private', type=Path, required=True)
        parser.add_argument('--output', type=Path, required=True)
        parser.add_argument('--baseline-sequence', required=True)
        args = parser.parse_args(argv)
        require(args.baseline_sequence.isascii() and args.baseline_sequence.isdecimal()
                and len(args.baseline_sequence) <= 20 and str(int(args.baseline_sequence)) == args.baseline_sequence)
        require(args.private.is_absolute() and args.output.is_absolute())
        info = args.private.lstat()
        require(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode)
                and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        cancel = private_json(args.private / 'cancel/owner-dialog.json')
        approve = private_json(args.private / 'approve/owner-dialog.json')
        summary = project(cancel, approve, int(args.baseline_sequence))
        publish(args.output, summary)
        return 0
    except (Refused, ValueError, TypeError, OSError, KeyError, RecursionError):
        print('Integration Native evidence projection refused', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
