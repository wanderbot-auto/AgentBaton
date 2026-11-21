# AB-REL-006 GitHub Actions 观测映射

- 基准 `53da4ce20fdfedae4a745e704394072ad6b742ae`。只写认领的 Core 模块、直接测试与本目录。
- 使用获批 GitHub 仓库的只读 Actions REST 响应核对 attempt、候选工作流列表和作业字段。仅接入受信 actor 已获取的响应。现有决策继续检查独立配置规则、权限和预算。
- 固定 repo、workflow、候选提交和分支。按独立完整列表判定最新 run，比较 attempt 与终态。通过实际作业完成时间核验终态，保守处理 rerun、PR 与不完整响应。
- 测试真实公开响应的字段投影以及合成负控，核对旧决策契约和格式。OWNER-001 已核对，集成窗口维护全局状态。
