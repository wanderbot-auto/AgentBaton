#!/usr/bin/env python3
"""Bound a native test and reap its private process group, including fatal exits.

The child is a session/process-group leader held unreaped until signalling ends.
No PID discovery, global kill, HTTP control server or production hook is used.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def native_report_error(report):
    if report.get('status') != 'passed':
        return 'native report did not confirm success'
    for section, fields in {
        'first_webview': ['domMounted', 'realComposerSubmitted', 'realTaskRendered', 'idempotent', 'invalidInputRejected', 'journal'],
        'reopened_webview': ['domMounted', 'restartPersisted', 'draftCancelled', 'stopEvidence'],
    }.items():
        values = report.get(section)
        if not isinstance(values, dict) or not all(values.get(field) is True for field in fields):
            return 'native report lacks required WebView/invoke evidence'
    task = report['first_webview'].get('taskId')
    if not isinstance(task, str) or not task or report['reopened_webview'].get('taskId') != task:
        return 'native report does not bind the same persisted task'
    for field in ['window_close_daemon_survived', 'second_writer_rejected', 'owned_daemon_restarted', 'owned_daemon_cleanup']:
        if report.get(field) is not True:
            return 'native report lacks required daemon lifecycle evidence'
    return None


def run(command, output, timeout):
    # A reused evidence directory must never turn an old passing report into a
    # new success. Retain all previous evidence under an explicit archive name.
    previous = [output / name for name in ['native-smoke.json', 'native-runner.json',
                'native-run.log', 'native-screen.png', 'native-smoke-before-runner.json']
                if (output / name).exists()]
    if previous:
        archive = output / 'previous' / str(time.time_ns())
        archive.mkdir(parents=True)
        for path in previous:
            path.rename(archive / path.name)
    subreaper = False
    if sys.platform.startswith('linux'):
        # Linux containers may have an init that does not reap orphans. Adopt
        # only our descendants, then wait for children in this owned group.
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
            raise OSError(ctypes.get_errno(), 'cannot enable test child subreaper')
        subreaper = True
    with (output / 'native-run.log').open('w') as log:
        child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                 stdin=subprocess.DEVNULL, start_new_session=True)
        deadline = time.monotonic() + timeout
        timed_out = False
        method = 'waitid-WNOWAIT' if hasattr(os, 'waitid') else 'ps-owned-child-zombie-state'
        try:
            while True:
                if hasattr(os, 'waitid'):
                    exited = os.waitid(os.P_PID, child.pid,
                                       os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None
                else:
                    # macOS Python versions may omit waitid. Do not call poll/wait
                    # (which reap): this exact owned child must retain its PID.
                    state = subprocess.run(['ps', '-o', 'stat=', '-p', str(child.pid)],
                                           capture_output=True, text=True, check=True).stdout.strip()
                    if not state:
                        raise RuntimeError('owned child state vanished before reaping')
                    exited = state.startswith('Z')
                if exited:
                    break
                if time.monotonic() >= deadline:
                    timed_out = True
                    break
                time.sleep(0.05)
        finally:
            # Holding the original unreaped group leader prevents PID/group reuse.
            # Both signals precede wait(); no unrelated PID lookup is involved.
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            time.sleep(0.1)
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            code = child.wait(timeout=5)
            reaped = 0
            if subreaper:
                reap_deadline = time.monotonic() + 2
                while time.monotonic() < reap_deadline:
                    try:
                        pid, _ = os.waitpid(-child.pid, os.WNOHANG)
                    except ChildProcessError:
                        break
                    if pid:
                        reaped += 1
                    else:
                        time.sleep(0.01)
    record = {'exit_code': code, 'timeout': timed_out, 'owned_group_cleanup': True,
              'leader_observation': method, 'adopted_owned_group_children_reaped': reaped, 'status': 'passed' if code == 0 and not timed_out else 'failed'}
    native_record = output / 'native-smoke.json'
    native = None
    if native_record.exists():
        raw = native_record.read_bytes()
        try:
            native = json.loads(raw)
        except (ValueError, UnicodeError):
            record['evidence_error'] = 'native report is malformed'
        if isinstance(native, dict):
            error = native_report_error(native)
            if error:
                record['evidence_error'] = error
    else:
        record['evidence_error'] = 'native report was not produced by this run'
    if 'evidence_error' in record or not isinstance(native, dict):
        record['status'] = 'failed'
    if record['status'] != 'passed' and isinstance(native, dict):
        (output / 'native-smoke-before-runner.json').write_bytes(raw)
        native['status'] = 'failed'
        native['external_runner'] = record
        native_record.write_text(json.dumps(native, indent=2) + '\n')
    (output / 'native-runner.json').write_text(json.dumps(record, indent=2) + '\n')
    return 0 if record['status'] == 'passed' else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=int, default=80)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command or not 1 <= args.timeout <= 180:
        parser.error('provide an explicit test command and a bounded timeout')
    output = args.output.resolve(strict=True)
    if not output.is_dir():
        parser.error('output must be an existing directory')
    return run(command, output, args.timeout)


if __name__ == '__main__':
    sys.exit(main())
