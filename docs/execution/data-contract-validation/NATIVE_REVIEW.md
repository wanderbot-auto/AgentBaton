# Independent native bridge and read-only harness review

Reviewer: native `/root/native_ci_fix`, independent of the new data bridge implementation. Reviewed at **2026-10-26T23:50:50.833612+00:00**, worktree `/workspace/.worktrees/data-contract-validation`, committed baseline **d0c714b69c9114ab38cbeb702ebf815e9a6a501f** plus the frozen uncommitted FIRST/browser-readiness/shell assertions. Core/schema7 files were concurrently owned by the Core worker and are outside this review. Only this document was written; no source, lock, stage, commit, push, CI trigger, sidecar or nested agent was created by this reviewer.

Verdict: **no blocking authorization or false-success issue found in this native/adapter delta**. This is a static/syntax review, not native compilation, native-unit execution, real owner-dialog testing, current browser acceptance or macOS verification. Those integrated checks remain pending until the primary prepares actual sidecars after Core freeze. Full130 requirements remain open.

## Native command and approval boundary

The production dispatcher preserves all25 previous commands and adds exactly five data commands: `data_validation_policies_list`, `data_validation_policy_approve`, `data_validations_list`, `data_validations_run`, `data_validation_policies_revoke`. They map to fixed authenticated Core namespaces. Task/policy IDs, observed revision and idempotency key have fixed argument positions; the approval selection is a JSON Value sent only to the fixed server data-selection parser, not a caller-selected RPC method. The native bridge does not assert renderer input is authority; current Core validation/stop/root/spec/fence enforcement is separately reviewed by its owner.

`DataValidation` chooses proposal method `data_validation_policies.propose` and requires exact server kind `data_validation`, rejecting the existing artifact-integrity `validation` kind before any dialog/finalizer. New source tests assert the distinction, denial does not finalize, and successful dispatch sends only server proposal ID/revision. Those Rust assertions were read but not run in this review. Server proposal IDs/revisions and object-shaped snapshots are validated. The complete pretty JSON is measured in UTF-8 bytes and refused above16384 bytes; no substring, omission or truncation is introduced. Data wording expressly limits checks to approved JSON structure/ranges/hashes and owner-declared metadata, excludes running reproduction scripts, algorithm/scientific correctness, TaskAccepted and integration permission. The common native OS/account warning is retained.

Only native Rust calls `approvals.finalize` after its native dialog. The renderer capability still has an empty permissions list; no shell, filesystem, dialog plugin permission or renderer finalize command is introduced. The shipping dispatcher contains no native-smoke report hook. The separate smoke binary still requires its test feature, and release+test-feature compile guards/default shipping entry remain unchanged. These structural safeguards do not replace an actual current release executable marker audit.

The standalone native Cargo.lock has the identical package/version/checksum universe to8da2bee. Its sole change is the workspaces package gaining a direct edge to the already-locked **serde_json1.0.154**; no new package, version, source or checksum appears.

## Read-only native and browser evidence design

The pending native Rust delta outside FIRST is byte-identical to d0c714b: window labels, phase/CAS terminal guards, scheduling, daemon ownership/cleanup and error semantics are preserved. FIRST uses the actual product composer and then the same task's two new native read-only lists. It requires both actual results to be empty arrays, opens the actual Artifacts tab, requires the new panel to start folded, expands it, and waits for the real form fieldset to be enabled. Source tracing confirms that fieldset remains disabled until both current scoped list reads have succeeded; an empty initial label alone cannot satisfy the check. It then requires no alert, checks the approval button remains disabled without an eligible stopped session, compares the real daemon sequence before/after all data reads/mounts, and returns to Conversation before reporting success. It never clicks native approval or data run and never invokes a Provider/model. Missing controls, failed reads or mismatched sequence take the failing report path rather than a simulated pass.

