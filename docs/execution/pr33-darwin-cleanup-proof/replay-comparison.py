"""Repeat the pinned Darwin syscall observation with the old and repaired runner."""

from hashlib import sha256
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import types


def git_blob(commit, path):
    return subprocess.check_output(["git", "cat-file", "blob", commit + ":" + path], timeout=5)


def load_tracer():
    tracer_path = Path(__file__).with_name("diagnose.py")
    raw = tracer_path.read_bytes()
    if sha256(raw).hexdigest() != "49ae2025e83fca185d5c6aaba5b706c5f49b54b1bcac008d227f82a34662da82":
        raise ValueError("Unexpected Darwin tracer bytes")
    source = raw.decode()
    # Reuse the same observer. Parameterize only the pre-/post-fix assertions.
    changes = {
        "def trace(runner, output, case):": "def trace(runner, output, case, refreshed_proof):",
        "assert proof_calls == []": "assert proof_calls == ([True] if refreshed_proof else [])",
        'assert record["runner_report"]["owned_group_cleanup"] is False':
            'assert record["runner_report"]["owned_group_cleanup"] is refreshed_proof',
    }
    for original, replacement in changes.items():
        assert source.count(original) == 1
        source = source.replace(original, replacement)
    tracer = types.ModuleType("pinned_darwin_tracer")
    exec(compile(source, str(tracer_path), "exec"), tracer.__dict__)
    return tracer


def summarize(trace):
    wait = trace["waits"][0]
    groups = wait["group_before_wait"]["rows"]
    [pid, pgid, state] = groups[0].split() if len(groups) == 1 else (None, None, "")
    record = trace["runner_report"]
    return {
        "case": trace["case"],
        "wall_seconds": trace["wall_seconds"],
        "harness_timeout_seconds": trace["harness_timeout_seconds"],
        "signal_syscalls": [{key: row[key] for key in ("signal", "errno", "result") if key in row}
                            for row in trace["signals"]],
        "fresh_group_proof_from_runner": trace["proof_calls_from_run"],
        "only_original_zombie_at_wait": pid == pgid == str(trace["child_pid"]) and state.startswith("Z"),
        "pre_wait_independent_quiescence": wait["unchanged_quiescence_proof_before_wait"],
        "wait_exit": wait["exit"],
        "second_waitpid_errno": wait["waitpid_after_wait"]["errno"],
        "no_pid_or_group_after_wait": not wait["leader_after_wait"]["rows"] and not wait["group_after_wait"]["rows"],
        "runner_report": record,
        "run_exit": trace["run_exit"],
    }


def main(candidate):
    assert sys.platform == "darwin" and platform.machine() == "arm64"
    assert sys.version_info[:2] == (3, 12)
    root = Path(tempfile.mkdtemp(prefix="darwin-contrast.", dir=os.environ["BATON_RUNTIME_DIR"]))
    tracer = load_tracer()
    summary = {"platform": platform.platform(), "python": sys.version.split()[0], "snapshots": {}}
    for label, commit in (("base", "53da4ce20fdfedae4a745e704394072ad6b742ae"), ("candidate", candidate)):
        snapshot = root / label
        snapshot.mkdir()
        runner_file = snapshot / "native-run.py"
        runner_file.write_bytes(git_blob(commit, "scripts/native-run.py"))
        runner = tracer.load_runner(runner_file)
        runs = [summarize(tracer.trace(runner, snapshot / case, case, label == "candidate"))
                for case in ("observation-error", "timeout")]
        if label == "candidate":
            fault = runs[0]["runner_report"]
            assert "cleanup_errors" not in fault
            assert fault["group_signals"][-1] == {"signal": "SIGKILL", "result": "observed_quiescent"}
        summary["snapshots"][label] = {"commit": commit, "cases": runs}
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main(sys.argv[1])
