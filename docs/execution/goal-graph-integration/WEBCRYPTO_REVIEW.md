# Independent review: native WebCrypto test leaf

Candidate `579f4ca9ab07d1fd3039ab71ad506af47ca237aa`, baseline `652c5f0`, checkout `/workspace/.worktrees/native-webcrypto-smoke`. Review scope is the three changed test/runner sources and four execution records, for the native dependency evidence relevant to AB-TSK-003/009/010 and AB-SEC-002. No source, candidate commit, dependency, lockfile, CI, permission or production file was modified by this reviewer. The checkout remained clean. During the initial source/Python/Node phase no Cargo or target writes occurred. The subsequently authorized exact native Cargo follow-up is recorded below. No native GUI, credentials, Provider models, CI, merge or formal approval were performed.

## Findings

No new blocking source-diff finding was identified within this narrow leaf. The exact Linux Rust harness compilation and 26 tests, selected-bin Clippy and fmt passed in the later authorized follow-up. The remaining unrun dimensions are actual WebView execution, macOS Intel/Apple Silicon and actual default-production binary inspection; none is counted as passed.

- `native_smoke.rs:905` and `:1037`: both actual injected scripts construct the original string, use the real `window.TextEncoder` and await `window.crypto.subtle.digest('SHA-256', bytes)`. The digest returned to Rust is derived from actual digest bytes; there is no embedded expected digest or crypto fallback in either injected script. The before/after/first and reopened calls are separate invocations. Missing WebCrypto/TextEncoder throws into the existing failed-report path.
- `native_smoke.rs:24`: the vector remains original UTF-8 text, including Chinese, emoji, LF and unsafe-integer decimal literals. It is never JSON-parsed/stringified through JS Number. Actual extracted function execution under Node's unmodified WebCrypto produced exactly 109 bytes and `d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892` for all four tags. Preserving the final LF but parsing/stringifying the numbers changed the digest to `0c68e740a81ddcf1832d9db0a9fdc5a426dfd4a08b52cee2708b728f887e0cea`.
- `native_smoke.rs:30–78`: exact JSON value equality rejects missing/extra fields, wrong digest/input/algorithm/source/types and context tags. Before requires absent prior checkpoint storage; after requires exactly one valid before proof; duplicate/out-of-order invocation fails. Final first/reopened reports require complete before+after storage, their own context tag and, for reopen, the first-window witness. Existing phase/window checks and failure publication remain in force around the additive validator.
- `scripts/native-run.py:32–66`: Python requires actual bool availability, exact integer version/byte count and an exact proof dictionary; it then verifies the four context-specific proofs. The native Rust phase/window machine provides the temporal-order witness; Python's tagged record check does not independently prove wall-clock order from an arbitrary hand-written JSON file. This is a trusted test harness, not cryptographic attestation against an attacker controlling its renderer/report.
- Existing checks were not replaced by the digest: all three readonly SQL literals are byte-identical; removing only the new crypto helper/calls restores the entire old FIRST/SECOND scripts byte-for-byte; all 22 existing Rust test bodies and all 25 existing Python test ASTs remain unchanged. This preserves schema 12, six separate zero Graph counts, readonly Goal/execution/Graph interval cross-bindings, the sequence=3 rejection controls, old missing-run denials, lifecycle/late-failure handling and owned cleanup. No actual final sequence or native database run is claimed by this source review.
- Default production exclusion is preserved at source level: this is a separate binary with required `native-smoke` feature and a non-debug compile error; product lib/approval/parent/manifest/lock/CI/packaging sources are unchanged. The production marker-audit fixture tests passed as part of Python validation. An actual newly built default production executable was not inspected, so binary-level exclusion remains a build/audit gate.

## Actual checks

1. `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s apps/desktop/src-tauri/tests -p test_native_runner.py -v`: exit 0; 29 tests, 0 failures/errors/skips, 2.627s. Python 3.12.14. Log `python-tests.log`.
2. `PYTHONDONTWRITEBYTECODE=1 python3 /workspace/.onboarding/native-webcrypto-independent-review/review_checks.py`: exit 0; all 18 preservation/scope/hash checks true. Existing Rust test-body count 22, candidate body count 26; existing Python count 25, candidate count 29. Both Python files compiled through `compile(...,'exec')` without writing candidate bytecode. Results `preservation.json` and `preservation.log`.
3. `node /workspace/.onboarding/native-webcrypto-independent-review/check-node.cjs`: exit 0, Node v24.19.0. Parsed both actual full script bodies; invoked their actual extracted crypto functions for before/after/first/reopened using unmodified native Node WebCrypto/TextEncoder; 4 exact vector checks and 4 unavailable-crypto/encoder rejection checks passed. LF-preserving numeric-roundtrip negative control produced a different digest. This is Node evidence only, NOT Native WebView/macOS. Results `node-result.json`, `node.log`.
4. `git diff --check 652c5f0..579f4ca9ab07d1fd3039ab71ad506af47ca237aa`: exit 0. `git status --porcelain=v1`: empty. Author's three source hashes match the independently read bytes.

