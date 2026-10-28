# Independent Goal native read-only review

Reviewer: `/root/review`, independent execution context. Reviewed initial source `ee4024e93ad5adaee8e9fee14dc5ba8fc548e28c` and fixed source `9244f652af1c08ec1eea5a5851b3627ef34eee50` against Goal baseline `b71492fd49fb9e72a775572d4933cbc2789e6939`. Final documentation HEAD: `ad30ad0b77dba3a2992ba881d28ce0a6d7e3dc13`; the author's documentation closure preserves all five source hashes. No source, locks, index, CI runs, commits, or remote state were changed by this review.

Verdict: **no remaining blocking finding in these five source files after P2's fix**. This approves the evidence extension for integration and fresh exact-head CI, not completed Goal execution or a new macOS GUI result. AB-START-001 / OWNER-001 / OWNER-002 remain the authority boundary. This is partial evidence for Goal scope/plan visibility and evidence-based desktop UX (AB-TSK-009/011, AB-UX-002/004/007/009); it does not close those complete requirements.

## Finding and independently verified fix

**P2 — fixed: generated native fixture database was incompatible with the real private daemon.** Initial `prepare_readonly_goal_project` opened Store before the daemon and created `baton.sqlite3` as 0644 under umask 022. The original host tests checked project creation but did not bind the actual daemon to that helper-created database. An independent executable copied the exact old helper and used the public `Daemon::bind`: it observed schema 9, mode 644, then `PERMISSION_DENIED`; the original assertion failed with exit 101. This is actual Linux evidence, not an inferred historical macOS failure.

Fixed `native_smoke.rs:110` precreates only a fresh database with `create_new(true)` and mode 0600, closes that descriptor before Store opens it, and refuses an existing file without chmod or replacement. The generated-project test now asserts 0600 and performs actual `Daemon::bind` after dropping Store. An additional test preserves an existing database's bytes and mode on refusal. The independently compiled control kept its same main assertions and replaced only the helper with the fixed source: schema 9 / mode 600 / actual daemon bind succeeded, exit 0. Production runtime permission checks and process umask were not relaxed.

Original failure evidence is retained under `/workspace/.onboarding/goal-native-readonly-review/`:

| Evidence | SHA-256 |
| --- | --- |
| `old-private-db-probe.rs` | `4826ec89aa8a90a5ad5ea270b178dee7e7bf3f3f98a1052808de50bc250e61e1` |
| `old-private-db-probe` | `2c8a0125c709fa0ef98dbda54d7eaf34a92c63cd1c7a6adc9389c1ba05958ca6` |
| `old-private-db-failure.log` | `0c19b02b32aad36d645bd094b881df199d786437005121151b4130273b5b41a9` |
| `fixed-private-db-probe.rs` | `1ecc4108c50d2392b14e7ece06ec47d6171fb81a7b0a57a6807ae1fafbd9cf4a` |
| `fixed-private-db-success.log` | `c7fc20ad4743dd9609f52bed7b3c6155d32b03d416708283fa7ad0afd78b800e` |

The author's earlier unregistered-project `NOT_FOUND` browser failure also remains in its original evidence directory. It was corrected by importing a real generated-directory project and selecting it in the actual composer, not by swallowing the error, inventing empty lists, retagging SQL, or authorizing a Goal.

## Source and authority review

The first real native WebView retains the existing Command3/Data3 assertions and invokes the actual fixed Tauri `grants_list` and `goal_scopes_list` commands. It selects the registered fixture project, verifies the Goal panel starts folded, expands it, waits for successful reconciliation, and checks that approval/plan/preview controls are absent or disabled without a root grant. Exactly three strict JS booleans are required: `goalScopeLists`, `goalScopeEmptyControls`, and `readonlyGoalNoMutation`. Independent structured database counts and sequence are additional evidence; there is no fourth invented JS flag.

The read-only checkpoints reuse the test-only report command, accept only the first main-window phase, reject wrong windows/replay/malformed counts, and do not advance the lifecycle. Their Python subprocess uses `-I -S`, fixed SQL and the owned runtime database, SQLite URI `mode=ro`, `query_only=ON`, and `BEGIN`. Renderer input cannot select a path, executable, or query. Output is capped at 4096 bytes and the direct owned child has a two-second bound. This is not a general guarantee about escaped descendant processes. Original JS/native/runner/cleanup deadlines and original six harness tests remain intact.

The runner and macOS wrapper require real report fields with strict types, matching project/task identity, and unchanged Goal counts/sequence. Missing, stale, contradictory, boolean-as-integer, or malformed reports cannot satisfy acceptance. Existing default-production versus debug feature isolation and release audit remain intact; no production permissions, command registrations, Core methods, dependency locks, account credentials, or owner confirmation authority are added.

