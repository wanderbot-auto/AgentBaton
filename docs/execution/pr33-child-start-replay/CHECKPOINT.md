# PR #33 子 Goal Start 排查交接

原始驱动在同一托管树的 Linux ARM64 重放中通过 normal、lost-start、cancel。两轮纯观察 normal 也通过。两次受控并发让真实启用按钮的前置 Context 收到 Core 的 `WORKSPACE_BUSY / false / not_started`，两次均显示警告、退回禁用且 pending 为 none，桥接 Start 为 0，复现 2143 → 1801 的原期限失败。第三次 Core 同样拒绝并发读取中的两条，渲染器自身读取先完成，实际 Start 发送一次。两次只改浏览器交付的对照也复现警告和零 Start。

两条真实 stale CAS 拒绝在各组保留。跟踪确认了同类拒绝能造成给定失败形状。托管真实点击和竞争来源仍缺直接证据。后续候选应在面板内协调背景 Context 与用户前置读取，并保持对外部拒绝的原行为。 [复现和认领建议](REPLAY.md) · [有限证据](EVIDENCE.json)
