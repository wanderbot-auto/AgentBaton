#!/usr/bin/env python3
"""Local foreground command leases, not a sandbox or a background supervisor."""
import contextlib
import fcntl
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile


@contextlib.contextmanager
def lease(path):
    # Never unlink flock files: another process may still hold the inode.
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid() or info.st_nlink != 1:
            raise ValueError('unsafe command lease')
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError('command already running: ' + str(path)) from None
        yield fd
    finally:
        os.close(fd)


def runtime(state, task):
    record = state / ('runtime-' + task)
    if record.is_symlink():
        raise ValueError('unsafe runtime record')
    if record.exists():
        directory = Path(record.read_text().strip())
    else:
        # Short canonical Unix socket paths on both macOS and Linux. Do not use
        # the (potentially long) worktree path or inherit a caller's TMPDIR.
        directory = Path(tempfile.mkdtemp(prefix='ab-collab-', dir='/tmp')).resolve()
        with record.open('x') as output:
            output.write(str(directory) + '\n')
    info = directory.lstat()
    if (not directory.is_absolute() or directory.resolve() != directory
            or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o700
            or len(os.fsencode(directory / 'batond.sock')) >= 104):
        raise ValueError('task runtime is missing or unsafe; inspect it before recovery')
    return directory


def main():
    mode, state_text, task, *args = sys.argv[1:]
    state = Path(state_text)
    with lease(state / ('running-' + task + '.lock')) as task_fd:
        claim = state / 'active' / task
        if not claim.is_file():
            raise ValueError('task reservation is no longer active')
        if mode == 'finish' and not args:
            claim.rename(state / 'closed' / task)
            return 0
        if mode != 'run' or not args:
            raise ValueError('invalid command lease invocation')
        top, *args = args
        native = bool(args and args[0] == '--native')
        if native:
            args.pop(0)
        if len(args) < 2 or args.pop(0) != '--':
            raise ValueError('use run [--native] -- COMMAND [ARG...]')
        with contextlib.ExitStack() as stack:
            descriptors = [task_fd]
            if native:
                descriptors.append(stack.enter_context(lease(state / 'native.lock')))
            environment = os.environ.copy()
            environment['BATON_RUNTIME_DIR'] = str(runtime(state, task))
            environment['CARGO_TARGET_DIR'] = str(Path(top) / 'target')
            print('collab: task runtime ' + environment['BATON_RUNTIME_DIR'], flush=True)
            # Keep leases alive in the direct child if this wrapper is killed.
            # Commands must stay in the foreground; this is not a daemon manager.
            child = subprocess.Popen(args, cwd=top, env=environment,
                                     start_new_session=True, pass_fds=descriptors)

            def forward(signum, _frame):
                try:
                    os.killpg(child.pid, signum)
                except ProcessLookupError:
                    pass

            previous = {s: signal.signal(s, forward) for s in (signal.SIGINT, signal.SIGTERM)}
            try:
                result = child.wait()
            finally:
                for s, handler in previous.items():
                    signal.signal(s, handler)
            return result if result >= 0 else 128 - result


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print('collab: ' + str(error), file=sys.stderr)
        sys.exit(2)
