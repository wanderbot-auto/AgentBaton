# AB-REL-007 交接检查点

| 项目 | 固定引用 |
|---|---|
| 任务分支 | `collab/update-readiness` |
| 认领基线 | `ae236688c1727e1a4610c424dd42bdd832f76dab` |
| 已测最终实现 | `882a3dcc950a1ebd3436cc978fa8cbd57c36b091` |
| 预检目标 | `collab/host-registration-integration` @ `b8573a221e152c6f47d8590739f031a7bbbd9058` |
| 接续 | `leader-control` 独立审阅、验证精确合成树、决定合并 |

`Store::assess_update_readiness` 从实际 Core schema 19 读取运行信息，区分后台可用、确认停止、未知、活动执行和取消待确认。工作区和执行栅栏、未结清预留及缺失停止证据也阻塞。对历史 `session.turn_ended/outcome_unknown` 同样保留阻塞，现有正常入口缺少关联确切会话、尝试和 turn 的持久副作用对账。已确认进程停止不能解除这项阻塞。

就绪绑定 Apple Silicon 开发候选的版本、平台、commit、包摘要和目标 schema。备份绑定当时的 Core 数据库边界，迁移回执绑定同一候选和精确备份，恢复使用独立的原备份精确还原样本。实际文件须匹配 SHA-256，SQLite 样本须为独立私有副本并通过只读检查。回执可信来源由调用方解析。

同一个已打开 SQLite 连接的 Unix VFS `SQLITE_FCNTL_HAS_MOVED` 用于验证其文件仍对应当前路径。检查水位时在前后比较文件身份和数据库数据版本。审阅确认修复前可通过同名换位重锚并误报就绪，保留实际红绿证据。该检查是时点证据。

现行 `BackgroundStatus` 仅涵盖 Provider worker。后续 Daemon 正常入口须在可信 actor 侧捕获其他本地 worker、待处理结算、调度点和远端执行边界，停接新工作并持有排他维护权限，在切换前用新观察调用 `recheck_update_readiness`。本阶段是只读 Rust API，生产更新、实际安装或关停、签名、IPC 和真实跨版本更新演练尚未验证。

Apple Silicon 验证包含 9 项定向、8 项派发、4 项旧迁移、2 项会话 IPC 和 2 项后台回归。正例在私有数据库执行真实同 schema Core reopen，且在合成 ACP 完成一轮并经 Supervisor 结清后重新取备份。迁移回归使用既有结构夹具。本次合成包不代表真实可安装产物。见 [EVIDENCE.json](EVIDENCE.json) 和 [范围与冲突预检](logs/scope-preview.txt)。
