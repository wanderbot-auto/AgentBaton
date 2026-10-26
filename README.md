# Development authorized · Codex Cloud handoff

The owner has approved complete Design v0.2 development. Read [the execution prompt](codex/MASTER_PROMPT.md), [the goal](codex/GOAL.md), [the runbook](codex/RUNBOOK.md) and [the approval record](docs/execution/launch/APPROVAL.md).

[Development status](docs/execution/STATUS.md) · [Owner instructions](docs/execution/OWNER_INBOX.md) · [Current execution permissions](infra/execution-gate.json)

[Product development commands](docs/DEVELOPMENT.md) cover the current Rust daemon/CLI, desktop frontend, product tests and development packaging. Exact implemented and unverified scope remains in the execution status and requirement ledger.

This handoff is not evidence that a Cloud task has started or that the product is implemented. Preserve main protection; use branches/PRs and real CI. Production release, extra paid resources, signing and product Provider test accounts/calls still require separate approval. Do not rerun the historical repository-creation script.

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

## 当前状态与使用方式

GitHub 仓库、环境准备和主分支保护已完成核验；用户已明确批准研发。当前尚未登记真实 Cloud 开发任务、应用实现、产品级测试或安装包，不把基础设施预检当作产品通过。

从 `AGENTS.md`、`codex/MASTER_PROMPT.md` 和当前执行授权开始，实时状态见 `docs/execution/STATUS.md`。保留完整需求；缺少实机或认证仅阻塞相关验证。原设计文档中的初始化状态属于历史快照，不替代当前授权和执行记录。具体许可证、依赖兼容和最低系统版本仍按验证程序确定。

文档校验命令（需要Python与jsonschema）：

```bash
python3 scripts/validate_design.py
```

该命令不编译产品、不运行Agent、不产生安装包。文件manifest仅用于完整性核对，不是数字签名。
