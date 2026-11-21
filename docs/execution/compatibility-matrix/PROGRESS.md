# 兼容证据矩阵进度

已核对 AB-START-001、OWNER-001 至 OWNER-005、固定远端基线和独立路径认领，scope 通过。已查阅 AB-EXT-004、AB-REL-009、Provider 规格、测试发布设计和实施验证门槛。

现有资料可核对生成 Codex 进程契约、本地 Apple Silicon 库与浏览器、真实 Apple Silicon 隔离 App、历史托管 CI 及开发 DMG。Tauri 配置写有 10.15。按当前 Apple Silicon 范围，最低系统验收需独立核对实际候选的安装与更新、后台、原生核心交互和真实账户组合。

第一次读真实证据时，索引将 handshake 命令中 --lib 的数组位置写为 8，CLI 按预期拒绝并退出 2。核对实际 argv 后使用位置 6，全部固定源记录均可读取。

固定代码与索引提交为 32bfe65cf645150eb1e5cae10b2aad4f18219315。实际 CLI 读取 7 份与基线 Git blob 逐字节一致的存档，共投影 13 条记录。10 条通过，1 条 App 早期重开失败，2 条人工安装未运行。两种格式各重跑一次，输出逐字节相同。选择历史 App 与 CI checkout 后，分别匹配 2 条和 4 条记录，Intel 数据仍保持历史属性。

11 个专项通过。全量 Python 974 项中 958 项通过，16 项依照原有远程 companion 规则跳过。4 项 Node smoke、2 项 Rust 环境夹具和 Clippy 通过。Rust 夹具从准确代码提交取出，在任务 runtime 中与原件逐文件比对后运行，以避免嵌套 worktree 的外层 workspace 识别问题。设计、基础设施、引导清单、gate 和 doctor 均通过。

Leader 独审要求校正浏览器 schema 并加固真实账户证据。浏览器改用固定正常场景原报告，保留 source_dimension_label 的 Core18 历史文案。原报告摘要与 browser-verified.json 中的正常场景记录一致，后者保留 12 条 schema 19 实际快照计数。修正后报告按该证据说明 schema 19，系统版本字段维持未登记。

真实账户观察读取固定原始回执中的执行、认证、完成状态和构建身份。索引将完整版本和构建号的生成数据改标为真实账户时，报告明确拒绝。回执自身记载的失败继续显示。保留同一构建全维度齐备的合成正控，以及调用未发生、认证或任务未完成、挪用同一原始文件内生成测试结果等反例。

固定修复代码为 3221f26f03f805b2200f4c832f1ce2b27bb34234。同一合成输入由旧 CLI 错误判为 ready_for_release_review，修复后退出 2，指出缺少原始执行回执。实际 7 份主档和 1 份快照核验均与固定基线 Git blob 一致，JSON 和 Markdown 重跑相同。15 项专项、全量 Python 978 项（962 通过、16 既有跳过）、设计和基础设施校验、4 项 Node smoke 通过。新验证和旧版本结果均保存在 EVIDENCE.json。
