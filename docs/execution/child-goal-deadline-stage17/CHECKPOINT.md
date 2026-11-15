# Root review and actual-process evidence remain pending

Worktree `/tmp/resume-rework-exact-busy17`; branch `codex/resume-rework-exact-busy17`. This diagnostic change is a separate commit after reviewed-source helper transplant `6be4d71`. Its only source paths are the existing runtime fixture, one included Python diagnostic helper and one infrastructure pure-control file. See EVIDENCE.json for exact source hashes and command.

Original Intel run `41162216164`, job `124422036740`, still reports 25 passed / 2 failed. In the one-second failure the child attempt was absent and parent had stopped waiting for an unknown child; stderr was empty. This does not identify the failed MCP stage and cannot substitute for owned Goal deadline validation. Original log SHA256 `1bc77449a04bb02eff3de01128caba671acfeed3e1ce8d1a7e0327d51cfecfdb` is preserved.

Root must independently review, format/compile and run actual current Factory cases. No Rust, Node, browser, SQL, network, CI or push was performed here. No new CI fix, owned timeout success or performance result is claimed. Parent15 UI normal/cancel failures remain separate and unchanged; their proposed diagnostic fields await Root's public-contract decision.

Correction after independent review: c8b6190's JSON-copy collector is superseded by `factory_child_goal_deadline_evidence.rs`, with strict finite projection and pinned nonblocking regular-file reads. Rustfmt was explicitly authorized and passed; Rust compilation and four added Rust pure tests remain NOT_RUN by this subagent. Latest Python pure/source controls are 8/8 PASS. Parent15 v3 fields were subsequently approved with conservative unknown/null semantics; its separate worktree has uncommitted projector-only WIP while this security correction takes priority.
