# Rework observation diagnosis

Requirements: AB-TSK-004/009/011, AB-VAL-003/007; AB-START-001. ACK OWNER-001/002 from protected origin/main653679db; gate remains product development, no real Provider credentials/calls or paid resources.

Base3b4c4b9dfa15588358716643c81006c4bd1e95a8, isolated `codex/factory-rework-observation`. Only the existing rework test fixture and these four records are owned. Root tree, production, schema, UI, Native, locks, CI, STATUS and permissions stay outside this leaf.

First classify Product40947058067/job123705085895 retained Rust failure, then add bounded non-sensitive test observations. No unchanged rerun. A future authorized single-case run must retain failure and actual cleanup; no deadline renewal, swallowed error, Owner write retry or budget increase.

The 55s test observation spans first rework, exact Owner decision and parent continuation; its origin is after `factory_runs.start` returns. The enclosing Grant/Scope originally approve60s, effective root/project runtime remains30000ms, child20000ms. These dimensions do not substitute for each other. Three observer methods use configured IPC client5s and Actor request60s. Existing fixed command1000ms+cleanup500ms, Work reservation4000ms, Validator6000ms, Owner1000ms are unchanged.

## Diagnostic field correction

Keep the diagnostic object-presence fact separate from transport reply receipt. Run-state booleans must be nullable when the method has no string `result.run.state`. This changes no observation deadline, RPC, permission or success criterion.

## Approved fixture-only polling optimization

Retain old diagnostic branches/commits, including a7/5edd. New branch codex/factory-rework-observation-polling changes no production path. Discover the immutable child address once with original scope/parent assertions, then obtain fresh authenticated summary on every poll. Recheck scope/root/parent/task/Factory-run/Goal-id/adopted-plan-revision+digest bindings. Summary plan2 plus awaiting_acceptance gates the expensive full history read; it is never execution/acceptance authority. Full Owner node/Validator/candidate/parent continuation assertions remain mandatory. Keep every original55/60/30/5/60/10ms/retry2 boundary and fixed three-method readonly whitelist. Record aggregate per-method call counts and IPC elapsed inside the original4KiB last-scalar record; these are diagnostic observer times, not consumed budget, CPU self-time or refunds.

## Bounded progress and post-owned-shutdown collection (727 follow-up)

Requirements AB-TSK-004/009/011, AB-VAL-003/007; authority AB-START-001 and parent-approved paths. OWNER-001/002 remain acknowledged; no scope/permission change. Parent owns workflow integration and all heavy execution. Preserve the previous a7/5744/99fb source and evidence.

The two new 727 Mac failures have a late, successful list reply but no array progress projection; they do not establish the same internal cause, Grant expiry, or a production performance defect. Add only a finite array projection to the original overwritten 4KiB scalar; revision1 failed/blocked remains legitimate rework input. Unknown/malformed summaries, early acceptance/completion, or revision2 failed/blocked/cancelled/cancel_requested/unknown refuse immediately with a fixed code. Original fresh list bindings/full-view Owner and Validator proofs remain mandatory.

A fixture-private wrapper owns the original RunningDaemon, preserving its exact explicit Drop location. On normal Drop or panic unwind it first consumes/drops that original object (stop/join/copy unchanged), then checks the original pre-spawn main-file identity, bounded nofollow FD reads and before/after metadata, main/copy exact bytes+cleanup hash, and source/copy WAL/SHM/journal absence. It writes only a small create-new0600 receipt. Errors become incomplete and never replace the original panic. No receipt exists before the wrapper is constructed. This is a generated fixture's point-in-time post-join observation, not an atomic general SQLite snapshot, cutoff-instant projection, or proof every OS descendant stopped.

The new standalone collector reads only fixed files under private0700 rework-UUID directories with strict UID/mode/nlink/nofollow and size/count budgets. Joined+daemon-ok, the new consistent-copy receipt and exact raw hash are required; old evidence without the receipt stays pending. Only a private temporary derivative is opened by SQLite mode=ro/immutable/query_only, with trusted_schema off. It emits strict states, typed flags/counts and fixed classification codes; raw DB, IDs, paths, RPCs, messages, stderr and approval data are never published. Root's always-step must retain raw inputs privately and upload only stdout JSON; exit0 means collected, never test passed.

First run only generated filesystem/SQL/scalar controls and standalone formatter/static inverse checks. Freeze source and independent review before parent-authorized exact runtime/CI. Do not increase 55/60/30/5/60 deadlines,10ms polling or2 retries, modify production/locks/schema/workflow, or repeat the unchanged failed candidate.

## Collector review correction (51723)

Source `51723cdb4b015ac50add469ac9c52c1c2f4b9156` follows4d00. Independent review of4d00 found two real collector-only boundaries: compact JSON budget versus pretty stdout (12 finite cases emitted139849B above131072 yet exit0), and argparse unknown-argument stderr echo. Original4d source/20green and independent counterexamples remain intact. This is not a Rust/Actor failure or revised Mac result.

The correction budgets the exact sorted/indented UTF-8 serialization including newline and uses that same renderer for stdout. Exceeding the bound emits the fixed171-byte PUBLIC_PROJECTION_LIMIT invalid result with exit1, without truncating fields. FixedParser rejects unknown/missing CLI arguments with only INVALID_COLLECTOR_ARGUMENTS and exit2; no argv or exception text is copied. All original20 pure controls are byte-equal after removing the2new tests. Final22distinct Python tests PASS0.043s; do not add20+22. Rust/RAII and every production file are byte-equal to4d00, so this correction needs no new source-level runtime claim.

Author old→new exact pure control: old139849B/exit0/canary echo true; new171B/exit1/canary echo false. One initial author argument probe omitted the required root and therefore hit argparse's missing-required check; it was preserved, and only the generated probe argv was corrected to reproduce the actual unknown-arg echo. Artifact `/workspace/.onboarding/factory-rework-progress-diagnostics/COLLECTOR_FIX_MANIFEST.json` SHA`581290486f269e15ba924159c4cd67f58221246379e28b393a8ffa36a6a3dca3`. Parent separately reports4d integration3faa compiled18.81s and a real case in flight; this author did not execute that command and claims no final result or Mac evidence. Independent review of51723 and parent runtime/CI remain next.

## a866 Environment generated temporary-root correction

The existing collector deliberately rejects symlinks in every directory component. A generated test fixture must therefore use the physical location of its own TemporaryDirectory, including on macOS where the returned logical temporary path may contain the /var alias. Canonicalize only the trusted generated Fixture root with resolve(strict=True); never canonicalize an arbitrary collector input or loosen its O_NOFOLLOW guard. Preserve all22 original test bodies and negative-path inputs, add one generated-alias positive/direct-collector-refusal regression, and retain original double-Mac failure logs. Verify the candidate on Linux using an explicitly created ancestor symlink; new true Mac CI remains required. No product, Rust, collector, permissions, timeout, workflow or lock changes.
