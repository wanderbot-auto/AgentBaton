# ADR: explicit planning policy inside the existing Factory approval

Status: implemented source candidate; compiled tests and independent review pending.

Planning-only authority is a new complete optional policy in the same independent Factory approval. It is not a Goal approval or a generic model-tool authorization. True requires both the complete selection and the complete server-resolved planning binding. False uses Core's omittedNone representation and retains the old bytes/action; explicit null is refused rather than silently canonicalized. Root confirmed the UI follows this representation.

Native uses the existing full-snapshot bounded JSON pages, per-page cancellation and final private CAS frame. Every field, including unknown nested approved-definition content and original u64 pins, remains in the displayed snapshot. It checks exact structural/reference/boundary conditions and full receipt equality. Core remains responsible for original Native validator provenance, freshness, semantic validation and canonical SHA recomputation; Native adds no hash dependency or second implementation. Definition source_runs0/[] has Core's limited meaning and is not evidence that a validator never ran historically.

The four fixed Owner bridges reuse the existing daemon transport and preserve errors/unknown outcomes. Submit carries raw plan text, both revisions and the original key; each plan is not re-approved. Context/list/get remain reads, with list returning only ascending references under the existing cap. No new kind, general IPC selector, Agent token or finalize method is exposed. Child Goal execution/workspace creation remain false; plan persistence alone establishes no execution or Task acceptance.

Using explicit JSON checks preserves old-core compilation compatibility and full server snapshot bytes. Public DTO fields and limits are coordinated with the single Core writer. No Core/UI/schema/lock changes occur in this slice, and source-static checks cannot substitute for actual Rust/Core/macOS verification.
