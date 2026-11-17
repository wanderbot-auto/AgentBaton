# RHOST-001 plan

Base b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c; branch collab/remote-host-foundation; PR main.
ACK OWNER-001/002 as merged; task-card OWNER-003/004 supplements: startup deferred, leader integration only.
Write scope: crates/remote-host, scripts/verify-remote-host.py, tests/infrastructure/test_remote_host.py, this directory. Global STATUS belongs to leader.

1. Freeze v1 ADR: bounded typed stdio protocol; strict system SSH; trusted local grant provisioning; fixed binding and directory mapping.
2. Implement isolated Rust workspace, durable transactional journal/receipts/budget/fencing, recoverable bounded worker and control adapter.
3. Exercise real subprocess framing, filesystem effects, disconnect/reopen, cancellation, identity/auth/time/idempotency/journal failures on Apple Silicon.
4. Run fmt/locked tests/clippy plus explicit verifier; independent read-only review, fix findings, commit/push/draft PR; scope and target conflict preview.

No real remote/Provider/private auth directories/paid resources/signing. Native tool subagents available, use read-only review after implementation; no nested paid calls. Root CI does not discover this workspace. Linux/cross-machine SSH/Core/UI remain separate.
