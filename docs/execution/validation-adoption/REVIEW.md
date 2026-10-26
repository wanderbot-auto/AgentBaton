# Independent review · validation and local adoption

Reviewer: native `/root/review`, separate from implementation. Reviewed source commit `c856e462125d2cb546a4af4206cbe4b263bbc76b`, documentation checkpoint `347ac43`, based on `636dbdd`. Authority is AB-START-001; no Provider account/model, external system, paid API, production release, signing key, push or main operation was used by this review.

## Verdict and integration conditions

No unresolved source-level blocker found in the new validation/adoption primitives after the corrections below. The slice is suitable for integration into the authorized feature branch, followed by exact integrated-source tests and review. It is a partial library implementation, not global task acceptance, a complete validation engine, remote Git delivery, or complete product acceptance.

The shared Git helper overlaps the separately reviewed focused guard. During integration retain the later `b11218506e835554b2d08600e8cc1ab25e4fe7e9` shared include-indirection refusal and its tests; do not replace it with this earlier library's legacy helper state. This new adoption path itself audits included/conditional configuration in its pinned common-directory context and rejects indirection before mutation. The caller must refresh and commit root/desktop lockfiles for the new sha2 dependency edge, preserve existing stronger index/lease/stop safety checks, and rerun on the actual final candidate. No permission to skip required CI or branch protection is inferred.

## Findings and corrections

1. **Opened artifact root identity.** The earlier code checked a path stat and subsequently opened the root again without comparing the descriptor's identity. It now verifies the actual opened root descriptor against the approved device/inode before traversal, and rechecks the path root at completion. The held-wrong-descriptor regression rejects the substituted root before reading an artifact. This was a static finding modeled by that targeted regression; this review does not claim a timed old-version race exploit was executed.
2. **Repository identity and selectors.** The integration token previously retained only working-root identity and lost the approved common Git directory. It now pins the common directory's canonical path and device/inode, verifies both identities, and explicitly selects the pinned Git directory for object preparation, mutation, head/worktree checks and observed results. The two-repository regression includes the candidate object in repository B, so rejection cannot accidentally be explained by a missing object; neither repository's target nor source index changes. Replacement metadata at the same path also rejects. The original defect was identified statically; the fixed negative tests were executed independently.
3. **Implicit remote-helper execution.** A generated local direct-Git control with a missing promisor blob executed a repository-configured local `ext::` helper before worktree checkout failed. No network, key or real credential was used; the helper wrote only a temporary marker. This control reproduced Git semantics under the relevant safe flags, not a run of an old compiled Baton API. Forced no-lazy-fetch and explicit partial/promisor rejection now prevent that entry point. The fixed library import/isolation tests pass without executing the helper or changing the source/index.
4. **Replacement objects and grafts.** A direct generated-Git control showed approved HEAD A and a clean tree could expose replacement B's bytes/tree. The helper now selects raw objects and raw parent ancestry with `GIT_NO_REPLACE_OBJECTS=1` and an empty graft source. Actual negative tests preserve user replace refs, graft contents, index and source bytes while returning the approved raw content.
5. **Conditional configuration context.** A linked-root config query and an explicitly selected common Git directory can see different includeIf configuration. This was independently observed as query status1/no merge key versus status0/visible generated driver; that control did not execute the driver. The new adoption path checks unsafe config through the exact bound command context and refuses all include/includeIf indirection, filters, merge-driver and partial/promisor settings before effects. The conditional fixture proves the visibility difference and fail-closed rejection. The later shared guard separately covers a future checkout's changed Git-directory context.
6. **Complete approved artifact contract.** ApprovedRule now binds canonical roots and identity, relative paths, required input hashes, output expectations, byte limits, immutable subject and analysis metadata. Same-ID substitutions of root/path/hash/limit/scope reject. Serialized proposal views do not confer authority; private wrappers have no Deserialize path. The proposal digest includes the complete artifact/subject/analysis fields and unambiguous length/count prefixes. Only trusted host policy may construct these wrappers.

