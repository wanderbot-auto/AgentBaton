#!/usr/bin/env python3
"""Exercise built daemon/CLI processes with a private disposable database.

No Provider calls. This validates local IPC, not native desktop behavior.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINARIES = ROOT / 'target/debug'


def main():
    with tempfile.TemporaryDirectory(prefix='baton-smoke-') as temporary:
        runtime = Path(temporary).resolve()
        runtime.chmod(0o700)
        context = dict(os.environ, BATON_RUNTIME_DIR=str(runtime))

        def start():
            return subprocess.Popen([str(BINARIES / 'batond')], env=context,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)

        def cli(*arguments):
            result = subprocess.run([str(BINARIES / 'baton'), *arguments], env=context,
                                    capture_output=True, text=True, timeout=8)
            if result.returncode:
                raise RuntimeError(result.stderr)
            return json.loads(result.stdout)['result']

        def ready(process):
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(process.stderr.read())
                try:
                    return cli('snapshot')
                except RuntimeError:
                    time.sleep(0.05)
            raise TimeoutError('Daemon did not authenticate within readiness deadline')

        daemon = start()
        try:
            assert ready(daemon)['tasks'] == []
            task = cli('tasks', 'create', '本地 IPC persistent task', 'smoke-create')
            retry = cli('tasks', 'create', '本地 IPC persistent task', 'smoke-create')
            assert task['id'] == retry['id']
            stopped = cli('tasks', 'cancel', task['id'], str(task['revision']))
            assert stopped['state'] == 'cancelled'
            journal = cli('events', '0')
            assert journal['sequence'] == 2
            assert journal['events'][-1]['payload']['stop_evidence'] == 'no_attempt_dispatched'
            # SIGKILL deliberately leaves the Unix socket behind. Restart must
            # obtain the writer lock and reconcile local state without replay.
            daemon.kill()
            daemon.wait(timeout=8)
            daemon.stderr.close()
            daemon = start()
            snapshot = ready(daemon)
            assert snapshot['tasks'][0]['id'] == task['id']
            assert snapshot['tasks'][0]['state'] == 'cancelled'
            assert snapshot['sequence'] == journal['sequence']
            print('PASS real local daemon/CLI: Unicode create, idempotency, evidenced draft cancellation, journal, SIGKILL/restart persistence')
        finally:
            if daemon.poll() is None:
                daemon.terminate()
            daemon.wait(timeout=8)
            daemon.stderr.close()


if __name__ == '__main__':
    main()
