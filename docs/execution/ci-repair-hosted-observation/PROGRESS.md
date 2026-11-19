# 执行进展

- GitHub 连接器确认真实作业结论。其投影省略候选 SHA、attempt 和完成时间，随后用本仓库已授权的只读 REST GET 核对缺失字段。
- 本机于 2026-11-19T03:38:04Z 采集有限字段。workflow_dispatch 使用同仓库的真实 run、attempt 作业列表和独立候选查询。PR 控制使用同仓库真实 run 与 job。仅保存公开 ID、候选字段、结论与时间。
- 确认为同一 attempt 的 GET 回复与工作流列表可能有不同的 updated_at。完成时间取完整作业列表的实际 completed_at，同时约束两个 run 的时间上界。调用端的时间窗支配观测及失败作业的新鲜度。
- 映射接入现有 CiObservation 和决策。直接响应正控只得到 ContinueRequired，因为测试没有配置修复规则。负控覆盖查询身份、分页缺失、旧 run、rerun、PR、过期及未知状态。
- 精确源码在 Apple Silicon 通过 GitHub 映射 8 项、既有决策 9 项、workspace 格式检查和严格 Core Clippy。测试无需网络，工作树及认领范围已核对。
