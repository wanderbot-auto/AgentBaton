# Progress

Implemented standalone child for the existing TaskCodeWorkspace surface: explicit bilingual source/isolated choice, one in-flight, parent/selection epoch guards, same-document uncertainty retention, and no automatic handoff/retry. A later explicit refusal preserves prior unresolved handoff uncertainty. Choices are recorded metadata; Native always reads fresh Core context itself.

Native uses Core strict DTOs plus the agreed public pin_open_context helper, with no new Cargo dependencies or unsafe FFI. It restricts the real calling main window/origin, uses original Owner ipc::call only, rechecks directory pins under the existing caller lifecycle guard, and invokes fixed /usr/bin/open with separate -a/Terminal.app/absolute-directory argv and cleared child environment. Only opener exit0 before the single5s deadline is accepted. Error/nonzero/timeout after spawn is unknown; an unreaped owned opener is retained in one private slot, blocking another spawn until a later explicit call observes it reaped. No background reaper or automatic retry; Terminal is never signalled.

Root hooks are a separate private patch /tmp/workspace-open-desktop-hooks-e874.patch, not edits to shared files. Added API/wire/component/native pure negative test source; runtime execution remains Root-owned.

Core interface bound to candidate5d7ac35fba06efedc07bc2c3ef4d386e31245795; no Core source or dependency changed here.

## Bounded caller observation closure

Retain independent f566 CHANGES_REQUIRED: locked Wry getter blocks on an unbounded reply when URL is read directly on a worker. URL reads now execute only inside a main-thread read-only job. The worker uses a strict5s observation deadline with recv_timeout; cancellation/deadline checks revoke unexecuted and late jobs, and no callback carries handoff/Core authority. Original CallerOwner, all13 other Native functions and opener5s deadline remain byte-identical. Added4 Rust channel/fake-clock tests (NOT_RUN);8 Python source controls and rustfmt pass. Root compile and independent closure review remain required.

## Root076 mount integration tests (NOT_RUN)

Added only a new six-case real-mount component test and own docs. Source/ready directory opening remains independently Owner-validated when code preparation is refused, dirty, non-code or pending; shared TaskCodeWorkspace mount is Root-owned and unchanged here. Project/task switch cases reject old catalog/Native success and require explicit current selection. A task-remount settlement subscription gap is source-inferred only and preserved as a real test expectation pending Root reproduction; the private render-only fix proposal is not applied.

## Flight settlement notification closure (source only)

Preserve Root actualfde0429 six-case result5PASS/1FAIL, specifically the task-remount manual-refresh gate. Applied only the authorized module-local render subscription proposal; no API/Core/Native/sharedhook/test changes, no automatic read/open/retry, no identity/result migration or gate/unknown32 relaxation. Complete component inverse matchesc7 before the narrow subscription additions. Runtime validation and independent review are Root-owned and pending.

## Root selected runtime closure recorded

Root sourcefde042968891cea879f55038cf92025d0cc608ac actually ran mounted6:5PASS/1FAIL in3.210s (logSHA07f223c83d88a1fec5867b993a790de0047ad6a43991472352220d654887f0a0). That disabled-Refresh failure remains unchanged. After render-only source5d529df2fd7da7e5232f5b6ef279d960046b61cc and Root composition08723aefacd6ab7a8d8db6cba19e2521e8df51ff, the original mounted6 plus existing component/API41 all passed:47/3files in2.508s (RESULT SHA169f7c5afe7404398e4722e5625ffd260ac4280734cac4c133dce34459cdf73f, logSHA1fa418d7e9305df293c972e0b1d80a198343487b4e374a088b5e875ab9101c2d).

Independent source review5d529 is SOURCE_READY,9source/pure checks, reviewJSON SHA0c47839f68915f2aa04d28e85a77d1754546a80e904df95ed004bc89ef00ad92. It is source evidence, separate from the actual47. These tests use mocked daemon record methods/Tauri transport; no true IPC-to-Native handoff, Native Mac/Terminal/observed directory/manual smoke or full130 acceptance is established. Types/fullUI beyond the selected files and Native runtime remain separate pending gates. This worker only recorded the retained files; no Node/Cargo/SQL/process/network was run.
