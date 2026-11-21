# AB-REL-007 实现记录

- `Store::issue_update_backup` 在已有连接的读取事务中使用 SQLite Backup API，将已提交的 WAL 数据写入独立暂存库。签发前比较原库文件身份、schema、事件序号、连接变更计数和 SQLite `data_version`。
- 目标父目录须为当前有效用户所有且模式为 `0700` 的规范绝对路径。暂存库以 `0600` 创建。使用已有预检核验独立 inode、SQLite 完整性、schema、事件序号、SHA-256 和 sidecar 状态。以不覆盖的链接操作公布，再检查最终路径和源水位。
- 目标或 sidecar 已存在时保留原有条目。源库变化、晚到的碰撞或暂存 sidecar 会拒绝签发。清理通过持有的目录和文件身份识别本次操作的条目。不能确认清理时返回结构化错误并标明副作用未知。
- `IssuedUpdateBackup::proof()` 提供既有 `UpdateBackupProof`。就绪测试使用真实 Store 和生成的本地 ACP 子进程，确认活动会话继续阻止更新且源库与 WAL 字节保持一致。

源码提交为 `aa040e8977f6c48d26678dad50e3d4d8bcfb3cc4`。运行详情见 [EVIDENCE.json](EVIDENCE.json)。
