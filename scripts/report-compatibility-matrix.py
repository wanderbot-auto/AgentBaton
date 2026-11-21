#!/usr/bin/env python3
"""Project pinned local compatibility evidence without probing Providers or platforms."""

from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
SPEC = "specs/provider-compatibility.json"
TAURI = "apps/desktop/src-tauri/tauri.conf.json"
INDEX = "docs/execution/compatibility-matrix/OBSERVATIONS.json"
DIMENSIONS = {
    "provider_contract", "provider_real", "library", "browser", "hosted_ci",
    "package_archive", "build", "native_app", "credential_store", "background", "install", "update",
}
RESULTS = ("failed", "passed", "blocked", "unsupported", "not_run", "not_applicable", "unknown")
BUILD_KEYS = ("source_commit", "product_version", "os", "os_version", "os_build",
              "architecture", "artifact_sha256")
PROVIDER_KEYS = ("provider_version", "adapter", "adapter_version", "protocol_version")
# Labels permit only reviewed context carried by older evidence. Identity used
# by the release gate must come from fields in the pinned evidence itself.
CONTEXT_KEYS = {"provider_id", "adapter", "os", "os_version", "os_build", "architecture", "runner"}
FACT_KEYS = set(BUILD_KEYS + PROVIDER_KEYS) | CONTEXT_KEYS | {
    "feature_commit", "binary_sha256", "artifact_id", "ci_run", "ci_job",
    "signature", "recorded_at", "test_count", "platform_description", "tree", "log_sha256",
    "driver_commit", "driver_sha256", "source_dimension_label", "browser_version",
}
PLATFORM_GATES = ("build", "native_app", "credential_store", "background", "install", "update")
COMMIT = re.compile(r"[a-f0-9]{40}")
DIGEST = re.compile(r"[a-f0-9]{64}")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def token(value, name, pattern=None):
    require(isinstance(value, str) and bool(value.strip()), f"{name}: expected a nonempty string")
    if pattern:
        require(pattern.fullmatch(value) is not None, f"{name}: invalid value")
    return value


def mapping(value, name):
    require(isinstance(value, dict), f"{name}: expected an object")
    return value


def array(value, name):
    require(isinstance(value, list), f"{name}: expected an array")
    return value


def choice(value, allowed, field):
    require(isinstance(value, str) and value in allowed, f"{field}: invalid value")
    return value


def unique_keys(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, f"duplicate JSON key {key!r}")
        obj[key] = value
    return obj


def bad_constant(value):
    raise ValueError(f"invalid JSON constant {value!r}")


def evidence_path(path):
    token(path, "evidence path")
    parts = path.split("/")
    require(parts[:2] == ["docs", "execution"] and len(parts) >= 4 and path.endswith(".json")
            and all(part not in ("", ".", "..") for part in parts)
            and not any(ord(char) < 32 or char in "\\:" for char in path),
            f"expected a repository-relative JSON path under docs/execution: {path!r}")
    return path


def read_document(path):
    """Open each directory and the final file relative to the pinned checkout."""
    flags = os.O_RDONLY | os.O_NOFOLLOW
    with ExitStack() as opened:
        parent = os.open(ROOT, flags | os.O_DIRECTORY)
        opened.callback(os.close, parent)
        parts = path.split("/")
        for directory in parts[:-1]:
            parent = os.open(directory, flags | os.O_DIRECTORY, dir_fd=parent)
            opened.callback(os.close, parent)
        fd = os.open(parts[-1], flags | os.O_NONBLOCK, dir_fd=parent)
        with os.fdopen(fd, "rb") as stream:
            require(stat.S_ISREG(os.fstat(fd).st_mode), f"{path}: expected a regular file")
            raw = stream.read()
    try:
        obj = json.loads(raw, object_pairs_hook=unique_keys, parse_constant=bad_constant)
    except (ValueError, RecursionError) as exc:
        raise ValueError(f"{path}: {exc}") from exc
    return obj, {"path": path, "sha256": hashlib.sha256(raw).hexdigest()}


