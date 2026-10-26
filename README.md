# Preparation status: REPOSITORY + CI VERIFIED; CLOUD SETUP PENDING

Private repository preparation and Linux / macOS arm64 / macOS x64 environment checks passed. See [verified checklist](infra/PREPARATION_CHECKLIST.md), [Cloud setup](codex/ENVIRONMENT_SETUP.md), and [execution gate](infra/execution-gate.json).

**Codex Cloud has not been created/published by this session. Product development remains unauthorized.** No application build, real Agent integration or signed installer is claimed. CI evidence is bound to commit `02508d4eb1c32d35cc9deda546f334938abdd357`; this later report update does not claim another remote run.

---

# AgentBaton · 完整产品与工程设计

**Design v0.2 · 决策整合版 · 2026-10-26**

一个主入口，一套明确归属的能力配置，多个原生Agent执行引擎；目标驱动自动执行并提供可验证的交付。

本包替代v0.1作为后续研发设计基线，完整目标不缩减为短期原型。用户完成27题及此前主入口/环境决策；精确依赖版本、许可证和最低macOS仍按验证程序确定。**设计定案不等于软件已经实现，也不等于对真实账户、生产发布或云付费授予执行权限。**

## 阅读路径

1. [已确认决策](docs/12-decisions.md)：本轮选择与验证待办。
2. [产品需求](docs/01-prd.md)、[交互](docs/02-experience.md)、[架构](docs/03-architecture.md)。
3. [连接与运行环境](docs/04-providers-sessions.md)、[能力管理](docs/05-capabilities.md)、[任务编排](docs/06-orchestration.md)。
4. [远程与运行维护](docs/10-operations-extension.md)、[安全](docs/08-security.md)、[测试发布](docs/09-testing-release.md)。
5. [工程工作包](docs/11-engineering-program.md)、[Codex主指令](codex/MASTER_PROMPT.md)、[执行规范](codex/RUNBOOK.md)。
6. [修改清单](CHANGELOG.md)、[机器规范](specs/README.md)、[验证待办](docs/15-validation-gates.md)。

## 当前交付状态

v0.2设计基线、需求、设计schema与静态示例已纳入环境准备。私有GitHub仓库已创建；基础设施测试与CI单独记录。**没有产品功能实现、真实Agent集成、macOS应用测试、安装包或Codex产品长程任务**。当前状态见 `infra/PREPARATION_CHECKLIST.md`。原资料索引是实施时需要重新核验的参考，不将设计或环境预检冒充产品测试。

## 使用方式

将本包纳入真实项目仓库，读取`AGENTS.md`和`codex/MASTER_PROMPT.md`，提供执行授权、仓库/分支、资源范围及预算后再实施。不要因缺少某项实机或认证而删需求；标记阻塞并继续独立可推进的获批工作。

文档校验命令（需要Python与jsonschema）：

```bash
python3 scripts/validate_design.py
```

该命令不编译产品、不运行Agent、不产生安装包。文件manifest仅用于完整性核对，不是数字签名。
