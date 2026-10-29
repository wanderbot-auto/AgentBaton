# Core Factory constructor Clippy maintenance

Base: `a915be0e6d6d394ffbc100cbaf0fe517c0934eaa`. Scope: only goal_plan.rs, goal_store.rs and goal_factory.rs; preserve every authorization/budget/input predicate and evaluation order. OWNER-001/002 and AB-START-001 remain acknowledged; no authority change is involved.

The existing strict migration-feature Clippy invocation fails on one eight-argument private constructor and two redundant borrows. Group the naturally atomic approval ID/revision into a tuple, destructure it before the original condition, update its sole caller, and remove the two redundant outer references. No lint allowance, new DTO, public authority constructor, migration, fixture, lockfile or schema change.

Engineering decision: use `(&str, u64)` rather than another private record or an allow attribute. The existing ID and revision expressions retain their original order, and the original constructor body remains byte-identical after one local destructuring binding. Seven arguments satisfy the reported count without changing policy.

Run only standalone rustfmt/diff and mechanical source-equivalence checks. Root owns Cargo/Clippy/product validation and a different context must review this authored change before integration. Source-only verification does not close the original actual Clippy failure.
