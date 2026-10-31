# Checkpoint

Worktree: `/workspace/.worktrees/opaque-legacy-schema17-compat`; branch `codex/opaque-legacy-schema17-compat`; base `4f962940533241fdec74b96a87c01a99788a62ee`. No PR created or remote action taken. All work confined to integration_store.rs, integration_schema17.rs and this evidence directory.

Next integration steps: independently review known-object proof and opaque classification; run `cargo test -p agentbaton-core --test integration_schema17` and the unchanged `--test command_validation_migration`, then the ordinary migration consumers and genuine14/15/16 suites on the resulting exact source. Preserve baseline failed log and bind new results to the integrated commit. Root alone schedules heavy execution. New tests must not be called passing before actual execution.

Conservative compatibility deliberately excludes opaque FKs (even opaque-to-opaque), generated columns, partial/expression indexes, triggers, views, virtual/shadow tables and reserved names. No schema17→18 or authority evolution occurs. Missing Provider/native/signing validations stay separate from storage evidence.

Review follow-up: inspect ordinary_main_table's direct PRAGMA implementation, then run the new opaque_named_pragma_table_list_cannot_forge_virtual_or_shadow_table_metadata at both versions and the expanded positive. Runtime results are not inferred from rustfmt.
