# AgentBaton 环境准备清单

状态：准备中，产品开发未授权。

| 项目 | 状态 | 证据或限制 |
|---|---|---|
| 私有仓库 wanderbot-auto/AgentBaton | verified | 用户创建，connector get_repo 确认 private/main；已写入初始提交 |
| 产品开发批准检查 | closed | infra/execution-gate.json 保持 false；不自动解锁 |
| 完整 v0.2 设计与契约 | uploading | 原始文本和机器规范按原哈希保存；最终以 Git tree 和校验为准 |
| Python 基础设施测试 | local_passed | 当前容器重跑 11 项全部通过；不替代远端 CI |
| Linux / macOS arm64 / macOS x64 预检 | prepared | 仅手动触发或修改 infra/ci-request.json 触发；最多 3 jobs、每 job 25 分钟 |
| GitHub Actions 实际运行 | pending | 尚无可据此宣称通过的 run ID |
| Codex Cloud 环境 | not_published | 操作说明已准备；服务端创建和发布未完成 |
| 分支保护 | not_configured | 没有应用规则，不将仓库权限等同于规则保护 |
| 产品原生构建、真实 Provider、签名发布 | not_run | 没有产品源码实现，不注入账号凭据或签名材料 |

本清单不等于完成报告。真实 CI、Cloud 发布和最终用户确认必须分别记录。
