# Latest pre-finalize review — malformed envelope follow-up closed

Reviewed immutable `e70105b81eecbe4c465a9d1fb0da5dd008b88d75`, one-file delta in approval.rs from ff494. Exact exported source SHA-256 `901ca5854bf20d8a337919c8653413d523e4368bb0bd07c161b3704c68f163e0`. Mutable command-ui HEAD and its later command functionality were not compiled. The ff494 review and its discovered UX gap are preserved below as historical evidence, superseded only for this fixed error-envelope handling.

Verdict: no unresolved blocker in the e70105b pre-finalize classification fix. The previously reported conservative unknown/locked UX gap is closed for malformed pre-approval error envelopes. No full application/native GUI/true IPC or later CommandKind/bridge/UI acceptance is implied.

A top-level error is now preserved only if its value is an object with string code/message/side_effect_state and boolean retryable. No authority is granted by that object; it returns before either closure. Additional server fields and unknown future string code/side-effect values are preserved exactly. Invalid null/scalar/array/empty errors, missing required fields or wrong field types return INVALID_APPROVAL_PROPOSAL/not_started with0 native confirmations and0 finalizations, even beside an otherwise valid proposal result. This is a local pre-confirmation classification, not a downgrade of any finalized/transport-ambiguous result.

The rest of proposal validation, complete16384-byte snapshot guard, exact server-reference finalize request and native decline are unchanged. After native confirmation the code still directly returns finalize's Result. There is no new post-finalize envelope validation: malformed/missing responses and transport Err remain unchanged/unknown. Serialization-error handling is source-reviewed and was not artificially triggered, as in the historical review.

Independent exact-source rustc execution using the same existing serde_json rlib and dependency path as below:

```sh
CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustc --edition=2021 --test /tmp/agentbaton-pre-finalize-independent-e70105b/src/approval.rs -L dependency=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target/debug/deps --extern serde_json=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target/debug/deps/libserde_json-9c3b0a65d1abc43f.rlib -o /tmp/agentbaton-pre-finalize-independent-e70105b/approval-tests
/tmp/agentbaton-pre-finalize-independent-e70105b/approval-tests --nocapture
```

Compilation and execution exit0:7 exact-source unit tests pass. The separately exported matching capability file remains permission-empty. A second temporary boundary-tests.rs imports the unchanged exact source; the same rustc command with that input/output and `boundary-tests --nocapture` exits0:12 tests, comprising the7 originals and5 independent boundary tests, not12 additional unique cases.

The new counter matrix independently checks each required error field missing or wrongly typed, malformed error with and without a valid proposal result, and valid fully typed unknown server errors preserved byte-for-value even with an accompanying result: all0 confirms/0 finalizes. Other independently rerun comparisons retain malformed proposal rejection, exact16384/16385 UTF-8 snapshot accept/refuse, native decline1/0, post-finalize malformed/unknown response1/1 and transport Err1/1 unchanged. Thus the malformed-before versus malformed-after-finalize distinction is exercised directly.

No source/dependency/lock edit, Cargo/full-application build, staging, commit, push or subagent. Only this review document and temporary independent source/harness/logs changed.

