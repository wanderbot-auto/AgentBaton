# Evidence

Executed in `/tmp/root17-budget-details-e874`:

- `python3 docs/execution/ancestor-budget-detail-transport/source-audit.py` — PASS, 23 source-only checks; output and exact source hashes in SOURCE_AUDIT.json. This performs read-only `git show` of fixed base and source inspection, not Rust or SQL execution.
- `CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 --check crates/core/src/goal_factory.rs crates/core/src/goal_admission_detail_codec_tests.rs crates/core/src/goal_admission_detail_store_tests.rs` — PASS.
- `git diff --check` — PASS.

NOT_RUN: Rust compilation, all original/new Rust tests, SQL/Store execution, product processes, Node/browser, network, CI, Intel/ARM Mac validation. Source checks do not substitute for these.

The inverse audit removes only documented codec/helper additions and test includes, restores the old parser and old single IPC wrapper line, then checks entire files byte-for-byte against e874. This proves existing production guard constructors, accounting, SQL, error message/refusal behavior, and event write code were not edited; it does not prove the new code compiles or behaves correctly. Earlier source-audit script development briefly failed on newline reconstruction and a comment containing “cache”; those audit-script assertions were corrected, with no product behavior change.
