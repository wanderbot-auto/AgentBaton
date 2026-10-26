# 13 · 官方资料与证据边界

资料来源：继承v0.1的2026-10-26引用索引。本轮v0.2只整合用户决定，**未重新在线核验这些接口或执行真实集成**。下表保留为实现者的核验入口与历史说明，不作为现行版本兼容证明。锁版实现前须逐一重新读取官方资料并记录证据。

| ID | 来源 | v0.1记录的参考范围（实现时重新核验） |
|---|---|---|
| S01 | [Codex Cloud](https://developers.openai.com/codex/cloud/) | 云任务使用项目环境，每任务独立工作区，可检查变更与结果；不代表本会话已启动任务 |
| S02 | [Run long horizon tasks with Codex](https://developers.openai.com/blog/run-long-horizon-tasks-with-codex) | 长程工作实践使用规格、计划、执行规则和验证/状态记录 |
| S03 | [ACP overview](https://agentclientprotocol.com/protocol/v1/overview) / [initialization](https://agentclientprotocol.com/protocol/v1/initialization) | Client-Agent方法、事件、版本与能力协商；恢复是有条件能力 |
| S04 | [Official ACP Rust SDK](https://github.com/agentclientprotocol/rust-sdk) | 存在官方Rust SDK；具体API和协议扩展须锁版 |
| S05 | [Codex App Server](https://developers.openai.com/codex/app-server/) | 提供skills/list、mcpServerStatus/list等原生接口；返回结果有runtime/session范围 |
| S06 | [codex-acp](https://github.com/agentclientprotocol/codex-acp) | ACP stdio adapter转换到Codex App Server |
| S07 | [Qoder Skills](https://docs.qoder.com/cli/Skills) | 官方Skills文档与不同作用域的配置来源 |
| S08 | [Qoder MCP servers](https://docs.qoder.com/cli/mcp-servers) | 原生MCP配置/管理文档；不等于Baton能无条件查询所有运行会话 |
| S09 | [Qoder ACP](https://docs.qoder.com/cli/acp) | 官方qoder --acp、stdio、原生登录状态与能力说明 |
| S10 | [claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp) | 基于Claude Agent SDK的ACP adapter项目存在；不是任意TUI会话接管保证 |
| S11 | [MCP specification](https://modelcontextprotocol.io/specification/2025-06-18) | 协议的tools/resources/prompts分工；本引用为有日期的概念基线而非宣称最新版本 |
| S12 | [Qoder configuration order](https://docs.qoder.com/cli/settings) | Qoder有原生配置作用域和应用顺序，需按版本解析 |
| S13 | [Ghostty shell integration](https://ghostty.org/docs/features/shell-integration) | 历史引用；外部会话观察已退出产品范围，不再作为实施依赖 |
| S14 | [Tauri capabilities](https://v2.tauri.app/security/capabilities/) | Tauri应用IPC权限控制；不是所有Agent子进程的OS沙箱 |
| S15 | [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/) | 推荐WDIO服务、embedded支持macOS；直接tauri-driver路径有不同平台限制 |
| S16 | [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/) | GitHub Actions构建分发路径；仍需要真实仓库、权限、runner与工作流 |
| S17 | [Running Codex safely](https://openai.com/index/running-codex-safely/) | 沙箱、审批、网络、身份和原生日志是不同边界 |
| S18 | [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/) | macOS签名、公证流程需要实际开发者凭据与验证 |
| S19 | [AGENTS.md guidance](https://developers.openai.com/codex/guides/agents-md/) | 项目指令分层读取；入口保持短并引用详细文档 |

## 未核验与不可推断

本次没有实机测量Provider版本、认证兼容性、具体额度、ZCode适配器质量、AgentBaton名称商标、开源许可相容性或云账户权限。不沿用过去对竞品每个功能的结论作为本设计事实。

设计建议（默认权限、状态机、工作包、UI与schema）是本方案提出的工程设计，不是外部官方标准。它们需产品负责人批准和软件测试，不能因为附有文献就视为已实现。
