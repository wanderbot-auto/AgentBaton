# HEXP-001 delivery record

Base: `b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c`. Branch: `collab/history-export`. PR target: `main`.
Product source: `7cc8ed418a8d3ff17f25bc35ee3aa885858fb93e`; guard/ADR candidate: `1aabbc8900206fb32e547ea8818b94749806f827`.

Project details now provides bilingual history-category/sequence selection, frozen preview, publication, original-intent/result reconciliation and restricted Finder retrieval. Core captures existing SQLite metadata and attributable normalized journal history; the JSON history package contains a versioned manifest, exact logical-file SHA-256 values, selection digest and redaction/exclusion report. It is not a restorable database backup.

The renderer persists a UUID-v4 intent and exact selection before IPC. Existing intent lookup precedes capacity checks and current-history capture. Lost preview replies, restart and publication replies reconcile the same frozen result. Changed payloads conflict. Original deadlines remain in force, failed/corrupt partials are retained, files are published without overwriting, and filesystem workers are serialized and joined. Native commands receive typed selections/references, never renderer paths or programs.

Boundaries: 10,000 scanned rows, 4 MiB captured input, 256 KiB per record, 8 MiB package and a bounded capture deadline. The private directory reserves at most 32 intents; normally 64 names, at most 96 generated names if interrupted unlink evidence remains. Existing results remain recoverable at capacity. Metadata is current at the snapshot; sequence ranges apply to journal bodies/events. Unattributable events, raw protocol, tool arguments/outputs, configurations, identities, external file contents and recovery/idempotency records are excluded. Known markers are redacted; arbitrary free text is not guaranteed secret-free.

| Dimension | Result |
| --- | --- |
| Core targeted | 16 passed |
| New authenticated daemon integration | 1 passed |
| Root workspace clippy, all targets, `-D warnings` | passed |
| Frontend typecheck/build and full Vitest | 111 files, 1548 tests passed; mocked transport |
| Infrastructure including new bridge negative controls | 884 passed; original bridge SHA pin unchanged |
| Real daemon Unix IPC / SQLite / filesystem | passed; synthetic retained history, no Provider calls |
| Apple Silicon development App build | passed; arm64, ad-hoc linker signature, no TeamIdentifier |
| Native host tests | 179 passed, 2 original ignored |
| Native WebView / Finder interaction | blocked/not_run: computer-use access to AgentBaton rejected |
| Extra native all-target clippy | candidate failed with 14 errors in unchanged files; source-equivalence only, base command not reexecuted |
| Final frozen full workspace Rust | failed in unchanged Provider installation probe: `invalid` returned TimedOut rather than InvalidOutput; original 500 ms budget unchanged |
| Installation focus rerun | 10 passed; focus only, original full-run failure remains |
| Remaining Providers/Workspaces package/doc suites | passed in a separately recorded invocation; original full-run failure remains |

The latest real IPC package is `artifacts/history-export/intent-ipc-round1/sample.history.json`, SHA-256 `27e573cb094133f522bbd82788ba076702c2cc6c88a8620c015781f3f853a241` (2709 bytes). Its report verifies internal hashes, redaction, cross-project exclusion, lost preview reply plus daemon restart, selection conflict, lost publication reply, no-overwrite retry and unchanged business-history hashes. Earlier `f339…` evidence remains untouched.

The actual development App was started in a private fixture runtime. `cua.getApp` returned `Computer Use was not approved to use AgentBaton`; no UI actions or GUI success are claimed. The owned App/daemon were stopped and joined. See `NATIVE_GUI.json`. The owner reported allowing the App, but a second same-entry attempt against the same built path was still denied. Both isolated App/daemon pairs were stopped and joined; no alternative automation bypass was attempted.

Raw failures, environmental fixes, source hashes and exact commands/results are retained in `EVIDENCE.json`, the accompanying logs and `NATIVE_CLIPPY_BASELINE.json`. The Python dependency installation used the repository's pinned requirements, an explicit vendor CA bundle and verified TLS; no global trust settings were changed. The only scope extension was the leader-authorized exact inverse addition in the original source guard; no existing pin or assertion was relaxed.

Independent read-only reviews closed the implementation findings. Leader acceptance, the exact integration candidate and GitHub's required checks remain separate. This delivers the export slice of AB-SEC-008; deletion, automatic cleanup, real Provider calls, physical disk exhaustion/power-loss testing and production release are outside this verification result.
