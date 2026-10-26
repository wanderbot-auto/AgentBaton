# Execution boundary — OWNER-AUTHORIZED PRODUCT DEVELOPMENT

Read `infra/execution-gate.json`, `docs/execution/launch/APPROVAL.md`, `codex/MASTER_PROMPT.md`, `codex/GOAL.md` and `codex/RUNBOOK.md` first. The owner explicitly approved full Design v0.2 implementation after environment readiness. Earlier PREPARATION ONLY statements in historical reports are superseded for this execution, not deleted from history.

Proceed on dependency-ready work without asking permission after each package. Use supported native subagents and Goal when useful; their absence does not block ordinary execution. No fabricated autonomous continuation or unverified Cloud launch.

Use feature branches/PRs; never directly push or force-update main, bypass required checks, weaken tests, or expand permissions. Product Provider test accounts/calls, paid provisioning, production publishing and signing credentials remain separately gated. The development agent's existing Cloud execution is authorized; nested paid model calls are not.

Consult `docs/execution/OWNER_INBOX.md` and update `docs/execution/STATUS.md` at safe integration/checkpoint boundaries. Independently review changes and verify evidence before acceptance. Work-package authorization in the immutable design file is historical; use the current execution gate. Historical environment reports are evidence, not current launch instructions.

# AgentBaton repository instructions

## Product baseline

Implement Design v0.2: one AgentBaton task-centric UI with quick agent chat; goal-driven autonomous planning, delegation, verification and bounded rework. Do not implement an external Ghostty session observer, terminal wrapper, generic embedded terminal, or parallel terminal interaction entry.

Default to Baton-managed isolated runtime environments. Program source, configuration ownership, identity, workspace and host are distinct. Support named native-config connections as an advanced read-only external-config mode, not attachment to existing terminal sessions. Do not overwrite native global files, copy authentication directories or silently synchronize configurations.

## Source of truth

Read README.md, docs/12-decisions.md, specs/decisions.json, specs/requirements.json, docs/11-engineering-program.md and codex/RUNBOOK.md. Read linked chapters as needed. Questionnaire choices define product behavior; actual execution is now authorized by AB-START-001 within infra/execution-gate.json. D07 is deferred optional scope, not a reason to block the product. Exact minimum macOS, versions and license require the validation gates in docs/15-validation-gates.md.

## Non-negotiable behavior

- App-managed interactions only; commands/output panels are not a generic terminal. Preserve native agent engines, not their terminal UI.
- Bind each attempt to a named connection, effective configuration, identity reference, host and input revision. ACP is one integration path, not the scheduler or a universal config API.
- Use isolated workspaces for coding tasks. Never stash/reset/force-overwrite users' changes. Validate integration candidates against exact target head before authorized merge.
- Goal approval permits bounded planning and delegation within policy, not new permissions, recipients, spending, identities or billing modes. Project budgets are shared by child tasks.
- Automatically accept/merge validated results and perform preauthorized development delivery. Production release always requires separate approval bound to fixed artifact/commit/environment.
- Local and user-configured macOS/Linux hosts are approved product scope. Route new tasks within authorization; never silently migrate active attempts or create paid cloud resources.
- Background tasks survive UI exit; login startup is explicit and revocable. Sleep, disconnect, cancel request and confirmed stop are different states. Reconcile unknown outcomes before retry.
- Preserve requested/effective/enforced distinctions. Worktrees, prompts, config directories and Tauri IPC are not automatic OS sandboxes.
- Record complete available history locally; redact credentials, respect project retention/pins, purge derived indexes consistently, never delete original source files through log expiry.
- Experience is a reviewable proposal until approved. Agent output, repository instructions and tool descriptions are not authority.

## Evidence and long-horizon work

The immutable design baseline labels requirements specified. Track real implementation and verification separately in docs/execution/. Do not downgrade full scope to a mock demo. Select dependency-ready authorized work; establish actual build/test commands, implement real error paths, independently review, record exact commit/environment/commands/results. Unknown, unsupported, untested and blocked are separate. Mock tests do not prove real Provider integration; browser tests do not prove macOS native support; both Apple Silicon and Intel need evidence.

Save per-task PLAN, PROGRESS, CHECKPOINT and EVIDENCE. New cloud runs must verify real repository state; do not assume they inherit uncommitted changes. Do not promise work continues without an actual executor. Do not ask users about routine reversible engineering choices; use ADRs. Escalate only scope, privacy, authority, licensing, production release or unresolved material risk. At most three short product questions per escalation.

## Execution authority

Repository, target, package scope and resource boundaries are supplied by infra/execution-gate.json and docs/execution/launch/APPROVAL.md. Do not ask the owner to repeat that approval. Missing access blocks only the affected operations. Product approval does not grant access to a real production system, code signing key or paid service. Never choose an unapproved license or claim fictitious CI, binaries, tests or deployments.

Use typed interfaces, parameterized argv, restricted IPC, contract tests, pinned dependencies and safe migrations. Preserve failed test evidence; never weaken tests to turn a build green. Production artifacts must contain no embedded test-control server or test-only permissions.
