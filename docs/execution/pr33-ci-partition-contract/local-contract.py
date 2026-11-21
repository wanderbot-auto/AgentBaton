"""Docs-only executable oracle for PR #33's partition candidates."""
from copy import deepcopy
from dataclasses import dataclass
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import zipfile


CORE = ("frontend", "core_frozen_workspace", "normal_local_smoke")
HISTORY = (
    "schema14_generate", "schema15_generate", "schema16_generate",
    "schema14_test_and_clippy", "schema15_test_and_clippy", "schema16_test_and_clippy",
)
DEFAULT_HOST = ("release_sidecars", "tauri_default_tests")
NATIVE = (
    "webview_lifecycle", "owner_dialog", "factory_planning", "native_smoke_lib",
    "background_library_and_real_ipc", "integration_last_page", "integration_first_page",
    "integration_projection",
)
PACKAGE = ("default_release_app", "release_binary_audit", "dmg_bundle_audit_and_package", "development_upload")
FRESH = ("schema18_fresh_migrate_restore",)
CONTEXT_KEYS = {
    "repository_id", "workflow", "event", "pull", "base_ref", "merge_ref", "head", "base",
    "checkout", "tree", "run_id", "attempt",
}


@dataclass(frozen=True)
class Layout:
    preceding_id: str
    required_id: str
    limits: tuple
    preceding_gates: tuple
    required_gates: tuple


LAYOUTS = {
    "prefix": Layout(
        "desktop", "desktop_release", (60, 30),
        HISTORY[:3] + CORE + DEFAULT_HOST + HISTORY[3:] + ("history_projection", "rework_projection"),
        ("native_frontend_preparation",) + FRESH + ("downstream_release_sidecars",) + NATIVE + ("native_diagnostics_upload",) + PACKAGE,
    ),
    "shared-release-in-required-job": Layout(
        "desktop", "desktop_release", (60, 30),
        HISTORY[:3] + CORE + HISTORY[3:] + ("history_projection", "rework_projection"),
        ("native_frontend_preparation",) + FRESH + DEFAULT_HOST + NATIVE + ("native_diagnostics_upload",) + PACKAGE,
    ),
    "history-and-package-first": Layout(
        "desktop_preparation", "desktop", (30, 60),
        HISTORY + FRESH + ("history_projection", "release_sidecars") + PACKAGE,
        CORE + DEFAULT_HOST + NATIVE + ("rework_projection", "native_diagnostics_upload"),
    ),
}


class Refusal(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise Refusal(reason)


def exact(obj, fields):
    return type(obj) is dict and obj.keys() == set(fields)


def positive(value):
    return type(value) is int and 0 < value < (1 << 53)


def hex_string(value, size):
    return type(value) is str and re.fullmatch(f"[0-9a-f]{{{size}}}", value) is not None


def verify_context(actual, expected):
    require(exact(actual, CONTEXT_KEYS) and exact(expected, CONTEXT_KEYS), "context_fields")
    require(actual["event"] == "pull_request" and actual["base_ref"] == "main", "event")
    require(actual["workflow"] == ".github/workflows/product.yml", "workflow")
    require(all(positive(actual[name]) for name in ("repository_id", "pull", "run_id", "attempt")), "context_ids")
    require(actual["merge_ref"] == f"refs/pull/{actual['pull']}/merge", "merge_ref")
    require(all(hex_string(actual[name], 40) for name in ("head", "base", "checkout", "tree")), "context_git")
    require(actual == expected and actual["checkout"] != actual["head"], "context_mismatch")


def unique_object(items):
    result = dict(items)
    require(len(items) == len(result), "duplicate_json_key")
    return result


def metadata(row):
    require(exact(row, ("id", "sha256", "name")) and positive(row["id"]) and hex_string(row["sha256"], 64),
            "artifact_metadata")
    require(type(row["name"]) is str and re.fullmatch("[a-z0-9-]{1,180}", row["name"]), "artifact_name")


def open_receipt(raw, observed_artifact, announced_artifact, expected_name):
    metadata(announced_artifact)
    metadata(observed_artifact)
    require(observed_artifact == announced_artifact and observed_artifact["name"] == expected_name, "artifact_identity")
    require(0 < len(raw) <= 64 * 1024 and sha256(raw).hexdigest() == announced_artifact["sha256"], "artifact_bytes")
    try:
        with zipfile.ZipFile(BytesIO(raw)) as archive:
            require(archive.namelist() == ["receipt.json"], "artifact_members")
            member = archive.infolist()[0]
            require(stat.S_IFMT(member.external_attr >> 16) in (0, stat.S_IFREG), "artifact_file_type")
            require(0 < member.file_size <= 32 * 1024 and member.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED),
                    "receipt_size_or_encoding")
            blob = archive.read(member)
        return json.loads(blob.decode("utf-8"), object_pairs_hook=unique_object)
    except (zipfile.BadZipFile, KeyError, UnicodeError, json.JSONDecodeError, RuntimeError, RecursionError) as error:
        raise Refusal("malformed_receipt") from error