def lookup(document, path):
    """Resolve a JSON Pointer within evidence already loaded by the report."""
    require(isinstance(path, str) and path.startswith("/"), "expected a non-root JSON Pointer")
    value = document
    try:
        for part in path[1:].split("/"):
            require(re.search(r"~(?![01])", part) is None, f"invalid JSON Pointer {path!r}")
            key = part.replace("~1", "/").replace("~0", "~")
            if isinstance(value, list) and re.fullmatch(r"0|[1-9][0-9]*", key):
                value = value[int(key)]
            elif isinstance(value, dict):
                value = value[key]
            else:
                raise KeyError(key)
    except (KeyError, IndexError) as exc:
        raise ValueError(f"evidence field unavailable: {path}") from exc
    return value


def exact_field(document, path, expected):
    seen = lookup(document, path)
    require(type(expected) in (str, int, bool, type(None)), f"{path}: expected a scalar assertion")
    require(type(seen) is type(expected) and seen == expected,
            f"evidence field does not match the index: {path}")
    return seen


def extract_fact(document, selector):
    if isinstance(selector, str):
        return lookup(document, selector)
    spec = mapping(selector, "field selector")
    path = spec.get("path")
    line = token(spec.get("line"), "build record key", re.compile(r"[A-Za-z][A-Za-z0-9 ]*"))
    record = token(lookup(document, path), path)
    matches = re.findall(rf"^{re.escape(line)}[=:][ \t]*(\S[^\r\n]*)$", record, re.MULTILINE)
    require(len(matches) == 1, f"expected exactly one {line!r} in {path}")
    return matches[0].strip()


def result_status(kind, value):
    if kind == "exit_code":
        require(type(value) is int, "exit_code requires an integer result")
        return "passed" if value == 0 else "failed"
    outcomes = {
        "status": {**{state: state for state in RESULTS}, "error": "failed"},
        "ci_conclusion": {"success": "passed", "failure": "failed", "cancelled": "blocked",
                          "skipped": "not_run", "timed_out": "failed"},
        "package_audit": {"PASS": "passed", "FAIL": "failed"},
        "historical_manual_install": {"NOT_RUN; checklist delivered, not performed": "not_run"},
    }
    choice(kind, outcomes, "result kind")
    return outcomes[kind][choice(value, outcomes[kind], "recorded result")]


def real_provider_execution(record, document, identifier):
    """Read a single execution receipt using fixed fields in the pinned source."""
    document = mapping(document, "real Provider evidence")
    require(type(document.get("schema_version")) is int and document["schema_version"] == 1
            and document.get("kind") == "real_provider_execution",
            f"{identifier}: expected a raw real_provider_execution receipt")
    require(not record.keys() & {"result", "fields", "context", "corroboration"},
            f"{identifier}: the index cannot override real Provider receipt fields")
    execution = mapping(document.get("account_execution"), "account_execution")
    require(execution.get("mode") == "real_account" and execution.get("provider_invoked") is True,
            f"{identifier}: evidence must record an actual real-account Provider invocation")
    for field in ("authenticated", "task_completed"):
        require(type(execution.get(field)) is bool, f"{identifier}: {field} must be a boolean")
    status = choice(document.get("status"), ("passed", "failed"), f"{identifier}.status")
    if status == "passed":
        require(execution["authenticated"] and execution["task_completed"],
                f"{identifier}: pass requires successful authentication and task completion")
    build = mapping(document.get("build"), "real Provider build")
    provider = mapping(document.get("provider"), "real Provider identity")
    facts = {key: token(build.get(key), f"build.{key}") for key in BUILD_KEYS}
    facts.update({key: token(provider.get(key), f"provider.{key}")
                  for key in ("provider_id", *PROVIDER_KEYS)})
    if "recorded_at" in document:
        facts["recorded_at"] = token(document["recorded_at"], "recorded_at")
    return status, facts, {
        "provider_run_reference": token(execution.get("run_ref"), "account_execution.run_ref"),
        "account_execution": {key: execution[key] for key in
                              ("mode", "provider_invoked", "authenticated", "task_completed")},
    }


