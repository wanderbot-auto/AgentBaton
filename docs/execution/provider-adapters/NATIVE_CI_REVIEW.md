# Independent native CI fixture and cleanup review

Reviewer: native `/root/integration_review`, separate execution context. Reviewed the three native-agent frozen sources plus the integrator's two-line macOS unit-test invocation, at 2026-10-26T21:29:26.335078+00:00. Only this review document was written; no sources, locks, staging, commits, pushes or permission settings were changed by this reviewer. Concurrent UI/conditional-Git changes were reviewed separately and are excluded from this native source checkpoint.

Verdict: no unresolved source blocker in this native test-harness correction. Independent 15 Python tests, the actual one-test private-runtime/daemon admission fixture under 022, native format and feature/all-target Clippy checks pass. Darwin cleanup behavior and successful native WebView execution still require fresh exact-candidate macOS CI.

## Actual failure provenance

Independently verified downloaded artifact 12689352960 against its retained GitHub metadata digest and SHA-256 `f4324ed37e6981d55fce03033d35285fb45a8ce84aa748d67e13f0f39102294d`; both ZIP members exactly match the extracted evidence. It contains only lane.log and native-run.log. The latter reports owned daemon exit before authentication; lane.log reports EPERM in the old external runner's group cleanup. There is no native-smoke.json and no successful window result. These bytes do not establish the historical startup permission cause.

The preserved old-Builder/real-Daemon control under 022 fails 101 at private-runtime admission. Explicit 0700 creation fixes that same local prerequisite. This is a meaningful mechanism/control, distinct from proving why the historical Mac process exited. Initial zero-test filter and formatting failures remain in the 13 hash-verified evidence files and are not counted as successful execution.

## Source assessment

- `private_fixture_runtime` creates only a fresh test-owned TempDir with explicit 0700 mode. Main and the regression use that same helper. The unit invokes actual core Daemon::bind/storage initialization, confirms the socket exists and 0700 remains, then confirms owned socket cleanup on drop. Existing directories are not chmodded and production private-root admission is unchanged. Startup diagnostics additionally preserve the actual exit status without forwarding raw daemon/provider output.
- The external runner holds the original private-session/group leader unreaped until both group signals finish, preventing PID/group reuse. Only the original known group is a signal target; PIDs returned by ps are never used as signal targets.
- An EPERM exception is accepted only for errno EPERM after observed leader exit and a bounded query proving that original zombie leader is present and every returned group member is a zombie. Missing/unknown leader, live members, malformed output, query failure and other permission errors cannot supply this proof. No live-process permission bypass is introduced.
- Observation/cleanup failures are caught into structured failed evidence, the original child is reaped where permitted, and a passing native report cannot override cleanup/observation failure. The previous report is archived before the run; invalid, stale or missing native evidence still fails. Original report bytes are preserved before failure normalization.
- External timeout 80s, native watchdog 60s, screenshot 3s and existing acceptance assertions remain unchanged. Cleanup now has a bounded 3s deadline; the bounded group query is at most 1s each. Unknown cleanup remains failure rather than invented confirmed stop.
- The macOS script now runs the native-smoke-bin unit tests before building/running that same feature-gated binary. The default host/approval dispatcher, capabilities, Cargo manifest/lock and shipping entry point are unchanged. The separate required-feature debug harness still rejects release builds; no production smoke command or permission is added. The script's set-e behavior preserves failure if the unit test fails.

## Independent validation

Used `CARGO_HOME=/workspace/.toolchains/cargo`, `RUSTUP_HOME=/workspace/.toolchains/rustup`, matching PATH, CARGO_BUILD_JOBS=2 and `umask 022`. Native linking used the existing private `/workspace/.toolchains/linux-ui/sysroot` through PKG_CONFIG_SYSROOT_DIR/PKG_CONFIG_LIBDIR and prefixed LD_LIBRARY_PATH/LIBRARY_PATH with existing values preserved. No system path was modified or sandbox disabled.

| Command | Actual result |
| --- | --- |
| `python3 apps/desktop/src-tauri/tests/test_native_runner.py` | PASS: 15 (original 10 retained + 5 new synthetic permission/cleanup/observation contracts). |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-smoke fixture_tests::fixture_runtime_allows_the_real_private_daemon_authority -- --exact` | PASS: 1 actual test; core private daemon admission, not a window. |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS, exit 0. |
| `cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings` | PASS, exit 0. |

The EPERM regression injects a syscall outcome on a real owned Linux zombie group. It tests the failure contract, not Darwin kernel/ps behavior. All 3 agent source fingerprints and all 13 recorded evidence-file fingerprints match actual bytes; the added macOS script fingerprint is bound below. No successful current macOS/native screenshot, confirmation dialog, release bundle audit or installation is claimed. Linux WebKit's separate helper-path limitation is not bypassed or relabeled. No real Provider/model/account, private identity/key, production action or additional paid resource was used. Full Design v0.2 remains incomplete.

## Exact reviewed source fingerprints

| File | SHA-256 |
| --- | --- |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `4952d4cdf7cb78909264092a45c46c7035f2c2f0ea3e2fcef5936e0934ec2b2c` |
| `apps/desktop/src-tauri/tests/test_native_runner.py` | `4e8165bfd0df98a5db63d5ce19ed429625773e2e93fba35ad07c272b43d63a3b` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |

Native-agent evidence JSON SHA-256: `88e1e0d1d8cc621a9a5a191e5abcb46e0de5b0d6a822fdbc0eb4622994eb2ff4`.
