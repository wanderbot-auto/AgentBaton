## 当前完整子 Goal 正常链与超时负例通过 — 2026-10-29T13:04:21.684375+00:00

准确84023b1的原完整childGoal case实际1通过/0失败/0跳过（7.53s，含编译总9.028s）。冻结工具8fd与测试源的生产内容逐字相同，仅fixture/docs变化已穷尽核对，不声称新二进制构建。真实82事件连通计划→Worker回收→候选→私有Owner验收→child TaskAccepted→父新attempt恢复→Factorycompleted，3会话全部停、零active intent/lease；原Factory绑定Grant完整不变。独立审查只读核原运行证据，并非独立重跑。父generic Task不是TaskAccepted，不把Factory完成冒整Goal/产品验收。

准确e392c9e的原1000ms负例实际1通过/0失败/0跳过（5.04s，总6.6178s）：typed Timeout、剩余原预算到期、pending，真实回收/未知输出/不自动重试和累计预算不退款；唯一stop断言已恢复，独立运行证据审查进行中。原泛reason断言失败未抹。当前Rootself真实派发门拒OutcomeUnknown/rev5已由固定tag确认；候选8a因缺停止语义证明独审拒，补04待核。合法Owner-first首次预算counter旧控准确291c0b0实际INPUT_CHANGED0/1，生产窄修另进行。

新15真实浏览器三case仍未运行；Native新profile全回归、Mac/完整CI/15开发包未运行，既有14双包不冒新15。完整130/WP01–14继续，权限不变；main653679d、OWNER001002已ACK无003。见 [准确正常与负例证据](NORMAL_AND_TIMEOUT_RESULTS.json)。以下保留历史。

## 当前真实执行链边界 — 2026-10-29T12:44:15.237500+00:00

当前 Root已整合receipt参数编码修复、同审批预算内的正常Work初始4秒（Owner仍1秒）以及完整Graph全停证明向Factory confirmed_stop投影。准确6d2：正常case已到真实Worker停止1287ms/28B、候选、Owner和TaskAccepted，但原缺stop投影使父等待；准确8fd窄修后父已真实恢复，随后测试生成器误执行原legacy委派体、造成额外子任务，整体仍FAIL。该测试控制流修正进行中，不能把阶段推进当整链通过。

准确52cb的receipt2与失败状态负控2项真实Store/SQLite通过，原失败与source-blocked夹具问题分别保留。准确6d根任务自身Goal真实PERMISSION_DENIED于发送前，尚未闭合；1秒负控实际回收真实进程/1002ms/null输出/unknown，但固定reason断言漏pending-poll timeout分支而失败，精确类型和早期证据需补，不泛认协议错误为超时。Owner-first空counter合法路径亦保待修。新浏览器/Mac/全量回归/CI未运行；现有双Mac14开发包不冒15。完整130需求继续，权限不变；main653679d/OWNER001002ACK无003。详见 [分阶段原始结果摘要](LIFECYCLE_BOUNDARIES.json)。

## Receipt correction and next real boundary

Exact3385 normal still FAIL0/1 (test16.50s,total35.3986s,no guard), but now exact receipt check succeeds and real Worker reaches ready and a3673-byte queued Goal prompt. It then hits command-channel RESOURCE_LIMIT and the original1000ms reservation expires (actual1003ms): outcome_unknown/outputnull/ownedstoptrue, no TaskAccepted. Launch→prompt1178ms exceeds that original reservation; raw try_send variant was not retained. Do not convert unknown to success or relax runtime limits. Normal fixture initial reservation and independent1s negative are under source work.

Exact52cb421: actual receipt2units PASS (3.29s tests), fresh-fixture refusal2units PASS (7.96s tests), zero ignored. Earlier wrong-filter0tests and original8173 INPUT_CHANGED0/2 are explicitly not tag8 old-regression acceptance. Four units are Store/SQLite, not process/Mac proof. The optional real Native preapproval/revoke fixture isolates receipt encoding; it does not repair the separately retained Owner-first empty-counter product boundary. All raw failures/ELFs/DBs retained. See RECEIPT_FIX_RESULT.json. New browser/Mac/CI and full130 remain incomplete.

## Fixed-layer diagnostic actual failure