def corroborate_browser(spec, report, report_sha256):
    spec = mapping(spec, "browser corroboration")
    path = evidence_path(spec.get("path"))
    expected = token(spec.get("sha256"), path, DIGEST)
    document, metadata = read_document(path)
    require(metadata["sha256"] == expected, f"{path}: SHA-256 mismatch")
    case = choice(lookup(report, "/case"), ("normal", "command-failure"), "browser case")
    link = f"/{case}/report_sha256"
    exact_field(document, link, report_sha256)
    count_pointer = f"/{case}/actual_schema19_snapshot_count"
    snapshots = lookup(document, count_pointer)
    require(type(snapshots) is int and snapshots > 0,
            f"{path}: expected a positive schema 19 snapshot count")
    return {**metadata, "report_sha256_pointer": link,
            "schema19_snapshot_count_pointer": count_pointer, "schema19_snapshot_count": snapshots}


def project_record(record, document, source, provider_ids, selected_source):
    record = mapping(record, "observation")
    identifier = token(record.get("id"), "observation ID")
    dimension = choice(record.get("dimension"), DIMENSIONS, f"{identifier}.dimension")
    origin = choice(record.get("origin"), ("local", "hosted_ci", "unknown"), f"{identifier}.origin")
    mode = choice(record.get("provider_execution"), ("none", "generated", "real_account"),
                  f"{identifier}.provider_execution")
    require(dimension != "provider_real" or mode == "real_account",
            f"{identifier}: provider_real requires real_account execution")
    require(mode != "real_account" or dimension == "provider_real",
            f"{identifier}: real_account requires a provider_real observation")
    require(dimension != "hosted_ci" or origin == "hosted_ci",
            f"{identifier}: hosted_ci dimension requires hosted_ci origin")

    for path, value in mapping(record.get("expect", {}), "expect").items():
        exact_field(document, path, value)
    evidence = dict(source)
    if dimension == "provider_real":
        status, facts, receipt = real_provider_execution(record, document, identifier)
        # A result, identity and invocation from this one receipt remain bound.
        labels = {}
        evidence.update(result_pointer="/status", observed_result=status, **receipt)
    else:
        result = mapping(record.get("result"), "result")
        require("value" in result, f"{identifier}: missing result value")
        result_path = result.get("path")
        observed = exact_field(document, result_path, result["value"])
        status = result_status(result.get("kind"), observed)
        evidence.update(result_pointer=result_path, observed_result=observed)
        fields = mapping(record.get("fields"), "fields")
        labels = mapping(record.get("context", {}), "context")
        require(fields.keys() <= FACT_KEYS and labels.keys() <= CONTEXT_KEYS,
                f"{identifier}: unsupported evidence field")
        require(not fields.keys() & labels.keys(), f"{identifier}: field also supplied as context")
        facts = {key: extract_fact(document, selector) for key, selector in fields.items()}
        if "corroboration" in record:
            require(dimension == "browser", f"{identifier}: expected a browser observation")
            evidence["corroboration"] = corroborate_browser(
                record["corroboration"], document, source["sha256"],
            )
    for key in labels:
        token(labels[key], f"{identifier}.{key}")
    for key, value in facts.items():
        if value is None:
            continue
        if key in {"artifact_id", "ci_run", "ci_job", "test_count"}:
            require(type(value) is int and value > 0, f"{identifier}.{key}: invalid counter or ID")
        else:
            token(value, f"{identifier}.{key}")
        if key.endswith("_commit") or key == "tree":
            token(value, f"{identifier}.{key}", COMMIT)
        if key.endswith("_sha256"):
            token(value, f"{identifier}.{key}", DIGEST)

    combined = {**labels, **facts}
    provider = combined.get("provider_id")
    require(provider is None or provider in provider_ids, f"{identifier}: unknown Provider ID")
    checkout = facts.get("source_commit")
    historical = combined.get("os") == "macos" and combined.get("architecture") == "x86_64"
    if historical:
        relevance = "historical_intel"
    elif checkout is None:
        relevance = "unknown_source"
    else:
        relevance = "selected_source" if checkout == selected_source else "other_source"

    expected_fields = {*BUILD_KEYS, "recorded_at"}
    if provider or dimension == "provider_real":
        expected_fields.update(("provider_id", *PROVIDER_KEYS))
    return {
        "id": identifier, "dimension": dimension, "origin": origin, "provider_execution": mode,
        "scope": token(record.get("scope"), f"{identifier}.scope"),
        "status": status, "relevance": relevance, "facts": facts, "context": labels,
        "unrecorded_fields": sorted(key for key in expected_fields if combined.get(key) is None),
        "evidence": evidence,
    }


