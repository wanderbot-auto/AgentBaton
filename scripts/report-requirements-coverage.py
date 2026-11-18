#!/usr/bin/env python3
"""Report recorded AgentBaton requirement coverage from local, read-only inputs."""

from __future__ import annotations

import argparse
from collections import Counter, deque
from contextlib import ExitStack
import errno
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SPEC_PATH = Path("specs/requirements.json")
LEDGER_PATH = Path("docs/execution/requirements-progress.json")
DIMENSIONS = {
    "unit": "单元",
    "integration": "集成",
    "provider": "真实 Provider",
    "native_arm64": "原生 Apple Silicon",
    "ci": "CI",
    "native_x86_64": "Intel 原生历史记录",
}
RECORDED_RESULTS = {
    "passed", "failed", "blocked", "not_run", "not_applicable", "unknown",
    "unsupported", "untested",
}
RECORD_REFERENCES = {
    "actual_failure", "browser_evidence", "delivery", "evidence", "independent_review",
    "latest_evidence", "local_evidence", "post_parent_composed", "review",
}
COVERAGE_BASIS = (
    "部分实现按台账 coverage=partial 计数。整项验收按台账 status=accepted 计数。"
    "两项均是需求台账统计。验证结果与适用性取自明确记录，引用检查只确认本地路径是否可访问。"
)
MACOS_SCOPE = "现行 macOS 验收范围为 Apple Silicon（ARM64）。Intel 记录仅供历史追溯。"


def unique_keys(pairs: list[tuple[str, Any]]) -> dict:
    obj = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError(f"duplicate JSON key {key!r}")
        obj[key] = value
    return obj


def finite_number(token: str) -> float:
    number = float(token)
    if not math.isfinite(number):
        raise ValueError(f"non-finite JSON number {token!r}")
    return number


def load_input(path: Path) -> tuple[dict, dict]:
    try:
        raw = path.read_bytes()
        obj = json.loads(raw, object_pairs_hook=unique_keys,
                         parse_constant=finite_number, parse_float=finite_number)
    except (OSError, ValueError, RecursionError) as exc:
        raise ValueError(f"{path}: {exc}") from exc
    if not isinstance(obj, dict):
        raise ValueError(f"{path}: expected a JSON object")
    version = text_field(obj, "design_version", str(path))
    return obj, {"path": str(path), "design_version": version,
                 "sha256": hashlib.sha256(raw).hexdigest()}


def text_field(obj: dict, key: str, source: str) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{source}: {key} must be a nonempty string")
    return value


def requirements_by_id(data: dict, source: str) -> dict[str, dict]:
    rows = data.get("requirements")
    if not isinstance(rows, list) or not rows:
        raise ValueError(f"{source}: requirements must be a nonempty array")
    by_id = {}
    for index, row in enumerate(rows):
        at = f"{source}.requirements[{index}]"
        if not isinstance(row, dict):
            raise ValueError(f"{at}: expected an object")
        identifier = text_field(row, "id", at)
        if identifier in by_id:
            raise ValueError(f"{source}: duplicate requirement ID {identifier}")
        text_field(row, "work_package", at)
        text_field(row, "status", at)
        by_id[identifier] = row
    return by_id


def check_reference(root: Path, reference: str) -> dict:
    """Inspect repository entries by descriptor without opening referenced files."""
    file_reference, fragment, _ = reference.partition("#")
    path, marker, selector = file_reference.partition("::")
    if not (marker and path.endswith(".rs") and selector):
        path = file_reference
    has_selector = bool(fragment or path != file_reference)
    parts = Path(path).parts
    if (not parts or path.startswith("/") or ".." in parts or "\\" in path or ":" in path
            or any(ord(char) < 32 or ord(char) == 127 for char in reference)):
        return {"status": "unsafe", "reason": "expected a repository-relative path"}

    result: dict[str, Any] = {"path": path}
    if has_selector:
        result["selector_checked"] = False
    try:
        result.update(inspect_path(root, parts))
    except (FileNotFoundError, NotADirectoryError):
        result["status"] = "missing"
    except (OSError, ValueError) as exc:
        result.update(status="unavailable", reason=getattr(exc, "strerror", None) or str(exc))
    return result


