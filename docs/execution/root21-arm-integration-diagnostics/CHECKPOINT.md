# Checkpoint

Prepared for Root review from exact baseline `7ee2437db08695347650a28f445cb0034b5f3f58`; no product fix or root cause claimed. Only integration_ipc test sources, a pure infrastructure source test and this unit's documents changed. Existing worktrees and WIP remain untouched.

Local command: `python3 tests/infrastructure/test_integration_ipc_diagnostics.py` — 4 passed (Python 3.12.14). `git diff --check` — passed. Root supplied the existing formatter at `/workspace/.toolchains/cargo/bin/rustfmt` with CARGO_HOME `/workspace/.toolchains/cargo` and RUSTUP_HOME `/workspace/.toolchains/rustup`; formatting and `--check` passed for all three changed Rust files with `--edition 2021` (rustfmt 1.10.0-stable, 58cf8f9c35, 2026-10-13). This is formatting/parser evidence, not Rust compilation. No toolchain download was attempted.

Root pending: independently inspect the error/path projection, run focused Linux `cargo test -p agentbaton-core --test integration_ipc -- --test-threads=1 --show-output`, then collect a new exact-SHA ARM CI log. New Rust pure tests appear in the same suite. Only actual RPC error method/fixed phase and same-environment Git metadata can resolve whether the failure is before context, in physical observations, or at the macOS system Git ownership/link boundary.

Do not adjust PATH, copy Git, relax nlink/symlink/INPUT_CHANGED, change capture or reinterpret a refusal as successful integration to obtain green CI. Intel Factory and UI setup diagnostics remain separate.
