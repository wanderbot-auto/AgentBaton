# Progress · Integration Git path observation

生产源码0793完成同次三路径观察，6个纯逆向检查通过；这些不计入Rust产品测试。原代码/所有原测试体可逆向恢复。首次新Python guard生成串有语法错误，加载失败、零测试体，窄修后6PASS；该历史未删除。

主树 `f0e642674e7f2ab887eb15ac6e418b43c0d756db` 编译0793时12个E0616，exit101、零Rust测试、wrapper1.704311937s。3b仅把新测试的6个布尔读取改为原公开getter，6个计数读取用已有Serialize的严格u64只读投影；没有扩大生产字段可见性或删除断言。

主树 `18c0365e248ea294a0eff5792f57812a0937ab2e` 实际 Workspaces --lib45PASS（含新增10+原35，不能再叠加），测试35.67s/wrapper38.399629203s；Integration IPC33PASS，测试101.84s/wrapper125.602976819s。每次 source/tracked/frozen tools 未变。该Root与3b只差先前child-plan-local-activation元数据、STATUS和manifest，运行源码一致。准确命令、tree、INPUTS/RESULT/log哈希见EVIDENCE。

独审SOURCE_READY附Root45实际证据；本次metadata待独审。未跑新Mac/CI，没有测得Mac耗时改善，不声称原db281 ARM21/12失败已修复。原2s/5s、TOCTOU、cancel/unknown、真实IO计费和全部130需求状态保持。

同一Root18c另有严格Clippy（Workspaces+Core/all-targets/-D warnings）exit0，wrapper18.667750135s；完整基础设施793项PASS，测试19.105s/wrapper19.455937190s。793已含本单元6纯guard，不再相加。末尾failed_or_interrupted为有意负向fixture输出，实际外层unittest793 OK；不是总体失败，也不代表产品或Mac验证。新增两个准确命令的INPUTS/RESULT/log哈希已登记。

## Follow-up · same-boundary isolation config, draft

At exact Rootd309 the complete Workspaces package command exited0 in110.495211801s: lib56, then command-target6, command-validation28, controlled-preparation16, data24, deterministic21, lazy-fetch5, safety23, source-capture21, validation-adoption24 and doc7 all passed. Four filtered nested child summaries are included in parent groups, not additional tests; the new11 are within lib56. The compile-fail doc errors and caught controlled panic are successful contract output. RESULT/INPUT/log pins are appended in EVIDENCE.

Original Root4ec55PASS1FAIL36.25s/wrapper40.611604129s is retained: literalLF Git128 and both guards NotRepository; nonUTF8 Git0 emitted the actual forbidden promisor name while oldregex missed it and current byteguard refused. c900 only corrects that test expectation while preserving oldoracle measurement, all original10 bodies and current refusal. Standalone locale reproduction is not proof of the old wrapper's unsaved locale.

Exact Rootd309 Integration returned32PASS1FAIL101.92s/wrapper123.669280353s, exit101. The sole final failed test is block_policy_with_three_round_limit_refuses_real_descendant_advance at integration_runtime.rs3026:9, bounded Host Git expired. An independent agent is diagnosing read-only; no cause or fix is accepted and original clocks/budgets remain. Caught controlled unitpanic in the passing stage-evidence test remains successful evidence. This result is separate from complete Workspaces PASS. Root strictClippy/infra remain pending; an in-progress different Core Clippy is not Root evidence. No rerun was started here and this draft is not CIready. Independent review02da is conditional pre-test source review; cc6a is the test-only source review with actual rerun then pending. Their historical status text is preserved. Static two fewer children is not a measured Mac benefit or a repair of ab4's Product failure.

## Staged b8 validation · local checks passed, commit pending

Main actually tested kind=staged_tree `b8ed174d16c43112f8b7b7dd4ba042e118f114d1`, baseHEADd309; it was not a tested cleancommit. Integration33PASS101.52s/w124.132, infra802PASS19.808s/w20.444, strict workspaceClippy excludingdesktopPASS19.983, formatPASS1.504 and bootstrap93/design/staticPASS. Source/tool stability is recorded per invocation. V6 records child0/wrapper0/interruptionNone/ownedleaderjoined; the infra call used reviewedv5 with nointerruption and does not establish a generic boundedshutdown guarantee. Fullsource/44checks and independentpure30 reviewfad8867 remains source-only history; actual results are separate.

The originald30932/1 and precise failingstep uncertainty remain: target had advanced, but the expired Git substep was not observed. New33pass demonstrates this candidate locally; it does not retroactively prove a unique rootcause or Macfix. Existingd309 completeWorkspaces result remains atd309; proof fc392a8c shows onlyCoretestfixture+infra changes and unchangedworkspace/dependencyinputs, not a b8Workspaces rerun. Nested/pure counts are not added to parent suites.

Main has not yet supplied an actual candidatecommit/normalRootmerge mapping; both staypending. No new remoteCI/Mac/privateProvider/full130 acceptance, metadata freeze or push is claimed.

## Source mapping resolved after checks and audit

Actual evidence independentreview3ae8b937 has374checks and no mustfix. The controlled localcommit operation first passed15 no-write negatives plus1 fakecallbackpositive, then created2d04 with tree exactlyb8; main's normal FF d309→2d04 retains every testedblob/mode and adds no mergecommit. Raw checks remain typed as stagedtreeb8/baseHEADd309; this later equalitymapping does not rewrite their executionidentity or pretend a newWorkspaces run. Original pendingfields and failures remain historical, with an appended resolvedmapping in EVIDENCE.

Metadata's own branch safely FF'd to2d04 while preserving all seven known draftfile bytes; no unknown edits, reset or stash. Author only stages these seven metadata paths for review, with no metadata commit or push. NewremoteCI/Mac and fullproduct acceptance are stillunestablished.