def inspect_path(root: Path, parts: tuple[str, ...]) -> dict:
    # Open directories with O_NOFOLLOW even after checking their metadata, so a
    # concurrent symlink replacement cannot redirect the next lookup.
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    with ExitStack() as opened:
        root_fd = os.open(root, flags)
        opened.callback(os.close, root_fd)
        directories = [root_fd]
        pending = deque(parts)
        followed = 0
        while pending:
            name = pending.popleft()
            if name == "..":
                if len(directories) == 1:
                    return {"status": "unsafe", "reason": "symlink leaves the reference root"}
                directories.pop()
                continue

            parent = directories[-1]
            mode = os.stat(name, dir_fd=parent, follow_symlinks=False).st_mode
            if stat.S_ISLNK(mode):
                followed += 1
                if followed > 40:
                    return {"status": "unsafe", "reason": "too many symlink hops"}
                target = Path(os.readlink(name, dir_fd=parent))
                if target.is_absolute():
                    if not target.is_relative_to(root):
                        return {"status": "unsafe", "reason": "symlink leaves the reference root"}
                    target = target.relative_to(root)
                    directories = [root_fd]
                pending.extendleft(reversed(target.parts))
            elif pending:
                try:
                    directory = os.open(name, flags, dir_fd=parent)
                except OSError as exc:
                    if exc.errno == errno.ELOOP:
                        return {"status": "unsafe", "reason": "reference changed during lookup"}
                    raise
                opened.callback(os.close, directory)
                directories.append(directory)
            elif not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
                return {"status": "unsafe", "reason": "reference is not a regular file or directory"}
    return {"status": "exists"}


def nested_references(value: Any, field: str):
    """Yield named reference fields from historical records, ignoring narrative values."""
    if isinstance(value, dict):
        for key, child in value.items():
            source = f"{field}.{key}"
            if key in RECORD_REFERENCES and isinstance(child, str):
                yield source, child
            elif key in RECORD_REFERENCES and isinstance(child, list):
                for index, reference in enumerate(child):
                    if isinstance(reference, str):
                        yield f"{source}[{index}]", reference
            else:
                yield from nested_references(child, source)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from nested_references(child, f"{field}[{index}]")


def references_for(row: dict, source: str, notes: dict):
    yield from nested_references(notes, source)
    for field in ("implementation_refs", "verification_refs", "evidence", "supporting_evidence"):
        refs = row.get(field, [])
        if not isinstance(refs, list) or any(not isinstance(ref, str) for ref in refs):
            raise ValueError(f"{source}.{row['id']}.{field}: expected an array of strings")
        for index, reference in enumerate(refs):
            yield f"{source}.{field}[{index}]", reference


def verification_for(spec: dict, ledger: dict) -> tuple[dict, dict]:
    identifier = spec["id"]
    results = ledger.get("verification", {})
    if not isinstance(results, dict) or any(not isinstance(value, str) for value in results.values()):
        raise ValueError(f"ledger.{identifier}.verification: expected an object of strings")
    applicability = {}
    for source, row in (("spec", spec), ("ledger", ledger)):
        supplied = row.get("verification_applicability", {})
        if not isinstance(supplied, dict) or any(
            not isinstance(value, str) or not value.strip() for value in supplied.values()
        ):
            raise ValueError(f"{source}.{identifier}.verification_applicability: expected an object "
                             "of nonempty strings")
        for key, value in supplied.items():
            if key in applicability and applicability[key] != value:
                raise ValueError(f"{identifier}: conflicting verification applicability for {key}")
            applicability[key] = value

    dimensions = {}
    for key in DIMENSIONS.keys() | applicability.keys():
        value = results.get(key)
        dimensions[key] = {
            "applicability": applicability.get(key),
            "status": value if value in RECORDED_RESULTS else None,
            "recorded_note": value if value not in RECORDED_RESULTS else None,
        }
    notes = {key: value for key, value in results.items() if key not in dimensions}
    return dimensions, notes


