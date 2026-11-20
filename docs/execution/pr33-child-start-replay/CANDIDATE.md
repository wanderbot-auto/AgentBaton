# 子 Goal Start 同面板 Context 仲裁

工作树基准为 `36a99a4fbcc58424b01815cdd44414317a1fbb3c`。原版排查证据独立封存于 `ab7a45ca5a8ca8550882d44a63057592be380b29`。候选实测源码为 `d87ce5e0cf6b3d37cd32728b535d7ae671f9320b`，树为 `bb2b369480b777f26e52ab0ded7750d2d05131b5`。

`ChildPlanPanel` 对当前 scope/task 的计划历史和子 Goal 的 Context 请求使用同一个串行入口。前一个 Context 完成后，新的读取才派发，并获取自己当前的 Core 证明。等待中的旧历史 generation 会在派发前被现有 epoch/source token 检查舍弃，留下最新的一轮。其它摘要、历史和原始计划验证按原逻辑执行。用户关闭子 Goal 详情时，已排队的前置读取会退出。在前置读完成后、Start 真正派发前，再核对详情仍打开且面板仍挂载。

外部占用返回的原始 Busy 仍进入既有拒绝路径。已发送请求继续使用原回执调和逻辑。此次修复没有新增 Start 重试、切换权限、改变 Core/IPC 或共用已缓存证据。两条真实 CAS 拒绝、计划绑定、键、权限检查，以及驱动的原 5 秒／15 秒窗口保持原样。

## 判别性重放

使用真实 Chromium 的单次可信 Start 点击。私有探针触发实际父记录刷新处理程序，暂缓浏览器交付子的 Goal list，直到同一面板的子计划历史 Context 进入。原版让两条真实 Context 在同一轮派发，两次均出现真实 Core Busy，一次由显式前置收到，一次由背景读取收到。候选必须在背景 Context 回应后才派发前置 Context，各自走原 bridge 和 Core，随后才允许恰好一次 Start。

额外三条同 scope/task 的只读 Context 从面板仲裁外部直接进入私有 bridge 用于外部占用对照。若原前置请求收到 Core Busy，要求零桥接 Start。关面板对照在同面板历史占用窗口内关闭真实 Goal 详情，要求零前置 Context 和零 Start。所有探针在私有目录，原驱动和有限投影器与托管版本逐字相同。

## 历史守卫

`SOURCE_INVERSE.json` 对修改后的整个 Goal 组件做精确源逆转，先回到原 plan-read-reuse 整文件。共用 `normalize_child_goal_outer_layer` 只对已钉住的新版 v4 测试守卫做 `GUARD_INVERSE.json` 的逆转，验证每一段、manifest 和整个原文件，再进入现有 Schema18 与 capture 链。历史 pin 和负控仍由原入口验证。

## 已验证的浏览器结果

| 对照 | 结果 |
| --- | --- |
| 原版，同面板重叠两次 | 两次均在原回执等待失败，各有一条实际 Core Busy，桥接 Start 均为 0。 |
| 候选，同面板重叠两次 | 两次均等背景 Context 回应后发独立前置 Context，各有一个 Start 回执并完成清理。 |
| 候选，等待期间关闭 Goal 详情 | Goal list 已交付，详情在背景 Context 完成前关闭。前置 Context 被丢弃，Start 为 0。后续父刷新仍读取计划历史。 |
| 候选，外部并发三次 | 两次实际 Core 拒绝前置 Context，均显示原失败且 Start 为 0。一次原前置读取成功且只发送一次 Start。 |
| 候选，原驱动无探针 | normal、lost-start、cancel 全部通过，投影完整。lost-start 的显式调和重用同一请求键、CAS 和精确计划。 |

每个案例都通过两条实际 `STALE_REVISION` 拒绝。故障对照仍命中 `2143 → 1801` 的原回执期限。其投影失败是对照的预期结果，未将它计为成功用例。关闭详情后，后续父刷新照常读取同面板的历史 Context 和 list。候选的本地浏览器证据覆盖实际 Core，但托管失败具体读取者仍未识别。 [托管失败](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44778549757/job/136434794905)

Desktop 全量 Vitest 1601 项通过，TypeScript 检查通过。初次 Core 冻结遗漏目标目录挂载，冻结脚本拒绝准备候选。保留错误记录，在补挂载且重新生成前端证明后成功完成冻结。第一次基础设施全量测试缺主机 `jsonschema`，保留该次记录并用仓库 pin 的依赖重跑，全量 1092 通过、16 项跳过。

[最终有限证据](CANDIDATE_EVIDENCE.json)
