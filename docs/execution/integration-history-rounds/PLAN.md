# Integration receipt and attempt rounds

Baseline: `f1938820728bcae3ecf011d3ab710dab1e77e48c`.
Branch: `codex/integration-history-rounds`.
Worktree: `/workspace/.worktrees/integration-history-rounds`.
Requirements: AB-VAL-009, AB-PRJ-004, AB-PRJ-005; D19 complete retained history.

ACK OWNER-001 and OWNER-002 at the fixed baseline. AB-START-001 authorizes this narrow implementation; real Provider calls, production, paid resources and signing remain closed. Root owns integration and all Node/browser/Core execution. Preserve the previous author branch and tree.

1. Read the existing receipt/attempt DTO fields and renderer.
2. Display each record's own `round` with the existing EN/CN `t.round` label; give each original record a named accessible group. Keep the complete original receipt/attempt bodies.
3. Add four mocked component cases: EN/CN retained failed rounds 1/2 and passed round 3 with distinct candidates, and EN/CN unsettled/unknown values beside measured zero.
4. Freeze source, perform source-only boundary checks, and hand off to Root for Node tests and independent review.

ADR: retain the existing folded history and original record order. Each group names its record type, actual round and attempt ID. The renderer does not derive rounds from list position, current run round, candidate or ID; it does not group away or replace older failed evidence. No DTO, locale, policy, budget or permission changes are needed.

## Review revision: align mocked histories with Core contracts

The initial `eb8c67b` fixture was independently rejected (P2), before Root integration or Node execution. Its completed attempts incorrectly used receipt outcomes as attempt states, and inherited a one-round policy, empty shared ledger and unstarted run/fence fields. Correct only the new test and these four records. Keep the production component and existing tests unchanged.

Use independent three-round consumer DTO mocks: legal `finished` attempts; false/false/true receipt outcomes; three preparation stages, three validators and one CAS; one earlier Graph Work in shared usage. For unknown, retain the first two observed receipts, six attempts including the unsettled third validator, no CAS, unknown fence and no manufactured current receipt. These are consumer-contract fixtures for the intended retained-round history, not proof that Core has executed target drift or approved this policy.
