# 复查固定旧版和恢复文件

旧 Core 和 CLI 的来源是 a4e03303b54bd5a5544131e95ad39662f8310e29。冻结工具沿用真实 Schema 18 迁移验收的 Apple Silicon 原件。构建、源码归档及二进制摘要见 [历史构建证据](../schema19-genuine-legacy18-runner/EVIDENCE.json)。

恢复输入以 1026b0df118d0931cf531c9f43dd5e520939c14e 中的 docs/execution/development-update-rehearsal/receipt.json 为锚。先独立核对该收据和备份、恢复文件、Store 19 对照的摘要。旧版与 SQLite 观察只在 task runtime 内的副本上运行。运行目录、HOME 和 XDG 各项取新建、仅当前用户可读写的目录。进程 PATH 为空，CLI 使用已验证二进制的绝对路径。

在 scripts/collab.sh run --native 的串行租约中，对正在运行的自有旧 daemon 依次执行固定旧 CLI 参数。

~~~text
snapshot
projects list
tasks list
events 0
~~~

对照恢复数据库的全部项目、任务和事件行，记录原命令退出与 SHA-256。发送 SIGTERM 并等待原进程组正常收尾后重查逻辑数据和物理摘要。Store 19 对照则单独使用干净运行目录等待旧 daemon 自行拒绝，退出码应为 1，且没有 IPC socket 或会话 token。

本轮使用的控制器及原始 CLI 记录保留在任务运行目录；公开回执保存其 SHA-256，不包含原始历史或 token 内容。 [输入、命令与结果](EVIDENCE.json)
