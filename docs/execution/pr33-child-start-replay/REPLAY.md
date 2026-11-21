# PR #33 子 Goal Start 回执失败复现

托管正常用例在真实 CAS 拒绝检查之后完成 CG01、CG02，随后在驱动 `2143 → 1801` 的 Start 成功回执等待失败。窗口内桥接 Start 为 0，固定投影记录邻近的一条 `factory_child_plans.context → WORKSPACE_BUSY / retryable=false / not_started`。同一托管工件的 lost-start 和 cancel 通过。托管投影缺少失败点击及对应 Context 的生产者绑定，无法确定是哪个页面读取占用了 worker。 [托管 Product UI 作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44778549757/job/136434794905)

## 构建与执行

- 托管记录和重放使用 merge 提交 `91fc92d2121cfc52a76c32c21d65965b0794ac28`。它与 PR head `36a99a4fbcc58424b01815cdd44414317a1fbb3c` 的树均为 `43aea670c1008f86d7244001586a24376d70c124`。本地在独立 checkout 执行，无跟踪文件变更。
- 通过 `scripts/collab.sh run` 在 Docker Linux ARM64 容器内运行，`--network none --pull never`。使用任务独立 socket、token、输出和生成 ACP 测试程序。Core 在 Rust 1.91.1 容器执行工作流的 `prepare-schema19-browser-ci.py core`，底层为 `cargo build --locked --bin batond --bin baton --features agentbaton-core/preview-evidence`，调试信息级别 0。浏览器容器使用现成的 Node、Playwright 和 Chromium 151.0.7922.34 依赖，执行同脚本的 `frontend` 和 `freeze`，随后运行原驱动和证据投影器。
- 原驱动用 `factory-child-goal-real-ipc.py --run --case normal|lost-start|cancel --tools … --frontend apps/desktop/dist --manifest … --chromium … --output …`。每次启动独立的实际 `batond`。Chromium 照常点击更新父记录、计划历史、r1、子 Goal 刷新和 Start。正常观测仅在浏览器脚本里记录可信指针、命令、有限错误、DOM 状态和相对时序。
- 真实并发对照在可信 Start 点击后，等子 Goal list 成功，再对前置 Context 原请求加三条相同 scope/task 的只读请求。第四条是渲染器自身的 Context。所有请求走原 HTTP 验证和 Unix socket。等待三条对照读收齐后把渲染器原回应交还给页面。无回应替换、无额外 Start、无新审批或运行权限。每次保留真实 wire 回应的有限字段和站点分类。
- 浏览器交付对照让 Core 照常成功处理前置 Context，只在浏览器收到时将那条回应改为相同拒绝。这个对照确定消费端行为。真实并发对照另行验证 Core 能产生相同拒绝并由 UI 原样消费。

## 前后端路径

1. 父 records poll 可每秒触发一次有限刷新。即使业务 DTO 相同，读 generation 也会增加。打开的子计划面板依 generation 有独立的 Context → list → get/验证链。子 Goal 面板有另一条 Context 请求链。 [父刷新](../../../apps/desktop/src/FactoryRuntime.tsx#L52) · [子计划刷新](../../../apps/desktop/src/FactoryChildPlans.tsx#L24)
2. Start 的真实按钮点击进入 `act('start')`。它先重新读取 scope、Factory run、snapshot、grants，再读取 child Goal 列表与 Context。Context 错误结束本次刷新，撤销 fresh proof 并展示警告。只有 Context、最新已保存引用、原始字节、父绑定及当前权限验证通过才生成并发送 Start。 [Start 前置读取](../../../apps/desktop/src/FactoryChildGoals.tsx#L95) · [Start 动作](../../../apps/desktop/src/FactoryChildGoals.tsx#L200)
3. 真并发拒绝对应 `factory_context_for_request` 的 “Stop active workers before a new Factory owner read” 分支。该 Context 的 child planning 路径允许普通 Provider worker 共存，仍检查共享 data worker 的活动或待收尾状态。后续 `defer` 也有 Busy 分支，错误文案不同。托管有限投影没有错误文案。 [Core owner read guard](../../../crates/core/src/ipc.rs#L2901) · [Core worker guard](../../../crates/core/src/data_workspace_workers.rs#L469)

## 后续文件认领与验收

优先认领 `apps/desktop/src/FactoryChildPlans.tsx`、`apps/desktop/src/FactoryChildGoals.tsx` 和现有的 `apps/desktop/src/FactoryChildPlans.read-cycle.test.tsx`。以当前 scope/task 面板为边界协调后台 Context 与显式 Start 的前置 Context。同步标记显式前置读取，让已派发的同面板 Context 结束，后台仍保留最新一轮待刷新，动作前置读取独立取得新证据。用现场的两条读取链和现有 read-cycle 测试固定行为，避免把前一 generation 的结果当作刚取到的授权依据。释放后执行最新的待办只读刷新。若采样发现其它读取者仍能占住 worker，再按具体调用方单独认领。当前证据支持先收窄同面板竞争，未建立托管归因。

- 固定正向测试。父 generation 触发的后台 Context 正在运行，显式点击后前置 Context 在其结束后才派发。前置读验证精确 r1 后只发送一次 Start。
- 固定拒绝测试。前置读取遇到外部 `WORKSPACE_BUSY/false/not_started`、期间换 scope/task 或 revision、计划变化、授权失效或关闭面板，Start 为 0。恢复读取只更新渲染状态，启动仍需要新的显式点击。
- 保留真实驱动的两条 `STALE_REVISION` CAS 拒绝和原 5 秒／15 秒窗口。保留 lost-start 的已提交回执调和和 cancel 的确认。若托管仍红，再认领驱动和现有有限投影测试，发布有界阶段和请求来源，足以区分父/子历史自动刷新与 click preflight。
