# Progress

Baseline `9b695892a1814dc9daf7e09486e94cbefdc25b2f`. Isolated branch `codex/schema17-active-fixture-guards`; source `cc95c2c954049253d7e90d704e62c35089fc0548`.

Changed only two current-header assertions: `run_data_mode` in factory_runtime_ipc.rs (seven Data cases), and `actual_generated_child_goal` post-run storage in fixtures/factory_child_goal_runtime.rs (two successful child/root Goal cases). Original SQL u32 reads, all counters/receipts/stop/fee/Owner/authority assertions and all other bytes of the two files remain unchanged except their expected literal and diagnostic text.

31 source-level checks/inverse controls pass, configured rustfmt --check and git diff --check pass. Contract inverse controls model preserved typed SQL extraction and exact17; they are not Rust, SQLite or product execution. No Cargo/Actor/browser/Provider/macOS execution by this author.

Root original matched-schema17 Factory run at `9b695892a1814dc9daf7e09486e94cbefdc25b2f` failed21/9 in91.94s. All nine are the two stale16 assertions; original RESULT/log and failures remain. No corrected runtime pass is claimed.

Historical pre_factory_schema.rs, integration_runtime.rs, child rework, large-read gates, production code and locks were not changed. See EVIDENCE.json for separate migration audit follow-ups.
