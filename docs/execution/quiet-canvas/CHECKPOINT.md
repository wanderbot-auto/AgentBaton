# Quiet Canvas checkpoint

Source frozen at `d4672ce4e3aa8256a4bce2f974e6cc4af1b6472e` on `codex/quiet-canvas`, based on `14e8690` (compatible session source plus protected main `2d9361d` OWNER-002). No core/native/root/Node lock changes. 39 mocked-transport UI tests and 22 real browser/UnixIPC checks pass. Eight screenshots are preserved under `screenshots/`.

The reviewer owns REVIEW.md. Merge requires independent source review and exact protected PR checks. The primary must push this dedicated UI branch, create its PR, preserve later runtime changes during integration and execute combined CI/native GUI tests. Native macOS GUI acceptance is separate from this browser run.

The remaining full design, current-source macOS packages, artifact/standalone-chat/router APIs and real Provider authorization are explicit unfinished scope. This checkpoint does not end the overall task or promise continuation without a live executor.

Pagination update: preserve prior evidence in EVIDENCE-pre-pagination.json, obtain independent diff review and let the primary push/update UI PR#7. This branch has not changed core/native/locks or imported later vendor-route changes. Combined backend byte-budget validation is separate from the22ordinarybrowserchecks.
