# ADR draft: record only the already-integrated PR18 squash ancestry

Status: SOURCE_READY, conditional on the exact guards below. Draft only; this reviewer has not committed this ADR, executed a marker merge, accessed the network, approved a PR, or merged protected main.

## Context and exact objects

PR19 content merge `23d7638bcc08260c644359ccff8a40b2bec4f2ad` has exactly these parents:

1. PR19 schema16 first parent `e3ae624287a76bbaaead96f45f587a58aace1c66`.
2. Approved PR18 original head `ae42fcdc55473ef1574f1844432c19395c7373a6`.

The actual content merge base is `674d0a3502776c7996a0cf2e6f00ad3e5126b14d`. PR18 was subsequently represented on protected main by squash `7ecf0c51e53de26cb443b328ad660b4b1f491e5a`, whose parent is `653679db60d5c68f7e3c5b5e146ef58aed10de23`.

The original approved PR18 head and protected squash have exactly the same complete tree:

`b6bdb713755a427827baa76533ad5391304a4d52`.

The original PR18 head is a real ancestor of PR19 through the ordinary content-bearing merge. The protected squash is not yet an ancestor. Their shared content has therefore been integrated and reviewed, while the squash's new commit identity is absent from PR19 ancestry.

PR19 content-merge tree is `7a3203d0812f3b433aa91c2efdcc7600219f619d`; it is not the main tree. During this review Root added metadata-only commit `72fc53eba8ee2bf8c3020af6947b8791f20fe1a1`, tree `e12acc4de9efb60c92795333482f4cd82c564c36`. Its delta contains eleven metadata/screenshot paths, without product/CI source changes. Acceptance of those new evidence claims belongs to their separate metadata review. The marker must preserve the immediate premarker tree, which will also include the separately committed ADR/proof; neither tree hash above substitutes for that future measurement.

## Evidence of the completed content integration

The three resolved conflicts of content merge23d were `crates/core/src/ipc.rs`, `docs/execution/STATUS.md` and `BOOTSTRAP_MANIFEST.json`. This review independently reconstructed complete IPC and STATUS bytes from actual base and parent Git blobs. IPC combines all nonconflicting edits and both end-of-file insertions, retaining TaskBinding, Goal stop-service and Owner read-queue test modules. STATUS combines both histories with the exact one-line boundary whitespace resolution; no history text is omitted. The manifest semantically differs from the approved PR18 manifest only at the STATUS hash, which matches the actual resolved STATUS bytes.

Eighteen additional actual-source checks preserve the reviewed schema16 adaptations, old history worker guards/tests, TaskBinding/resolver/UI files from the PR19 parent, and Factory UI/Goal implementation from the approved PR18 parent. The source proof reuses the independently sealed content review `/tmp/pr19-23d-independent-review.json`, SHA256 `e5aad8d0602d5cf8ec2d215a4d5cac59ebfe215809fa1b8d7d16aa2d112e9c57`. Its verdict is SOURCE_READY for merge23d. This ancestry review is not a fresh audit of every product change or certification of the latest runtime evidence.

The earlier direct-to-main add/add attempt was aborted and retained separately. It is not the completed integration and is not reused as proof of conflict resolution. The completed ordinary ae42 merge remains the content integration evidence.

## Decision

Root may create one content-preserving ancestry marker using exactly:

`git merge --no-ff -s ours 7ecf0c51e53de26cb443b328ad660b4b1f491e5a`

This is conditionally safe because the exact incoming tree is the same complete tree as an approved original head already included through a genuine reviewed merge. The marker records that squash identity as ancestry while retaining the existing PR19 result. It supplies no new implementation, authority, runtime acceptance, approval, or protected-main merge.

First commit the independently reviewed ADR and immutable before-proof in a separate ordinary commit. Suggested paths are `docs/execution/task-workspace-integration/ADR-001-safe-squash-ancestry.md` and `docs/execution/task-workspace-integration/SQUASH_ANCESTRY_BEFORE_PROOF.json`. Review any other intervening content change separately. The marker itself must introduce no file delta.

## Mandatory checks immediately before the marker

1. Root obtains a fresh authoritative observation of remote main using its existing authorized access. Main must still be exactly `7ecf0c51e53de26cb443b328ad660b4b1f491e5a`. A local `origin/main` ref alone is not freshness evidence. Reject every different main SHA, even a new commit with an equal tree. Do not generalize this decision to future main.
2. Recheck the complete tree equality of exact squash7ecf and original ae42, and verify ae42 is an ancestor of the current PR19 head. Verify current PR19 descends from reviewed merge23d and account for all intervening commits. Commit the ADR/proof separately first.
3. Record the immediate premarker commit and complete tree. The tracked worktree and index must be clean, with no in-progress merge/rebase/cherry-pick. Existing untracked dependency link `apps/desktop/node_modules` is not source evidence and must not be staged or deleted; unknown untracked path conflicts must be resolved before proceeding.
4. Any failed/missing equality, ancestry, source-review or fresh-main check stops this plan. New main content requires an ordinary content integration and independent review. Do not select an ours marker to suppress it.

## Mandatory checks immediately after the marker

1. The marker has exactly two parents: the immediate premarker PR19 commit first, exact squash7ecf second.
2. The marker's complete tree is byte-for-byte identical to the immediate premarker tree. The tracked worktree/index have no new content changes. Both ae42 and squash7ecf are now real ancestors. Attach immutable before/after object, tree and parent proof.
3. Recheck the target identity before publishing/accepting the candidate. If main advanced, stop using this exact-target decision and review/integrate the new target normally. Do not rewrite history, force-push main, or bypass protection.
4. All ordinary exact-candidate PR checks, independent review and protected-main rules still apply. A source-preserving marker does not turn pending CI, UI, native acceptance or artifact checks into passes.

## Consequences and limits

Git can thereafter recognize the already-accounted squash ancestry instead of treating the approved original series and its squash as unrelated additions. PR19's reviewed schema16 work and intentional source differences remain unchanged. This does not transplant Root17/schema17 code or claim that PR19 equals main.

The principal risk is falsely recording ancestry for unintegrated incoming content. Exact SHA/tree/real-parent-ancestry guards, complete conflict reconstruction, separate reviewed ADR/proof and before/after tree identity address that risk only for the pinned objects. Broadly applying `-s ours`, substituting another main SHA, allowing unreviewed code changes, or skipping normal protected checks is outside this decision.

This review executed only read-only Git/source inspection and wrote `/tmp` artifacts. Remote main freshness is unobserved here. Root-reported Core/history/Factory/sessions/Task/migration subsets and running Clippy are not independently rerun or declared complete by this ADR. Full130, real Provider, remote, signing and production release remain separate unfinished dimensions. No marker has been executed by this reviewer.
