# Current session portability checkpoint

## CI portability follow-up

Tested source `e261740f9e102142f3be47219dc495d26909a8b1` minimally adds explicit blocking reset before socket-worker first-frame reads and a bounded delayed-frame regression. Only fresh test-owned policy/registry managed roots get explicit Builder0700 permissions; imported sources, production root guards and all original negative assertions remain unchanged. No schema5/native-provider or primary-tree source copied.

The old-code controls failed101 as expected: the new delayed-frame test hits BrokenPipe; under022 all13 policy tests fail on private managed-root checks. These original failures remain in artifacts/session-portability. The corrected source passes under022:192 top-level Rust tests(core69/config45/providers58/workspaces18/daemon signal2; nested child not double-counted),35 UI tests, typecheck/build/fmt/Clippy, actual daemon/CLI SIGKILL restart, design and28 Python/4 Node infrastructure tests. Prior80e evidence is archived verbatim as EVIDENCE.history-bounds-80e.json. Current Mac/native/remote CI is not run and the historical ARM CI root cause is not claimed proven. No push/main merge or live vendor/account/model calls.

## Preserved prior history checkpoint

# Current session history resource checkpoint

Tested source695144b on isolated codex/session-supervision.4MiB raw journal pages are limited before fetch/decode;8MiB UTF-8 JSON/500-record visible history window cannot accumulate indefinitely. All persistent records retained, oversized first record not skipped; explicit page/window notices and history-only resource error handling preserve task controls.

191 top-level Rust tests(core68/config45/providers58/workspaces18/daemon signal2; nested child not double-counted),35 UI/helper tests, typecheck/build/fmt/Clippy, actual daemon/CLI restart, design/infrastructure28 and integrity92 pass. Full old-UI cursor navigation remains a separate UI7 unit; no current native/macOS/remote CI or full product acceptance. No push/main merge or real model/account activity. Parent next independently reviews this resource fix before retarget/push.

## Preserved previous8302 integration checkpoint

# Session PR6 integration checkpoint

Branch codex/session-supervision in isolated /workspace/.worktrees/session-integration; no primary-tree source modified. Fast-forward7a6a46b audit, prerequisite mergesa159aed(registrydfe3)/8f94696(a429docs), tested recipient fixbc47ea1. Protected main1f34b44 includes reviewed foundationPR3; registryPR4/sessionPR6 remain independent gated increments. OWNER001/002 ACK; QuietCanvas implementation stays dedicatedUI branch.

Schema4/session/actor/signal/native-owner-confirmation code retained.189 top-level Rust tests,28 UI mocks/typecheck/build, fmt/Clippy, actual daemon/CLI SIGKILL restart, design/infrastructure28 and integrity92 pass. Prior7 Linux host tests are historical; no currentcandidate Mac/native/remote CI claimed. Candidate awaits independentreview; parent handles push/retarget and exact required checks. Full product remains incomplete and all remaining requirements persist.

No real vendor/account/model, production, signing or added cost action. Preserve primary schema5/runtime work separately; never replace it with this older integrated branch. See EVIDENCE/INTEGRATION for exact logs, hashes and conflict decisions.