The browser adapter adds only a read-only Goal list route. Project import is private generated-fixture setup outside the HTTP adapter's allowlist. The full setup intentionally writes before the read-only Goal observation interval; the claim is unchanged Goal counts/sequence during that interval, not an entirely write-free harness. No Goal mutations, approval finalization, model requests, or dispatch are used to manufacture this evidence.

## Independent commands and results

All native Cargo commands ran with umask 022, existing SDK/sysroot, `CARGO_INCREMENTAL=0`, jobs 2, and only the explicitly handed-off native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`. The main Core target was not used. The native target was formally released afterward; subsequent Data UI validation belongs to its separate reviewer.

| Actual command/control | Result | Log under the review evidence directory |
| --- | --- | --- |
| `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol` | 13 host + 11 harness tests passed; 0 ignored | `native-fixed-tests.log` |
| `cargo clippy --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings` | passed | `native-fixed-clippy.log` |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`; `git diff --check b71492f..HEAD` | passed | `native-fixed-fmt.log` |
| `PYTHONDONTWRITEBYTECODE=1 python3 -B apps/desktop/src-tauri/tests/test_native_runner.py` | 17 tests passed | `runner17.log` |
| Python source compilation, Node syntax checks of extracted FIRST/SECOND, `bash -n scripts/native-macos-smoke.sh` | passed; FIRST/SECOND also compared unchanged across the private-DB fix | `syntax.log` |
| Independent exact-helper probes, each executed with umask 022 | old exit 101; fixed exit 0, actual bind succeeded | `old-private-db-failure.log`, `fixed-private-db-success.log` |
| `python3 -B apps/desktop/tests/quiet-canvas-real-ipc.py --batond /workspace/.onboarding/goal-editor-compatible-binaries/batond --chromium /usr/bin/chromium --output /workspace/.onboarding/goal-native-readonly-review/browser` | 38 actual browser/Unix-IPC checks and 24 PNGs passed | `browser.log`, `browser/evidence.json` |

The browser run used built frontend assets recorded in `frontend-hashes.json`, Playwright 1.62.0 and Chromium 154.0.8037.57. Its independently hashed compatible Linux schema-9 daemon is `2b4a302c4e992df9c01cde404c03ba526cf411eb24ecbc7aadc4e85fb8be771e`. It is **not** the primary's later fresh `4fa` daemon or macOS release/native run. All 24 PNG hashes were recomputed; desktop 1488×981 and mobile 390×844 Goal views were visually inspected. Across four Goal viewports, actual SQLite counts remained scopes 0 / plans 0 / attempts 0 and sequence 7 before and after. Screenshots alone do not prove lower offscreen controls; DOM checks supply those assertions.

## Exact reviewed source

| Path | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `e63ae80dc190b1884e96b55b127ef648325b583585bad939a94f0eae60a842b9` |
| `apps/desktop/src-tauri/tests/test_native_runner.py` | `844fd6410315d6c3fb99828b212d46b1f1d5caf76e02a471a04a0caf5e84dd2e` |
| `scripts/native-run.py` | `c46145848ec5ae4ee966f0781b92d5a4079232a4cb054dda4ce82e0505bb1845` |
| `scripts/native-macos-smoke.sh` | `29ea54f7e94229a46934686f73a73c7e215c5dc5da3d123523c2b2734b29bb61` |
| `apps/desktop/tests/quiet-canvas-real-ipc.py` | `d6a3f683e7ffcd354fb21c200488c25e228ebbf5a7ef290bef56c952e7598ebd` |

Verification manifest: `/workspace/.onboarding/goal-native-readonly-review/verification-manifest.json`, SHA-256 `7dc96abc0468c6527124048a33ff2694c46488db6857dbdddfef8872584fdd21`. It records all five source hashes, exact probes, independent test/syntax/lint logs, browser evidence, frontend hashes, and all 24 PNG hashes. Key test log SHA-256: native tests `5d74fdd84414e6e915c575e1df14d8a119e92eec87c4748ab188a58a1146a95b`; runner `95c44ee92cd631b64bf794ceca7b94f2528e3d70b4d417f7d78e2c2e319eb2e8`; browser evidence `5aedf589a0be4d26d5a1fa4d80eca6e14b8979f785983937236915837a9be0ee`.

Separately inspected integration-only commit `2b8b6d7c82f5bce9a888e06a4c973b9dff24a444`: changing the imported admission-performance test's exact fresh-schema assertion from 8 to Goal schema 9 preserves the assertion and all deadlines. This is a reasonable integration alignment, not a claim that an old failing execution was observed; its full workspace verification belongs to the primary.

## Remaining validation dimensions

Fresh exact-head macOS execution of this extended Goal WebView lane, actual Cocoa screenshots/window lifecycle, manual native owner confirmation, real Provider/model/account behavior, signed distribution, populated Goal history and complete Goal scheduling/dispatch were not run by this review. Existing Command/Data/native lifecycle checks are preserved in source and host tests; their new macOS proof still requires the new CI subject. Empty read-only Goal visibility does not establish those capabilities or finish the overall approved product goal.
