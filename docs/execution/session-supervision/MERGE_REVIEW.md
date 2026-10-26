# Independent protected-main session merge review

Reviewer: native `/root/integration_review`, independent execution context. Candidate `b39ae1e` contains tested merge `60715b9137e89ed20659094c9308e52b06b94956` of session `7124256326d94efb55e184613fa93865316aecc9` and protected main `dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7`. Reviewed at 2026-10-26T20:45:07.479728+00:00. Only this new review document was written; no sources, locks, previous reviews, commits, pushes or merges were changed.

Verdict: no unresolved blocker in this merge. The complete session actor and both delayed-frame regressions are preserved. Source diff from 7124256 is limited to `crates/core/src/ipc.rs`: the approved blocking/three-second IO setup is consolidated into `configure_accepted_stream`, called by the existing worker, and the incoming authenticated first-frame regression is added alongside the prior worker test. Failed setup closes only that client worker. The listener, actor scheduling, authentication, MAX_FRAME, fixed deadlines, session cancellation and unknown-outcome fencing remain intact. Incoming timeout-configuration errors now fail closed rather than being ignored.

Verified protected-main ancestry. Both manifest parents have the identical 92 paths and equal hashes except STATUS; current bootstrap verification passes. The two archived STATUS blobs match their respective parents byte-for-byte. Existing INTEGRATION_REVIEW.md and PORTABILITY_REVIEW.md exactly match 7124256 and their recorded hashes. No session/history source was overwritten with older registry code, and no native-route/schema5/UI feature was imported by the merge.

## Independently executed checks

At the session worktree, under `umask 022`, with configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2:

| Command | Actual result |
| --- | --- |
| `cargo test -p agentbaton-core --locked --lib frame_tests` | PASS: 3, including original oversized-parser and both worker/authenticated delayed-first-frame tests. |
| `cargo test -p agentbaton-core --locked --test ipc --test sessions_ipc` | PASS: IPC 5 + sessions 13, covering actual owned generated children, cancellation responsiveness, restart/unknown fencing, recipient grants and failure paths. |
| `python3 scripts/verify_bootstrap_manifest.py` | PASS: 92 paths. |
| `git diff --check` | PASS. |

Independent targeted total: **21 tests**. No real vendor executable, account/model request, production action, private credentials, signing or additional paid resource was used.

## Evidence verification

All **67 current source fingerprints** and **17 log/evidence fingerprints** in EVIDENCE.json match actual bytes. The actual full Rust log has 194 passing result-line tests including the nested FIFO child, hence **193 top-level tests** (core 70/config 45/providers 58/workspaces 18/daemon signals 2). The actual UI log reports 35 tests. Full local fmt/clippy/build/restart/design/infrastructure/manifest commands are recorded with exit 0; these results were read and hash-verified, not independently rerun in full here. Previous portability evidence is preserved with its exact recorded hash.

Current EVIDENCE.json SHA-256: `a71acd46efffb488899c47feb12b427e2880ff5d620d37e80272f19782279d43`. Merged IPC source SHA-256: `b2f8de10fffd5f67b1380002fcae874a99e2363e1c08282077d58b7768f3fa51`.

Fresh exact-candidate remote/required CI and native macOS remain separate. Protected main PR4's observed CI/artifacts are not evidence that this merged session actor passed native UI or lifecycle tests. Full Design v0.2 remains incomplete. This review supports the integrator's next feature-branch push/retarget and CI validation; it does not authorize bypassing protected checks.