## e70105b independent SHA-256 evidence

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/src/approval.rs` | `901ca5854bf20d8a337919c8653413d523e4368bb0bd07c161b3704c68f163e0` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/capabilities/main.json` | `090491e6dce371a7853acf44c4a65c38e2e372ac50df6a5e313a36a5ec8acf9a` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/compile.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/tests.log` | `69572a728575884539afd7032ea3e5965e0b592ec81dda194d475a76ab7a6983` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/boundary-tests.rs` | `ca8e1ec475952670eff2606195a451ec0e179be3580a471508f3d5c8c7a61613` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/boundary-compile.log` | `dbaffc07f91ca3554b1ae6e0fb046f083f9684f49beb169e395bbd26761520c7` |
| `/tmp/agentbaton-pre-finalize-independent-e70105b/boundary-tests.log` | `bb3fe4e20124dbd6e5e31e24f61d319836ab134cbc0547c3b8bb0fa72aec92b6` |

---

# Historical ff494 pre-finalize review

Reviewed immutable commit `ff49430977f206c829cbfe9e9264e381f3e2cc32`, only `apps/desktop/src-tauri/src/approval.rs`. Exported this exact git object plus its referenced capabilities/main.json to a private temporary review directory. The command-ui worktree and mutable HEAD were never used as the compiled source. Later CommandKind/bridge/UI work is outside this review and needs separate review/build evidence.

Verdict: no unresolved blocking finding in this narrow fix. Deterministic local rejection before native confirmation/finalize now produces structured `INVALID_APPROVAL_PROPOSAL`, `retryable:false`, `side_effect_state:not_started`; the user can correct a refused selection rather than treating approval as ambiguously finalized. A persisted proposal is possible, and the source comment correctly scopes this receipt to approval/finalization, not every side effect of proposing. No authority is minted by a malformed server response.

Follow-up boundary clarification: ff494 still returns any top-level `error` key unchanged, including `error:null`, a scalar/non-object error or a missing-result malformed envelope. The already-run counter tests prove0 confirmations/finalizations for these inputs. Inspecting this exact commit's api.ts and DataContractValidation.tsx confirms that null-without-result becomes generic Error, while malformed non-null error fields can produce a DaemonError with missing side-effect state; both remain conservative unknown requests in the UI. This is a remaining pre-finalize refusal/UX completeness gap, not an approval or permission vulnerability. Recommend a narrow follow-up that preserves valid server errors exactly but classifies malformed pre-confirmation envelopes as INVALID_APPROVAL_PROPOSAL/not_started. This review does not call that malformed-envelope UX complete. Any malformed response after finalize must continue to remain unchanged/unknown and must never be downgraded to not_started; the existing finalize counter tests protect that contrast. This conclusion is source/counter evidence, not a full application reproduction.


Code audit confirms all local proposal-validation exits are before both closures: missing result, wrong/missing/non-string kind, missing/empty/non-string/over128-byte ID, absent/non-u64/zero revision, missing/non-object snapshot, serialization Result error, and a pretty-serialized snapshot over16384 bytes. No scope truncation occurs. Native decline remains the original PERMISSION_DENIED/not_started receipt. A response containing an error key is returned unchanged before native confirmation, including unknown server errors. After native confirmation the function directly returns finalize's Result: transport Err, structured unknown server error or malformed finalize response cannot be rewritten as not_started. No catch-all error reclassification is introduced. The finalize request still contains only server proposal ID, expected revision and deterministic proposal idempotency key, never renderer-approved scope.

## Independent execution

Exact snapshot directory `/tmp/agentbaton-pre-finalize-independent-ff494/`. Existing serde_json and dependency cache were used read-only; no Cargo target or dependency build/install/lock edit.

Actual compile command (environment and paths literal):

```sh
CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustc --edition=2021 --test /tmp/agentbaton-pre-finalize-independent-ff494/src/approval.rs -L dependency=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target/debug/deps --extern serde_json=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target/debug/deps/libserde_json-9c3b0a65d1abc43f.rlib -o /tmp/agentbaton-pre-finalize-independent-ff494/approval-tests
/tmp/agentbaton-pre-finalize-independent-ff494/approval-tests --nocapture
```

Both exit0; all6 original pure Rust units pass on the exact exported source. The empty renderer capability fixture was likewise exported from ff494, not read from the changing worktree.

A separate temporary boundary-tests.rs imports the unchanged exact source by path. Compiled with the identical rustc/dependency arguments, substituting boundary-tests.rs and output boundary-tests, then executed `boundary-tests --nocapture`: exit0,11 tests (the original6 repeated plus5 independent adversarial tests). Counted unique new coverage is5, not11 additional tests. Explicit Cell counters verify:

- Null/array/scalar/missing result, absent proposal fields, typed-malformed ID/revision/snapshot and >128 UTF-8-byte ID:0 confirms,0 finalizes, structured not_started rejection.
- Complete UTF-8 snapshot serialized at exactly16384 bytes:1 confirm,1 finalize, complete untruncated data shown. At16385 bytes:0 confirms,0 finalizes.
- Server errors, including an error:null or malformed error envelope: exact response preserved and0 confirms/finalizes; they do not become approval authority.
- Decline:1 confirm,0 finalizes, original PERMISSION_DENIED/not_started semantics.
- Finalize transport Err or missing/malformed/unknown-result envelopes:1 confirm,1 finalize, original Result/value unchanged with no local not_started downgrade.

The serialization Result-error branch was source-reviewed but not synthetically induced: the ordinary already-materialized serde_json::Value fixture serializes successfully, and no custom serializer/fault hook was added. No full Tauri/Cargo application build, native GUI/owner click, true daemon IPC, frontend ambiguous-retry integration, macOS or complete product acceptance is established by these pure units. No account, Provider/model or paid resource call.

Only this repository review document and independent temporary files were written; no source/lock edit, staging, commit, push or subagent. Later command-validation approval kinds, bridges and UI are explicitly excluded.

## SHA-256 evidence

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/agentbaton-pre-finalize-independent-ff494/src/approval.rs` | `75c7ffc2817605524613c5ca709641589c51620aa04822fb2bb95e4394cf83d7` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/capabilities/main.json` | `090491e6dce371a7853acf44c4a65c38e2e372ac50df6a5e313a36a5ec8acf9a` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/compile.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/tests.log` | `346c44a12d7d2346e35da0fa2bd36ef65de9129711907c2abf81f19b9b432054` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/boundary-tests.rs` | `7386c5e6e86d685f77d4a4cd1d65a6ddd6f2d82a8517dc3f7587a705d1d6e508` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/boundary-compile.log` | `3aad12962a587fca77194b379ee65e0618f1b83c7be02e25a79e4a1b48473bf2` |
| `/tmp/agentbaton-pre-finalize-independent-ff494/boundary-tests.log` | `162408a895faaa1997b3965287d9687d76bc5cb00510c48c68882a9f3da9d15b` |
| `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target/debug/deps/libserde_json-9c3b0a65d1abc43f.rlib` | `6a61eb9f02da47ad4c0743f37aff926a38667b3217e18e3ce706466e33de3944` |
