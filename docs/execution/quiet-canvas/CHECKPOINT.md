# Quiet Canvas checkpoint

Source frozen at `88b0a4d5f1164740f04a5bc11f894a3ba1a87aaf` on `codex/quiet-canvas`, based on `14e8690` (compatible session source plus protected main `2d9361d` OWNER-002). No core/native/root/Node lock changes. 35 mocked-transport UI tests and 22 real browser/UnixIPC checks pass. Eight screenshots are preserved under `screenshots/`.

The reviewer owns REVIEW.md. Merge requires independent source review and exact protected PR checks. The primary must push this dedicated UI branch, create its PR, preserve later runtime changes during integration and execute combined CI/native GUI tests. Native macOS GUI acceptance is separate from this browser run.

The remaining full design, current-source macOS packages, artifact/standalone-chat/router APIs and real Provider authorization are explicit unfinished scope. This checkpoint does not end the overall task or promise continuation without a live executor.
