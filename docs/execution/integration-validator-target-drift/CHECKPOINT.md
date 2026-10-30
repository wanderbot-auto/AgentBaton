# Frozen source checkpoint

Core/test source: `1af2a8acb77902696dd6907639af5cc20b6ec1f8`. Library ancestor: `775d723b243e7613d6731e1e6054e3e54ceea093`; initial library4a and subsequent348 remain separate history. Root source base: `fc8fbd10815051c4f3e1587a7e31c2f7a69c1ee4`. Only six approved Core files and the eighth fixture changed in this source commit. Schema, public DTO/IPC, shared meters, fees, caps and permissions remain unchanged.

Static evidence: `/workspace/.onboarding/integration-validator-target-drift-core-source/SOURCE_MAP.json` and `STATIC_CHECKS.json`. Absolute standalone rustfmt --check and git diff --check exited0. Source-only comparisons prove the original seven-case prefix and helpers, runtime CAS all-pass arm, complete IO CAS method, and settlement CAS+budget/cancel/meter tail byte-identical to base. No Cargo, Actor, Git-product command, Node, browser, network or subagent was run by this author.

Root commands after immutable assembly (not executed here):

```text
cargo check --locked --workspace --all-targets
cargo test --locked -p agentbaton-core --lib target_drift_stage_ownership_tests -- --test-threads=1
cargo test --locked -p agentbaton-core --test integration_ipc integration_runtime::three_real_rounds_revalidate_each_merged_candidate_after_consecutive_descendant_advances -- --exact --test-threads=1
cargo test --locked -p agentbaton-core --test integration_ipc -- --test-threads=1
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Root must supply its existing supported fixed Git PATH and original approved resource/profile guard. The exact integration_runtime:: filter follows the existing include/module registration. Zero selected tests would not establish coverage. Root alone owns execution/target leases. Existing unknown/failed runs must not be replayed. No fresh full-three-round success, Mac proof, fullVAL009 acceptance or performance improvement is claimed.
