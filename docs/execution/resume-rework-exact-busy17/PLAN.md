# Exact readonly Busy reconciliation on Root17

Base: `7ee2437db08695347650a28f445cb0034b5f3f58`. Authorization remains AB-START-001; OWNER-001/002 were read, with no OWNER-003 present. Root owns integration and all Cargo/Node/browser/SQL/CI execution.

Apply only the reviewed `58fad1977c6a9ddfea4aa5c17484b335491a9191` diff for `crates/core/tests/fixtures/factory_child_goal_rework.rs`. Do not copy Parent's integration fixture, alter Actor behavior, replay Owner writes, renew the original 55s observation/Scope TTL, or enlarge the existing shared two-retry pool.

Root must independently review and run the three added pure controls and the actual current Factory suite before claiming any recovery or CI fix. The separate one-second deadline failure and Parent15 UI normal/cancel failures remain failures.
