#!/usr/bin/env python3
"""Local development helper. No product transport, permissions or test controls."""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import socket
import stat
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
DESKTOP = ROOT / "apps/desktop"
KEY = hashlib.sha256(os.fsencode(ROOT)).hexdigest()[:12]
STATE = ROOT / "artifacts/desktop-dev"
# AF_UNIX paths must fit macOS' 104-byte sun_path, even in long worktrees.
CONTROL = Path("/tmp").resolve() / f"ab-dev-{os.getuid()}-{KEY}"


def private_directory(path):
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = path.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700 or path.resolve() != path):
        raise RuntimeError(f"Expected an owned private directory without symlinks: {path}")
    return path


def environment():
    env = os.environ.copy()
    # Share only within this checkout. Separate worktrees retain separate caches.
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
    env["CARGO_TARGET_DIR"] = str((ROOT / env["CARGO_TARGET_DIR"]).resolve())
    env.setdefault("CARGO_PROFILE_DEV_DEBUG", "1")
    env.setdefault("CARGO_INCREMENTAL", "1")
    # Runtime is distinct from the installed application and other worktrees.
    env.setdefault("BATON_RUNTIME_DIR", str(CONTROL / "runtime"))
    return env


def command(argv, *, cwd=ROOT, env=None):
    print("+ " + " ".join(map(str, argv)), flush=True)
    return subprocess.run(argv, cwd=cwd, env=env, check=True)


def metadata(manifest, env):
    return json.loads(subprocess.check_output(
        ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1",
         "--manifest-path", str(manifest)], cwd=ROOT, env=env))


def copy_changed(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() and source.read_bytes() == destination.read_bytes():
        return
    # Avoid needless watcher rebuilds; replace rather than truncate a running file.
    temporary = destination.with_name(destination.name + ".new")
    shutil.copy2(source, temporary)
    temporary.replace(destination)


def prepare(env):
    """Also called by the raw `npm run tauri -- dev` beforeDev hook."""
    # Explicit targets (including the host) change Cargo's output layout. Keep
    # interactive native dev unambiguous instead of reading a stale host binary.
    if env.get("CARGO_BUILD_TARGET"):
        raise RuntimeError("Unset CARGO_BUILD_TARGET for interactive native development")
    command(["cargo", "build", "--locked", "--bin", "batond", "--bin", "baton"], env=env)
    triple = next(line[6:] for line in subprocess.check_output(
        ["rustc", "-vV"], text=True).splitlines() if line.startswith("host: "))
    core = Path(metadata(ROOT / "Cargo.toml", env)["target_directory"]) / "debug"
    native = Path(metadata(DESKTOP / "src-tauri/Cargo.toml", env)["target_directory"]) / "debug"
    for name in ("batond", "baton"):
        copy_changed(core / name, DESKTOP / f"src-tauri/binaries/{name}-{triple}")
        if core != native:
            copy_changed(core / name, native / name)
    return core


def setup(env, force=False):
    lock = hashlib.sha256((DESKTOP / "package-lock.json").read_bytes()).hexdigest()
    stamp = STATE / "npm-lock.sha256"
    if (force or not (DESKTOP / "node_modules/.bin/tauri").exists()
            or not (DESKTOP / "node_modules/.bin/vite").exists()
            or not stamp.exists() or stamp.read_text() != lock):
        command(["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund"], cwd=DESKTOP, env=env)
        stamp.write_text(lock)


def request(message):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2)
        client.connect(str(CONTROL / "control.sock"))
        client.sendall(message.encode())
        return json.loads(client.recv(8192))


def running():
    try:
        return request("status")
    except (OSError, ValueError):
        return None