def describe_requirement(spec: dict, ledger: dict, evidence_root: Path) -> dict:
    identifier = spec["id"]
    title = text_field(spec, "title", f"spec.{identifier}")
    document = text_field(spec, "document", f"spec.{identifier}")
    acceptance = spec.get("acceptance")
    if not isinstance(acceptance, dict):
        raise ValueError(f"spec.{identifier}.acceptance: expected an object")
    for field in ("given", "when", "then"):
        text_field(acceptance, field, f"spec.{identifier}.acceptance")
    if "coverage" in ledger:
        text_field(ledger, "coverage", f"ledger.{identifier}")
    dimensions, verification_notes = verification_for(spec, ledger)
    notes = {key: value for key, value in ledger.items() if key not in {
        "id", "work_package", "status", "coverage", "verification", "verification_applicability",
        "implementation_refs", "verification_refs", "evidence", "supporting_evidence",
    }}
    references = []
    for source, reference in references_for(ledger, "ledger", notes):
        references.append({"field": source, "reference": reference,
                           **check_reference(evidence_root, reference)})
    for source, reference in [("spec.document", document), *references_for(spec, "spec", {})]:
        references.append({"field": source, "reference": reference,
                           **check_reference(ROOT, reference)})

    return {
        "id": identifier, "title": title, "work_package": spec["work_package"],
        "specification_status": spec["status"], "status": ledger["status"],
        "coverage": ledger.get("coverage"), "acceptance_criteria": acceptance,
        "partial_implementation": ledger.get("coverage") == "partial",
        "whole_requirement_accepted": ledger["status"] == "accepted",
        "verification": dimensions, "recorded_verification_notes": verification_notes,
        "references": references, "recorded_ledger_notes": notes,
    }


def counts(rows: list[dict]) -> dict:
    return {
        "requirements": len(rows),
        "by_status": dict(Counter(row["status"] for row in rows)),
        "partial_implementation": sum(row["partial_implementation"] for row in rows),
        "whole_requirement_accepted": sum(row["whole_requirement_accepted"] for row in rows),
    }


def build_report(ledger_path: Path, evidence_root: Path) -> dict:
    spec, spec_info = load_input(ROOT / SPEC_PATH)
    ledger, ledger_info = load_input(ledger_path)
    if spec_info["design_version"] != ledger_info["design_version"]:
        raise ValueError(f"{ledger_path}: design_version mismatch with {SPEC_PATH}")
    spec_rows = requirements_by_id(spec, "spec")
    ledger_rows = requirements_by_id(ledger, "ledger")
    missing = sorted(spec_rows.keys() - ledger_rows.keys())
    extra = sorted(ledger_rows.keys() - spec_rows.keys())
    if missing or extra:
        raise ValueError("ledger IDs differ from the active design"
                         f" (missing: {', '.join(missing) or 'none'}"
                         f"; extra: {', '.join(extra) or 'none'})")

    packages: dict[str, list[dict]] = {}
    rows = []
    for identifier in sorted(spec_rows):
        spec_row, ledger_row = spec_rows[identifier], ledger_rows[identifier]
        package = spec_row["work_package"]
        if package != ledger_row["work_package"]:
            raise ValueError(f"{identifier}: work_package mismatch "
                             f"(spec {package}, ledger {ledger_row['work_package']})")
        row = describe_requirement(spec_row, ledger_row, evidence_root)
        rows.append(row)
        packages.setdefault(package, []).append(row)

    reference_counts = Counter(ref["status"] for row in rows for ref in row["references"])
    summary = counts(rows)
    summary["by_work_package"] = {package: counts(packages[package]) for package in sorted(packages)}
    summary["references"] = {state: reference_counts[state]
                             for state in ("exists", "missing", "unsafe", "unavailable")}
    return {
        "schema_version": 1,
        "inputs": {
            "spec": {**spec_info, "reference_root": str(ROOT)},
            "ledger": {**ledger_info, "approval_id": ledger.get("approval_id"),
                       "reference_root": str(evidence_root)},
        },
        "coverage_basis": COVERAGE_BASIS,
        "macos_scope": MACOS_SCOPE,
        "summary": summary,
        "requirements": rows,
    }


def markdown_text(value: str) -> str:
    """Escape input text used in Markdown tables, headings and list items."""
    escaped = re.sub(r"([\\<>*_|`\[\]~#&!])", r"\\\1", value)
    return escaped.replace("\r", r"\r").replace("\n", r"\n")


