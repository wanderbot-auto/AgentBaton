# AB-EXT-006 所有者审定材料

D09 已确认本地核心可独立使用和开源，允许增值能力商业化，并重视对核心改进的回馈。项目许可证、分发方式和贡献条款均由所有者正式决定。当前交付列出可审定方案。 [D09](../../12-decisions.md) · [验证门](../../15-validation-gates.md#4-开源与分发)

## 回馈与贡献

| 机制 | 对回馈的影响 | 需审定事项 |
| --- | --- | --- |
| 宽松许可与自愿上游 | 下游保留所选许可要求。可用公开变更指引、提交模板和 DCO 促进上游回馈。 | 决定专利条款、署名及 NOTICE 义务，确定自愿回馈是否达到 D09 目标。 |
| 文件级回馈 | 以 MPL 一类文件级机制为候选，对分发的受覆盖文件提供对应源码。独立文件或独立商业服务的边界按实际版权与组合方式审核。 | 确定覆盖范围、代码生成路径、对第三方依赖的兼容性和源码交付方式。 |
| 更强回馈策略 | 如需覆盖更广的衍生作品或服务访问场景，单独评估其对插件、远程伴随、服务部署和商业产品的影响。 | 在确定许可证前完成依赖兼容和权利链审核，明确适用触发条件。 |

可从贡献者提交来源证明、公开 PR 和可追踪的维护记录开始。DCO 证明来源与贡献权利，额外的双许可权或著作权转让须由权利人单独约定。按文件、模块、进程及 API 明确核心与商业代码的权属和交付边界，并验证核心能独立运行。 [DCO 1.1](https://developercertificate.org/) · [OSI 定义](https://opensource.org/osd) · [Mozilla MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)

## Provider 独立权利与认证

| 路线 | 可复核材料 | 审定缺口 |
| --- | --- | --- |
| Codex 原生 App Server | 固定 OpenAI 源码提交 970111f6 的 CLI 项目声明 Apache-2.0，保留该版本的 LICENSE、NOTICE 和版本清单。该清单使用开发版本标记。 | 验证拟支持的具体发行包、其平台二进制及附属组件。ChatGPT 和 OpenAI API 的账户、计费和集成条款依用户自己的适用合同分别审核。 |
| Codex ACP | 固定 6cce0354 的 @agentclientprotocol/codex-acp 2.3.0 声明 Apache-2.0。适配器和 Codex 可执行文件分别取证。 | 按最终选择的原生包或 ACP 包核对来源、跨平台依赖及分发权。 |
| Qoder CLI | 官方提供本机下载安装路线。供应商服务条款第 4 节的使用权有限，并含转让、再许可限制。独立登记 npm 旧渠道的样本版本与摘要。 | 原生发布物的转分发许可及商业编排或订阅认证适用性未获核准。拟接入与分发范围须由供应商和所有者核实。 |
| Claude Agent ACP | 固定 782e463f 的 @agentclientprotocol/claude-agent-acp 0.97.0 声明 Apache-2.0。其依赖的 @anthropic-ai/claude-agent-sdk 0.3.310 的发布元数据引用 README 的许可。 | Anthropic 官方要求第三方产品在未获事先批准时使用 SDK API key 路线。Claude.ai 登录、订阅额度和 SDK 发布物分发应凭各自授权审核。 |
| 自定义 ACP | 当前实际开发包只包含 AgentBaton 的连接实现。用户选择的外部命令没有统一发行物或统一许可证。 | 在引入预置、镜像或随包组件之前，逐个核对来源、版本、许可证与认证条款。 |

[固定 Codex LICENSE](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/LICENSE) · [OpenAI 服务条款](https://openai.com/policies/service-terms/) · [OpenAI API 协议](https://openai.com/policies/services-agreement/) · [Qoder 安装](https://docs.qoder.com/cli/installation) · [Qoder 服务条款](https://qoder.com/product-service?lang=en) · [Claude Agent ACP 固定清单](https://github.com/agentclientprotocol/claude-agent-acp/blob/782e463ff418a6093e09cef3f3f5ce813f4ee270/package.json) · [Anthropic SDK 认证说明](https://code.claude.com/docs/en/agent-sdk/overview) · [Anthropic 账户条款](https://www.anthropic.com/legal)

## 待定案

1. 审定具体项目许可证、版权声明以及回馈触发方式。
2. 审定核心与商业模块、合入权利证明和拟再许可权的边界。
3. 审定发布物的源码、署名与 NOTICE 渠道，以及哪些 Provider 路线具备另行授权的安装、分发或服务身份。
