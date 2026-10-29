# a2201 开发测试安装包

本次是真实双架构开发包，不是完整 Design v0.2 验收或生产发布。Feature `a2201fe2729eddb0d76bf3995667d3692126ee0f`，实际 checkout `7f359dc8807e4d11f8e2dedbf4b0a5cc83c81f16`，tree `520d7aab6abdd4d659d0a9d301d9e28bdfffafd2`。[Product 40806944290](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40806944290) 的 Rust 与双 Mac 成功、UI command-timeout 观察器失败；[Environment 40806943416](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40806943416) 三项成功。总六项成功、一项失败，不能据此合入。下一诊断候选需准确新检查。

| 架构 | 开发 artifact | DMG bytes | DMG SHA-256 | 签名 |
|---|---|---:|---|---|
| Apple Silicon | [12870111178](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40806944290/artifacts/12870111178) | 8577817 | `f58ef4c78221176459ccae247cc66b660cd8fb30e1b097419acc62e1427b4fd1` | ad-hoc/linker-signed，TeamIdentifier未设置 |
| Intel | [12870337298](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40806944290/artifacts/12870337298) | 8978755 | `e443fc0870c97cf5c34a1d170b5eea436d942d185721818c8fd7d8757a59f933` | 未签名 |

均无 Developer ID/公证。每份官方开发 ZIP 四文件含 DMG、SHA256SUMS、实际构建记录及人工清单；API digest/CRC/路径、DMG hash/UDIF footer由独立审计实际核验。ARM ZIP SHA `649f3568b58384cb0538ab56b2548f3ca907e539f2ab56f158d89dc669b141c7`；Intel ZIP SHA `c3633716bc8aa98046d62eadc453624b94d8f63ec2d799c53a7ec2bb1de88dfc`。原件本地 pr16-a2201-review 与官方 artifacts 保留；不会把 bb343 旧包当本轮新包。

两架构各自实际通过 Core/native、readonly/fourWebCrypto、24 Owner 页/完整 Grant/CAS/event/cleanexit、default release audit 与开发打包。每架构5张代表截图实际目视，其余20张仅逐件哈希，不声称本次50张全部目视。release audit只覆盖原有检查范围，不是全程序穷尽安全证明。尚未 mount、人工安装或验证这些 DMG 内默认 App；最低 macOS、真实 Provider/私人账号、远程 Host 和完整 Factory/schema13 未验。本包属于 schema12 GoalGraph 分支。

人工验收尚未执行：下载后核对对应 DMG SHA，记录实际系统版本及系统签名提示；遵循系统正常安全流程安装/启动，不关闭系统安全机制；检查首页/任务/中英文、原生批准取消/确认、关闭 UI 后后台和重开行为、卸载与用户数据处理。真实 Provider 身份/模型调用、远程 Host 需另获明确许可后测试。此清单不得标为已完成。官方工件沿既有14天保留设置，非正式 Release。
