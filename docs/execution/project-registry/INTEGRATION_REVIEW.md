# Independent review · registry integration

Reviewer: native `/root/review`, a separate execution context. Reviewed local HEAD `770b94a` (evidence commit), version-probe backport `aa258a6a80da7642c063ef2f2e014d9439508338`, and merge `57f4665bd8c8925621049586609f54e2d8aa1f95`. This is registry integration review, not full-product acceptance or a native registry test claim.

## Source and merge preservation

`57f4665` has parents registry `4e1efd378e03cbfeddf11351299da88e911df85c` and foundation `222aafd1089701946fc77e477764ab8ee4bc367c`. Registry implementation is preserved. Relative to the previously reviewed registry source, the product changes are the reviewed foundation IPC fd-close/error-diagnostic and malformed-client test fixes plus the readiness transport/small-pipe regression; other changes retain OWNER-002/reference/evidence documents. No source conflict or dropped registry feature was found.

Independent manifest verification passes 92 entries. The merged manifest preserves the union of both parents' keys, with zero missing entries and zero changed nonconflicting hashes. The two differing entries are OWNER_INBOX and STATUS; their selected hashes match the actual reviewed content. The original design/specification and gate are not broadened by this integration. OWNER-002 changes UI presentation while preserving complete functional/safety scope.

GitHub independently confirms PR #3 is MERGED at `1f34b444821640bf88f32a8bb39c6434d4ca26cc`. The API reports its full tree `233bf80bb4b1f55067318a229bbcc93253a380a5`, exactly equal to local reviewed foundation `222aafd^{tree}`. Thus integrating that newer main merge commit introduces no new product/source bytes relative to the foundation already included. Commit identity and exact-candidate CI still matter: previous green results must not be substituted for fresh PR #4 checks after retargeting to main.

## Version-probe backport

Reviewed the complete three-file `aa258a6` diff. Useful stdout/stderr progress no longer incurs an unconditional 2ms sleep; true idle waits use `poll` with the original monotonic deadline and at most 2ms observation cap. Read batches remain bounded to sixteen per stream, EOF descriptors are disabled, interrupted polling returns to deadline checks, and owned-group cleanup/reaping semantics remain intact. This does not add a Provider route, credential access, model call or billing fallback.

The new regression deliberately shrinks the Linux stderr pipe to 4096 bytes while retaining exactly 4MiB output and the original 500ms timeout. It still requires Succeeded, the exact version line, exact stderr byte count and reaped child. Read the retained old-production control log: the same regression failed with TimedOut after 0.52s. Read the corrected full-suite evidence and independently executed `cargo test --locked -p agentbaton-providers --test installation`: 10 PASS, including that unchanged workload. This is real local causal evidence; it does not independently establish any historical Darwin failure's root cause.

## Verification and verdict

Audited all 69 product source hashes and all 15 recorded log hashes against actual files: MATCH. Read the real Rust log and independently summed 123 passing tests (core34/config26/providers50/workspaces13); no nested test double-counting occurs here. The UI log reports 15 mocked-transport tests. Typecheck/build, formatting, Clippy, infrastructure/design checks and the actual daemon/CLI SIGKILL/restart smoke logs all record success. These aggregate commands were primary execution with independent log/hash inspection; the ten installation tests and 92-entry manifest check were also independently executed by this reviewer.

No unresolved source-level blocking issue found for this integration. Safe to merge the unchanged-byte main commit, commit the review/evidence checkpoint, push and retarget PR #4 to main. Merge approval remains conditional on that exact final candidate's required checks and resolved review, including fresh macOS jobs; no branch protection bypass or main direct push is authorized. Native registry/macOS behavior and current-source installers are not proved by local tests or older foundation artifacts. The rest of Design v0.2 remains unfinished and authorized work continues.

## Reviewed backport fingerprints

Actual source SHA-256 at 2026-10-26T19:28:47.240768+00:00:

| File | SHA-256 |
| --- | --- |
| `crates/providers/src/installation.rs` | `c623fcb56a356977eb406c93693c30504c7aff1ece153f713d090f5e3c323d87` |
| `crates/providers/src/transport.rs` | `6b5c96580e3cbe4e8ee0e950b301d3bc8d1074dcd9c42290bfba253cb723860b` |
| `crates/providers/tests/installation.rs` | `2eeafe3f388b9cd5d1c1e63af174cd87c166e18f4909cac8f09064d00e450205` |
