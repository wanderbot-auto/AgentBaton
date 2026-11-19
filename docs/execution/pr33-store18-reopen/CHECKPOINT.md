# AB-REL-007 旧版恢复验收检查点

实际用 a4e03303 的历史 Apple Silicon Core 和 CLI 重开了上轮从 Store 19 恢复得到的 Store 18 副本。旧 CLI 通过 Unix IPC 确认 API 1、Store 18、事件序号 5、1 个项目和 3 个任务。任务状态为两个 draft、一个 cancelled。5 条事件和相关完整行均与恢复 SQLite 一致。

旧 Core 收到 SIGTERM 后正常退出 0，进程组与 socket 已回收。备份、恢复原件和重开后主文件的 SHA-256 均为 2ef73ba584bd368923a5834bb0e84189f414cd43f739bfe0a020fd4777b9a71d。旧 Core 对同次 Store 19 副本返回 VERSION_UNSUPPORTED 并退出 1，既未接纳该库，也未改变字节。

此回执补足恢复副本的真实旧版重开验证。验收范围限于固定历史 Core/CLI 和生成测试历史。当前用户库、GUI、正式签名与发布不在本轮验收范围，安装权限独立审批。 [证据索引](EVIDENCE.json) · [实际闭合回执](receipt.json)
