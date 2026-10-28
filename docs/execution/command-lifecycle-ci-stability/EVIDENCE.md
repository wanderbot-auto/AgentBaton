# Evidence

Original Intel Product40740806703/job123001572356: `/workspace/.onboarding/pr16-652-ci/product-intel.log` (retained). Its1000ms positive TTL raced real admission/startup. Original source and new diagnostic variants are retained beside logs; no failed evidence was deleted.

All local evidence: `/workspace/.onboarding/command-lifecycle-ci-stability/`.

| Evidence | Actual result |
| --- | --- |
| `original-ttl-diagnostics.log` + `.rs` + `.json` | Original1000ms positive, bounded diagnostics;1PASS3.81s |
| `original-ttl-controlled-delay.log` + `.rs` + `.json` | Original1000ms positive plus real1100ms response delay;1FAIL4.34s, expected exact assertion demonstrates policy-stop-before-failure |
| `separate-lifetimes-first.log` | Test10000ms positive + real delay + revoke then independent command;1PASS5.03s (before added final budget preservation assertions) |
| `final-command14.log` | `cargo test -p agentbaton-core --test command_validation -- --nocapture`;14PASS/0failed/0ignored18.53s, exact frozen source |
| `final-clippy.log` | `cargo clippy -p agentbaton-core --all-targets -- -D warnings`;PASS9.44s |
| `final-fmt.log` | `cargo fmt --all -- --check`;PASS |
| `final-diff-check.log` | `git diff --check`;PASS |
| `final-positive-evidence.json` | Actual pre-revoke Task/session/event/budget snapshot and post-revoke independent command record |
| `MECHANICAL_DIFF.json` | Target function only; other13 functions and whole negative expiry block byte-identical |
| `FINAL_SOURCE_MAP.json` |104 tracked Rust/Cargo file SHA-256 values |
| `FINAL_VERIFICATION.json` | Source commit/tree/file hashes, original failure/log hashes, actual commands, results and limitations |

Environment: Linux; Rust toolchain under `/workspace/.toolchains`; CARGO_HOME=/workspace/.toolchains/cargo; RUSTUP_HOME=/workspace/.toolchains/rustup; CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1; sole Native cache lease. An initial direct rustfmt invocation lacked RUSTUP_HOME and failed read-only default home creation; corrected explicit environment then final formatting passed, no source safety workaround.

No real Provider/model/account/OS Owner/production invocation. The generated Python ACP subprocess and fixed command really ran. No product deadlines, permission gates, expected states or expired negative were weakened. No new daemon/CLI/package needed for this test-only patch. New Apple Silicon/Intel CI and independent review remain separate pending evidence.
