# Progress

Clean base, scope, gate and prior handshake verified. Copied two frozen accepted ARM64 sidecars into this task tree after verifying digests and unchanged Core, CLI/daemon and root locked inputs. Baseline bin unit run follows.


- Reproduced on the fixed base. Preserved the duplicate Info.plist build error
  and the first custom-protocol attempt's missing frontend distribution. The
  real local locked frontend build passed. The baseline ARM64 run then returned
  26 passed and 10 failed with the old version guards and fixtures.
- Updated all four guards and corresponding reports and fixtures. Explicit tests
  cover integers 18/20, string and float 19, and missing fields. SQL controls
  include actual versions 18/20 and byte comparisons before and after reads.
- Reproduced the old raw-source pin failure with the Rust changes applied and the
  consumer unchanged. Added the independent fixed-byte inverse. The two restored
  blobs match Git, and the consumer matches base after removing the import and
  two-line route.
- Verified exact source 17a6539f. ARM64 bin units 37 passed, Python source 17
  passed, infra 920 passed and 16 skipped, and workspace fmt passed. The whole
  desktop fmt and strict Clippy failures are documented against the fixed base.
- Target moved to 3519dcf2 with Settings. Scope and merge preview passed. The
  local source and evidence are ready for independent leader review and handoff.
