# Independent Graph UI / Native review — final revision

Disposition: PASS for the reviewed source and Node verification scope. Both previously confirmed findings R1 and R2 are fixed. No remaining blocking finding was identified in this bounded review. This is an engineering review, not a GitHub Approve action or full product/Goal acceptance.

Subject: 4eadffe73368c61bc26e9aaf112a572f8b4b675f.
Git tree: a1595eeb766c7b112d19df0e930b85d266727e50.
Independent detached worktree: /workspace/.worktrees/goal-graph-ui-review-4eadffe.
Original reviewed source: f81979c45e7d86f5b21b045f9b5329d433aec19a; original docs 9d8e5ffdfa0a28fa0ee865d938e553258bfe04f0.
This reviewer did not author Graph UI, lib.rs or approval.rs. No product source was changed. No Cargo, target lease, installation, remote write or formal review submission was used.

## Actual independent checks

| Check | Actual result |
| --- | --- |
| Complete Vitest / jsdom suite | 537 passed, 32 files passed; exit 0; command elapsed 32.916 s |
| Original independent R1 artifact controls | Both passed; original assertions and helpers retained |
| Original independent R2 foreign Owner decision component control | Passed; original assertion and helpers retained |
| Combined independent controls | 3 passed, 2 files passed; exit 0; 4.976 s |
| TypeScript tsc --noEmit | Exit 0; 1.992 s; empty diagnostics |
| Vite production build to this private evidence directory | Exit 0; 1.568 s; original over-500kB chunk warning remains |
| git diff --check f819 to exact 4eadffe | Exit 0 |
| Source bytes after execution | All 28 reviewed changed/new application files match exact Git subject and frozen copies |
| Original evidence preservation | All 46 artifacts recorded by the original CHECKPOINT retain their SHA-256 |

Exact argv, working directories, times, exits and log hashes are in the five result JSON files and verification.json. Existing dependencies were temporarily symlinked; generated cache and bundle output stayed in this evidence directory. No shared Node dependency files or product dist were modified.

## Findings and correction review

R1, original goal-graph-guards.ts:55: safe numeric identity was accepted without complete artifact-reference equality. The corrected refMatches checks canonical complete reference equality, including the full root and file_pin rather than path/content fields alone. The second-pass graphReferencesBound additionally matches all non-null consumed-input, validation, Owner and Task receipt references against the full details candidate set. Unsafe integers remain rejected; signed-second and nanosecond/u32 domain checks are preserved. Both unchanged independent counterexamples now pass. The original failing logs remain outside this newrevision directory.

R2, original GoalGraphExecution.tsx:70/93: a reply could acknowledge the target node while using an older decision ID for another node. The corrected pending receipt retains its approval ID and fixed action. Refresh binds the actual decision to the original run, node, plan revision, plan digest, approval ID, decision ID, decision value and trusted source before releasing the hold. A foreign decision preserves the hold and becomes uncertain; it does not re-enable Owner authority. The unchanged independent component counterexample now passes. Added permanent author tests cover foreign node, approval, action and plan; the exact valid Owner acknowledgement positive also passed.

The original exact-key historical plan-adoption replay test still passes and confirms that newer run/ledger floors do not roll back when a consumed older receipt is recovered. Trusted-summary cancellation remains usable when unsafe artifact details block Owner trust. Legacy and Graph pending modes remain separate. A lost Owner operation after a changed node/plan remains conservatively held under the existing policy; this leaf adds no new replay/authority interface.

## Real packet and preservation checks

The new canonical actual-Core packet is exactly 39,181 bytes and SHA-256 60c205a9ca559150b52b5bdffb2418a706b93cbb3b8aeda9e3c17d670b885754, identical to the received actual packet retained in the author's evidence directory. The original packet SHA-256 da00c1ca8bfbe1efc0230a02b8865e4a7ffff96096c5b055ccd7eace6e1c3317 remains byte-identical and now has a permanent rejecting control for noncanonical agent execution kind. No agent alias was added; the canonical execution kinds remain connection / validator / owner. The new positive is a static renderer parse/binding check, not a new Core execution by this reviewer.

All 24 baseline renderer test files remain byte-identical to 3b99f2. The complete previous Graph component and guard test suites remain exact prefixes of their extended versions. Native source remains unchanged from f819:
- approval.rs: ed15fe18bca38f102c41f77cd87c31b2a644f23201652d367670465ebc7bb80a
- lib.rs: f4c9676026dcb8492d58ea381f0e04661cc039bff23fff868e558bbee7560af6

The earlier independent preservation proof retains all 22 original approval and 6 original lib tests as exact, unique blocks and preserves every old source line in order. No original kind, 16KiB review boundary, fixed private finalize behavior, Cancel semantics, or old test was weakened.

## Native source review and attributed execution evidence

The fixed Native Graph execution / Owner acceptance / explicit rejection actions remain private and distinct; renderer input cannot select a generic approval action. Review pages preserve the complete snapshot, including candidate receipts and stop/capture origin, through UTF-8-safe 12KiB JSON chunks with 16KiB displayed-page bounds. The separate 1MiB complete envelope cap remains; no fields are allowlisted away. Every page and final decision must be confirmed before fixed private finalize/decline. Cancel never creates an Owner rejection decision.

Root-executed composition 94157aed2cb69c6b503c5b6b12af5aff554476af used these identical Native bytes. This reviewer independently rehashed its manifest dd76425e46e99fad9e6fdd58c17f232f992ffecde13b8cdb2bd002f546e4b325, all 84 Git source bindings, five execution logs and two frozen ELF files. The actual logs show 43 library tests + 22 harness tests passed, Clippy and fmt passed. This is independently checked evidence of Root execution, not this reviewer's own Rust execution and not macOS proof. See the unchanged parent-directory root-native94157-evidence-audit.json. No such evidence can be transferred to a later Native source revision.

Native candidate_receipts intentionally contain the direct Owner-referenced candidate subset; transitive consumed inputs must not be incorrectly required to exist within that subset. UI details contain the full candidate history and use the full-reference second pass. Source questions concerning future validator-ref overlap/join behavior were passed to author/Root; the actual supplied packet is Owner-only, so it is not evidence of complete validator execution.

## Limits and remaining acceptance

This bounded source/Node review does not prove real browser IPC, complete graph validators/dispatch/finalization, macOS WebView, actual Owner OS dialog, real Provider integration, Apple Silicon/Intel builds or installer delivery, minimum OS, signing, manual installation, or full Design v0.2. Requirements remain partially verified at the overall project level. The separate OS-dialog feasibility report remains Linux/source-only and no production dialog parent was changed here.

No account, credential, spending, CI, merge, release or signing authority was added. Existing bundle-size warning is recorded rather than suppressed. Original f819 failures and intermediate tool configuration errors remain preserved as historical evidence. The final manifest hashes this report, the original checkpoint/report references, frozen sources, commands, logs and emitted private bundle.
