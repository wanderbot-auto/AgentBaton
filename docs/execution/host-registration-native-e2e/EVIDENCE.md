# Host 登记原生验收证据

## 固定构建

真实 Apple Silicon，macOS 26.6.1 (25G76)，Darwin 25.6.0 arm64。源码提交 `cde501db847d1bbfc27a298e16cd772afc1a41e9`，源码树 `10f221cd61296df382fdb03ef4c122f035e0aafe`。[构建环境和签名状态](logs/build-environment.log)

通过 `bash scripts/collab.sh run -- …` 依序运行 `npm --prefix apps/desktop ci --no-audit --no-fund`、`cargo build --release --locked --bin batond --bin baton`、`bash scripts/prepare-desktop-sidecars.sh`，再在 `apps/desktop` 运行 `npm run tauri -- build --bundles app --ci --no-sign -- --locked`。默认 App 构建后未启动。隔离开发包使用相同构建参数和单项 `--config` overlay。两份 plist 的唯一差异是 `CFBundleIdentifier`，隔离值为 `dev.agentbaton.hoste2e-15adcdcc0da8`。Tauri 跳过显式签名，Mach-O 带 ARM64 linker ad hoc 签名，Info.plist 未绑定。原始构建日志使用 gzip 无损归档。[默认构建](logs/build.log.gz) · [隔离构建](logs/build-isolated.log.gz) · [原始日志校验值](logs/raw-build-sha256.txt) · [overlay](logs/bundle-overlay.json) · [默认 plist](logs/default-Info.plist) · [隔离 plist](logs/isolated-Info.plist)

| App 文件 | 架构 | SHA-256 |
| --- | --- | --- |
| `agentbaton-desktop` | arm64 | `b0c7e73b35d70348a981ebfba8b402e7aa4fa9c4e6eea2ed9822d7cd38b0d324` |
| `batond` | arm64 | `48d0db5f9b4c85ef8c79724a3fc84520ff48cbde09c9a5d77fe1f3d63c7d9a1b` |
| `baton` | arm64 | `6f70be052ebe1c072b8cc23f62a0b75f768d598ecae36f55311b4752f42f201c` |
| `Info.plist` | — | `cde8c5e2ec300fd99c8dc6c04c27a38b0b0c827e81bcf8db4e85d27132fc4eca` |

两个侧车分别与锁定 release 构建逐字节一致。原生结果记录了前端 `dist`、Tauri 配置和 capability 文件 SHA-256。构建输入来自固定源码，overlay 只设置隔离标识。

## 真实 App 验收

以现有 macOS Accessibility 和窗口捕获权限操作实际 WKWebView。首次打开设置中的远程主机，界面显示空列表。逐一填入并核对两个选项和十个字段，再提交实际产品表单。App 显示 Host `bc10b94e-ebcf-48ce-9f42-54905a299042`、创建序号 `1` 和确认按钮。确认前，权限 `0600` 的恢复日志保留原键 `2b5af6fc-fcfc-4691-808c-9e2f773354ab` 与完整提交体。点击确认取得界面反馈后，日志内容为 `entries: []`。实际关闭并观察设置消失，重开后读到该记录。随后终止 App 进程并启动新 PID，在同一 daemon 实例 `c1cf135c-cfc8-4462-9898-f0246eab44bc` 查询并打开详情，看到同一 UUID。[最终 App 结果](logs/app-final/result.json) · [待确认回执截图](logs/app-final/pending-receipt.png) · [App 重开截图](logs/app-final/app-reopened.png)

独立 `mode=ro` / `PRAGMA query_only=ON` SQLite 审计从 schema 19 空登记表开始，结束时恰有一条 Host、一条绑定原键的回执和一个 `host.registered` 事件。事件的聚合 ID、序号和提交体摘要与回执、登记行相符。项目、任务、会话、会话轮次、连接和执行授权六个表的行数与内容摘要前后相同。这是空私有数据库场景。两个 owned App PID 由 SIGTERM 退出并 join，同一 daemon 正常退出且 socket 消失。两张窗口截图均已目视核对。[待确认 AX 树](logs/app-final/pending-receipt.ax.json) · [重开 AX 树](logs/app-final/app-reopened.ax.json)

较早完整试跑也通过。冻结后的一次重复试跑完成登记与确认，重开设置时旧驱动的前台激活检查失败。最终驱动先核对本次 PID 内的准确 AX 控件已聚焦，再发送定向按键，并在重开前核对设置确实关闭。两份历史记录已归档。[较早成功](logs/app-button/result.json) · [中间失败](logs/app-frozen/result.json)

## 真实 Unix socket 恢复

使用另一套新建的权限 `0700` 私有短 runtime，启动相同生产 `batond`。客户端发送完整鉴权的 `hosts.register` 帧并保持连接。独立的只读 SQLite 看见提交后，客户端以零字节读取关闭原连接。新连接用原键 `a1d192ad-9f24-4ed3-a741-9ec6de8126b1` 查询 `hosts.find_registration`，取回 Host `4a2b0b07-47fd-4028-8798-6941d2929716` 的同一不可变回执。再次用原键和同一请求体重放，以及重启 daemon 后重新查询，均对应一个 Host、一条回执和一个创建事件，记录与首个提交逐项一致。两个 daemon 都正常处理 SIGTERM、退出 0 并被 join，私有 socket 已移除。[冻结驱动运行 JSON](logs/socket-final/result.json) · [首次 daemon](logs/socket-final/daemon-socket.log) · [重启 daemon](logs/socket-final/daemon-socket-reopened.log)

较早的立即断开试跑未观测到提交，证据仍保留。此次通过的断点在客户端消费 Unix socket 回复之前，Tauri/WebView 回包丢失尚未验证。[较早失败](logs/socket-first.log) · [失败运行 JSON](logs/socket-first/result.json)

## 驱动和覆盖范围

冻结驱动提交 `522db444e96ccd1b92b3ebf400bd86c133390ba9`，SHA-256 `6c4d538672fcfdebb596b6d5019ea350607b397ae7fb31e1761ad89bac5e5e6b`。3 条 Python 控制测试通过，覆盖回执／记录／事件唯一性、只读审计比对的失败路径、私有日志文件形状及 socket 请求 ID 绑定。[冻结测试](logs/driver-final-tests.log)

App 启动、交互、重启和清理均在同一个 `bash scripts/collab.sh run --native -- … --phase app` 会话执行。只截取当前核验 PID 所属的 App 窗口。所有测试输入均来自 `host.example.invalid` 等虚构声明，独立结果保持 `not_contacted`、`not_probed`、`execution_enabled=false`。用户日常 App 和 daemon 的 PID 前后相同，任务短 runtime 在退出后为空。验收覆盖 ARM64 隔离开发包，正式签名、安装升级、登录启动和真实远程执行仍未验证。[进程和 runtime 核对](logs/process-boundary.log)
