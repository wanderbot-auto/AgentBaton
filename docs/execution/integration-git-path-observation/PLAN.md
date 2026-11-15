# Integration 同次 Git 路径观察

基线 `89f83e6bc2968f3d70cb449135f828e4ade2dafe`；源码 `0793d53eee3127c1b263411fb82a329c7c00a5a2`，仅测试访问修复 `3b2ac87bc5a511ef8fe78fdc64cd0ec8f079afd4`；分支 `codex/integration-git-path-observation`。这是既有完整设计的窄性能切片，不改需求状态或验收标准。

1. 每次原 recheck 内把三个路径 rev-parse 合成一次 fresh bounded observation；严格三完整字段、原实体 pin/路径规则；保留独立 bound HEAD、全部原重检点和真实原时钟计费。
2. 保留原生产测试，新增10项实际 Git/路径/退出/取消/超时/HEAD负向合同；整源码和测试做严格逆向证明。
3. 独立审查后在主树验证全部 Workspaces45与实际 Integration IPC33；保留首编译失败，不重复叠加10+35。
4. 准确提交的 macOS 双架构、原2s/5s预算、完整CI与桌面交付仍需主代理后续验证；第二 config batching 不在本单元。

路径比较保持原 Rust Path 组件语义，并非逐字节词法拒绝；没有跨 recheck/tick 缓存、预算扩张、IO折扣或未知结果提升。

## Follow-up · isolated config fresh read

Root `d3098c40921ec3f3cb46266e60234775b156f13b` normally integrates reviewed production2a1 and test-onlyc900. Extend this existing narrow workstream with the separate same-boundary isolation configuration batch: three declaration reads become one fresh bounded name-only/NUL read. Keep every original source/HEAD/FD/path/cancellation/effects/accounting boundary and original runtime/observer/retry/round limits. The earlier plan's exclusion of a second config batch remains historical; this follow-up records its now separately reviewed implementation.

Keep old55/1 failure and oldquery actual measurements; require byte refusal for forbidden nonUTF8 names. Record conservative all-name4MiB behavior explicitly. Workspaces has passed, but exact Root Integration32/1 is a blocker: resolve the bounded Host Git expiry under original limits, obtain independent review and actual validation before freeze/commit/push. Root strictClippy/infra pins remain pending; different Core Clippy evidence cannot substitute. New precise Mac/CI, desktop and full130 acceptance remain separate.

## Staged fixture validation phase · awaiting commit mapping

For the same workstream, fixed generated drift objects can be prepared before policy/start under independent setup500 bounds. Actual live-stage/round/unsettled proof, targetCAS+confirmation+release stay within the unchanged original marker500; source/target snapshots prove no early ref/source changes. Only four negative cases change; positive/withdrawal and product code stay intact. Freeze/commit mapping remains a main-owned next step after independent actual evidence audit; this metadata draft must not invent an exactcommit for testedtreeb8.

## Resolved source mapping · metadata review next

After applicable stagedchecks and independent actualaudit, main committed exactb8 as `2d04d6cc8b666e6110b1cb901fd5e804876be793` and normally fast-forwarded the existing local assembly branch fromd309 to that samecommit. No separate mergecommit exists. Metadata now bases on2d04; only its seven documentation paths are staged for independentreview and applicable static/infra checks before any metadata commit. RemoteCI/dualMac/native/installers/full130 remain separate.
