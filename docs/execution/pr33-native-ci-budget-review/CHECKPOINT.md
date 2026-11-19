# 检查点

待 Leader 审查。Base `ae844ca33406aabd62f44ea73818f9127f211748`，已测修复源 `443556eedd09a74ff51dc13859c78a142e7fdacc`，树 `b1b705e2581c0da061dd8441e0ebf294d6062f7b`。精确交付 HEAD 见本任务 `artifacts/pr33-native-ci-budget-review/HANDOFF.json`。

- 当前托管 macOS 在外层 WebView 证据校验失败，时间 53:13。实际驱动退出 0，报告保留真实运行和回收事实。后续审批、正常关闭到后台恢复、默认发布审计和打包均无验收记录。
- Store19 修复通过同一真实报告的离线红到绿回放，53 项来源护栏和 Linux arm64 38 项合同通过。macOS 完整 runner 的一个基线负控失败仍保留。
- 超时、检查、权限和 CI 文件均维持原有配置。没有启动新 GUI 或触发托管作业，未更新 PR。时长建议待独立审查和实测。

[时长审计](BUDGET_REVIEW.md) · [机器可读证据](EVIDENCE.json)

## 历史构建器兼容复验

前交付 `e8060878b4df5c0f40d2e59647ee82a6d1c4e00b` 的 Store19 修复与证据已保留。已测兼容源码为 `5d55e86356f6f1269d850dcbcdfcc4474d0a756a`。历史构建器先验证精确当前 runner 与反演文件字节，恢复后只执行原历史 SHA 的 runner。测试直接核验传入编译器的字节。

14 项 schema18 和 8 项 schema19 定向通过。全量运行 1065 项，1049 通过、16 跳过。真实 Apple Silicon 迁移和恢复门通过，包含 14 个原拒绝检查。编译预算、清理、GitHub CI、PR 和历史输入保持原配置。见 [兼容取证](SCHEMA18_COMPATIBILITY.json)。
