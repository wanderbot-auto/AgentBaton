# PR #33 Owner Store19 验证修复

- 基准 `ae844ca33406aabd62f44ea73818f9127f211748`，目标 `origin/collab/host-registration-integration`。
- 核对 Actions 作业 135276914618 与真实普通 Cargo 工具的基准复现。以同一对构建工具运行修改后的直接测试，避免把源码、数据和环境变化混在一起。
- 将 Grant 和 Factory 只读证据绑定到 Store19，并按原零副作用契约检查 Host 注册表。覆盖旧版、未来版、类型错误、缺表、同名视图及 Host 行污染。
- 给当前 runner 增加一层固定字节反演，保留两层历史及最终哈希。运行直接测试、完整基础设施套件和合并预检。
- 已核对 OWNER-001。集成窗口负责 PR、组合验证和全局状态。此工作只运行无 GUI 的本地 fixture。
