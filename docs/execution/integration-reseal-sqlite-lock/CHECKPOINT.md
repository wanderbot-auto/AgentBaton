# Source handoff

Base: `84c530459fa165ad14f36bac0ace01eb9462f309`.
Source chain: `40a1a8d`, `317eedd`, final `dcc1a39871730fbce2cc7f711934481f5369b866` on `codex/integration-reseal-sqlite-lock`.

Allowed source changes are only a cfg(test) module declaration in integration_reads.rs and the new integration_reads_reseal_lock_tests.rs. No original read/reseal implementation changed. Root is the sole heavy executor and must independently review and run the exact compiled source.

Suggested focused command: `cargo test -p agentbaton-core --lib integration::reads::reseal_lock_tests -- --test-threads=1` using the existing bounded evidence runner/toolchain. Expected new top-level declarations: 3, comprising 2 real tests plus 1 inert child helper; each actual child runs exactly that helper in a separate process. Do not treat the helper count as a third independent lock experiment.

Linux and macOS are source-enabled via cfg. Both actual platform results remain NOT_RUN at this source checkpoint. The rollback control establishes a VFS/POSIX hazard for DELETE only; it does not prove a production WAL writer was previously unlocked. Full integration correctness requires separate actual IPC/Actor/policy/clock/fees/cancel tests.
