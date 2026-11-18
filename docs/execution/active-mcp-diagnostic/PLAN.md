# AB-CAP-006 · 受管本地 MCP 主动诊断

基准 `1c9b6b0b2d7a72c73d05a153c5e24cc6470adbfc`，目标 `origin/collab/host-registration-integration`。按 AB-START-001 执行。已核对 OWNER-001 / OWNER-002，工作台实现由集成窗口负责。

- 复用已登记的本地 Codex 受管 MCP 闭合配置及源身份读取。预览绑定已登记项目和连接的版本、配置、具体服务、程序哈希及文件身份、有效期和允许动作。
- 由受信调用方消费一次性本机确认，写入诊断意图后，从受验文件描述符固化并启动私有入口，记录入口实际文件身份和摘要。执行私有目录、清除继承环境、受管空白状态目录、无项目文件或认证输入。按指定 MCP 版本协商，有限列举工具，校验取消、超时和子进程回收。关闭本次受验入口文件和空目录，保留未知子进程需要的副本直到取得确认退出。留下诊断独立证据与崩溃前未结结果。
- 定向测试真实生成的 Python stdio 对端。覆盖拒绝、身份/配置漂移、最后校验后的确定性程序换位、分页及边界、返回结果与会话分离、真实取消/超时确认退出、协议失败。执行 Core 适用回归、格式和 Clippy，保存精确提交与原始结果并交独立复核。
- 可写范围为 `managed_mcps.rs`、`managed_mcps/diagnostics.rs`、`managed_mcps_tests.rs` 及本证据目录。Owner IPC、受控 Actor worker、Tauri 原生确认、工作台、更新门槛识别和全局状态由集成窗口协调。HTTP/外部 Host 的接收方、凭据和执行授予需单独确认。

协议基线 [生命周期](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle)、[stdio](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)、[工具列举](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)，兼容旧 [2024-11-05 协商](https://modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle)。