def output_name(role, scope):
    artifact_role = role.replace("_", "-")
    return f"{artifact_role}-{scope['checkout']}-{scope['run_id']}-{scope['attempt']}"


def verify_stages(observed, gates):
    require(type(observed) is dict and list(observed) == list(gates), "gate_inventory")
    require(all(value == "success" for value in observed.values()), "gate_result")


def completion_receipt(receipt, role, context, gates):
    require(exact(receipt, ("format", "scope", "role", "runner_label", "os", "arch", "stages", "development")), "receipt_fields")
    require(receipt["format"] == 1 and type(receipt["format"]) is int and receipt["role"] == role, "receipt_role")
    require((receipt["runner_label"], receipt["os"], receipt["arch"]) == ("macos-15", "macOS", "ARM64"), "runner")
    verify_context(receipt["scope"], context)
    verify_stages(receipt["stages"], gates)
    development = receipt["development"]
    if "development_upload" not in gates:
        require(development is None, "unexpected_development")
        return
    require(exact(development, ("artifact", "dmg_sha256", "release_proof_sha256", "sidecars_sha256")), "development_fields")
    metadata(development["artifact"])
    require(development["artifact"]["name"] == output_name("development-macos-15", context), "development_name")
    require(all(hex_string(development[key], 64) for key in ("dmg_sha256", "release_proof_sha256")), "development_digest")
    pair = development["sidecars_sha256"]
    require(exact(pair, ("baton", "batond")) and all(hex_string(digest, 64) for digest in pair.values()),
            "development_sidecars")


def dependency_passed(layout, needs):
    require(exact(needs, (layout.preceding_id,)), "dependency_inventory")
    require(needs[layout.preceding_id] == "success", "dependency_result")


def validate_prefix(layout, expected, needs, claimed, downloaded, archive):
    dependency_passed(layout, needs)
    role = layout.preceding_id
    receipt = open_receipt(archive, downloaded, claimed, output_name(role, expected))
    completion_receipt(receipt, role, expected, layout.preceding_gates)
    return receipt


def canonical_json(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=True).encode()


def validate_direct_output(layout, expected, needs, raw, evidence_output):
    dependency_passed(layout, needs)
    require(type(raw) is bytes and 0 < len(raw) <= 32 * 1024, "direct_output_size")
    try:
        output = json.loads(raw.decode("ascii"), object_pairs_hook=unique_object)
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise Refusal("direct_output_json") from error
    require(exact(output, ("receipt", "evidence")) and canonical_json(output) == raw, "direct_output_encoding")
    metadata(output["evidence"])
    metadata(evidence_output)
    require(output["evidence"] == evidence_output, "direct_evidence_identity")
    completion_receipt(output["receipt"], layout.preceding_id, expected, layout.preceding_gates)
    return output["receipt"]