def markdown_report(report: dict):
    yield "# AgentBaton 需求覆盖报告"
    yield ""
    yield report["coverage_basis"]
    yield ""
    yield report["macos_scope"]
    yield ""
    yield "## 输入"
    yield ""
    for label, info in report["inputs"].items():
        yield f"- **{label}** · 版本 {markdown_text(info['design_version'])}"
        yield f"  - 文件 {markdown_text(info['path'])}"
        yield f"  - SHA-256 {info['sha256']}"
        yield f"  - 引用检查根目录 {markdown_text(info['reference_root'])}"
        if info.get("approval_id"):
            yield f"  - 授权记录 {markdown_text(str(info['approval_id']))}"

    summary = report["summary"]
    yield ""
    yield "## 统计"
    yield ""
    yield "| 工作包 | 活动需求 | 部分实现 | 整项验收 | 台账状态 |"
    yield "| --- | ---: | ---: | ---: | --- |"
    for package, totals in [("全部", summary), *summary["by_work_package"].items()]:
        states = " · ".join(f"{markdown_text(key)} {value}"
                            for key, value in sorted(totals["by_status"].items()))
        yield (f"| {markdown_text(package)} | {totals['requirements']} | "
               f"{totals['partial_implementation']} | {totals['whole_requirement_accepted']} | {states} |")
    refs = summary["references"]
    yield ""
    yield (f"引用检查共 {sum(refs.values())} 条。存在 {refs['exists']}，缺失 {refs['missing']}，"
           f"不安全 {refs['unsafe']}，无法检查 {refs['unavailable']}。锚点和 Rust 符号只检查文件路径。")
    yield ""
    yield "## 需求明细"
    for row in report["requirements"]:
        yield ""
        yield f"### {markdown_text(row['id'])} · {markdown_text(row['title'])}"
        yield ""
        yield (f"{markdown_text(row['work_package'])} · {markdown_text(row['status'])} · "
               f"部分实现 {'是' if row['partial_implementation'] else '未登记'} · "
               f"整项验收 {'是' if row['whole_requirement_accepted'] else '未登记'}")
        yield ""
        yield markdown_text(row["acceptance_criteria"]["then"])
        yield ""
        yield "| 验证维度 | 适用性 | 台账状态 | 既有说明 |"
        yield "| --- | --- | --- | --- |"
        for key, value in sorted(row["verification"].items()):
            label = DIMENSIONS.get(key, key)
            applicability = value["applicability"]
            status = value["status"]
            note = value["recorded_note"]
            yield (f"| {markdown_text(label)} | {markdown_text(applicability or '未登记')} | "
                   f"{markdown_text(status or '未登记')} | {markdown_text(note or '—')} |")
        yield ""
        for ref in row["references"]:
            reason = f" · {markdown_text(ref['reason'])}" if "reason" in ref else ""
            yield (f"- {markdown_text(ref['field'])} · {markdown_text(ref['reference'])} · "
                   f"**{ref['status']}**{reason}")
        notes = {key: row[key] for key in ("recorded_verification_notes", "recorded_ledger_notes")
                 if row[key]}
        if notes:
            yield ""
            yield "<details><summary>台账中的其他验证记录、限制和历史说明</summary>"
            yield ""
            for line in json.dumps(notes, indent=2, ensure_ascii=False, sort_keys=True).split("\n"):
                yield f"    {line}"
            yield ""
            yield "</details>"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=("json", "markdown"), default="markdown")
    parser.add_argument("--ledger", type=Path, default=ROOT / LEDGER_PATH,
                        help=f"input ledger (default: {LEDGER_PATH} in the script's checkout)")
    parser.add_argument("--evidence-root", type=Path,
                        help="root for ledger references (default: the ledger's checkout when "
                             "using its standard path, otherwise the script's checkout)")
    args = parser.parse_args()
    try:
        ledger = args.ledger.resolve(strict=True)
        evidence_root = args.evidence_root
        if evidence_root is None:
            if ledger.parts[-len(LEDGER_PATH.parts):] == LEDGER_PATH.parts:
                evidence_root = ledger.parents[len(LEDGER_PATH.parts) - 1]
            else:
                evidence_root = ROOT
        evidence_root = evidence_root.resolve(strict=True)
        if not evidence_root.is_dir():
            raise ValueError(f"not a reference directory: {evidence_root}")
        report = build_report(ledger, evidence_root)
        if args.format == "json":
            output = json.dumps(report, indent=2, ensure_ascii=False, sort_keys=True, allow_nan=False)
        else:
            output = "\n".join(markdown_report(report))
    except (OSError, ValueError, RuntimeError) as exc:
        parser.error(str(exc))
    print(output)


if __name__ == "__main__":
    main()
