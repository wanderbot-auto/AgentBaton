# C2 · 回执确认阶段交接

基线及 feature target `origin/collab/host-registration-integration` 固定于 `a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28`。验证源码 `438bc935611c886ad86f45bb34011f816692d670`，tree `f68a474307d638d841cac691b3ad31383d647fca`。工作分支 `collab/host-registration-receipt-ack`，仅四条 Rust 认领路径和本任务证据目录有改动。

`HostRegistrationIntent::validate_lookup` 核查询协议版本、回显原键、原登记正文及现有 Core 回执语义，验证无副作用。`ClientRecoveryJournal::acknowledge_host_registration` 接收预期 entry ID、Host ID、created sequence 和受信任 Rust 层的固定只读查询。初始缺失或过期 entry、成功查询显式 null、或查询期间被更换的完整条目返回 false。查询失败、回执不匹配或日志异常返回原错误或关联冲突。清理前取锁并比较完整条目。清理在同一 Host 日志内持久写入合法空包装，保留既有日志及 Store 记录；目录同步的未知结果指向重读。

Apple Silicon 本地验证 8 项本阶段新增、64 项既有有效用例通过。另有 3 个无环境变量的辅助入口计入 harness 输出，未计入有效数。workspace fmt、严格 Core Clippy、scope、diff 和固定 feature target 合并预检通过。所有子进程已退出，时段已交回。Leader 静态审阅通过，证据待集成复核。

验证覆盖真实本机 0700 私有日志和 SQLite Store19，不含 Native、Tauri、UI、真实 Provider 或远端 Host。回调的真实来源由未来受信任原生层接线保证，闭式 DTO 结构校验不能替代来源校验。现有持久写入的目录同步异常未单独做故障注入。