def validate_required(layout, expected, needs, claimed, downloaded, archive, required, status, selected_development):
    preceding = validate_prefix(layout, expected, needs, claimed, downloaded, archive)
    require(status == "success", "required_result")
    completion_receipt(required, layout.required_id, expected, layout.required_gates)
    built = preceding["development"] or required["development"]
    if built is not None:
        metadata(selected_development)
        require(built["artifact"] == selected_development and built["artifact"]["id"] != claimed["id"], "development_identity")


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def fixture_scope():
    checkout = "83c22bcc479c271d3389d812f954b63873066422"
    actual, tree, base, head = git("show", "-s", "--format=%H %T %P", checkout).split()
    require(actual == checkout and git("rev-parse", f"{head}^{{tree}}") == tree, "fixture_commit")
    return {
        "repository_id": 1434422493, "workflow": ".github/workflows/product.yml", "event": "pull_request",
        "pull": 33, "base_ref": "main", "merge_ref": "refs/pull/33/merge",
        "head": head, "base": base, "checkout": checkout, "tree": tree,
        "run_id": 44792307596, "attempt": 2,
    }


def fixture_receipt(context, role, gates):
    development = None
    if "development_upload" in gates:
        development = {
            "artifact": {"id": 7002, "name": output_name("development-macos-15", context), "sha256": "d" * 64},
            "dmg_sha256": "e" * 64, "release_proof_sha256": "f" * 64,
            "sidecars_sha256": {"baton": "a" * 64, "batond": "b" * 64},
        }
    return {
        "format": 1, "scope": dict(context), "role": role, "runner_label": "macos-15", "arch": "ARM64", "os": "macOS",
        "stages": dict.fromkeys(gates, "success"), "development": development,
    }


def fixture_zip(content, extra=None, mode=stat.S_IFREG):
    raw = BytesIO()
    with zipfile.ZipFile(raw, "w") as archive:
        entry = zipfile.ZipInfo("receipt.json")
        entry.create_system = 3
        entry.external_attr = (mode | 0o600) << 16
        archive.writestr(entry, content)
        if extra is not None:
            archive.writestr(extra, b"private synthetic sentinel")
    return raw.getvalue()


