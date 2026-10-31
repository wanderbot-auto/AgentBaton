# Checkpoint

New modules only; no App/config/notifications/protocol/migration/lock/workflow/shared hook edits. Native requires Root to export WorkspaceOpenSelection/Context/pin_open_context from Core, register one main-window command, and mount the child in TaskCodeWorkspace using the supplied narrow patch.

Native caller checks reuse existing native_parent lifecycle without opening an approval dialog or carrying consent to an external tool. Directory pins are observation-time proof, not pathname lifetime immutability or an OS sandbox. Opener exit0 means OS handoff accepted, not a measured Terminal cwd/window, managed session, Agent start or task success.

NOT_RUN: Node/type/UI tests; Cargo/native compile/tests; actual Core/SQLite; OS opener/Terminal; browser; real Mac directory observation; network/CI. Root must run focused then full checks and fresh end-to-end/no-write evidence. Preserve all earlier failures as historical evidence.
