# AB-REL-007 可信 Store 备份证明

任务分支 collab/update-backup-issuer，基线 53da4ce20fdfedae4a745e704394072ad6b742ae。ACK OWNER-001。写范围为 update_readiness.rs、其定向测试及本目录。

用现有 Store 连接和 SQLite Backup API 捕获私有独立副本，使用原就绪文件核验。证明由 Store 出具，绑定 SQLite 身份、schema、事件序号和连接前后水位。最终路径排他公布。覆盖错误路径、占用、真实外部修改、文件边界及失败清理。备份能力与更新批准、运行时阻塞独立。
