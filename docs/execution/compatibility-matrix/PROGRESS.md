# 兼容证据矩阵进度

已核对 AB-START-001、OWNER-001 至 OWNER-005、固定远端基线和独立路径认领，scope 通过。已查阅 AB-EXT-004、AB-REL-009、Provider 规格、测试发布设计和实施验证门槛。

现有资料可核对生成 Codex 进程契约、本地 Apple Silicon 库与浏览器、真实 Apple Silicon 隔离 App、历史托管 CI 及开发 DMG。Tauri 配置写有 10.15。按当前 Apple Silicon 范围，最低系统验收需独立核对实际候选的安装与更新、后台、原生核心交互和真实账户组合。

第一次读真实证据时，索引将 handshake 命令中 --lib 的数组位置写为 8，CLI 按预期拒绝并退出 2。核对实际 argv 后使用位置 6，全部固定源记录均可读取。

固定代码与索引提交为 32bfe65cf645150eb1e5cae10b2aad4f18219315。实际 CLI 读取 7 份与基线 Git blob 逐字节一致的存档，共投影 13 条记录。10 条通过，1 条 App 早期重开失败，2 条人工安装未运行。两种格式各重跑一次，输出逐字节相同。选择历史 App 与 CI checkout 后，分别匹配 2 条和 4 条记录，Intel 数据仍保持历史属性。

11 个专项通过。全量 Python 974 项中 958 项通过，16 项依照原有远程 companion 规则跳过。4 项 Node smoke、2 项 Rust 环境夹具和 Clippy 通过。Rust 夹具从准确代码提交取出，在任务 runtime 中与原件逐文件比对后运行，以避免嵌套 worktree 的外层 workspace 识别问题。设计、基础设施、引导清单、gate 和 doctor 均通过。
