# Codex Cloud 环境创建配置（已准备，尚未在服务端创建/发布）

本文件是操作配置说明，不是 Codex 自动读取即可创建云环境的 API 文件。

- 名称：`AgentBaton Development`；访问范围：Only me / 私有。
- 仓库：`wanderbot-auto/AgentBaton`（私有，已由用户创建）；默认分支 `main`。
- 网络：以包管理器来源为起点，按实际缺失域名追加，不开放无关网络。
- 运行时偏好：Node 24、Python 3.12、Rust stable；首次实际安装成功后记录精确版本并审定工具链锁定，当前不声称已锁定或安装。
- Install script：在实际 checkout 根目录运行 `bash scripts/setup-dev.sh`。
- Start instructions：读取 `AGENTS.md` 与 `infra/execution-gate.json`；只检查依赖、运行环境 smoke checks、报告失败；不要调用真实 Agent、执行产品工作包或启动收费服务。
- 暂不添加个人 Agent 登录文件、生产密钥、Apple 签名材料或 API 计费密钥。
- 发布环境仅表示准备镜像可用于后续任务，不代表用户批准启动产品开发。

## 官方当前流程
在桌面或 Web 端创建环境，选择真实仓库，让 Codex 准备和测试，审阅后发布。保存仓库脚本并不等于云环境已发布。手机可使用已经发布的环境，不替代创建发布步骤。

来源：
- https://help.openai.com/en/articles/20001545-using-codex-cloud
- https://learn.chatgpt.com/docs/environments/cloud-environments

## 完成证据
填写环境名/标识、关联仓库与提交、实际安装日志、网络范围、已发布状态和预检结果。缺失任何一项则仍为 prepared / blocked，不得标 ready。

## 产品开发的独立确认
收到用户在环境报告之后的新确认前，保持 gate=false。创建环境时的准备检查不启动 `codex/MASTER_PROMPT.md` 所描述的产品长程任务。
