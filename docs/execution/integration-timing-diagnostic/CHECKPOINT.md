# Checkpoint

ITDIAG-001 at parent `cbf3bb65fa5922660778a74e5db1ec546ae73d59`. Only the three reserved source/test files and this task directory changed. No parent mutation, CI, push or merge.

Stage1 joined successfully: Python source/inverse 22/22; Rust phase filter 9/9 (three new settlement, five existing trace, one existing read), none ignored; rustfmt, cargo check, diff check and scope passed. Raw logs and hashes are in EVIDENCE.json. An early second run-wrapper invocation was correctly refused because the first owned command was still running; no second build started.

Provenance limitation: Stage1 ran with uncommitted task changes on parent cbf. It did not capture input digests before/after execution. These logs must not be described as an execution on clean commit 8b298fde. The subsequent Stage2 run below supplies the required actual clean HEAD and before/after input binding, without retroactively changing Stage1's provenance. A documentation-only follow-up commit does not change the tested code, but its actual execution HEAD must still be recorded honestly.

The initial heavy build slot was released to the owner's separately coordinated native App acceptance. After leader released Stage2, it completed as one bounded package and joined; the slot was then immediately returned to A's real IPC work. Independent source review passed with no blockers; see REVIEW.md. Leader integration review remains separate.

Stage2 ran on actual clean `f95db43f78def107aabc4afb403e2927a82915e3`, whose code equals reviewed `8b298fde82931928bbd22f532dfe50df1f2921b7`. It repeated only nine Rust and twenty-two source controls once, then built default debug/release ARM64 batond with two jobs. All commands passed. Both strings invocations exited zero; the contract marker was present in debug and absent in release. This is daemon binary proof, not actual default release native application audit, which remains not run.

Before/after manifests contain the same 2,758 tracked inputs with identical SHA-256 `a51827cb0e06a88fa11aafb96dfa21e14977f41615cc001012ff4564d1d450eb`; HEAD and clean status were unchanged at completion. Raw command logs, manifests and report are retained in `artifacts/collab/itdiag/stage2/`; EVIDENCE.json embeds the report, selected input digests and raw artifact hashes. Documentation was updated only after the after-manifest completed. No full suite, stress run, CI or parent mutation was performed.

The complete report, four logs and one deduplicated copy of the byte-identical manifests are also committed under this task's stage2 directory. Leader and separate native reviewer independently checked these evidence dimensions; no blockers. Leader authorized this branch's push and a draft PR targeting unchanged `collab/operation-performance` at cbf. This remains dependent on PR29; main admission still requires all five checks. No merge, parent update, hosted CI or native App release audit is authorized or claimed here.

The existing hosted 2009/2000ms failure is not fixed or waived. The six original unchanged base/head trials and the runner-capacity cancellation remain separate parent evidence. No fourth unchanged hosted retry is allowed. Diagnostic samples cannot assign the 409ms difference to unmeasured phases.
