# Progress

The exact prior IPC client tests passed on Apple Silicon. Its socket worker counts the newline, but emits the original response-limit error with not_started for any oversized actor reply. Direct IPC clients can misread that error after a write. The prior test target was cloned for the new worktree using APFS copy-on-write. The previous test executables and evidence remain at their original paths.

The worker captures the method and original key presence before moving the request to the actor. Only an accepted enqueue carries potential write semantics into oversized reply encoding. The same response remains small and keeps its request ID, error code and the original-key reconciliation hint. Read overflow still asks for a smaller batch. The client keeps the legacy replacement frame handling.

Apple Silicon tests passed on 97dfeab06a0d97fd5c5c5d67ec685036819ae3ef. The real socket worker forwards generated replies through a private actor channel and proves three wire cases, including a 33,554,432-byte frame with its newline. A one-byte oversize produces an unknown write reply and the unmodified read error. All 13 IPC frame tests and six real daemon/Core integration tests passed. The previous build cache was reused via APFS clone, and both original pinned executables were reverified. Workspace formatting and exact-base path and merge checks passed.
