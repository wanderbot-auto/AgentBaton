# Codex Cloud 环境设置：仅准备，不启动产品开发

仓库：`wanderbot-auto/AgentBaton`，私有，`main`。
建议环境名：`AgentBaton Development`；共享范围：Only me。

GitHub三平台基础设施预检已通过；Cloud环境仍须在用户账号下实际创建、检查及发布。当前对话未完成此服务端操作。

## 设置流程

在网页或桌面端使用`Work in → Cloud → Select environment → Create environment`，或`Settings → Codex Cloud → Environments → Create environment`。选择仓库后让Codex准备依赖和检查；审阅结果，完成Publish，确认Environment published。

官方来源：https://learn.chatgpt.com/docs/environments/cloud-environments

## 在环境准备对话中使用的指令

```text
只准备 wanderbot-auto/AgentBaton 的开发与测试环境，不执行产品功能开发。
先读 AGENTS.md、INFRASTRUCTURE.md、infra/execution-gate.json 和本文件。
准备 Node 24、Python 3.12、Rust stable（rustfmt、clippy）；缺少工具时按官方方式安装。
在仓库根目录执行 bash scripts/setup-dev.sh，保存实际版本、命令与测试结果。
可检查后续Tauri所需系统依赖，缺失项如实记录；不要生成产品应用或修改既定设计。
不添加个人CLI登录文件、API计费密钥、生产或Apple签名凭据，不启用额外收费服务。
保持product_development_authorized=false；不要执行codex/MASTER_PROMPT.md中的实施工作包。
准备检查通过后提交环境报告，等待用户审阅和发布环境；不要开始新的产品任务。
```

## 可复用安装与启动设置

Install script可使用仓库根目录的`bash scripts/setup-dev.sh`，前提是Node/Python/Rustup已准备好。
Start skill读取批准检查状态，只做环境诊断，不启动产品任务。网络先使用所需包管理器来源，缺少域名按实际错误核验；不默认添加私人登录或生产秘密。

本脚本测试的是设计和基础设施，不会证明全部未来Tauri/Provider依赖已兼容。精确工具链版本需保留实际记录，不能把stable当作已锁定版本。

## 发布证据

记录环境名称/标识、关联仓库和提交、安装日志、检查结果、网络及共享范围、Environment published状态。只有脚本或保存草稿时不得标为ready。发布环境本身不批准产品开发，用户尚需在最终准备报告之后另行确认。