def check_layout(name, layout, scope):
    preceding = fixture_receipt(scope, layout.preceding_id, layout.preceding_gates)
    required_job = fixture_receipt(scope, layout.required_id, layout.required_gates)
    state = {
        "preceding": preceding, "required": required_job, "dependency": {layout.preceding_id: "success"},
        "required_result": "success", "development": deepcopy((preceding["development"] or required_job["development"])["artifact"]),
        "observed_id": 7001, "claimed_id": 7001, "claimed_sha": None, "extra": None,
        "mode": stat.S_IFREG, "corrupt": False, "duplicate_key": False,
    }

    def run(data):
        content = json.dumps(data["preceding"], separators=(",", ":"))
        if data["duplicate_key"]:
            content = '{"format":1,' + content[1:]
        raw = fixture_zip(content, data["extra"], data["mode"])
        name = output_name(layout.preceding_id, scope)
        actual = {"id": data["observed_id"], "name": name, "sha256": sha256(raw).hexdigest()}
        claimed = {**actual, "id": data["claimed_id"]}
        if data["claimed_sha"]:
            claimed["sha256"] = data["claimed_sha"]
        if data["corrupt"]:
            raw += b"changed bytes"
        validate_required(layout, scope, data["dependency"], claimed, actual, raw,
                          data["required"], data["required_result"], data["development"])

    run(state)
    print(f"pass {name}: matching PR, merge checkout, attempt, gates and artifact")
    cases = []
    for outcome in ("failure", "skipped", "cancelled", "timed_out", "neutral", None):
        cases.append((f"upstream-{outcome}", "dependency_result",
                      lambda value, result=outcome: value["dependency"].update({layout.preceding_id: result})))
    cases.extend([
        ("missing-dependency", "dependency_inventory", lambda value: value["dependency"].clear()),
        ("required-cancelled", "required_result", lambda value: value.update(required_result="cancelled")),
        ("old-attempt-same-sha", "context_mismatch", lambda value: value["preceding"]["scope"].update(attempt=1)),
        ("different-run-same-sha", "context_mismatch", lambda value: value["preceding"]["scope"].update(run_id=44)),
        ("same-tree-pr-head", "context_mismatch", lambda value: value["preceding"]["scope"].update(checkout=scope["head"])),
        ("wrong-tree", "context_mismatch", lambda value: value["preceding"]["scope"].update(tree="7" * 40)),
        ("wrong-base", "context_mismatch", lambda value: value["preceding"]["scope"].update(base="8" * 40)),
        ("wrong-head", "context_mismatch", lambda value: value["preceding"]["scope"].update(head="9" * 40)),
        ("manual-event", "event", lambda value: value["preceding"]["scope"].update(event="workflow_dispatch")),
        ("head-ref", "merge_ref", lambda value: value["preceding"]["scope"].update(merge_ref="refs/pull/33/head")),
        ("wrong-repository", "context_mismatch", lambda value: value["preceding"]["scope"].update(repository_id=99)),
        ("wrong-source-workflow", "workflow", lambda value: value["preceding"]["scope"].update(workflow="unrelated.yml")),
        ("intel", "runner", lambda value: value["preceding"].update(arch="X64")),
        ("different-runner-image", "runner", lambda value: value["preceding"].update(runner_label="macos-latest")),
        ("stale-required", "context_mismatch", lambda value: value["required"]["scope"].update(attempt=1)),
        ("missing-native-gate", "gate_inventory", lambda value: value["required"]["stages"].pop(layout.required_gates[0])),
        ("extra-build-step", "gate_inventory", lambda value: value["required"]["stages"].update(extra_build="success")),
        ("skipped-package-or-native-stage", "gate_result",
         lambda value: value["required"]["stages"].update({layout.required_gates[-1]: "skipped"})),
        ("wrong-artifact-id", "artifact_identity", lambda value: value.update(observed_id=7999)),
        ("wrong-artifact-digest", "artifact_identity", lambda value: value.update(claimed_sha="2" * 64)),
        ("changed-archive-bytes", "artifact_bytes", lambda value: value.update(corrupt=True)),
        ("private-db", "artifact_members", lambda value: value.update(extra="private.sqlite3")),
        ("executable-transfer", "artifact_members", lambda value: value.update(extra="batond")),
        ("path-traversal", "artifact_members", lambda value: value.update(extra="../receipt.json")),
        ("link", "artifact_file_type", lambda value: value.update(mode=stat.S_IFLNK)),
        ("duplicate-receipt-key", "duplicate_json_key", lambda value: value.update(duplicate_key=True)),
        ("wrong-development-artifact", "development_identity", lambda value: value["development"].update(id=8999)),
    ])
    for test, expected, mutate in cases:
        data = deepcopy(state)
        mutate(data)
        try:
            run(data)
        except Refusal as error:
            if str(error) == expected:
                print(f"refused {name}/{test}: {error}")
                continue
            raise
        raise AssertionError(f"accepted {name}/{test}")
    return len(cases)


def check_direct_output(scope):
    layout = LAYOUTS["prefix"]
    role = layout.preceding_id
    passed = {role: "success"}
    evidence = {"id": 8001, "name": output_name("desktop-history", scope), "sha256": "c" * 64}
    output = {"receipt": fixture_receipt(scope, role, layout.preceding_gates), "evidence": dict(evidence)}
    assert validate_direct_output(layout, scope, passed, canonical_json(output), evidence) == output["receipt"]
    print("pass direct-needs-output: same attempt, exact evidence ID and digest")
    old = deepcopy(output)
    old["receipt"]["scope"]["attempt"] = 1
    missing_gate = deepcopy(output)
    missing_gate["receipt"]["stages"].pop("schema14_test_and_clippy")
    cases = [
        ("skipped", {role: "skipped"}, canonical_json(output), evidence, "dependency_result"),
        ("missing", passed, b"", evidence, "direct_output_size"),
        ("old-attempt", passed, canonical_json(old), evidence, "context_mismatch"),
        ("missing-migration", passed, canonical_json(missing_gate), evidence, "gate_inventory"),
        ("wrong-artifact", passed, canonical_json(output), {**evidence, "id": 8002}, "direct_evidence_identity"),
        ("wrong-digest", passed, canonical_json(output), {**evidence, "sha256": "d" * 64}, "direct_evidence_identity"),
        ("noncanonical", passed, canonical_json(output) + b"\n", evidence, "direct_output_encoding"),
    ]
    for name, needs, raw, expected, reason in cases:
        try:
            validate_direct_output(layout, scope, needs, raw, expected)
        except Refusal as error:
            if str(error) == reason:
                print(f"refused direct-needs-output/{name}: {error}")
                continue
            raise
        raise AssertionError(f"accepted direct-needs-output/{name}")
    return len(cases)


