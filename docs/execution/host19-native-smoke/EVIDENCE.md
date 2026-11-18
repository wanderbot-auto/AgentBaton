# Schema 19 native smoke evidence

## Binding

- Base `6fe6e7a06826020f72081a8f80dc39f995393c8b`
- Tested source `17a6539fb561f6b72e954052511710fd27c2821e`
- Published integration target at preview `3519dcf205618dbf1b8a0303f9315b163cfc3221`
- macOS 26.6.1, Mach-O 64-bit executable arm64. Toolchain, artifact path, lock
  hashes and frontend distribution hashes are in
  [the candidate binary receipt](logs/candidate-arm64-binary.json). Both native
  unit runs used the same locked input and the same test command.
- Baseline test executable SHA-256 `569a04a1c791291cd16f68e040e2cf39d81bbdbbf0482f5ee738564cad3747d9`
- Candidate test executable SHA-256 `2b34a3bde45d2816f7b19b30a3da9721b0b29089f97c6612ef5298eb8ef750e1`

The two ARM64 sidecar build inputs were reused from the earlier native handshake
worktree after byte checks and a Git comparison of the root Cargo inputs, CLI,
daemon and crates. Their SHA-256 values are in [the sidecar receipt](logs/sidecars.json).

## Verification

| Check | Baseline | Candidate |
| --- | --- | --- |
| ARM64 `agentbaton-native-smoke` bin unit tests | 26 passed, 10 failed | 37 passed, no failures or skips |
| Original native smoke source guard | Passes at base. With the candidate Rust bytes and original reader it rejects the new raw SHA | Original reader plus exact inverse passes |
| Targeted Python source tests | Existing cases retained | 17 passed |
| Full infrastructure suite | Prior handshake receipt records 931 tests with 16 skips | 936 total, 920 passed, 16 original skips |
| Root workspace fmt | | Passed |
| Desktop crate fmt | Compared to fixed baseline Git bytes | Same pre-existing diffs in `approval.rs`, `native_parent.rs` and the existing restart-test lines in `bin/native_smoke.rs` |
| Desktop strict Clippy | Primary diagnostics occur in files byte-identical to fixed baseline Git | Exit 101, 13 library diagnostics |
| Desktop advisory Clippy | Compared diagnostic source ranges to the fixed baseline | Exit 0, 13 library warnings and 6 prior bin warnings on unchanged lines |

Native command, run through `bash scripts/collab.sh run --native -- ...`

```sh
env CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 cargo test --offline --locked   --manifest-path apps/desktop/src-tauri/Cargo.toml   --features native-smoke,custom-protocol --bin agentbaton-native-smoke -- --test-threads=1
```

The first native feature-only compile failed on the duplicate
`_EMBED_INFO_PLIST` symbol from the two existing desktop library contexts. The
first custom-protocol build reported missing `../dist`. The worktree-local
`npm --prefix apps/desktop ci --offline --no-audit --no-fund` and
`npm --prefix apps/desktop run build` commands produced the real distribution
before both compared unit runs. The first full Python discovery command used
`-t .` and failed to import the namespace directory. Discovery with
`-s tests/infrastructure -v` ran all 936 tests. These first outcomes are retained
with the command receipts.

The four existing real SQLite projections accept the current store at 19 and
reject old/future schema versions. File bytes held steady through accepted and
rejected reads. Existing tests still verify per-table counts, missing tables,
journal/head changes, scope and stage ordering, and the private daemon/readonly
Core proof. A separate JSON test verifies all four guards refuse string and
float 19, integer 18 and 20, and a missing field. No App or harness main was
launched. The Rust suite includes its existing in-process private daemon tests.

## Historical source provenance

- Current native Rust raw SHA-256 `58721865993d21d3aa7a34be91ae1080bcb850f9c157e7daa9611bf7d5f8ee9c`
- First inverse output equals the raw blob at `6fe6e7a06826020f72081a8f80dc39f995393c8b` with
  SHA-256 `5145fdce52955077dcfd591b712977452dec3cfd09bb4bb06807968e0a1bcdfa`
- Existing inverse output equals the raw blob at
  `63e8a35fcc22d5c08bd135fbe015934d8bc2c44e` with SHA-256
  `95da64173f62be210d35fd626006f76ff33cda5ce96b333517c1244d23d2a4e0`

The old consumer is byte-identical after removing the added import and the
native smoke path route. The old JSON and pinned checksums stayed unchanged.
The source tests verify both layers, unexpected paths and source types, and
rejection of BOM, CRLF, the prior blob, and changed, missing or duplicated spans.

[Logs and exact hashes](LOGS.json) include the baseline and candidate commands,
source-pin red run, Git blob and lint provenance, and scope and merge preview.
The preview with `3519dcf2` was conflict-free. Native UI, packaged execution,
CI and actual Provider or remote tests were outside the task.
