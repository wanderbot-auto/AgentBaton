# 复演固定开发包与历史 Store

演练接受一个固定的开发包和一份固定的生成测试备份。使用 Apple Silicon 和本任务的 `scripts/collab.sh run`。把下表文件复制到该命令所提供的 `BATON_RUNTIME_DIR/update-inputs`。目录权限为 `0700`，各文件由当前用户拥有且只有一个硬链接。

| 文件 | 模式 | 固定来源 |
| --- | --- | --- |
| `AgentBaton-arm64-14530c62a0b532e2ba43ca4be4fbc75ccc41d856-development.dmg` | `0600` | 锁定无正式签名构建的 `artifacts/development/` |
| `batond` | `0500` | 同一构建的 `AgentBaton.app/Contents/MacOS/batond`，与已验收的盘内逐文件摘要相同 |
| `historical-store.sqlite3` | `0600` | 已归档 Schema 18 测试备份 `input.pre-v19-75b6f92d-db51-478b-b7e4-9c30c88f38e9.sqlite3` |

原始输入固定位置、SHA-256 和构建证据见 [EVIDENCE.json](EVIDENCE.json)。历史留存目录里有空 WAL 和旧 SHM。复验先核对 WAL 为 0 字节、journal 缺失和固定的历史主文件摘要，再制作仅含主文件的任务输入。原件及 sidecar 保持原状。演练入口会拒绝包含任一 SQLite sidecar 的输入副本。

~~~sh
scripts/collab.sh run -- python3 -I scripts/rehearse-development-update.py
scripts/collab.sh run -- python3 -m unittest tests.infrastructure.test_rehearse_development_update -v
~~~

成功时打印新建的私有 `receipt.json` 路径，旁边有预备份、迁移结果和恢复文件。备份和各结果均可使用 SQLite 只读检查。整个过程不发送 IPC 请求。唯一启动的产品二进制是固定的 `batond` 副本，使用新建私有 HOME、配置、数据、缓存和运行目录。候选为本次运行生成的临时 IPC 凭据随候选目录清理，脚本不读取它。

恢复是从备份替换演练运行路径，并以只读 SQLite 重新验证数据。旧应用重启、用户库、当前后台状态、软件切换或安装不在本次实测中。
