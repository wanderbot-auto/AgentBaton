#!/usr/bin/env python3
"""Explicit checks for the deliberately independent RHOST-001 workspace.
Run: bash scripts/collab.sh run -- python3 scripts/verify-remote-host.py
All logs, including failures, are retained under the task's execution directory.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "crates/remote-host/Cargo.toml"


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def run_check(cmd, env, output):
    try:
        return subprocess.run(cmd, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=240, check=False).returncode
    except subprocess.TimeoutExpired:
        return 124


def main():
    if not os.environ.get("BATON_RUNTIME_DIR") or not os.environ.get("CARGO_TARGET_DIR"):
        raise SystemExit("run through scripts/collab.sh run for isolated runtime/target")
    os.chdir(ROOT)
    paths = sorted([*Path("crates/remote-host/src").rglob("*.rs"), *Path("crates/remote-host/tests").rglob("*.rs"), Path(MANIFEST), Path("crates/remote-host/Cargo.lock"), Path("scripts/verify-remote-host.py"), Path("tests/infrastructure/test_remote_host.py")])
    sources = {str(p): sha(p) for p in paths}
    source_hash = hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest()
    run = Path("docs/execution/remote-host-foundation/runs") / (time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + str(time.time_ns() % 1000000000))
    run.mkdir(parents=True)
    evidence = dict(schema=1, task="RHOST-001", head=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(), dirty=subprocess.check_output(["git", "status", "--porcelain"], text=True).splitlines(), source_hash=source_hash, source_sha256=sources, platform=platform.platform(), machine=platform.machine(), runtime=os.environ["BATON_RUNTIME_DIR"], target=os.environ["CARGO_TARGET_DIR"], checks=[], unverified=["real SSH handshake", "cross-machine macOS/Linux", "Linux execution", "Provider integration", "Core/UI wiring", "signed release"], processes="real fixture supervisors use EOF/join, crash groups use SIGKILL/OS reap; suite asserts no surviving process groups")
    env = os.environ.copy()
    env["BATON_REMOTE_HOST_TEST_BINARY"] = str(Path(env["CARGO_TARGET_DIR"]) / "debug/baton-host-companion")
    env["BATON_REMOTE_HOST_TEST_TRACE"] = str((ROOT / run / "stdio-trace.jsonl").resolve())
    commands = [
        ("fmt", ["cargo", "fmt", "--manifest-path", MANIFEST, "--", "--check"]),
        ("rust-tests", ["cargo", "test", "--manifest-path", MANIFEST, "--locked"]),
        ("clippy", ["cargo", "clippy", "--manifest-path", MANIFEST, "--locked", "--all-targets", "--", "-D", "warnings"]),
        ("build", ["cargo", "build", "--manifest-path", MANIFEST, "--locked"]),
        ("real-subprocess-faults", [sys.executable, "tests/infrastructure/test_remote_host.py"]),
    ]
    for name, cmd in commands:
        start = time.time()
        log = run / (name + ".log")
        with log.open("wb") as output:
            if name == "real-subprocess-faults" and any(c["name"] == "build" and c["exit_code"] != 0 for c in evidence["checks"]):
                output.write(b"not_run: current source build failed; refusing stale binary\n")
                code = 125
            else:
                code = run_check(cmd, env, output)

        evidence["checks"].append(dict(name=name, command=cmd, exit_code=code, started_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(start)), elapsed_seconds=round(time.time()-start, 3), log=str(log), log_sha256=sha(log)))
        print(f"{name}: exit={code} log={log}", flush=True)
        (run / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    binary = Path(env["BATON_REMOTE_HOST_TEST_BINARY"])
    if binary.is_file() and any(c["name"] == "build" and c["exit_code"] == 0 for c in evidence["checks"]):
        evidence["binary"] = dict(path=str(binary), sha256=sha(binary), size=binary.stat().st_size, signing="development, no production signing")
    trace = run / "stdio-trace.jsonl"
    if trace.is_file():
        evidence["sanitized_stdio_trace"] = dict(path=str(trace), sha256=sha(trace), bytes=trace.stat().st_size)
    evidence["source_unchanged_during_verification"] = sources == {str(p): sha(p) for p in paths}
    evidence["passed"] = all(c["exit_code"] == 0 for c in evidence["checks"]) and evidence["source_unchanged_during_verification"]
    (run / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(f"evidence={run / 'evidence.json'} source_hash={source_hash}")
    return 0 if evidence["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
