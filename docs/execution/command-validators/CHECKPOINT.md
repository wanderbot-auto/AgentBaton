# Fixed-command checkpoint

Worktree `/workspace/.worktrees/command-validators`; branch `codex/deterministic-command-validators`; starting commit `ddd6707e5c67ff66f65f1ab243f868ee35366959`. Latest read trusted main `afb52be241f7c4cef261f99c04f56caf21f12083`, OWNER-001/002 ACK, no new instruction. AB-START-001 authorizes this local development unit; real Provider calls, private identities, production, paid provisioning and signing remain closed.

Implemented and self-tested Linux sealed ELF and macOS root-protected Mach-O execution paths, pinned in-memory sh scripts, full rule/receipt binding, cancellation/timeout/capture cleanup and scoped command-only acceptance. This module is not wired to Core, native approval, IPC, UI, task acceptance or adoption. Both macOS paths compile; no macOS execution evidence is claimed.

Frozen source fingerprints:

- command_validation.rs: `fdc83ed40e68e4de3a590f69fa272dcb6a112b47d396dd598003623b80697f04`
- lib.rs (one module export plus explicitly authorized top-level API comment clarification): `e396e03e5fbdf99ee4ffa2016240c1771da2892977651340cef2b4f3afdd45a5`
- tests/command_validation.rs: `8896fd4c1062724771b01b5cd1b847491b0df0b225218ce13c9a35b04520f866`

Final source, including API comments and the escape-fixture release handshake, was tested at these exact hashes. Independent review must verify these source hashes. The source/doc commit is to remain local until parent integration review; no push or main update has been made by this unit.

Actual results/log hashes are in EVIDENCE.md. Shared build target `/workspace/.worktrees/validation-integration/target` was released to the primary engineer after the final self-checks; do not start another cargo run on it without coordination. Two Apple target standard libraries were installed for cross checking, with no Cargo dependency/lock change or new target directory.

Next: independent source review and exact-fingerprint tests; address findings in this isolated worktree. Parent then integrates on a separate approved branch, wires trusted Core policy/native confirmation, and runs the existing two-architecture macOS CI against the exact integrated candidate. Preserve all original assertions and failed logs. Any unreaped Child/unknown process outcome must be retained/reconciled by the future supervisor rather than blindly replayed. Never promote the command-only token to full task/Git/production acceptance.
