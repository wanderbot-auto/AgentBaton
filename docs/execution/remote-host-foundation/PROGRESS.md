# Progress

- Contract checked in precreated worktree; clean base, scope OK. macOS ARM64; rustc 1.91.1; OpenSSH_10.3p1.
- Required execution/parallel/product documents read. No change to main checkout.
- Initial cargo check failed: rusqlite 0.38 rejects u64/usize SQL bindings. Corrected persistent numeric representation to validated nonnegative i64 (SQLite INTEGER), preserving protocol bounds. No tests yet.

## Interface and first real chain

- ADR-001 freezes private stdio v1, strict OpenSSH registration, provisioned grant authority, immutable identity/mapping, finite budget/lease and typed record capability. No root/Core/UI changes.
- Real Apple Silicon pipe↔companion↔owned worker↔filesystem and SQLite reopen verified. First eight Rust tests passed after fixing private fixture permissions and unbuffered fd handling (failed evidence retained).
- Independent read-only reviewer remote_review found eight issues; fixes implemented for dispatch grant adoption, lock-time wall sampling, SSH Host binding, root dev/ino and private journals, drain deadline, reconciliation invalidation, idle handling and truthful spawn-failure evidence. Regression suite expanded.
- Second explicit verifier: 13 Rust tests and 12 actual subprocess fault tests passed; strict clippy failed on one bool assertion style (preserved run 20261117T145229Z-238823000). Corrected assertion without changing acceptance.
- Added post-filesystem journal failure, parallel worker race and future/corrupt journal tests. No real Host login, Provider call, private auth-directory read, global state write or scope expansion.

## Fault verification and independent review complete

- 16 Rust integration tests and 16 Python real-child fault tests pass; fmt, locked explicit build and strict all-target clippy pass on macOS ARM64.
- Verified post-effect SQLite write rejection: file exists once, job outcome_unknown + actual nonzero wait, replay does not redispatch. Verified whole process-group crash retains unknown stop, disconnect/reopen/cancel, clock rollback, old/new fence deadline, six parallel workers, half/oversize/timeout frames, stale binding, future/corrupt journal, root replacement and symlink output protection.
- Review follow-ups fixed stale epoch db_pause, CLOEXEC, bounded known_hosts read and future-version-before-write pragmas; future-WAL test covers both journals. Reviewer confirms no outstanding listed blocker (source review only).
- All historical failed runs remain in runs/. Run 20261117T145903Z-659748000 failed compilation and ran a prior binary; it is explicitly failed, not acceptance evidence. Verifier now refuses stale binary after build failure. Idempotency payload regression and earlier clippy/fixture failures are preserved without weakening assertions.
- Final verifier includes sanitized actual pipe send/response records, actual supervisor PIDs/exit/no-survivor checks and synthetic file hashes. Credentials and record bodies are excluded from outbound trace.
- No new remote resource/login, Provider call, native global configuration, license choice or production signing/publishing performed. All code is the bounded record foundation; not a full remote product.

## Committed implementation validation

- Implementation commit 6df13c36b5bbf82c608ff104588f0cd98e238121 tested from a clean worktree. Final run 20261117T150538Z-544397000 passes all five checks (16 Rust +16 actual subprocess tests). Source hash unchanged from prior successful run.
- Fetched origin/main=b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c; scope and conflict preview pass. No target sync/merge required.
- Task-local .gitattributes preserves raw command logs (including original trailing newlines and hashes) while source/doc whitespace checks remain enabled. No test or CI rule weakened.
- Build/test slot released to PR29 A/B window after exact-head verification. Remaining work is evidence-only commit, normal push and draft PR.

- Normal push of implementation succeeded; draft PR created: https://github.com/wanderbot-auto/AgentBaton/pull/30. Evidence-only descendant follows, with no rewrite/merge of main.

## Leader review P2: control capacity at new-effect admission boundary

- Leader identified a reachable low-frequency case: repeated same-lease pause/local higher-fence provisioning history reaches 9903 events; Record commits event 9904, then first Cancel/Revoke incorrectly uses the same 96-slot admission gate and rolls back. This is not a short ordinary-RPC exhaustion claim.
- Added a directional regression with equivalent legal 9903-event history/current trusted fence (no deletion), both first-control orders, duplicate keys, unchanged 32 jobs/4096 receipts limits, actual worker process + wait and capacity-safe reopen. Implementation correction in progress; build/test pending PR29 diagnostic slot.
- Receipt-capacity first-control natural exhaustion is not established by this evidence and no receipt-limit expansion is proposed.

- Capacity regression red reproduced journal_capacity on 4a2 product source. Final debt accounting pays terminal and wait independently, retains both first controls and required reopen, and checks hard cap on all terminal writes. Already-durable controls and unchanged paused opens no longer consume events; receipts remain bounded and payload conflicts unchanged.
- 32 actual worker terminal-before-wait boundary regression added after reviewer found conservative double counting. 18 Rust +16 actual subprocess tests, fmt/clippy/build all pass in run20261117T152826Z-576186000. Intermediate failed run retained; rollback fault now requires actual running precondition without loosening expected stopped_without_effect.
- Final targeted independent read-only capacity review confirms no blocker. New product commit and clean-commit verification follow; no changes outside original scope.
