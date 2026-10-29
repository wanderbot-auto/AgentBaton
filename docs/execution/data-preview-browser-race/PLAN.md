# Data preview UI cancellation observation

Base79d73e3e56e07a3c96fbfb0962415fda5ee8e05f; gate AB-START-001 and OWNER-001/002 acknowledged. This test-only regression supports the existing full Designv0.2 requirements; it does not reduce their acceptance scope.

Remove only the extra manual preview-cancel refresh after the existing actual start-ACK refresh. Go directly to the exact record and real cancel button. Add same-process Python monotonic handler/IPC/click-call boundaries to distinguish observations in the next run. Do not treat time measurements as authority, dispatch arrival or worker-stop evidence.

Allowed: data-workspace-real-ipc.py, dedicated pure driver test and these four records. No production component/Core/Native/CI/lock changes, test hook, budget/fixture size increase, retry, new key, Provider identity or paid resource. Root runs actual product only after independent review.
