# Desktop dev helper

Base: `origin/main` f6133388622f5b83bf648fc088ae63463735c593.
Branch/worktree: `collab/desktop-dev`, registered with `scripts/collab.sh`.
Scope: claimed helper scripts, desktop native Cargo/config, development docs, this evidence directory and STATUS. No product/provider/authorization changes.

Acceptance: reproduce missing target-suffixed sidecars; repair raw Tauri dev preparation; provide checkout-local start/dev/stop/restart/status/logs/setup with owned process cleanup, separate runtime/port, no name-wide kills; share only this worktree's Cargo cache and preserve runtime optimization overrides. Verify meaningful lifecycle failures and actual Apple Silicon native startup/restart/cleanup. Independent read-only review, scope/conflict checks, PR checks then protected merge to actual default branch main (owner called it master).

OWNER-001/002 read and ACK; task does not alter product scope or UI design. No Goal created: this is the owner's scoped development tooling request, not a request to start the full product goal.
