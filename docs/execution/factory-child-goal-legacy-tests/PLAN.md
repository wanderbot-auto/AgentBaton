# Factory child Goal legacy migration tests

Authorization: AB-START-001, Design v0.2. Assigned WP01/WP08/SEC/COR migration evidence only. Baseline `5491c94cffbe40292ff977e0d9cfedcb55026c9e`; current merged main `3fd60f354236ddc5ab0ea2f7665428a8496f4a64`. Latest merged OWNER_INBOX entries 001 and 002 acknowledged; no 003 observed. No authorization expansion.

Allowed files: new `crates/core/tests/factory_child_goal_legacy_migration.rs`, its new `fixtures/child_goal_legacy_migration/` subtree and these records. Core/migrations/protocol/existing fixtures/CLI/UI/CI/locks/global STATUS are excluded. Core author is sole schema writer. No remote writes.

1. Prepare fail-closed tests against real nonempty schema14 input and a bounded generator using frozen f032 tools. Use actual private authenticated Core approvals, not synthesized Native authority.
2. Obtain the coordinator's process window; generate real Native Goal scope, Graph execution approval/capability and terminal joined Graph, plus separate real Factory14 scope/run/child plan. Retain exact RPC proposal/result bytes and ordered original database rows. Capture old daemon source/hash/manifest and clean shutdown.
3. Freeze original input. Run actual schema15 Store::open integration tests in the coordinator's Cargo window. Require original raw rows, original Native proof FKs, valid foreign keys, private original14 backup, rollback, corruption refusal and future16 refusal.
4. Record separate active/unknown recovery and derived-issuer cases as outstanding until backed by actual input/execution. Review independently, fix, and return isolated commit to coordinator. No overall product completion claim.

There is no public migration-only entry point. This suite deliberately calls Store::open, which performs migration and legitimate recovery. Its immutable-history fixture requires terminal joined work and expects zero recovery changes. The pre-v15 backup proves original14 raw history separately. It does not claim an independent DDL-only execution or active-worker recovery evidence.
