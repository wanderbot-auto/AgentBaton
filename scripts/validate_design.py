#!/usr/bin/env python3
"""Validate the AgentBaton design package only; this does not test the product."""
from __future__ import annotations
import copy
import json
import re
from pathlib import Path
from datetime import datetime, timezone
try:
    from jsonschema import Draft202012Validator, FormatChecker
except ImportError as exc:
    raise SystemExit("Install jsonschema in your Python environment to validate this document package.") from exc

ROOT = Path(__file__).resolve().parents[1]
def read_json(path: str):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))
def check(condition: bool, message: str):
    if not condition:
        raise AssertionError(message)

reqs = read_json("specs/requirements.json")["requirements"]
packages = read_json("specs/work-packages.json")["packages"]
decisions = read_json("specs/decisions.json")["decisions"]
req_ids = [r["id"] for r in reqs]
package_map = {p["id"]: p for p in packages}
decision_ids = {d["id"] for d in decisions}
check(len(req_ids) == len(set(req_ids)), "Duplicate requirement IDs")
check(len(package_map) == len(packages), "Duplicate work-package IDs")
for r in reqs:
    check(r["status"] == "specified", f"False implementation claim: {r['id']}")
    check((ROOT / r["document"]).is_file(), f"Missing design document: {r['id']}")
    check(r["work_package"] in package_map, f"Unknown work package: {r['id']}")
    check(all(d in decision_ids for d in r["decisions"]), f"Unknown decision: {r['id']}")
    check(all(r["acceptance"].get(k) for k in ("given", "when", "then")), f"Missing acceptance: {r['id']}")
    check(not r["implementation_refs"] and not r["verification_refs"], f"Unexpected product evidence: {r['id']}")
for p in packages:
    expected = {r["id"] for r in reqs if r["work_package"] == p["id"]}
    check(expected == set(p["requirement_ids"]), f"Traceability mismatch: {p['id']}")
    check(bool(expected), f"No requirements mapped to {p['id']}")
    check(all(d in package_map for d in p["dependencies"]), f"Unknown dependency: {p['id']}")
    check(all(d in package_map for d in p["release_dependencies"]), f"Unknown release gate: {p['id']}")
visited, visiting = set(), set()
def visit(key):
    check(key not in visiting, f"Dependency cycle at {key}")
    if key in visited: return
    visiting.add(key)
    for dep in package_map[key]["dependencies"]: visit(dep)
    visiting.remove(key)
    visited.add(key)
for key in package_map: visit(key)
for d in decisions:
    check(d["status"] in ("approved", "deferred_optional"), f"Unresolved product direction: {d['id']}")
    check(bool(d.get("decision")) and bool(d.get("approval_source")), f"Missing decision provenance: {d['id']}")
    check(d.get("approved_at") is None, f"Do not invent exact decision timestamps: {d['id']}")
q = read_json("specs/questionnaire.json")["questions"]
check(sorted(x["number"] for x in q) == list(range(1, 28)), "Question coverage is not exactly 1..27")
check(all(x["decision_id"] in decision_ids for x in q), "Question references missing decision")
retired = read_json("specs/retired-requirements.json")["requirements"]
check(not (set(req_ids) & {r["id"] for r in retired}), "Retired requirement reused")
check(all(r["status"] == "retired" for r in retired), "Retired scope incorrectly active")
check({r["id"] for r in retired} == {"AB-PRO-008", "AB-SES-001", "AB-SES-004", "AB-SES-005"}, "Unexpected retirement")
for d in decisions:
    if d["status"] == "approved":
        check(any(d["id"] in r["decisions"] for r in reqs), f"Approved decision lacks requirement coverage: {d['id']}")
for p in packages:
    check(p["authorization"] == "not_granted", "Product approval is not live execution permission")

validators = {}
for schema_path in sorted((ROOT / "specs/schemas").glob("*.json")):
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    name = schema_path.name.replace(".v1.json", "")
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    validator.validate(read_json(f"specs/examples/{name}.json"))
    validators[name] = validator

