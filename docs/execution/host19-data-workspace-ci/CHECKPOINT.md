# 交付检查点

- 固定构建 base `3d19a89f10756966da9f494f928d6962ad63b20b`，验证驱动提交 `ed26a5f3da080d6bc602a1b785e7102c05b263f6`。验证提交之后只有本目录证据。现行功能集成目标与该构建的 Rust、锁和前端输入一致，合并预览后待 leader 集成。
- 现行无清单路径使用实际 ARM64 preview-evidence Core、React 构建和 Playwright 1.62.0 的本机 Chromium 151.0.7922.34。可选清单实跑复核了相同工具与 Core 构建来源。IPC 与只读 `PRAGMA user_version` 两侧均得到实际整数 19。
- 保留了真实取消响应丢失和 remount 对账、重复键、原数据 bytes/mtime/SHA-256 与派生元数据断言。两份最终报告记录前置 gate、事件承诺、退出等待和浏览器关闭。两次 daemon 都 wait 0，HTTP 均 join，无强杀。
- 有针对性证明 `schema18`、`schema20`、布尔 schema 及混配伴随工具均拒绝。额外桌面全量仍有 4 个已独立复现的基线 Notifications/Integration 问题。托管 Linux、native WebView/OwnerDialog、Provider 与中途读写取消均未测。本工单无 workflow、产品权限或其他消费者改动。

[验证索引](EVIDENCE.json) · [源码合并预览](logs/source-preview.log.gz) · [现行无清单完整报告](logs/current-audited/report.json.gz) · [候选清单完整报告](logs/manifest-audited/report.json.gz)

## 独立审阅与交接

集成负责人独审接受 `f853be6b352ca050ff870de36709d3a594b1229b`。复核清理负例、58 份归档、83 个解包摘要、12 张最终截图及两次真实浏览器执行各 8 项通过、0 未运行。确认两次 daemon 均 wait 0，HTTP 均 join。独立复跑 9 条定向测试通过，Notifications/Integration 的 4 个既有问题在固定旧源及新组合均复现。本次交接只更新任务证据，保留构建、浏览器产物、工作树与分支。
