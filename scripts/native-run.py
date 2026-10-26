#!/usr/bin/env python3
"""Bound a native test and reap its private process group, including fatal exits.

The child is a session/process-group leader held unreaped until signalling ends.
Only its original group is observed/signalled; discovered PIDs are never signal
targets. No global kill, HTTP control server or production hook is used.
"""
import argparse
import ctypes
import errno
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


def owned_group_is_quiescent(group):
    """Prove the unreaped leader and its group contain only zombie processes.

    Darwin can return EPERM, rather than ESRCH, when killpg finds no live
    recipient. This query is restricted to the original private group; it never
    selects PIDs for signalling and cannot approve an absent or unknown leader.
    """
    result = subprocess.run(
        ['/bin/ps', '-o', 'pid=,pgid=,stat=', '-g', str(group)],
        capture_output=True, text=True, timeout=1, check=False)
    if result.returncode != 0:
        return False
    leader = False
    for row in result.stdout.splitlines():
        fields = row.split()
        if len(fields) != 3 or not fields[0].isdigit() or not fields[1].isdigit():
            return False
        pid, process_group = map(int, fields[:2])
        if process_group != group or not fields[2].startswith('Z'):
            return False
        leader = leader or pid == group
    return leader


def owned_child_exited(pid):
    """Observe only this owned leader without consuming its wait status."""
    if hasattr(os, 'waitid'):
        return os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None
    # macOS Python versions may omit waitid. Never call poll/wait here.
    state = subprocess.run(['/bin/ps', '-o', 'stat=', '-p', str(pid)],
                           capture_output=True, text=True, check=True,
                           timeout=1).stdout.strip()
    if not state:
        raise RuntimeError('owned child state vanished before reaping')
    return state.startswith('Z')


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
        exited = False
        observation_error = None
        cleanup_errors = []
        signal_outcomes = []
        code = None
        reaped = 0
        method = 'waitid-WNOWAIT' if hasattr(os, 'waitid') else 'ps-owned-child-zombie-state'
        try:
            while True:
                exited = owned_child_exited(child.pid)
                if exited:
                    break
                if time.monotonic() >= deadline:
                    timed_out = True
                    break
                time.sleep(0.05)
        except (OSError, RuntimeError, subprocess.SubprocessError) as error:
            observation_error = f'{type(error).__name__}:{getattr(error, "errno", None)}'
        finally:
            # Holding the original unreaped group leader prevents PID/group reuse.
            # Both signals precede wait(); no unrelated PID lookup is involved.
            cleanup_deadline = time.monotonic() + 3
            for sig in [signal.SIGTERM, signal.SIGKILL]:
                if sig == signal.SIGKILL:
                    time.sleep(0.1)
                try:
                    os.killpg(child.pid, sig)
                    signal_outcomes.append({'signal': sig.name, 'result': 'signalled'})
                except ProcessLookupError:
                    signal_outcomes.append({'signal': sig.name, 'result': 'already_absent'})
                except PermissionError as error:
                    try:
                        quiescent = (error.errno == errno.EPERM and exited
                                     and owned_group_is_quiescent(child.pid))
                    except (OSError, subprocess.SubprocessError):
                        quiescent = False
                    if quiescent:
                        signal_outcomes.append({'signal': sig.name, 'result': 'observed_quiescent'})
                    else:
                        cleanup_errors.append(f'{sig.name}:PermissionError:{error.errno}; owned group not proven quiescent')
                except OSError as error:
                    cleanup_errors.append(f'{sig.name}:{type(error).__name__}:{error.errno}')
            try:
                code = child.wait(timeout=max(0.01, cleanup_deadline - time.monotonic()))
            except (OSError, subprocess.SubprocessError) as error:
                cleanup_errors.append(f'wait:{type(error).__name__}:{getattr(error, "errno", None)}')
            if subreaper:
                reap_deadline = min(cleanup_deadline, time.monotonic() + 2)
                while time.monotonic() < reap_deadline:
                    try:
                        pid, _ = os.waitpid(-child.pid, os.WNOHANG)
                    except ChildProcessError:
                        break
                    except OSError as error:
                        cleanup_errors.append(f'reap:{type(error).__name__}:{error.errno}')
                        break
                    if pid:
                        reaped += 1
                    else:
                        time.sleep(0.01)
    cleanup_complete = not cleanup_errors and code is not None
    record = {'exit_code': code, 'timeout': timed_out, 'owned_group_cleanup': cleanup_complete,
              'leader_observation': method, 'group_signals': signal_outcomes,
              'adopted_owned_group_children_reaped': reaped,
              'status': 'passed' if code == 0 and not timed_out and cleanup_complete and not observation_error else 'failed'}
    if observation_error:
        record['observation_error'] = observation_error
    if cleanup_errors:
        record['cleanup_errors'] = cleanup_errors
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
