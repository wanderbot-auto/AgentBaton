# bb343 开发测试安装包

这是真实 CI 构建的开发包，不是完整 Designv0.2 验收或生产发布。精确 feature `bb343be5bb0403e995a97441ce16670c03bd0f33`，实际 checkout `1a7682e60354d4a80200ea487725edfc99068a4c`，tree `f85f9e19513d1ecfd85a058e5f6fd14ba4ed1123`，[Product40799957452](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957452)。四项 Product 成功，但 [Environment40799957732](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957732) 三项旧45/46分钟策略断言失败，故该提交仍不得合入。新修复提交必须另核准确检查，不复用本轮绿结果。

| 架构 | 真实开发 artifact | DMG bytes | DMG SHA-256 | 签名 |
|---|---|---:|---|---|
| Apple Silicon | [12866209525](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957452/artifacts/12866209525) | 8577813 | `d1e479eb701efa13ea7d62e9d5f6496f0877764aa09e1dbfe2ad0feca25af0d8` | ad-hoc/linker-signed，TeamIdentifier未设置 |
| Intel | [12866603973](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40799957452/artifacts/12866603973) | 8984245 | `c23937dd321e890cd881f9a10097d4ff15b53ab8840a9d2f95b4b6dc7624bf25` | 未签名 |

两者均无 Developer ID/公证。每份官方开发ZIP四文件：真实DMG、SHA256SUMS、构建记录和人工清单；完整downloadedZIP的API digest/CRC/safe paths、DMG实际hash与UDIF footer均核验，未把Linux下载/解析冒Mac安装。ZIP本身SHA：ARM `2ecdfed1115a9c217fbb560781424683ae7e6b58b1d45e421471df46fde92d19`；Intel `1f299b9d4f24e4c87a1952d5ac81bb756a17b9d884d43b65022363a2d0605115`。

两实际Mac通过 readonly/fourWebCrypto、24Ownerpages/各自完整Grant与CAS/event/cleanexit及defaultreleaseaudit。50新NativePNG逐张独审。releaseaudit只证明原三marker检查范围，不夸全程序穷尽审计。CI未mount/人工安装/启动这些DMG中的默认App，不宣称最低macOS兼容、真实Provider/私人账号、远程Host或全Factory13能力已验收。本包属于schema12 GoalGraph分支，Factory13另线。

人工验收（尚未执行）：在对应架构核对下载DMG的SHA、记录实际系统版本与系统签名提示；按系统正常安全流程安装/启动，不关闭系统安全机制；检查首页/任务/语言及真实审批取消/确认、关闭UI后的后台/重开、卸载与用户数据处理；真实Provider身份/模型调用及远程Host另获明确许可后再测。不得把本清单勾选为已完成。

完整原始证据保存在当前Cloud的 `pr16-bb343-review/EVIDENCE.tar.gz`（SHA `08b76e059993338f484478948384355f8e4dd206645bd2be1680dceae8ed675e`）和官方artifacts。Git audit子集保留原logs/reports/50PNG/metadata并显式列明未嵌入的官方ZIP/DMG二进制，避免重复安装包进入Git；不伪造缺失bytes。工件按现有14天保留设置，非正式Release。
