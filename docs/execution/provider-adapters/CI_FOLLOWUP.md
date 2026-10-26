# Independent CI FIFO fixture portability follow-up

Reviewed at 2026-10-26T20:44:04.110523+00:00 on root base `5eba068` plus the single uncommitted file `crates/config/tests/runtime.rs`. Actual first Product run 40397829141 ARM job 121835580552 fails compilation with E0425 because rustix::fs::mkfifoat is unavailable for its Apple configuration. Raw failure log is preserved; this is a real CI failure, not a successful macOS test.

The correction replaces only FIFO fixture creation with fixed `/usr/bin/mkfifo -m 600` and a literal path argument. It asserts successful creation and verifies the file type is actually FIFO before exercising before-launch rejection. The original no-writer behavior and bounded rejection assertions remain; there is no skip, production change, dependency change, timeout extension or weaker expected error.

Independent commands at repository root with the configured toolchain and CARGO_BUILD_JOBS=2: `cargo test -p agentbaton-config --locked --test runtime` passes 22 tests; `cargo fmt --all -- --check` exits 0. This verifies actual FIFO rejection on Linux; a fresh exact-candidate macOS build remains necessary to establish Apple compilation/execution. The earlier full workspace 243 count is historical until its next run; this one test correction does not add tests or establish a new native result.

No real vendor/account/model call, private identity, production action or additional paid service was used. Only this report and FINAL_REVIEW.md were edited by the reviewer; no source/lock/commit/push operation occurred.

| Reviewed file or preserved evidence | SHA-256 |
| --- | --- |
| `crates/config/tests/runtime.rs` | `f9d3446916a0ec4961c077a2dc9cd85fe39d2eb9cbc7059cce3a90a1c09821ae` |
| `/workspace/.onboarding/provider-runtime-ci-121835580552.log` | `64ed204991100de5753870e92c37010cbd164e33b6ddae3e3daf5201dec6cc21` |