def load_observations(index, provider_ids, selected_source):
    require(type(index.get("schema_version")) is int and index["schema_version"] == 1,
            "unsupported observation index schema_version")
    rows, inputs, identifiers = [], [], set()
    for source in array(index.get("sources"), "sources"):
        source = mapping(source, "source")
        path = evidence_path(source.get("path"))
        expected = token(source.get("sha256"), path, DIGEST)
        entries = array(source.get("observations"), f"{path}.observations")
        try:
            document, metadata = read_document(path)
        except FileNotFoundError:
            inputs.append({"path": path, "expected_sha256": expected, "state": "missing"})
            continue
        require(metadata["sha256"] == expected, f"{path}: SHA-256 mismatch")
        inputs.append({**metadata, "state": "pinned"})
        for entry in entries:
            try:
                row = project_record(entry, document, metadata, provider_ids, selected_source)
            except ValueError as exc:
                raise ValueError(f"{path}: {exc}") from exc
            require(row["id"] not in identifiers, f"duplicate observation ID {row['id']!r}")
            identifiers.add(row["id"])
            rows.append(row)
    return sorted(rows, key=lambda row: row["id"]), inputs


def candidate_for(path, formal_ids):
    if path is None:
        return None, None
    document, source = read_document(evidence_path(path))
    value = mapping(document, "candidate")
    for key in BUILD_KEYS:
        pattern = {"source_commit": COMMIT, "artifact_sha256": DIGEST}.get(key)
        token(value.get(key), f"candidate.{key}", pattern)
    require(value["os"] == "macos" and value["architecture"] == "arm64",
            "minimum-system candidate must be macOS arm64")
    providers = mapping(value.get("providers"), "candidate.providers")
    require(providers.keys() == formal_ids, "candidate must pin every formal Provider")
    for provider, version in providers.items():
        for key in PROVIDER_KEYS:
            token(mapping(version, provider).get(key), f"candidate.providers.{provider}.{key}")
    # Only report the documented fields even if the operator's file has extras.
    return {
        **{key: value[key] for key in BUILD_KEYS},
        "providers": {provider: {key: version[key] for key in PROVIDER_KEYS}
                      for provider, version in providers.items()},
    }, source


def candidate_match(row, candidate):
    if candidate is None:
        return "not_selected"
    facts = row["facts"]
    if any(facts.get(key) is not None and facts[key] != candidate[key] for key in BUILD_KEYS):
        return "different_build"
    if any(facts.get(key) is None for key in BUILD_KEYS):
        return "unrecorded_build_identity"
    if row["dimension"] == "provider_real":
        version = candidate["providers"].get(facts.get("provider_id"))
        if version is None or any(facts.get(key) != version[key] for key in PROVIDER_KEYS):
            return "different_or_unrecorded_provider_version"
    return "exact"