The first independent roundtrip experiment omitted LF and therefore changed both numeric formatting and termination; it was tightened to retain LF so the retained final Node evidence isolates numeric-roundtrip drift. No candidate change was made.

## Required remaining evidence and limits

- Initial-phase Rust status was unrun. Root later explicitly granted the unique native Cargo lease; all 26 selected harness tests compiled and passed, followed by selected-bin Clippy and fmt as recorded below. No Cargo was used before explicit authorization.
- Actual native Linux/macOS WebView crypto, secure-context availability, real first/reopened window behavior and both Apple Silicon/Intel platform runs remain unrun here. A Node pass cannot fill those dimensions.
- Actual default-feature artifact marker audit remains unrun here. Feature/source separation alone is not binary-delivery evidence.
- This vector validates an execution dependency for preserving literal text; it does not demonstrate real Goal Graph execution, model behavior, OS owner approval, signing, acceptance or full requirement completion.
- No formal approval, integration, CI or merge decision is supplied. Candidate is left unchanged for the designated build owner and integrator.


## Authorized exact Native Cargo follow-up (lease released)

Actual command interval: `2026-10-28T18:35:36.846796+00:00` through `2026-10-28T18:36:19.640678+00:00` (UTC), as recorded by native-lease.json. These commands occurred after the earlier source-phase not_run snapshot.

Root explicitly transferred the sole native Cargo lease after the preceding reviewer released it. The candidate stayed exactly `579f4ca9ab07d1fd3039ab71ad506af47ca237aa`. The manifest was read to confirm `agentbaton-native-smoke` and its required `native-smoke` feature. Only these commands were run, serially:

- `rustfmt --edition 2021 --check apps/desktop/src-tauri/src/bin/native_smoke.rs`: exit 0, 0.051s.
- `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-smoke -- --test-threads=1`: exit 0; **26 passed**, 0 failed/ignored/measured/filtered; test execution 4.81s; command wall time 33.937s. This includes the 22 retained Rust tests and all 4 new WebCrypto controls. Log `native-lease-01.log`.
- `cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-smoke -- -D warnings`: exit 0, 8.804s. Log `native-lease-02.log`.

Used Root's existing shared native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=1`, the supplied Cargo/Rustup homes and Linux UI sysroot. No new target directory or full-workspace/57-lib test run was created.

The isolated leaf lacked ignored build assets. As Root explicitly directed, two temporary exact symlinks pointed only to retained `/workspace/.worktrees/goal-graph-integration/apps/desktop/dist` and `src-tauri/binaries`. Their actual file hashes and provenance are in `assetmap.json`. These retained assets were used solely to satisfy unit/Clippy compile requirements; no current frontend rebuild, current sidecar repackaging, native GUI execution, CI asset or newly delivered package is claimed. Both own symlinks were removed in a finally block; shared asset directories were not removed or edited.

All three candidate source hashes before/after are identical and match the author manifest. Final candidate `git status --porcelain=v1` is empty. `native-lease.json` records exact commands, environment selectors, durations, exits and cleanup. Upon the command process exiting successfully the native Cargo lease was immediately released to Root before this report update, allowing the queued next writer to proceed. No further Cargo was run. Real WebView/secure-context behavior, macOS Intel/Apple Silicon, real production artifact audit and full Goal Graph acceptance remain unrun here. The independent review supplies evidence and no new blocking source-diff finding; it is not a formal approval or merge decision.

## Immutable phase snapshots

The original source-only REVIEW.md and manifest.json have been restored byte-for-byte and retain their original Rust not_run statements. The full original source phase, including all files referenced by its saved evidence hashes, is preserved in source-phase/. SOURCE_ONLY_REVIEW.md and SOURCE_ONLY_MANIFEST.json are convenient exact copies. This final report and FINAL_MANIFEST.json add the later explicitly authorized Linux Rust26/Clippy/fmt results without rewriting the earlier state.

Final report sealed at `2026-10-28T18:39:06.644867+00:00` (UTC). Earlier source-phase timestamps and not_run records remain in the immutable source-phase snapshot; they do not describe the completed later Linux compile/test follow-up.