The retained independent control observations are transcribed at `/workspace/AgentBaton/artifacts/workspace-fetch-guard/reviewer-original-observations.json`, SHA-256 `f7f884d2e910aeacb1dcaac14590150dd635e836e215d3583e2ba66bd61f9885`. This is explicitly a later recording of actual tool outputs, not an original raw execution log or a rerun. Original TemporaryDirectory paths were removed automatically and were not fabricated.

## Independent checks

Executed against the actual frozen source and the separately fingerprinted, primary-owned working lockfile:

- `cargo test --locked -p agentbaton-workspaces`: **40 PASS**, 0 failures/skips (2 unit +3 Git security +18 existing safety +17 validation/adoption tests).
- `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`: PASS.
- `cargo fmt --manifest-path crates/workspaces/Cargo.toml -- --check`: PASS.

These tests perform real generated local Git/filesystem operations: isolated merge-tree/commit-tree candidates; genuine conflicts; dirty and checked-out target rejection; current artifact drift; source/index/config preservation; exact-head competing CAS; stale/common-directory identity; hardlink/symlink/oversize refusal; reproducibility fields and production subject/expiry comparisons. They do not simulate remote success or run arbitrary project validation scripts. Independently verified all six recorded source hashes and the actual test lockfile hash: MATCH.

The source commit deliberately excludes the primary-owned lockfile update. The successful `--locked` command therefore proves the recorded source-plus-working-lock snapshot, not that a clean checkout of this source commit alone already contains the dependency refresh.

## Limits that must remain explicit

`builtin.artifact-integrity.v1` proves the declared file/integrity and reproducibility-metadata checks only. It does not prove code correctness, numerical validity, successful tests, a complete effective environment, or an overall accepted task. Missing output hashes may be valid for newly generated files but do not imply semantic correctness. Core/UI must record an integrity-only result and run the separately approved actual correctness validators before business acceptance/adoption. The new WorkspaceArtifacts binding cannot be used to drive Git adoption.

The host must resolve real actors, grants, registered roots, host/project scope and current policy; labels/DTOs from an Agent or renderer are not authority. Own writers must be fenced and current evidence rechecked at integration. Same-user uncoordinated directory/ref/config mutation remains a reconciliation boundary; worktrees and these checks are not an OS sandbox or a lock against external Git clients. Checked-out targets are rejected when observed, but a stronger atomic external-client exclusion is not claimed.

Only a local unchecked-out branch CAS is implemented. Failed preparation may leave isolated diagnostic objects/worktree state and must not be blindly replayed or hidden by deleting unknown files. Development delivery remains a typed not-dispatched intent; production requires separate exact artifact/commit/environment/expiry confirmation. No remote push/PR/deploy, daemon/IPC/UI integration, native macOS validation or full acceptance workflow has been verified by this slice.

## Reviewed fingerprints

Actual files at 2026-10-26T21:14:32.865413+00:00:

| File | SHA-256 |
| --- | --- |
| `crates/workspaces/Cargo.toml` | `dfeae56a6be3a7d2d0c44de528b9234753ac7f46af786055325eca2c8c91b884` |
| `crates/workspaces/src/lib.rs` | `331ff269ff91a208964ebd49a3049e9785ccba1b79caae178640eba40c8badc2` |
| `crates/workspaces/src/validation.rs` | `a278c52004c6bc2e90c16f112ac5d3c57c3f1caf9e9d909abbe699893cf715de` |
| `crates/workspaces/src/adoption.rs` | `be970ed01ee18bebe9b06e62087d0bd7c52e97bfca6db0dcaec60cbedeba9472` |
| `crates/workspaces/tests/lazy_fetch.rs` | `e514f8ecd8e1393ad8edd897b74dc11d39dadf2c76627412ab86a873e45e24d0` |
| `crates/workspaces/tests/validation_adoption.rs` | `8a4c7782a867fc5b9caa89a0d3d1200fe225faeb46ee4729c74b00ea6fb4ff25` |
| `Cargo.lock` (tested working delta, not in source commit) | `5996b785447afaafade8c6cbb63daae5c68fd459d831d520188286bcb5abf416` |
