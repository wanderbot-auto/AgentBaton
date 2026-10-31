# Schema15 concurrent review oversize assertion alignment

Scope: only the two `sessions_ipc` tests `actual_goal_fixed_concurrent_review_keeps_other_kinds_setup_and_corrupt_authority_refused` and `actual_graph_fixed_concurrent_review_keeps_other_kinds_setup_and_corrupt_authority_refused`. Base `0b5a88a572d70aa87ef4aab481a62ec2bfeb0af4`; isolated sparse worktree/branch `codex/schema15-concurrent-review-assertions`.

Read AGENTS, execution gate AB-START-001, launch approval, MASTER_PROMPT/GOAL/RUNBOOK and OWNER-001/002. Root remains integration/STATUS owner; no permission expansion or new owner directive is inferred. Keep production bounded reads, authority, idempotency, budgets and all fixture writes/count assertions unchanged. Preserve the original 56-pass/2-fail run. No Cargo, heavy lease, daemon, browser, Node, actual DB or remote actions.

1. Inspect original failure/code and the ordered preflight path.
2. Replace only oversize-specific assertion expectations, without broadening a shared error allowlist.
3. Prove complete test-file inverse, exact two-case location and unchanged production/locks, run standalone rustfmt/diff checks.
4. Commit source and record evidence. Independent source review and a serial real test run remain for Root; this author does not approve its own patch or claim a new runtime pass.