def assess_minimum(rows, candidate, formal_ids, evidence_available):
    exact = [row for row in rows if row["candidate_match"] == "exact"]

    def gate(check, provider=None):
        selected = [row for row in exact if row["dimension"] == check]
        if provider:
            selected = [row for row in selected if row["facts"].get("provider_id") == provider]
        statuses = {row["status"] for row in selected}
        return {
            "dimension": check, "provider_id": provider,
            "status": next((value for value in RESULTS if value in statuses), "unknown"),
            "observations": [row["id"] for row in selected],
        }

    gates = [gate(check) for check in PLATFORM_GATES]
    gates.extend(gate("provider_real", provider) for provider in sorted(formal_ids))
    if candidate is None:
        assessment = "candidate_not_selected"
    elif evidence_available and all(item["status"] == "passed" for item in gates):
        assessment = "ready_for_release_review"
    else:
        assessment = "evidence_incomplete"
    return {"candidate": candidate, "assessment": assessment, "confirmed_minimum": None,
            "required_checks": gates}


def build_report(index_path, candidate_path, explicit_source):
    spec, spec_input = read_document(SPEC)
    config, config_input = read_document(TAURI)
    index, index_input = read_document(evidence_path(index_path))
    targets = array(mapping(spec, "provider spec").get("providers"), "providers")
    ids = [token(mapping(target, "provider").get("id"), "provider.id") for target in targets]
    require(len(ids) == len(set(ids)), "duplicate Provider ID in the specification")
    formal = {target["id"] for target in targets if target.get("support_scope") == "formal_target"}
    require(formal, "the specification has no formal Providers")
    candidate, candidate_input = candidate_for(candidate_path, formal)
    if candidate:
        selected_source = candidate["source_commit"]
    elif explicit_source:
        selected_source = token(explicit_source, "--source", COMMIT)
    else:
        selected_source = subprocess.check_output(
            ["git", "rev-parse", "--verify", "HEAD"], cwd=ROOT, text=True,
        ).strip()
        token(selected_source, "Git HEAD", COMMIT)

    observations, source_inputs = load_observations(mapping(index, "index"), set(ids), selected_source)
    for row in observations:
        row["candidate_match"] = candidate_match(row, candidate)
    minimum = assess_minimum(observations, candidate, formal,
                             all(source["state"] == "pinned" for source in source_inputs))
    provider_gates = {row["provider_id"]: row["status"] for row in minimum["required_checks"]
                      if row["provider_id"] is not None}
    return {
        "schema_version": 1,
        "selected_source": selected_source,
        "inputs": {"provider_spec": spec_input, "desktop_config": config_input,
                   "observation_index": index_input, "candidate": candidate_input,
                   "evidence": source_inputs},
        "design_version": token(spec.get("design_version"), "design_version"),
        "platform_targets": {
            "desktop_macos": ["arm64"], "historical_desktop_macos": ["x86_64"],
            "remote_hosts": ["user-configured macOS", "user-configured Linux"],
            "declared_app_version": token(config.get("version"), "Tauri version"),
            "declared_bundle_minimum": token(config["bundle"]["macOS"].get("minimumSystemVersion"),
                                             "Tauri minimumSystemVersion"),
        },
        "provider_targets": [
            {"id": item["id"], "route": item.get("route"), "scope": item.get("support_scope"),
             "pinned_provider_version": item.get("pinned_version"),
             "config_modes": item.get("config_modes"),
             "named_connections": item.get("named_connections"),
             "documentation_refs": item.get("documentation_refs"),
             "candidate_real_account_status": provider_gates.get(item["id"], "unknown"),
             "observations": [row["id"] for row in observations
                              if (row["facts"].get("provider_id") or
                                  row["context"].get("provider_id")) == item["id"]]}
            for item in targets
        ],
        "observations": observations,
        "minimum_system_evidence": minimum,
    }


def cell(value):
    if value is None:
        return "未登记"
    return re.sub(r"([\\<>*_|`\[\]~#&!])", r"\\\1", str(value)).replace("\r", r"\r").replace("\n", r"\n")


