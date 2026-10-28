# Final independent closure — Native Grant typed contract

**Disposition: NG-FBFD-01 is closed within the reviewed test-evidence/typed-CAS scope. Source is ready for Root's final composition review.** No known must-fix remains in this bounded correction. This is not formal approval, a new macOS result or full-product acceptance.

Fixed source `655c6d2985dd8d2d0dcfa16864d12edf2dfca374`, baseline `1839c6c8ca82319ad18cd6c5dd3825b19dde5ee7`; author metadata head `0fa8a879f7e30bb6cb69a86c076eb6e02928b3b7`. Both preceding independent reports/seals remain unchanged. Eight fixed Git files are exported; only the two allowed Python files change in this source delta. No author/product source, Cargo/target, CI, STATUS, remote or approval state was changed by the reviewer.

## Closure and preserved contract

The earlier fix correctly replaced the invented Grant.approval_id==proposal.id relation with bounded opaque IDs and actual response/proposal-result/Grant/event binding. Its first typed comparison revision repaired the four cross-record cases but left confirmation revision and schema scalar coercions. This final revision addresses that family together:

- `pending_error` and `approved_error` compare schema to exact integer12 with `json_exact`, require integer sequence/baseline and positive integer reference revision, and compare revision/sequence arithmetic without bool/float coercion.
- Grant event revision/sequence retain explicit integer guards plus exact matching. Confirmation revision is now compared to the already typed current proposal revision using `json_exact`.
- `presentation_error` requires a real integer in-range reference index, positive integer reference revision, and exact typed observed index/count/revision. Parent/visible/complete flags still require true booleans.
- Complete snapshots, scope/enforcement/bindings, response/stored-result/Grant objects and event payloads retain recursive type-sensitive JSON equality. Boolean/integer/float types are distinct; nonfinite floats are rejected. No coercion, normalization or partial-field substitute was introduced.

The three-predicate scalar audit matches the actual fixed code. Existing Grant depth/revision, scope numeric fields, enforcement flags and all19 counters remain explicitly typed; their rejection is not falsely presented as newly failing on the old source.

Mechanical checks confirm **all old28 test ASTs are unchanged**, one seven-subcase test is added, and both Rust files are byte-identical to fbfd. The entire host OS `run` function, raw DB reader, bootstrap client, prompt encoder, message formatter reader, opaque-ID rule and scope constructors are unchanged. The same JSON helper from1839 is retained. Scope/refs/CAS, exact event sequences/uniqueness, original19zero checks, full24 page layout/cell/on-screen/capture/DB-permit/real-click behavior and75s/85s/4s/500ms limits are preserved. Production Core/NativeParent/permissions/schema/locks/CI do not change.

## Independent actual verification

Root provided an idle CPU window. This reviewer ran the **complete fixed29 Python tests: PASS29 /6.925s**, including the original three and added one generated real Core bootstrap/SQLite cases. Exact argv/cwd/times/exit and original stdout/stderr are retained. The selected frozen daemon SHA `40322bff82aa8ec0f1c019e5d723f42694a6266bcaa27b153f535d0d21dbadcd` matched before/after. The window was immediately returned. No Cargo, browser, real Provider/model/account or new dependency was invoked.

Both previous independent reproducer scripts were then copied **byte-for-byte** into the new review directory and run against fixed655:

| Exact independent reproducer | Old retained result | Fixed655 actual result |
| --- | --- | --- |
| Four independently JSON-parsed record/payload bool/int substitutions |fbfd:1test/4failed subcases|1test PASS, all4 refused|
| Confirmation2.0 plus pending/approved schema12.0 |1839:1test/3failed subcases|1test PASS, all3 refused|

These finite controls perform no Core/browser calls. They demonstrate repair of the exact prior counterexamples, rather than deletion or altered expectations. The full29 suite additionally executes the author's seven-case scalar matrix, including reference revision bool/float and the two Grant-event revision variants already refused before the final change. These repeated controls are not added to29 as distinct full-product cases.

Saved-evidence verification and fixed-range diff checks also exit0. All8 exports match Git; author manifest SHA `483d0ccac98bf44733d13f641dd45ec5d51d559f236ca6be65ab67064d33d0f6` and its six raw evidence hashes match. `provenance.json` records source/AST/Rust identity, prior-seal hashes and original Mac packet replay. Exact commands/stdout/stderr and script hashes are retained.

## Historical evidence remains correctly scoped

Author final29PASS6.868s is separate from this independent29 run. Its old1839 seven-subcase test records **five actual failures** and **two existing rejections**; those two Grant-approved revision bool/float cases are not fabricated old failures. Original fbfd four failures,1839 three scalar failures,24/28 successful suites and all old source/manifests remain sealed. No source stage is relabeled as another stage's execution.

The Rust controller/macOS files match the earlier Root10-controller-test/fmt/Clippy source exactly. That existing result still covers those unchanged Rust bytes only. This reviewer did not run Cargo, and no new Rust or AppKit execution is borrowed for Python changes.

The six original arm/intel after/response/fixture packets were re-read from the sealed fb7 Mac artifacts and pass the final typed predicate **offline**. Their saved hashes remain in provenance. That confirms compatibility with the original actual valid output; it does not rewrite both original failed CI jobs or assert the corrected source has performed new OS clicks. No images were reopened and no new installation package, signing result or live Provider compatibility is claimed.

The old 24-page-per-architecture observation and persisted Grant effect remain historical subordinate evidence; new exact CI, dualMac Owner acceptance, release/audit/packages and overall Design completion remain Root's next verification gates. The separate Factory CI work remains checkpointed at77 passing finite controls with its original-driver13.0 dependency failure; no result from this review clears that unrelated blocker.
