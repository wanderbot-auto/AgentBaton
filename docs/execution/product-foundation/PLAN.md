# Full product execution plan · AB-START-001

Base: e158ade1a99e063ddce703ce4c0748febe449b43. Feature branch: codex/product-foundation. OWNER-001 acknowledged. Goal tool unavailable; native subagents available and used. Cloud task URL/ID unavailable through this entry.

The completion contract remains codex/GOAL.md and all 130 requirements. No package is considered accepted from foundation work alone.

Dependency order:
1. WP01 domain/error/state contracts plus WP02 transactional storage, authenticated daemon, shared CLI; in parallel WP03 protocol contract implementation and WP11 task UI against the fixed IPC contract.
2. Integrate live task creation/list/cancel/event replay through desktop IPC; WP13 product CI, independent review and PR. Preserve three Environment checks. Every check targets exact candidate SHA.
3. WP05 project/workspace registry, isolation, leases, safe artifact access and adoption; WP03/04 real adapters and account/config bindings (fixture verification separate from gated live calls).
4. WP06/07 capability snapshots, configuration transactions, drift/rollback; WP08 durable dispatch/outbox, bounded orchestration, shared budgets, cancellation and unknown-outcome reconciliation.
5. WP09 validation/review/acceptance/workflows; WP10 sessions and revocable background lifecycle; extend WP11 UI across these real APIs.
6. WP12 security/privacy/retention/fault/load; WP14 user-host registration, remote execution/reconciliation and custom ACP. Build WP13 dual-architecture development artifacts and installation/upgrade/desktop acceptance evidence throughout.

Each work unit: implement → meaningful positive/negative tests → independent review → fixes → PR/actual CI → guarded integration → update evidence → next ready unit. Main integration, requirements-progress, shared locks and migrations have a single writer. Native agents may not recursively delegate or expand authority.

No final license, production release, paid resources, personal Provider identities or real model tests are authorized. Those block only applicable verification. No fabricated platform/Provider outcomes.