def markdown(report):
    yield "# AgentBaton 兼容性证据矩阵"
    yield ""
    yield f"所选源码 {report['selected_source']} · Design {cell(report['design_version'])}"
    platform = report["platform_targets"]
    yield (f"桌面范围为 macOS Apple Silicon（ARM64）。Intel 为历史范围。"
           f"应用版本 {cell(platform['declared_app_version'])}，"
           f"Tauri 打包配置最低版本 {cell(platform['declared_bundle_minimum'])}。"
           "最低支持系统待定。远程目标为用户配置的 macOS 与 Linux。")
    yield ""
    yield "## Provider 目标"
    yield ""
    yield "| Provider | 声明范围 | 路线 | 规格锁定版本 | 候选真实账户 | 证据记录 |"
    yield "| --- | --- | --- | --- | --- | --- |"
    for row in report["provider_targets"]:
        yield "| " + " | ".join(map(cell, (row["id"], row["scope"], row["route"],
                                             row["pinned_provider_version"],
                                             row["candidate_real_account_status"],
                                             ", ".join(row["observations"]) or None))) + " |"
    yield ""
    yield "## 最低系统验证"
    yield ""
    minimum = report["minimum_system_evidence"]
    yield f"候选评估 {cell(minimum['assessment'])}"
    if candidate := minimum["candidate"]:
        yield ""
        yield (f"候选版本 {cell(candidate['product_version'])} · macOS {cell(candidate['os_version'])} "
               f"({cell(candidate['os_build'])}) · 包 SHA-256 {candidate['artifact_sha256']}")
    yield ""
    yield "| 必需维度 | Provider | 结果 | 对应证据 |"
    yield "| --- | --- | --- | --- |"
    for gate in minimum["required_checks"]:
        yield "| " + " | ".join(map(cell, (gate["dimension"], gate["provider_id"], gate["status"],
                                             ", ".join(gate["observations"]) or None))) + " |"
    yield ""
    yield "## 已登记证据"
    for row in report["observations"]:
        yield ""
        yield f"### {cell(row['id'])}"
        yield ""
        yield " · ".join(cell(row[key]) for key in
                         ("dimension", "origin", "provider_execution", "status", "relevance"))
        if row["candidate_match"] != "not_selected":
            yield f"候选匹配 {cell(row['candidate_match'])}"
        yield ""
        yield cell(row["scope"])
        yield ""
        for label, values in (("固定字段", row["facts"]), ("已审上下文", row["context"])):
            for key, value in sorted(values.items()):
                yield f"- {label} · {cell(key)} {cell(value)}"
        evidence = row["evidence"]
        link = quote(evidence["path"], safe="/")
        yield f"- 来源 [{cell(evidence['path'])}]({link}) · {cell(evidence['result_pointer'])}"
        yield f"- SHA-256 {evidence['sha256']}"
        if corroboration := evidence.get("corroboration"):
            link = quote(corroboration["path"], safe="/")
            yield (f"- 核验 [schema 19 快照]({link}) · {corroboration['schema19_snapshot_count']} 条 "
                   f"· SHA-256 {corroboration['sha256']}")
        if run_reference := evidence.get("provider_run_reference"):
            yield f"- 真实账户运行证据引用 {cell(run_reference)}"
        if row["unrecorded_fields"]:
            yield f"- 未登记字段 {cell(', '.join(row['unrecorded_fields']))}"
    yield ""
    yield "## 输入"
    yield ""
    for key, source in report["inputs"].items():
        if key != "evidence" and source is not None:
            yield f"- {cell(key)} · {cell(source['path'])} · SHA-256 {source['sha256']}"
    for source in report["inputs"]["evidence"]:
        digest = source.get("sha256", source.get("expected_sha256"))
        yield f"- {cell(source['state'])} · {cell(source['path'])} · SHA-256 {digest}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=("json", "markdown"), default="markdown")
    parser.add_argument("--index", default=INDEX, help="repository-relative observation index JSON")
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--source", help="full source commit for comparison (default: checkout HEAD)")
    selection.add_argument("--candidate", help="repository-relative minimum-system candidate JSON")
    args = parser.parse_args()
    try:
        report = build_report(args.index, args.candidate, args.source)
        if args.format == "json":
            output = json.dumps(report, indent=2, ensure_ascii=False, sort_keys=True, allow_nan=False)
        else:
            output = "\n".join(markdown(report))
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as exc:
        parser.error(str(exc))
    print(output)


if __name__ == "__main__":
    main()
