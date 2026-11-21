# PR #33 macOS partition contract

Use integration base `9e3add27c37f4022dcd2517ab9d1c9cc8f3e9102`. This task owns only this directory. Use the current PR merge checkout and the observed hosted failures as evidence.

Evaluate the observed seam after original macOS step 12 and compare it to an upstream history-and-package job. Specify the fail-closed dependency, exact checkout and artifact contracts, and how to keep a single release sidecar pair when default Tauri and packaging share a runner. Exercise positive and negative cases locally using synthetic receipts, the actual sidecar staging helper and the existing historical source inverses.

Preserve the existing branch rules, job limits, platform and gate coverage. Record the remaining hosted timing, transport and quota checks for a later implementation. This task makes no workflow or server-side changes.
