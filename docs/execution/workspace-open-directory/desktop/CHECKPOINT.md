# Checkpoint

New modules only; no App/config/notifications/protocol/migration/lock/workflow/shared hook edits. Native requires Root to export WorkspaceOpenSelection/Context/pin_open_context from Core, register one main-window command, and mount the child in TaskCodeWorkspace using the supplied narrow patch.

Native caller checks reuse existing native_parent lifecycle without opening an approval dialog or carrying consent to an external tool. Directory pins are observation-time proof, not pathname lifetime immutability or an OS sandbox. Opener exit0 means OS handoff accepted, not a measured Terminal cwd/window, managed session, Agent start or task success.

NOT_RUN: Node/type/UI tests; Cargo/native compile/tests; actual Core/SQLite; OS opener/Terminal; browser; real Mac directory observation; network/CI. Root must run focused then full checks and fresh end-to-end/no-write evidence. Preserve all earlier failures as historical evidence.

## Bounded caller observation closure

Retain independent f566 CHANGES_REQUIRED: locked Wry getter blocks on an unbounded reply when URL is read directly on a worker. URL reads now execute only inside a main-thread read-only job. The worker uses a strict5s observation deadline with recv_timeout; cancellation/deadline checks revoke unexecuted and late jobs, and no callback carries handoff/Core authority. Original CallerOwner, all13 other Native functions and opener5s deadline remain byte-identical. Added4 Rust channel/fake-clock tests (NOT_RUN);8 Python source controls and rustfmt pass. Root compile and independent closure review remain required.
