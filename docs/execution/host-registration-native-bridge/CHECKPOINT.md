# C3 · Host 登记桌面桥接交接

窗口 B 在 `collab/host-registration-native-bridge` 完成七个固定 Tauri Host 命令。任务基线及远端 feature target `origin/collab/host-registration-integration` 为 `80eddd5997072203f3593becef47a2bf09ac9e16`。验证源码 `4cdbce3c565266892f36c7da60fd475ffefdacd2`，tree `851caab50253d7b0b5ceac2911773fd154395882`。生产 lib.rs 只增加模块及七个 handler。固定 feature target 和 leader 当前本地 feature head `1f756ce201b8cf10f4b1798053674f4e960ac77d` 的预检均无冲突。

登记持久预留后走 Core 认证直连。Pending 可离线读取，reconcile 不写，retry 仅在成功查到明确缺失且完整 intent 仍一致时用原键及正文重发。登记确认结果用原回执。ACK 依据受信任只读查询走 Core 的精确清理并返回真实 `acknowledged` 状态。写入后的传输/畸形回复保持未知及日志。Core 共用客户端在单连接内部生成请求 UUID，Host bridge 核对 ID 形状，沿用单请求流边界。查回回显键、typed 回执正文与原 intent 严格关联。未修改现有 schema 握手。

Apple Silicon 上 Tauri crate 编译、8 项测试和 3 项 Python source 检查通过。Rust 用例在无 WebView 的同进程局部桥接层驱动真实私有 Core socket，另用受控下一跳验证丢失回复和畸形返回。七个 handler 由实际 crate 编译及源检查覆盖。未运行安装 App、窗口调用、真实 SSH/Provider 或 CI。

新 Host 模块 fmt 和范围检查通过。整 crate fmt 的 3 个旧文件差异、strict Clippy 的 14 个旧文件诊断保留在日志，相关文件与精确基线 Git blob 一致。全部本任务进程已自然退出。已固定证据并提交，供 leader 独审和集成。
