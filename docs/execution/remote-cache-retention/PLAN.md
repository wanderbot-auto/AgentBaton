# AB-EXT-010 · 远端日志缓存保留与清理预览

固定基线 `4be7d4493a3e82624e2eb45329ec5c358503486a`，目标 `origin/collab/host-registration-integration`，工作分支 `collab/remote-cache-retention`。已核对任务卡、AB-START-001、D04/D05、OWNER_INBOX 和四个认领。

1. 沿用完整 `Binding`、fence、`Snapshot` 和连续游标语义。`Ledger::ingest` 的事件事务记录属于对账证据。远端临时详情还需要来自调用者的、未过期的本机主库持久化确认，精确对应逻辑 ID、事件序列和内容摘要。
2. 使用调用者已核实的当前项目策略、TTL、pin、活跃引用与 attempt 状态。只读生成逐项结论，未知、冲突、分页未完成、缺口和在途 attempt 均留存。项目规则决定缓存期限，无默认 TTL。
3. 记录可解释的本机确认状态与远端缓存预览。结果只含逻辑 ID、序列和有限状态，不包含本机路径、凭据、正文或删除动作。
4. 在 ARM64 上通过独立 manifest 运行 locked/offline 定向与全 crate、格式、全目标编译、严格 Clippy。记录确切 SHA、命令、原始日志及范围和目标组合，交集成窗口独审。