def busy():
    """No control response can mean cleanup/unknown, not confirmed stop."""
    if not (CONTROL / "lock").exists():
        return False
    with (CONTROL / "lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return False
        except BlockingIOError:
            return True


def available_port(port):
    if not 1024 <= port <= 65535:
        raise RuntimeError("BATON_DEV_PORT must be between 1024 and 65535")
    with socket.socket() as probe:
        # Permit TIME_WAIT after our previous Vite exits, not active listeners.
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(("127.0.0.1", port))


def tauri_command(port):
    override = {"build": {
        "devUrl": f"http://127.0.0.1:{port}",
        "beforeDevCommand": f"bash ../../scripts/desktop-dev.sh prepare && npm run dev -- --port {port}",
    }}
    return ["npm", "run", "tauri", "--", "dev", "--config", json.dumps(override),
            "--additional-watch-folders", str(ROOT / "crates"), "--", "--locked"]


class OwnedProcess:
    """Pin the process group ID until cleanup, even after the command exits.

    The guardian never gets polled/reaped before the final group signal, so a
    historical PID cannot be recycled underneath stop/restart. Actual command
    status arrives over a pipe. Completed setup groups are removed immediately.
    """
    def __init__(self, argv, cwd, env):
        read_fd, write_fd = os.pipe()
        os.set_blocking(read_fd, False)
        self.read_fd = read_fd
        self.returncode = None
        guardian = """
import os, signal, subprocess, sys
signal.signal(signal.SIGTERM, lambda *_: None)
signal.signal(signal.SIGINT, lambda *_: None)
try:
    code = subprocess.Popen(sys.argv[2:]).wait()
except OSError:
    code = 127
os.write(int(sys.argv[1]), str(code).encode())
os.close(int(sys.argv[1]))
while True:
    signal.pause()
"""
        try:
            self.process = subprocess.Popen([sys.executable, "-c", guardian, str(write_fd), *argv],
                cwd=cwd, env=env, start_new_session=True, pass_fds=(write_fd,))
        except BaseException:
            os.close(read_fd)
            raise
        finally:
            os.close(write_fd)
        self.pid = self.process.pid

    def poll(self):
        if self.returncode is None:
            try:
                data = os.read(self.read_fd, 32)
                self.returncode = int(data) if data else 127
            except BlockingIOError:
                pass
        return self.returncode

    def wait(self):
        self.process.wait()
        os.close(self.read_fd)


def terminate(children):
    # Only groups created by this supervisor; never pkill names or kill port owners.
    for child in reversed(children):
        with contextlib.suppress(ProcessLookupError):
            os.killpg(child.pid, signal.SIGTERM)
    deadline = time.monotonic() + 8
    while any(child.poll() is None for child in children) and time.monotonic() < deadline:
        time.sleep(0.1)
    for child in reversed(children):
        with contextlib.suppress(ProcessLookupError):
            os.killpg(child.pid, signal.SIGKILL)
        child.wait()


def supervise():
    private_directory(CONTROL)
    STATE.mkdir(parents=True, exist_ok=True)
    with (CONTROL / "lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("Development session already running", flush=True)
            return 1
        path = CONTROL / "control.sock"
        path.unlink(missing_ok=True)  # stale socket, only after exclusive lock
        children = []
        stopping = False
        phase = "preparing"
        port = int(os.environ.get("BATON_DEV_PORT", 15000 + int(KEY[:6], 16) % 30000))
        env = environment()

        def stop_signal(*_):
            nonlocal stopping
            stopping = True

        signal.signal(signal.SIGTERM, stop_signal)
        signal.signal(signal.SIGINT, stop_signal)
        with socket.socket(socket.AF_UNIX) as server:
            server.bind(str(path))
            server.listen(4)
            server.settimeout(0.2)

            def service():
                nonlocal stopping
                try:
                    client, _ = server.accept()
                except socket.timeout:
                    return
                with client:
                    client.settimeout(1)
                    if client.recv(32) == b"stop":
                        stopping = True
                    client.sendall(json.dumps({"phase": phase, "pid": os.getpid(),
                        "url": f"http://127.0.0.1:{port}", "runtime": env["BATON_RUNTIME_DIR"],
                        "log": str(STATE / "dev.log")}).encode())

            def spawn(argv, cwd=ROOT):
                child = OwnedProcess(argv, cwd, env)
                children.append(child)
                return child

            def wait_stage(child):
                while child.poll() is None and not stopping:
                    service()
                if stopping:
                    return False
                terminate([child])
                children.remove(child)
                if child.returncode:
                    raise RuntimeError(f"Development preparation failed (exit {child.returncode})")
                return True

            try:
                available_port(port)
                # Setup/build stay interruptible through the same control socket.
                if not wait_stage(spawn([sys.executable, __file__, "setup"])):
                    return 0
                if not wait_stage(spawn([sys.executable, __file__, "prepare"])):
                    return 0
                core = Path(metadata(ROOT / "Cargo.toml", env)["target_directory"]) / "debug"
                daemon = spawn([str(core / "batond")])
                deadline = time.monotonic() + 10
                phase = "starting daemon"
                while not stopping:
                    if daemon.poll() is not None:
                        raise RuntimeError("Dev daemon exited; inspect dev.log (existing writer/runtime conflict?)")
                    # An authenticated product snapshot, not socket existence alone.
                    try:
                        result = subprocess.run([str(core / "baton"), "snapshot"], cwd=ROOT,
                            env=env, capture_output=True, timeout=2)
                        if result.returncode == 0:
                            break
                    except subprocess.TimeoutExpired:
                        pass
                    if time.monotonic() >= deadline:
                        raise RuntimeError("Dev daemon readiness timed out")
                    service()
                if stopping:
                    return 0
                ui = spawn(tauri_command(port), DESKTOP)
                phase = "dev (building or running; see log)"
                while not stopping:
                    if ui.poll() is not None or daemon.poll() is not None:
                        raise RuntimeError("A development process exited; inspect dev.log")
                    service()
                return 0
            finally:
                phase = "stopping"
                terminate(children)
                path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description="AgentBaton native development preview")
    parser.add_argument("action", nargs="?", default="start", choices=[
        "start", "stop", "restart", "status", "logs", "dev", "setup", "prepare", "_supervise"])
    args = parser.parse_args()
    if args.action == "_supervise":
        return supervise()
    if args.action == "prepare":
        # Raw Tauri runs preserve their selected Cargo configuration/cache.
        prepare(os.environ.copy())
        return 0
    if args.action == "setup":
        STATE.mkdir(parents=True, exist_ok=True)
        setup(environment())
        return 0
    if args.action == "logs":
        if not (STATE / "dev.log").exists():
            print("No development log yet. Run desktop-dev.sh start.")
            return 0
        return subprocess.call(["tail", "-n", "80", "-f", str(STATE / "dev.log")])
    if args.action in ("stop", "restart"):
        current = running()
        if current or busy():
            if current:
                request("stop")
            deadline = time.monotonic() + 20
            # A stale socket is not enough: wait for supervisor's lock release.
            while time.monotonic() < deadline:
                with (CONTROL / "lock").open("a") as lock:
                    try:
                        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        time.sleep(0.2)
            else:
                raise RuntimeError("Stop not confirmed; inspect log before retrying")
            print("Development session stopped (UI, watcher, Vite and dev daemon).")
        else:
            print("Development session is stopped.")
        if args.action == "stop":
            return 0
    if args.action == "status":
        current = running()
        print(json.dumps(current, indent=2) if current else
              "Development session is busy/stopping (control unavailable)." if busy() else
              "Development session is stopped.")
        return 0
    if running():
        print(json.dumps(running(), indent=2))
        return 0
    if busy():
        raise RuntimeError("Development session is busy/stopping; wait for confirmed stop")
    if platform.system() == "Darwin" and platform.machine() != "arm64":
        raise RuntimeError("macOS development is supported on native Apple Silicon only")
    if args.action == "dev":
        return supervise()
    STATE.mkdir(parents=True, exist_ok=True)
    with (STATE / "dev.log").open("a") as log:
        log.write(f"\n--- dev start {time.strftime('%Y-%m-%d %H:%M:%S')} ---\n")
        log.flush()
        worker = subprocess.Popen([sys.executable, __file__, "_supervise"], cwd=ROOT,
            env=environment(), stdin=subprocess.DEVNULL, stdout=log, stderr=log,
            start_new_session=True)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline and worker.poll() is None:
        current = running()
        if current:
            print(json.dumps(current, indent=2))
            print("Starting in background. Use desktop-dev.sh logs to follow compilation.")
            return 0
        time.sleep(0.1)
    current = running()
    if current:  # concurrent start may have won the exclusive lock
        print(json.dumps(current, indent=2))
        return 0
    raise RuntimeError(f"Development start failed; see {STATE / 'dev.log'}")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f"desktop-dev: {error}", file=sys.stderr)
        sys.exit(1)