Exact646bf7d: typecheck PASS, diagnostic units3PASS, same-head bins/probe PASS. Real child chain FAILED0/1/test16.83s/total32.8076s/no guard; tag8 identifies exact receipt binding, not unknown measured runtime/output or count mismatch. Field-order mismatch in JSON text is source+SQLite-confirmed; one-parameter encoding correction and real writer positive/negative regression pending. Original failed DB/stderr/ELF retained and Actor joined. Full chain/root-self/browser/Mac/newCI remain unpassed. See BUDGET_DIAGNOSTIC_RESULT.json.

## Actual optimized SHA-256 and budget boundary — 2026-10-29T12:06:44.197047+00:00

Root exact0469863 built with actual sha2 opt-level3/debug assertions on. Full CLI remains86,731,728 bytes; complete verification measured51ms SHA update/finalize and72ms read/hash. No integrity check, permission, budget or timeout changed. Initial prepare stopped at the disk guard before probe; retained failure and exact-hash owned-staging recovery followed by unchanged-helper probe PASS are recorded separately.

Actual child process test still FAILED (exit101,0PASS/1FAIL,test16.77s,total38.7429s): now BUDGET_UNKNOWN after event41advance/42blocked. Reservation rolled back; Actor joined, failure DB/stderr/ELF preserved; root-self case NOT_RUN. This is not the earlier hash delay and not lifecycle success. Independent diagnosis excludes blindly marking message capture complete; exact transaction refusal layer remains to be identified without relaxing budget/unknown semantics.

Reviewed legacy authority fault tests now PASS2 and Native legacy-list positive PASS1 at ad8beb5; original failures preserved. New normal browser driver source0fb50feb independently ready for three planned cases only; actual browser NOT_RUN. Genuine migration6PASS remains bound to01a193e, Native147 to a07a7f9, not this new profile. New exact CI/Mac/profile-wide regression NOT_RUN. main653679d unchanged; OWNER001/002 ACK, no003. Full130/WP01–14 continues. See SHA2_RUNTIME_RESULT.json for bounded scalar evidence.

# Checkpoint 2026-10-29T10:39:43.400867+00:00

Current Root branch codex/factory-child-goal-integration, head0ab7cd8 (CI privacy-only after actualproduct source01a193e). Protected main653679d/PR17 exactseven success remains accepted14. OWNER001/002ACK/no003. Remote branch last saved13e6b41; these newer reviewed/tested changes are local pending current checkpoint push, not a nextPR or newCI claim.

Actualnew14sourcebuild/probe17.02s + newmanifest historygeneration3.80s passed; independent1f0c64c6/c54e53cf. Source/head/tree, private500singlelink pair, 69events/sevennonemptytables/FK0/ownedSIGTERMjoin0 verified. New fixture30c329 uses reviewed_source_build contract; rawDB/RPC/capabilities stayprivate.

Root817 typecheckPASS; libfirstcompileE0753/0tests preserved, exact2commentfix01. Exact01selected16unitsPASS/0skip; genuine14→15 sixfunctions/10logicalPASS/0skip/test1.86s, inputthreehashesunchanged. Actual15normaltools same01built20.12s/probed1.50s12zero/noForce. Normalshared14→01actual15 transition explicitly recorded, frozen14/a07/2a retained.

Currentblockingruntime2: exact01 actualchildprocesscaseFAILED exit101/test27.04s/total43.865, original90s/200MiB guards nothit. Parentactualstop/reap throughseq40, then noGoaladvance/Worker/Owner/TaskAccepted; twonodesnotstarted. Actorjoinedok/privateDB+stderr+failedELF preserved. RootSelf secondcaseNOTRUN. Independentbc66ad04/660c268f confirms limited observations, not a unique cause and not a failure/pass of postSnapshotphase, which was not reached. Coreauthor implements bounded debug-only observational trace; no TTL/CAS/worker/permission relaxation.

Desktop realdriver NOT_RUN; reviewer found/fixed14byte cancellation literal accidentallychangedwithschema and is closing strict failure preservation/noimplicitstart/historicalNativeGrant checks. ExplicitNativeGoal approval maychangefixture Grant0→3 beforeFactory consent; thereafter Factoryderivedscope cannotraise thatapprovedGrant. Additionaltwoauthorityfaulttests e95 are not integrated/run yet; readonlylegacylist classification gap pendingactualevidence.

Root CI newfinitepublicmetadata projection0ab replaces untyped dict forwarding;8pure+realmetadata projection passed, independentreviewpending. Existing7checks/runners/pins/timeouts/permissions preserved; no formalApprove/nextmerge/production/signing/privateProvider grant. Full130/27/WP01–14 continues.
