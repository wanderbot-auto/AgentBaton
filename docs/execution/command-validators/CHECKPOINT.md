# Fixed-command checkpoint

Worktree `/workspace/.worktrees/command-validators`; branch `codex/deterministic-command-validators`; starting commit `ddd6707e5c67ff66f65f1ab243f868ee35366959`. Latest read trusted main `afb52be241f7c4cef261f99c04f56caf21f12083`, OWNER-001/002 ACK, no new instruction. AB-START-001 authorizes this local development unit; real Provider calls, private identities, production, paid provisioning and signing remain closed.

Implemented and self-tested Linux sealed ELF and macOS root-protected Mach-O execution paths, pinned in-memory sh scripts, full rule/receipt binding, cancellation/timeout/capture cleanup and scoped command-only acceptance. This module is not wired to Core, native approval, IPC, UI, task acceptance or adoption. Both macOS paths compile; no macOS execution evidence is claimed.

Frozen source fingerprints:

- command_validation.rs: `d5fa7b28d6a694c4fa8f767a021499fb5c984a68937cfbf939fcf553cbd4e870`
- lib.rs (one module export plus explicitly authorized top-level API comment clarification): `e396e03e5fbdf99ee4ffa2016240c1771da2892977651340cef2b4f3afdd45a5`
- tests/command_validation.rs: `1ec8708817bb72197be4fb44afec25155810e70dc4c0fc6e3973f86fe912cfd5`

Initial freeze `eec524c559ec5e99ac8434fa45655a170bbbc0ad` is preserved. This checkpoint supersedes its source hashes after independent review F1: observed-exit/cancel checks run after I/O/waitid, cleanup includes ownership/signalling observations, and scoped acceptance requires recorded in-budget completion. Final source was tested at these exact hashes: 90 unit/integration plus 4 doctests, formatting and three-target Clippy PASS. Independent review must verify these source hashes. The source/doc commit is to remain local until parent integration review; no push or main update has been made by this unit.

Actual results/log hashes are in EVIDENCE.md. Shared build target `/workspace/.worktrees/validation-integration/target` was released to the primary engineer after the final self-checks; do not start another cargo run on it without coordination. Two Apple target standard libraries were installed for cross checking, with no Cargo dependency/lock change or new target directory.

Next: independent F1 exact-fingerprint revalidation, including the reviewer's original 1500ms delay versus unchanged 1000ms process budget. F1 is not marked independently closed by the implementer. Address any further findings in this isolated worktree. Parent then integrates on a separate approved branch, wires trusted Core policy/native confirmation, and runs the existing two-architecture macOS CI against the exact integrated candidate. Preserve all original assertions and failed logs. Any unreaped Child/unknown process outcome must be retained/reconciled by the future supervisor rather than blindly replayed. Never promote the command-only token to full task/Git/production acceptance.

## Independent review closure

Separate reviewer verified exact source1dae4566, all27 command cases,4 compile-fail docs, Linux Clippy/fmt and the original observation-delay fault control. F1 is closed; no remaining blocking library finding. REVIEW.md SHA301262f2893f46d109a44c42d46aa97115743ae2b78c8a344ef20ea2411a2e8d. Independent raw controls/logs archived under /workspace/.onboarding/command-validator-independent/manifest.json. macOS execution and Core/native-owner/IPC/UI integration remain pending. Latest protected main afb52be/OWNER001002 reread and ACK at parent integration.
