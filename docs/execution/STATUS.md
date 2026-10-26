# AgentBaton execution status

Authorization: AB-START-001, full Design v0.2; OWNER-001 ACK and in progress. Full goal remains codex/GOAL.md, all130 requirements retained without acceptance shortcuts.

Branch: `codex/product-foundation`; authorized base `e158ade1a99e063ddce703ce4c0748febe449b43` (PR#2 confirmed merged). No initial user modifications. Cloud task URL/ID unavailable through this entry. Native core/desktop/protocol/workspace/config workers and independent reviewer used; no Goal tool exposed.

Implemented, partial: persistent draft tasks, authenticated Unix daemon/CLI, narrow Tauri bridge and bilingual task UI; SQLite atomic journal and v1→v2 backup migration; immutable dispatch bindings, durable intent/restart reconciliation and root/project attempt budgets; ACP contract+bounded stdio transport; safe Git worktree/artifact and managed configuration transaction primitives. See requirements-progress.json for per-ID scope. Libraries for Provider/workspace/config are not yet connected through the complete policy/session/router/UI workflow.

Local verified:76 Rust tests (core18/config14/providers31/workspaces13), workspace fmt/Clippy, real daemon/CLI SIGKILL restart smoke;6 UI transport-mock tests, typecheck/frontend build;28 Python infrastructure +4 Node +2 Rust environment fixture tests. Native host Linux compilation cannot proceed without GTK/glib; it is not claimed passed. macOS product host/tests/build/runtime, real Provider compatibility and installation acceptance remain unrun.

Independent review: findings and source hashes in product-foundation/REVIEW.md. Corrections include stale sockets, bounded readiness, configured Git filter execution, cross-root configuration approval binding, snapshot truncation and stale ACP session/response reuse. No full package acceptance implied.

Repository protection: ruleset25893455 active with strict PR checks; existing three Environment contexts preserved. Additive Product CI and dual-architecture development DMG jobs implemented, actual PR/CI/artifacts pending. Do not merge until all applicable exact-candidate checks pass and review fixes verified.

External gates: no real Provider identities/model calls, extra paid resources, final license, formal signing/notarization or production release authorized. These block only corresponding validation/delivery.

Next: push reviewed candidate as PR, inspect exact-SHA CI and development artifacts, fix failures, guarded integration; continue persistent project registry/policy bindings and connect existing protocol/config/workspace primitives to the real session/task workflow. Full capability management, orchestration, validation/adoption, retention, remote Hosts and desktop lifecycle remain incomplete.
