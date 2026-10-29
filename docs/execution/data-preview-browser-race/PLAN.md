# Data preview UI cancellation observation

Base79d73e3e56e07a3c96fbfb0962415fda5ee8e05f; gate AB-START-001 and OWNER-001/002 acknowledged. This test-only regression supports the existing full Designv0.2 requirements; it does not reduce their acceptance scope.

Remove only the extra manual preview-cancel refresh after the existing actual start-ACK refresh. Go directly to the exact record and real cancel button. Add same-process Python monotonic handler/IPC/click-call boundaries to distinguish observations in the next run. Do not treat time measurements as authority, dispatch arrival or worker-stop evidence.

Allowed: data-workspace-real-ipc.py, dedicated pure driver test and these four records. No production component/Core/Native/CI/lock changes, test hook, budget/fixture size increase, retry, new key, Provider identity or paid resource. Root runs actual product only after independent review.

## IPC abort phase follow-up — 2026-10-29T18:52:11.254853+00:00

Follow-up: independently reported IPC BrokenPipe/ConnectionReset must retain the same timing as ordinary IPC failure and must not be classified as HTTP client abort. Limit the change to observational phase markers and two distinct regression tests; retain original no-response behavior, single IPC call and all budgets/authority/outcome semantics.
