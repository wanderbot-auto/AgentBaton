# Evidence

Base and fetched feature target `a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28`. Tested source `56c56e9efda3b7bd7d38d559e8ffb1e9bde2ff09`. Production reader changes fixed at `26e69d53fdccf31ce27690887b3c156addf94d4d`. Exact commands, full source and test binary digests, scope receipts and environment are in EVIDENCE.json.

| Command receipt | Source | Result | SHA-256 |
| --- | --- | --- | --- |
| wire-red | `5cd64202` | 0 passed, 3 failed. Existing guards reject ordinary Store19. | `9140e2c3306cc1ef0be507fae0731ff51ca943044bbe56b4c6b9540fa90e3631` |
| wire-green | `26e69d53` | 11 passed, including 3 ordinary daemon real socket tests. | `5a194ce6ec05d251a21ae572dbecf3ee168dda10c3f6d5feb4a9e703d72f9d60` |
| lib-background | `26e69d53` | 3 passed. | `255e60e0c3fba3a233f315046018460a7cef112c17cd4558c78b00d1b6bd6d5a` |
| lib-host | `26e69d53` | 55 passed. | `1441d5976cdd8140a6eeaa961904674cf00a16d32f9e6b9d80adc075e58df667` |
| lib-factory-reads | `26e69d53` | 57 passed, 2 stale current18 assertion failures. | `f74a5b3eeb0e82df5cb8947f690abe8f55e2c0b0ccf472faec8b6d521f255b89` |
| factory-final | `f87296a8` | 59 passed after approved current Factory test updates. | `dba08518806793a5000733e23bc6a8e43a1a244faeb8c1064ebfd87954ce6bbc` |
| lib-integration-reads | `26e69d53` | Zero matched tests. No verification counted. | `c6e0396f213fbdc5a152bc091724d3ef99c7d68cc82e283841f6282c5b586547` |
| integration-lease-final | `f87296a8` | Exact filter, 1 passed. | `576db90f5bd1372f27d62c13b14c9883e0b8cc70e4d880d24cae527e0d93328c` |
| legacy-integration-assertion | `26e69d53` | 0 passed, 1 stale current18 assertion failure. | `7de3b3e67a8e59f11670108884f9828af9fe06a31e15a525ed7ad238f01a38fe` |
| integration-targets-final | `56c56e9e` | Original failing exact case rerun, 1 passed. | `e4e9af25702c7e25a1f080cc689a0cebdbf2cbd30893793191a1ca09d2689b51` |
| integration-auth-final | `56c56e9e` | Adjacent unmanaged target/forged permission control, 1 passed. | `00949d25727739635778244e186d6c6f2eaef6b135225ecd627f7a82f6b4243e` |
| clippy-delivery | `56c56e9e` | Strict Core all-targets and actual18 feature check, exit 0. | `71924612ef88070fb824b92435f79f60ce475fb6d641f27f777902ec07d8c3d3` |
| fmt-delivery | `56c56e9e` | All 10 claimed Rust files formatted, exit 0. | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| source-pin-dependencies | `427a9da4` | 17 passed, 15 failed among 32 source-only tests. Follow-up required. | `1bdbf1f242626e22ebe86c96b51310e6904e7423da6e8d84b33e1db6beaae263` |

131 distinct Rust tests passed across fixed sources. The source-only checks are a separate set. This table keeps red runs and zero-match candidates visible. Production code remained byte-identical after the first green run. Later commits only adapted authorized current test expectations, and reran their corresponding cases.

The new socket harness uses ordinary Daemon::bind and Daemon::run with a private temporary runtime and Core IPC over a real UnixStream. Four daemon thread joins returned success and removed their sockets. The same path was reopened and returned the original Host receipt and sequence. Wrong owner token caused no registration, receipt or event. Two authorized rows paginated and replay preserved their counts. The Git fixture used normal project import, managed task workspace preparation and a read worker observing the real local branch. The orphan-fence control injected corruption only through its test SQLite connection and proved the normal import did not treat the record as absence. It does not exercise a valid held Integration run. The 18/20 negatives altered only the version discriminator of a disposable already-open Store19, with the original SQLite schema and sequence retained. These are controlled guard mismatches; no historical migration result is inferred. Test runners and in-process daemon threads were joined. There was no separate daemon executable launched.

The old source-only modules test_integration_ipc_diagnostics, test_integration_stage_evidence_source, test_integration_fixed_script_fixture and test_integration_drift_ref_preparation refuse the reviewed read-version updates. The 427a run measured 15 failures and retains their exact original messages. The old inverse accepted both original Git blobs. Independent byte checks on the final source prove only the three approved current version assertions differ in the two Integration test files, and both final inputs are still refused by the old pins. A later source closure must bind the new exact input and restore the exact original source before invoking the existing inverse chain. Existing historical pins, producer sources, inverse files and thresholds were retained. The standalone Factory runtime current18 assertion at crates/core/tests/factory_runtime_ipc.rs:2019–2022 is unclaimed and unrun. Complete old migration/fixture composition and the overall integration candidate remain pending.

Local arm64 process integration is the verified platform boundary. Standalone release daemon/CLI, desktop, native workflow, CI, full Integration runtime/PR29 suite, Provider and remote contact were outside this run. No workspace-level service operation was required.

Raw check logs are preserved in evidence-logs.tar.gz with SHA-256 `5c10478c8c7034f52658e4d1f0cce978da295374346b4f1f24a1e5dc68b86b35`. LOGS.json records all extracted log digests. No database, authentication token or executable is packaged.
