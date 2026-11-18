# Goal Graph 进度

已复核 AB-START-001 和 OWNER_INBOX，固定基线与当前目标一致，初始工作树干净。共享 CI 消费者的实际本机基线证明 schema 19 Core 的 IPC 和只读 SQLite 已返回 19，旧 driver 的清单与 Factory 元数据守卫仍各自要求 18。当前 owner-only 和五条 validation-case 与旧 Factory 三系列共用文件，须从入口层明确分开。当前共享 workflow 仍生成 schema 18 候选，由集成窗口协调更新。

初版源码 `281503775533c74b54b6a6031af9dd00c9dddfec` 的 macOS 实机 schema 读与九项前置拒绝通过。首轮 Linux 回放停在私有工具的网络枚举预检，六个 Graph 场景未启动。Docker 使用 `--network none`。后续用进程所在命名空间的 socket 接口和 `/proc/net/route` 复核回环。依赖下载、首次定向计数断言、初版打包和 Linux 回放预检的异常记录均保留。

交叉审查发现初版源码变量名与私有工作区路径重叠，另外退出异常可能留下预先写入的成功状态。修复源码 `4e67136acc90a273fae03603cb6ad49d536e6a7f` 单独绑定 Git HEAD/tree 与输入摘要，并在异常时撤销成功。补充定向正反例和两种真实业务后退出异常的独立控制。在新目标目录重新构建两平台工具，保留初版结果。

修复版两平台工具及前端从同一固定源码构建完成。macOS 实机 IPC 与只读 SQLite 均读到整数 19，九项前置候选反例和两个实库副本异版本混配均拒绝。Linux ARM64 私有容器使用真实 procfs，六条现行 Graph 场景合计 40 项检查和 14 张原 PNG。注入的浏览器关闭与 Playwright 退出异常都在实际业务成功后使 driver 退出 1，daemon wait 0 和 HTTP join 仍成立。Linux 专用静态检查 35+5 通过。定向 10+64、基础设施 985 通过与 16 跳过、桌面 1597、root Node 4、preflight 和 Rust smoke 通过。
