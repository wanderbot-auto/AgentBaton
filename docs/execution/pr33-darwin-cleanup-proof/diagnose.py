"""Observe the exact runner's use of Darwin cleanup on disposable child sessions."""

import ast
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import time
from unittest import mock


def load_runner(source):
    spec = importlib.util.spec_from_file_location("diagnostic_native_run", source)
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    return runner


def inspect_ps(option, identifier, columns):
    command = ["/bin/ps", "-o", columns, option, str(identifier)]
    result = subprocess.run(command, text=True, capture_output=True, timeout=1, check=False)
    return {"exit": result.returncode, "rows": result.stdout.splitlines(), "stderr": result.stderr}


def trace(runner, output, case):
    output.mkdir()
    fault = case == "observation-error"
    # These commands are the original direct suite's generated fixture inputs.
    code = ("import time;time.sleep(20)" if fault else
            "import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);time.sleep(20)")
    command = [sys.executable, "-c", code]
    budget = 3 if fault else 1
    real_popen, real_killpg = subprocess.Popen, os.killpg
    real_observation, real_proof = runner.owned_child_exited, runner.owned_group_is_quiescent
    signals, owner, proof_calls, waits = [], {}, [], []
    start = time.monotonic()

    def spawn(argv, *args, **kwargs):
        child = real_popen(argv, *args, **kwargs)
        if argv != command:
            return child
        owner["child"] = child
        owner["start_new_session"] = kwargs.get("start_new_session")
        real_wait = child.wait

        def observe_wait(*wait_args, **wait_kwargs):
            assert child.returncode is None
            waiting = {
                "timeout_seconds": wait_kwargs["timeout"],
                "leader_before_wait": inspect_ps("-p", child.pid, "pid=,ppid=,pgid=,stat="),
                "group_before_wait": inspect_ps("-g", child.pid, "pid=,pgid=,stat="),
                "unchanged_quiescence_proof_before_wait": real_proof(child.pid),
            }
            waits.append(waiting)
            waiting["exit"] = real_wait(*wait_args, **wait_kwargs)
            waiting["leader_after_wait"] = inspect_ps("-p", child.pid, "pid=,ppid=,pgid=,stat=")
            waiting["group_after_wait"] = inspect_ps("-g", child.pid, "pid=,pgid=,stat=")
            try:
                waiting["waitpid_after_wait"] = {"unexpected_result": os.waitpid(child.pid, os.WNOHANG)}
            except ChildProcessError as error:
                waiting["waitpid_after_wait"] = {"type": type(error).__name__, "errno": error.errno}
            return waiting["exit"]

        child.wait = observe_wait
        return child

    def observe_signal(group, sig):
        child = owner["child"]
        assert group == child.pid and owner["start_new_session"] is True
        assert child.returncode is None
        row = {"signal": sig.name}
        signals.append(row)
        if not fault:
            row["group_before_signal"] = inspect_ps("-g", group, "pid=,pgid=,stat=")
            row["unchanged_quiescence_proof_before_signal"] = real_proof(group)
        try:
            real_killpg(group, sig)
        except OSError as error:
            row.update(type=type(error).__name__, errno=error.errno)
            raise
        else:
            row["result"] = "signalled"

    def observe_leader(pid):
        assert pid == owner["child"].pid
        if fault:
            # Match test_observation_failure_still_reaps_child_and_writes_failed_report.
            raise OSError(errno.EINVAL, "controlled observation failure")
        return real_observation(pid)

    def observe_runner_proof(group):
        assert group == owner["child"].pid
        result = real_proof(group)
        proof_calls.append(result)
        return result

    try:
        with (
            mock.patch.object(runner.subprocess, "Popen", side_effect=spawn),
            mock.patch.object(runner.os, "killpg", side_effect=observe_signal),
            mock.patch.object(runner, "owned_child_exited", side_effect=observe_leader),
            mock.patch.object(runner, "owned_group_is_quiescent", side_effect=observe_runner_proof),
        ):
            result = runner.run(command, output, budget)
    finally:
        child = owner.get("child")
        if child is not None and child.returncode is None:
            # Diagnostic fallback only, constrained to the original unreaped owned child.
            try:
                real_killpg(child.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            subprocess.Popen.wait(child, timeout=3)

    record = {
        "case": case,
        "harness_timeout_seconds": budget,
        "wall_seconds": round(time.monotonic() - start, 3),
        "parent_pid": os.getpid(),
        "child_pid": owner["child"].pid,
        "start_new_session": owner["start_new_session"],
        "signals": signals,
        "proof_calls_from_run": proof_calls,
        "waits": waits,
        "run_exit": result,
        "runner_report": json.loads((output / "native-runner.json").read_bytes()),
    }
    (output / "trace.json").write_text(json.dumps(record, indent=2) + "\n")
    assert len(waits) == 1
    after_wait = waits[0]
    assert 0 < after_wait["timeout_seconds"] <= 3
    assert after_wait["waitpid_after_wait"] == {"type": "ChildProcessError", "errno": errno.ECHILD}
    assert not after_wait["leader_after_wait"]["rows"]
    assert not after_wait["group_after_wait"]["rows"]
    assert record["run_exit"] == 1 and record["runner_report"]["status"] == "failed"
    assert record["wall_seconds"] < budget + 3
    if fault:
        assert signals == [{"signal": "SIGTERM", "result": "signalled"},
                           {"signal": "SIGKILL", "type": "PermissionError", "errno": errno.EPERM}]
        assert proof_calls == []
        assert after_wait["unchanged_quiescence_proof_before_wait"] is True
        assert after_wait["exit"] == -signal.SIGTERM
        assert record["runner_report"]["owned_group_cleanup"] is False
        assert record["runner_report"]["timeout"] is False
        assert record["runner_report"]["observation_error"] == "OSError:22"
    else:
        assert len(signals) == 2 and all(row["result"] == "signalled" for row in signals)
        assert all(row["unchanged_quiescence_proof_before_signal"] is False for row in signals)
        assert after_wait["exit"] == -signal.SIGKILL
        assert record["runner_report"]["timeout"] is True
        assert record["runner_report"]["owned_group_cleanup"] is True
    return record


def main(root):
    assert sys.platform == "darwin" and platform.machine() == "arm64"
    assert sys.version_info[:2] == (3, 12)
    report = {"system": platform.platform(), "python": sys.version, "waitid": hasattr(os, "waitid"),
              "snapshots": {}}
    for short in ("ae844ca3", "443556ee"):
        snapshot = root / short
        source = snapshot / "scripts/native-run.py"
        raw = source.read_bytes()
        text = raw.decode()
        functions = {node.name: ast.get_source_segment(text, node)
                     for node in ast.parse(text).body if isinstance(node, ast.FunctionDef)}
        names = ("run", "owned_child_exited", "owned_group_is_quiescent")
        runner = load_runner(source)
        commit, tree = (snapshot / "source.txt").read_text().splitlines()
        report["snapshots"][short] = {
            "commit": commit, "tree": tree,
            "runner_sha256": hashlib.sha256(raw).hexdigest(),
            "cleanup_function_sha256": {name: hashlib.sha256(functions[name].encode()).hexdigest()
                                         for name in names},
            "cases": [trace(runner, snapshot / case, case)
                      for case in ("observation-error", "timeout")],
        }
    assert report["snapshots"]["ae844ca3"]["cleanup_function_sha256"] == (
        report["snapshots"]["443556ee"]["cleanup_function_sha256"])
    (root / "diagnosis.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve(strict=True))
