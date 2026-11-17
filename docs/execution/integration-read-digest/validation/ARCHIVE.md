# Validation archive

Execution HEAD was clean95637f62468fde04f132ba05af93998dfb1954eb, based on499b7aa7151820310460f8acd0ff4c7e00a2caf6. The complete report, twelve command logs and empty-runtime frozen-tool probe evidence are byte-identical copies of local artifacts. EVIDENCE.json maps original paths and hashes to this archive. Binaries and private runtime contents are not committed.

Five Cargo test logs are stored as lossless `.log.gz` files because their original trailing blank lines trigger committed-snapshot whitespace checks. Decompression reproduces the complete original bytes; EVIDENCE.json records both compressed and raw SHA-256 values. No raw log was trimmed or normalized. The initial committed archive-check failure is retained in ignored artifacts; this documentation-only storage correction does not rerun or alter tests.

The original before/after manifests were byte-identical. `inputs-identical.json` deduplicates those two copies, retaining all2,887 input paths/hashes. Both original manifests and this copy have SHA-256 `4a0d047e0ebefa624fd696a4c0b4efad52b646fd543b6b61c26f2e7b2b2287c5`. Source HEAD and clean status remained unchanged through execution; documentation was updated afterward.

All compilation and clippy completed before timed regressions. Same-source normal tools were frozen, probed and verified unchanged before/after tests. Top-level selected counts: reads6, filesystem12, Grant13, and each original failed scenario1, all passed/none ignored. Each original scenario ran once; no unchanged stress retries. Full-case test durations4.81s/4.61s are not preparation/hash latency or measured improvement.

The host was Apple Silicon macOS26.6.1/Rust1.91.1, not the hosted macOS15 environment. The old2011/2000 failure and interrupted/unknown case remain unresolved remotely. This archive proves the stated local regression dimension, not a unique root cause, speedup, CI acceptance, native GUI or production release.