def check_existing_sidecar_staging():
    """A private CLI shim dispatches only Cargo metadata to the actual Cargo."""
    rustc = shutil.which("rustc")
    cargo = shutil.which("cargo")
    if not (rustc and cargo):
        raise RuntimeError("Local staging controls need Rust tooling")
    host = next(line.split()[1] for line in subprocess.check_output([rustc, "-vV"], text=True).splitlines() if line.startswith("host: "))
    with tempfile.TemporaryDirectory(prefix="pr33-partition-") as temporary:
        folder = Path(temporary)
        scripts = folder / "scripts"
        scripts.mkdir()
        (scripts / "prepare-desktop-sidecars.sh").write_bytes(
            subprocess.check_output(["git", "show", "83c22bcc:scripts/prepare-desktop-sidecars.sh"])
        )
        (folder / "Cargo.toml").write_text('[package]\nname = "partition-oracle"\nversion = "0.1.0"\nedition = "2021"\n')
        (folder / "src").mkdir()
        (folder / "src/lib.rs").touch()
        shim = folder / "cargo"
        shim.write_text(
            '#!/usr/bin/env bash\nset -euo pipefail\n'
            'printf "%s\\n" "$*" >> "$CARGO_ORACLE_LOG"\n'
            '[[ "$*" == "metadata --offline --no-deps --format-version 1" ]] || exit 91\n'
            'exec "$CARGO_ORACLE_REAL" "$@"\n'
        )
        shim.chmod(0o700)
        target = folder / "target"
        release = target / "release"
        release.mkdir(parents=True)
        log = folder / "cargo.log"
        env = {
            **os.environ, "PATH": f"{folder}{os.pathsep}{os.environ['PATH']}",
            "CARGO_TARGET_DIR": str(target), "CARGO_ORACLE_LOG": str(log), "CARGO_ORACLE_REAL": cargo,
        }
        for role in ("baton", "batond"):
            payload = release / role
            payload.write_bytes(f"synthetic {role} fixture, never executed\n".encode())
            payload.chmod(0o700)
        staged = folder / "apps/desktop/src-tauri/binaries"
        script = ["bash", "scripts/prepare-desktop-sidecars.sh"]
        subprocess.run(script, cwd=folder, env=env, check=True)
        for role in ("baton", "batond"):
            assert (staged / f"{role}-{host}").read_bytes() == (release / role).read_bytes()
        assert log.read_text().splitlines() == ["metadata --offline --no-deps --format-version 1"]
        print(f"pass sidecar-staging: actual helper, exact bytes, cargo metadata only, {host}")
        for entry in staged.iterdir():
            entry.unlink()
        (release / "batond").unlink()
        incomplete = subprocess.run(script, cwd=folder, env=env)
        assert incomplete.returncode != 0 and not (staged / f"batond-{host}").exists()
        assert log.read_text().splitlines()[-1] == "metadata --offline --no-deps --format-version 1"
        assert subprocess.run(["cargo", "build", "--release"], cwd=folder, env=env).returncode == 91
        print("refused sidecar-staging/missing-release-input and unexpected cargo build")


if __name__ == "__main__":
    if len(sys.argv) != 1:
        raise SystemExit("Usage: python3 docs/execution/pr33-ci-partition-contract/local-contract.py")
    scope = fixture_scope()
    total = sum(check_layout(name, layout, scope) for name, layout in LAYOUTS.items())
    direct = check_direct_output(scope)
    check_existing_sidecar_staging()
    print(f"{len(LAYOUTS)} positive layouts, {total} negative ZIP controls, 1 positive/{direct} negative direct-output controls, 3 staging controls")
