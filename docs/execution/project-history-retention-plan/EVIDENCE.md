# Project history retention plan evidence

## Source and scope

- Fixed base `ec2ca48b12241451905157240238987c821e2b2a`.
- Tested source `c7e672cd2d92edfa232656c0cab1328aff86565f`.
- Public API `agentbaton_core::conversations_history::retention::plan_project_history_cleanup`.
- The existing history module has one new module entry. Removing that entry
  restores its exact base Git bytes. Planner and synthetic public-entry tests
  are in the two other claimed Rust files.
- Test binary `target/debug/deps/history_retention-19f0dbb04cdb6c66` on macOS 26.6.1, Mach-O 64-bit executable arm64.
  SHA-256 `037960b062ed7156553ecb7774b6f2d02bd5913479502a3e4c91a534d45c49ba`.
  [Artifact and toolchain receipt](logs/candidate-artifacts.json)

The plan evaluates the adjustable 90-day default, positive project overrides
and permanent retention at an explicit Unix millisecond instant. Cutoff is
inclusive and uses 24-hour days. Capture opt-out describes new body capture.
Previously stored detail still follows the chosen TTL. D04 treats 90 days as
the engineering default.

The record owner reuses Core's Project, Task, Conversation and ManagedSession
entities. Nonterminal or unknown execution/turn states remain protected. The
verified snapshot supplies pins and other active references. The caller
supplies the inventory and source-specific derived links from that same
snapshot. The preview contains necessary stable IDs, categories, keep reasons
and expired-detail gap reasons. Valid foreign records are excluded. Ambiguous
identity, ownership or lineage refuses the plan. Missing/future captured times
remain protected. Unrepresentable cutoffs refuse with `INVALID_REQUEST`.

## Verification

| Check | Fixed base | Tested source |
| --- | --- | --- |
| Locked offline public-entry `history_retention` integration test | | 7 passed |
| Locked offline existing history library tests | 12 passes and 1 failure | Same 12 passes and 1 failure |
| Locked offline existing `conversation_schema18` integration tests | 1 pass and 2 failures | Same 1 pass and 2 failures |
| Workspace `cargo fmt --all -- --check` | | Passed |
| Locked offline `cargo clippy -p agentbaton-core --test history_retention -- -D warnings` | | Passed |

The existing pagination test
`independent_latest_pages_preserve_global_order_and_fixed_snapshot` compares
schema 19 to an expected 18 at `conversations_history_tests.rs:101`.
The two schema18 integration failures are
`generated17_upgrade_preserves_every_old_row_and_private_backup_then_reopens_without_rewrite`
and `inverse_participates_in_callers_atomic_boundary_without_accepting_nested_outer_call`.
Their unchanged fixture refuses the controlled empty Conversation18 inverse.

Both old suites were rerun from an isolated `git archive` of the fixed base.
The replay verified the relevant raw Git bytes and lock first and built into a
separate task-local target. Both complete result sets, named failures and
exit codes match the candidate exactly. Old tests and storage/schema remain
byte-identical.
[Base source receipt](logs/baseline-source.json) ·
[Base history failure](logs/baseline-old-history.log.gz) ·
[Base schema18 failures](logs/baseline-old-schema18.log.gz)

The new tests use generated metadata only. The preview performs local
calculation. Persisting project policy, joining live inventory and pin state,
recording actual gaps and applying transactional cleanup remain future slices.
The preview against `46e38016256dfe3ff2646eb546cad5b8179e27ef` had no Git conflict.
[Archive hashes and exact command receipts](LOGS.json)
