# Progress

Implemented standalone child for the existing TaskCodeWorkspace surface: explicit bilingual source/isolated choice, one in-flight, parent/selection epoch guards, same-document uncertainty retention, and no automatic handoff/retry. A later explicit refusal preserves prior unresolved handoff uncertainty. Choices are recorded metadata; Native always reads fresh Core context itself.

Native uses Core strict DTOs plus the agreed public pin_open_context helper, with no new Cargo dependencies or unsafe FFI. It restricts the real calling main window/origin, uses original Owner ipc::call only, rechecks directory pins under the existing caller lifecycle guard, and invokes fixed /usr/bin/open with separate -a/Terminal.app/absolute-directory argv and cleared child environment. Only opener exit0 before the single5s deadline is accepted. Error/nonzero/timeout after spawn is unknown; an unreaped owned opener is retained in one private slot, blocking another spawn until a later explicit call observes it reaped. No background reaper or automatic retry; Terminal is never signalled.

Root hooks are a separate private patch /tmp/workspace-open-desktop-hooks-e874.patch, not edits to shared files. Added API/wire/component/native pure negative test source; runtime execution remains Root-owned.

Core interface bound to candidate5d7ac35fba06efedc07bc2c3ef4d386e31245795; no Core source or dependency changed here.
