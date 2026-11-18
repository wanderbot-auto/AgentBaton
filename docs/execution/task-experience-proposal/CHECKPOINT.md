# AB-VAL-008 交接检查点

| 项目 | 固定引用 |
|---|---|
| 任务分支 | `collab/task-experience-proposal` |
| 认领基线 | `f10344872a3d2aa7d4f65bf9d8a9c6de2c85ed6c` |
| 已通过桌面前端验证的实现 | `da7e3ebaeddeca8dd4be183f7a592d8d61b3eb40` |
| 已通过基础设施复跑的提交 | `1bbeaaadc65034158f0ff92ed3f12f23fdd55ce0` |
| 预检的集成目标 | `collab/host-registration-integration` @ `f4b1e5217085fe20c9122b3fae12bff535d24930` |
| 接续 | `leader-control` 集成窗口独立审阅、验证合成树并决定合并 |

产品代码范围限定为 `App.tsx` 的详情入口及经验候选组件、数据模型、双语文案和两个测试文件。基础设施复跑提交及后续交接记录只改任务内文档和证据。全局执行状态由集成窗口更新。

从任务卡片进入详情可在具有完整验收投影和对应项目的任务上创建候选。候选预览显示类型、实际项目和 ID、任务 ID、任务与规格修订、验收引用和序号、摘要与可复用内容。编辑和预览使用临时 React 状态。项目不匹配、验收投影失效或任务上下文变化时清除草稿。待后续接线的持久审查和启用流程继续受已有托管能力授权约束。

范围与冲突预检记录见 [scope-preview.txt](logs/scope-preview.txt)。测试和环境见 [EVIDENCE.md](EVIDENCE.md)。
