# Core IPC write acknowledgement

AB-DOM-005. Use reserved worktree core-ipc-write-acknowledgement and designated integration parent ca6fcf4be82c1c07a367bf4ca4a3977bbb1f919b. The original reservation starts at 70bcc2aeada84e454a128a4ef38db7251cc9b1fd. Both revisions have identical IPC production and test bytes.

Tie definite daemon acknowledgement to the generated request ID and a valid envelope. Classify unavailable, incomplete, oversized and invalid post-send write replies conservatively, keeping the physical frame limit and genuine daemon rejections. Preserve Session read-only receipt recovery and avoid any automatic resend. Verify against a genuine local Unix daemon and SQLite using a bounded relay that only alters a reply after the daemon answered.