bad_cases = []
x = read_json("specs/examples/task-request.json"); x["unexpected"] = True
bad_cases.append(("task-request", x))
x = read_json("specs/examples/task-request.json"); x["policy"]["output_access"] = "read_only"
bad_cases.append(("task-request", x))
x = read_json("specs/examples/task-result.json"); x["acceptance_status"] = "accepted"
bad_cases.append(("task-result", x))
x = read_json("specs/examples/event-envelope.json"); x["sequence"] = 0
bad_cases.append(("event-envelope", x))
x = read_json("specs/examples/analysis-output.json"); del x["sample_description"]
bad_cases.append(("analysis-output", x))
x = read_json("specs/examples/connection-profile.json"); x["silent_identity_fallback"] = True
bad_cases.append(("connection-profile", x))
x = read_json("specs/examples/connection-profile.json"); x["environment"]["writeback_external"] = True
bad_cases.append(("connection-profile", x))
x = read_json("specs/examples/project-policy.json"); x["production_release"] = "automatic"
bad_cases.append(("project-policy", x))
x = read_json("specs/examples/project-policy.json"); x["retention"]["delete_source_files"] = True
bad_cases.append(("project-policy", x))
x = read_json("specs/examples/execution-envelope.json"); x["active_attempt_migration"] = True
bad_cases.append(("execution-envelope", x))
for name, data in bad_cases:
    check(bool(list(validators[name].iter_errors(data))), f"Invalid example accepted: {name}")

link_count = 0
for md in ROOT.rglob("*.md"):
    text = md.read_text(encoding="utf-8")
    for target in re.findall(r"\[[^\]]+\]\(([^)]+)\)", text):
        if target.startswith(("https://", "http://", "#", "mailto:")): continue
        path = target.split("#", 1)[0]
        check((md.parent / path).resolve().exists(), f"Broken link in {md.relative_to(ROOT)}: {target}")
        link_count += 1
source_text = (ROOT / "docs/13-references.md").read_text(encoding="utf-8")
source_ids = set(re.findall(r"\bS\d{2}\b", source_text))
for md in ROOT.rglob("*.md"):
    for sid in re.findall(r"\[(S\d{2})\]", md.read_text(encoding="utf-8")):
        check(sid in source_ids, f"Unknown source {sid} in {md}")

# Check known superseded directives in active implementation instructions, not historical retired records.
for path in ["AGENTS.md", "codex/MASTER_PROMPT.md", "docs/14-api-contracts.md"]:
    text = (ROOT / path).read_text(encoding="utf-8")
    for forbidden in ["Preserve native terminal workflows.", "Keep native config as the source of truth unless", "| sessions.observe |", "保留Ghostty原生CLI；外部会话观察与受管控制明确区分。"]:
        check(forbidden not in text, f"Superseded instruction remains: {path}")
report = {
    "scope": "design_package_validation_only",
    "status": "passed",
    "recorded_at": datetime.now(timezone.utc).isoformat(),
    "requirements": len(reqs),
    "work_packages": len(packages),
    "approved_decisions": sum(d["status"] == "approved" for d in decisions),
    "deferred_optional_decisions": sum(d["status"] == "deferred_optional" for d in decisions),
    "answered_questions": len(q),
    "retired_requirements": len(retired),
    "schemas_validated": len(validators),
    "positive_examples_validated": len(validators),
    "negative_examples_rejected": len(bad_cases),
    "local_links_checked": link_count,
    "product_tests_executed": False,
    "real_agent_tests_executed": False,
    "macos_app_built": False,
    "limitations": [
        "Schema checks do not establish semantic authorization, path safety or runtime enforcement.",
        "This package contains design documents, not an implemented AgentBaton product."
    ]
}
(ROOT / "DESIGN_VALIDATION.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
