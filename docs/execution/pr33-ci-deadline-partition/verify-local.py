"""Local checks for the actual workflow structure and macOS source binding."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

import yaml


BASE = "9e3add27c37f4022dcd2517ab9d1c9cc8f3e9102"
WORKFLOW = ".github/workflows/product.yml"
ROOT = Path(__file__).resolve().parents[3]


def git(path, *args):
    return subprocess.check_output(["git", "-C", path, *args], text=True).strip()


def check_structure(raw, original):
    assert raw.split("  desktop:\n")[0] == original.split("  desktop:\n")[0]
    before, after = [yaml.load(source, Loader=yaml.BaseLoader) for source in (original, raw)]
    base = before["jobs"]["desktop"]
    core = after["jobs"]["desktop"]
    release = after["jobs"]["desktop_release"]
    assert (core["timeout-minutes"], release["timeout-minutes"]) == ("60", "30")
    assert base["strategy"] == core["strategy"] == release["strategy"]
    assert base["runs-on"] == core["runs-on"] == release["runs-on"]
    assert base["name"] == release["name"] != core["name"]
    assert sum(job["name"] == base["name"] for job in after["jobs"].values()) == 1
    assert release["needs"] == "desktop" and release["if"] == "${{ always() }}"
    assert not any("permissions" in job or "continue-on-error" in job for job in (core, release))
    step_names = {
        step["name"]: step for job in (core, release) for step in job["steps"] if "name" in step
    }
    preserved = 0
    for step in base["steps"]:
        name = step.get("name")
        if name is None:
            continue
        copied = step_names[name].copy()
        copied.pop("id", None)
        if name == "Retain finite Native integration evidence without private snapshots":
            assert "always()" in copied["if"] and "release_source_start.outcome" in copied["if"]
            copied["if"] = step["if"]
        assert copied == step, name
        preserved += 1
    for job in (core, release):
        checkout = next(step for step in job["steps"] if step.get("uses", "").startswith("actions/checkout@"))
        assert checkout["with"] == {"persist-credentials": "false", "ref": "${{ github.sha }}"}
        for step in job["steps"]:
            if "run" in step:
                subprocess.run(["bash", "-n"], input=step["run"], text=True, check=True)
    artifacts = [step["with"] for step in release["steps"] if step.get("uses", "").startswith("actions/upload-artifact@")]
    assert artifacts == [step["with"] for step in base["steps"] if step.get("uses", "").startswith("actions/upload-artifact@")]
    core_artifact = next(step for step in core["steps"] if step.get("id") == "core_artifact")
    assert core_artifact["if"] == "always()"
    assert all(name.endswith((".json", ".log", ".stderr")) for name in core_artifact["with"]["path"].splitlines())
    return core, release, preserved


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def fixture(root):
    full = root / "full"
    full.mkdir()
    git(full, "init", "-q", "--initial-branch=main")
    git(full, "config", "user.email", "generated@example.invalid")
    git(full, "config", "user.name", "Generated")
    git(full, "config", "commit.gpgsign", "false")
    git(full, "config", "core.hooksPath", "/dev/null")
    (full / "base").write_text("pr fixture\n")
    git(full, "add", "base")
    git(full, "commit", "-qm", "Initial")
    git(full, "switch", "-qc", "feature")
    git(full, "commit", "--allow-empty", "-qm", "Feature")
    head = git(full, "rev-parse", "HEAD")
    git(full, "switch", "-q", "main")
    git(full, "commit", "--allow-empty", "-qm", "Main")
    base = git(full, "rev-parse", "HEAD")
    git(full, "merge", "--no-ff", "-qm", "PR merge", "feature")
    clone = root / "shallow"
    git(root, "clone", "-q", "--no-local", "--depth=1", str(full), str(clone))
    assert git(clone, "rev-parse", "--is-shallow-repository") == "true"
    return clone, base, head, git(clone, "rev-parse", "HEAD")


def check_binding(core, release, root):
    source = next(step for step in core["steps"] if step.get("id") == "core_source_start")
    complete = next(step for step in core["steps"] if step.get("id") == "core_source_complete")
    binding = source["run"]
    assert binding == complete["run"]
    admission = release["steps"][0]
    assert "actions/checkout@" in release["steps"][1]["uses"]
    release_start = next(step for step in release["steps"] if step.get("id") == "release_source_start")
    assert release_start["run"] == binding
    repo, base, head, merge = fixture(root)
    event_path = root / "event.json"
    output = root / "out"
    repository = {"id": 123, "full_name": "local/synthetic"}
    event = {"repository": repository, "number": 13,
             "pull_request": {"base": {"ref": "main", "sha": base}, "head": {"sha": head}}}
    event_path.write_text(json.dumps(event))
    env = {
        "GITHUB_EVENT_NAME": "pull_request",
        "GITHUB_EVENT_PATH": str(event_path),
        "GITHUB_REPOSITORY": repository["full_name"],
        "GITHUB_REPOSITORY_ID": str(repository["id"]),
        "GITHUB_WORKFLOW_REF": "local/synthetic/.github/workflows/product.yml@refs/pull/13/merge",
        "GITHUB_WORKFLOW_SHA": merge,
        "GITHUB_REF": "refs/pull/13/merge",
        "GITHUB_SHA": merge,
        "GITHUB_RUN_ID": "456",
        "GITHUB_RUN_ATTEMPT": "2",
        "GITHUB_OUTPUT": str(output),
    }
    tests = {}

    def invoke(label, script, values, success):
        output.write_text("")
        call = subprocess.run(["bash", "-c", script], env=os.environ | env | values,
                              cwd=repo, text=True, capture_output=True, timeout=10)
        assert (call.returncode == 0) is success, (label, call.returncode, call.stderr)
        tests[label] = call.returncode
        return output.read_text(), call.stdout

    gate_outcomes = complete["env"]["MACOS_CORE_GATE_OUTCOMES"]
    gates = " ".join(f"{token.split('=')[0]}=success" for token in gate_outcomes.split()
                     if "=${{" in token)
    started, _ = invoke("PR shallow checkout", binding, {"MACOS_SOURCE_STAGE": "core-start"}, True)
    context = started.strip().split("=", 1)[1]
    core_success = {"MACOS_SOURCE_STAGE": "core-complete",
                    "MACOS_CORE_START_CONTEXT": context, "MACOS_CORE_GATE_OUTCOMES": gates,
                    "MACOS_CORE_ARTIFACT_ID": "600", "MACOS_CORE_ARTIFACT_SHA256": "a" * 64}
    produced, _ = invoke("Core gates and finite artifact", binding, core_success, True)
    receipt = produced.strip().split("=", 1)[1]
    gates_as_dict = json.loads(receipt)["gates"]
    assert len(gates_as_dict) == 11 and set(gates_as_dict.values()) == {"success"}
    upload = {"MACOS_CORE_ARTIFACT_ID": "600", "MACOS_CORE_ARTIFACT_SHA256": "a" * 64}
    upstream = {"MACOS_CORE_RESULT": "success", "MACOS_CORE_RECEIPT": receipt} | upload
    invoke("required entry accepts upstream success", admission["run"], upstream, True)
    for status in ("", "failure", "cancelled", "skipped", "neutral", "SUCCESS"):
        invoke(f"required entry refuses {status or 'missing'}", admission["run"],
               upstream | {"MACOS_CORE_RESULT": status}, False)
    invoke("required entry refuses missing receipt", admission["run"],
           upstream | {"MACOS_CORE_RECEIPT": ""}, False)
    invoke("required entry refuses missing independent artifact", admission["run"],
           upstream | {"MACOS_CORE_ARTIFACT_ID": ""}, False)
    release_values = {"MACOS_SOURCE_STAGE": "release-check", "MACOS_CORE_RECEIPT": receipt} | upload
    invoke("identical release receipt", binding, release_values, True)
    for key, wrong in {
        "event": "workflow_dispatch", "repository_id": "124", "workflow_path": ".github/workflows/other.yml",
        "os": "Linux", "architecture": "x86_64",
        "workflow_ref": "wrong", "workflow_sha": "1" * 40, "base_ref": "other", "merge_ref": "wrong",
        "pr_number": 14, "base": "1" * 40, "head": "1" * 40, "tree": "1" * 40,
        "checkout": "1" * 40, "run_id": "457", "run_attempt": "1",
    }.items():
        changed = json.loads(receipt)
        changed["source"][key] = wrong
        invoke(f"reject mismatched {key}", binding, release_values | {"MACOS_CORE_RECEIPT": encode(changed)}, False)
    changed = json.loads(receipt)
    changed["source"]["pr_number"] = 13.0
    invoke("reject equal but noninteger PR number", binding,
           release_values | {"MACOS_CORE_RECEIPT": encode(changed)}, False)
    invoke("reject independent upload ID mismatch", binding,
           release_values | {"MACOS_CORE_ARTIFACT_ID": "603"}, False)
    invoke("reject independent upload digest mismatch", binding,
           release_values | {"MACOS_CORE_ARTIFACT_SHA256": "d" * 64}, False)
    for reason, key, wrong in (
        ("missing receipt artifact ID", "id", ""),
        ("short receipt artifact digest", "sha256", "0" * 63),
        ("different receipt artifact ID", "id", "602"),
    ):
        changed = json.loads(receipt)
        changed["core_evidence"][key] = wrong
        invoke(f"reject {reason}", binding,
               release_values | {"MACOS_CORE_RECEIPT": encode(changed)}, False)
    invoke("reject noncanonical receipt", binding, release_values | {"MACOS_CORE_RECEIPT": receipt + " "}, False)
    invoke("reject unpassed Core gate", binding, core_success | {
        "MACOS_CORE_GATE_OUTCOMES": gates.replace("frontend=success", "frontend=skipped")}, False)
    invoke("reject receipt after Core attempt change", binding,
           core_success | {"GITHUB_RUN_ATTEMPT": "3"}, False)
    invoke("reject stale success from previous attempt", binding,
           release_values | {"GITHUB_RUN_ATTEMPT": "3"}, False)
    invoke("reject checkout not pinned to event", binding, release_values | {"GITHUB_SHA": head}, False)
    event["pull_request"]["base"]["sha"] = head
    event_path.write_text(json.dumps(event))
    invoke("reject wrong Git merge parents", binding, release_values, False)
    event["pull_request"]["base"]["sha"] = base
    event_path.write_text(json.dumps(event))
    (repo / "base").write_text("mutated byte\n")
    invoke("reject tracked source change", binding, release_values, False)
    (repo / "base").write_text("pr fixture\n")
    artifact_values = {
        "MACOS_SOURCE_STAGE": "release-complete", "MACOS_NATIVE_ARTIFACT_ID": "601",
        "MACOS_NATIVE_ARTIFACT_SHA256": "b" * 64, "MACOS_DEVELOPMENT_ARTIFACT_ID": "602",
        "MACOS_DEVELOPMENT_ARTIFACT_SHA256": "c" * 64,
    }
    _, final = invoke("record exact artifact roles", binding, release_values | artifact_values, True)
    assert json.loads(final)["development"]["id"] == "602"
    invoke("reject duplicate artifact identity", binding, release_values | artifact_values
           | {"MACOS_DEVELOPMENT_ARTIFACT_ID": "601"}, False)
    invoke("reject missing development digest", binding, release_values | artifact_values
           | {"MACOS_DEVELOPMENT_ARTIFACT_SHA256": ""}, False)
    event_path.write_text(json.dumps({"repository": repository}))
    dispatch = {"GITHUB_EVENT_NAME": "workflow_dispatch", "GITHUB_REF": "refs/heads/diagnostic",
                "GITHUB_WORKFLOW_REF": "local/synthetic/.github/workflows/product.yml@refs/heads/diagnostic"}
    dispatch_output, _ = invoke("manual dispatch is bound as a separate diagnostic source", binding,
                                dispatch | {"MACOS_SOURCE_STAGE": "core-start"}, True)
    diagnostic = json.loads(dispatch_output.strip().split("=", 1)[1])
    assert diagnostic["base"] == "" and diagnostic["head"] == merge and diagnostic["pr_number"] == 0
    invoke("manual dispatch cannot replace a PR source", binding, release_values | dispatch, False)
    return tests


def main():
    candidate = (ROOT / WORKFLOW).read_text()
    baseline = subprocess.check_output(["git", "-C", ROOT, "show", f"{BASE}:{WORKFLOW}"], text=True)
    core, release, preserved = check_structure(candidate, baseline)
    with tempfile.TemporaryDirectory(prefix="ab-macos-partition-") as fixture_root:
        results = check_binding(core, release, Path(fixture_root))
    print(json.dumps({"preserved_named_steps": preserved, "cases": results, "passed": len(results)}, indent=2))


if __name__ == "__main__":
    main()