The browser adapter adds only `data_validation_policies_list` and `data_validations_list`, with only taskId accepted for each. Its bounded localhost/origin/content-type controls and method/key whitelist remain. No data approval/propose/run/revoke/finalizer HTTP route is added. The browser's new fieldset readiness predicate also waits for successful real scoped reads, while its current explicit approval-disabled and successful method/error assertions remain. During this review the primary also scoped the preexisting exact integrity no-session hint assertion to `.artifact-validation`, since the newly adjacent data panel deliberately has the same localized text. The exact text/assertion remains and the scope narrows to its intended integrity component; no timeout or data check is removed. Current Python AST and the data JavaScript predicate were rechecked after that scoped selector change. These future checks are browser+real Unix IPC evidence, not native owner confirmation or GUI evidence.

All existing deadlines are unchanged. Exact current source values are FIRST/SECOND JavaScript10s, native watchdog60s and external runner default80s, plus existing3s cleanup/capture bounds. The change adds no wait extension, skipped assertion or permission bypass. The new three literal true result flags are produced only after the actual new assertions and are required by the macOS shell postcheck. Their current execution is **not claimed**.

## Observed nonblocking recovery limitation

A malformed or oversized server proposal rejected locally before native confirmation/finalization still returns a Rust ErrString. The UI conservatively classifies that non-DaemonError as unknown, preserves the same form/selection/key through remount, and offers only read reconciliation plus explicit same-request retry. An oversized complete snapshot can therefore remain blocked even when the smaller selection passed frontend preflight; the user cannot shrink that saved pending form within that request. This is fail-closed and does not approve/run anything, but it is a real recovery UX limitation. It is not new data-only escalation behavior and was not treated as a security/source acceptance blocker here.

A minimal future correction must distinguish a locally proven preconfirmation rejection with no finalizer call from finalizer/transport errors that may already have committed. It may return a narrowly scoped structured known-rejection result so that form can be edited, while retaining any proposal audit provenance. It must not blanket-map all ErrString/transport/finalizer failures to not_started or discard genuinely unknown intents. No such change was made by this reviewer.

## Independently executed static checks

All exited0/read assertions passed, with Node **v24.19.0**:

- Extracted FIRST and SECOND and ran `node --check` through stdin for each, without executing either script.
- Parsed browser Python with `ast.parse`, extracted and syntax-checked its new data-fieldset JavaScript predicate through Node stdin.
- `bash -n scripts/native-macos-smoke.sh` and AST parse of its embedded Python artifact assertions.
- `rustfmt --edition 2021 --check` on native lib/approval/smoke sources; syntax/format only, no Rust build or linked execution.
- Exact dispatcher25→30 union, empty capability permissions, unchanged non-FIRST bytes, and lock package/edge comparison described above.

No files were written by these syntax commands. No native compile/test, real sidecar preparation, actual current browser run, current macOS ARM/Intel run, native dialog, signing or default release audit was executed by this reviewer. The separate UI_REVIEW129-test result is another context's evidence and is not relabeled as native verification here. Fresh integrated execution and exact-head required CI remain necessary before acceptance.

## Frozen native/adapter source SHA-256

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/lib.rs` | `bae39101810b40ceae53b5d82001825f5dd59725c42aed7e883486a9676f4293` |
| `apps/desktop/src-tauri/src/approval.rs` | `c4111d08229607f2c5b33962f79af537f1723f865d39e15b1a2fe8116321a1f8` |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `81dacf1b3c39d1fcde3b3012809720cf1d35c0d6caf63b0d99138cd165eff5a1` |
| `apps/desktop/src-tauri/Cargo.lock` | `1a242354c2716ae834880acd7bcfe50c2b1ad77403d37bda18428794f6aec2ef` |
| `scripts/native-macos-smoke.sh` | `b1ae9e6f083baf152a44b09f69278e9d2ed12bf6e680ba9d8f33d659b204120b` |
| `apps/desktop/tests/quiet-canvas-real-ipc.py` | `c13b6fc6323ed68398f3348f64f702fd898cfb22f617afc4a6b5ab2ae15063c3` |
