# Resume / integration

Branch: codex/validation-adoption. Source commit: c856e462125d2cb546a4af4206cbe4b263bbc76b. Worktree: /workspace/.worktrees/validation-adoption. AB-START-001 scope only; no remote actions performed.

1. Read source/requirements and latest merged OWNER_INBOX, preserve primary work.
2. Integrate this worker's source commit, resolving shared Git helper overlap with focused workspace-fetch-guard fixes; never drop the later stronger guard/tests.
3. Refresh primary-owned Cargo lockfiles for approved sha2 crate dependency; worker's unstaged lock diff is not part of its source commit.
4. Re-run workspace product tests/fmt/Clippy on exact integrated SHA and complete independent review before PR acceptance.
5. Wire native-owner validation policy proposal/confirmation, durable storage and artifact_integrity_only service/UI. Server derives actual task/session/attempt/workspace/input IDs; never trust Agent policy/acceptance assertions.
6. Implement/run actual preapproved code/data correctness validators before any global task acceptance/adoption. The library's synthetic local CAS test proves mechanism and guards, not business acceptance.

Stable API: BoundArtifact::from_approved_root/from_verified_contract/contract_view; ApprovedRule::from_verified_policy(approval_id, policy_revision, subject, inputs, outputs, analysis)/contract_view; ValidationPolicyContractView::digest; validate_artifacts -> ValidationEvidence::view/verify_current/accept. AuthorizedIntegration::from_verified_grant -> prepare_candidate -> PreparedCandidate::version_binding/workspace; adopt_local_reference is local-only and checked-out target guarded. ConfirmedReleaseGrant always requires separate trusted exact-bound human confirmation.

No automatic continuation or full product completion is claimed.
