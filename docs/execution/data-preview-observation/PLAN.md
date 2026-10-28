# Data preview observation contract

Baseline `de150679f7af9915f00cae58e0686a3954041dee`; branch `codex/data-preview-observation`; task `/root/artifact_contract_fixture`, this second leaf only. Parent coordinates PR14 and independent review; no accessible Cloud task URL supplied. Read AGENTS, gate, approval, MASTER_PROMPT, GOAL, RUNBOOK and OWNER_INBOX; ACK OWNER-001/002. AB-START-001 applies with all existing credential/spend/release gates unchanged.

Partial requirement contribution: AB-PRJ-002/005 (real selected non-Git isolated workspace regression), AB-REL-001 (separate historical/local/macOS evidence). Edit only `crates/core/tests/data_workspaces.rs` and these four unit records. No product change is authorized in this leaf.

1. Preserve the exact ARM CI failure and original source; inspect asynchronous I/O, deadline, cancellation and join contracts.
2. Change only the 32 MiB copy-cancellation test's preparation observation to a finite maximum 35 seconds, including 30-second product I/O and IPC/scheduling margin. Keep ordinary and subsequent copy/join waits at 5 seconds and every existing safety assertion.
3. Measure start and concurrent snapshot/preview-read responsiveness with individual 3-second test bounds, then log safe stage/elapsed/terminal/receipt fields. Do not log paths, source contents or tokens.
4. Run original local case, complete Data 21 and migration 3 regressions, Core all-targets Clippy, format and diff checks. Freeze source/logs/hashes, release target, hand off for independent review and new exact macOS CI.

Reversible engineering decision: an observation deadline belongs to this known large-fixture preparation, not Fixture.wait globally. 35 seconds is a test observation ceiling; product I/O remains 30 seconds and `verified` remains required. The new 3-second response assertions are test checks; existing ordinary client socket timeout is 5 seconds and server frame socket timeout is 3 seconds. Preview has no io_completion field; its null diagnostic cannot be interpreted as a copy receipt. Copy terminal still must show worker_joined with all cancelled/ready race assertions.
