# AB-CAP-006 · 集成检查点

分支 `collab/active-mcp-diagnostic`。认领基准 `1c9b6b0b2d7a72c73d05a153c5e24cc6470adbfc`，产品源码 `2039ac842fcb13c87dcbc39a013ffd675c1b8dfe`。实际检查、目标树和未验证项见 [EVIDENCE.json](EVIDENCE.json)。

## Core 合同

1. 使用 `Store::preview_managed_mcp_diagnostic` 获取本次不可反序列化的预览及短期摘要。显示其完整审阅内容，由受信确认适配器持有原对象。项目、连接、配置、精确服务、选定 executable 字节及文件身份、空 argv、可独立移位的入口模式、私有状态、允许动作和启动有效期均固定。原生用户确认后由该适配器消费预览，输出只能在进程内构成的一次性确认。传入摘要本身不能证明来自 Owner。
2. `Store::run_managed_mcp_diagnostic` 在持久意图落盘后运行。打开并核对原文件描述符，在私有目录固化二进制或绝对解释器脚本，复核私有入口的文件属性和 SHA 后，仅从私有入口启动并采集实际入口身份。到期、配置漂移和执行前取消均有清晰边界。受管本地 stdio 服务器只能收到 `initialize`、`notifications/initialized` 和有限分页的 `tools/list`。枚举期间对服务端请求拒绝继续，详细返回值及错误正文不进入诊断证据。完成的列举仍需直接子进程已回收才算本次连接成功。
3. `ManagedMcpDiagnosticAttempt` 持有未回收的直接子进程或尚未持久化的回执。保留并轮询 `reconcile`，直到回执持久化且无待确认的直接子进程或入口清理。崩溃后 `get_managed_mcp_diagnostic` 可读持久意图。没有回执、退出未知或入口无法核对及清理都会维持单一 pending 门槛。重启后缺少原 owned child 证据时无自动重跑、猜测 PID 或清除 pending 的入口。此前一直阻塞需后续可信恢复合同。
4. 源路径换位会保留观测到的漂移结案，进程入口始终使用单独的私有文件。macOS 使用同卷、支持克隆的文件系统并核对保留的系统 provenance。已知不可移位、额外扩展属性或 ACL、quarantine、不可执行挂载和动态 shebang 均无启动回退。Linux 受限副本路径通过独立 ARM64 真实进程定向测试。[Darwin clonefile 语义](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/man/man2/clonefile.2)
5. 子进程以当前 OS 用户运行。私有目录是 AgentBaton 对路径的控制范围，同 UID 的任意外部程序或 OS 级强隔离未纳入保证，解释器、动态依赖、网络 I/O 和子孙进程不随入口身份固定。只清理本次入口文件与空入口目录，清理前核对仍持有的目录和文件描述符。工作目录和空白状态快照按未来的诊断保留策略处理。4 秒上限针对 stdio 协议，文件系统、SQLite 或 OS 启动调用的时限另行处理。

## 集成依赖

- 同一个信任域内导出 `sessions.rs` 下的诊断类型，并在 `ipc.rs` 设计受信 Owner 专属审阅／确认、启动、读取、取消接口。不得使用通用 `approvals.finalize`、分支文本或渲染器参数作为 native 确认。当前 Core 入口在调用线程借用 `Store`。接入前要用 Actor 负责的启动和回执提交、只承担 stdio 的 worker 或同等受控分工避免阻塞循环，保留启动前与结案时的漂移校验。
- 桌面 `src-tauri/src/lib.rs`、原生审批模块和 `ManagedMcps.tsx`／API／i18n 需独立接线，警告会运行本机程序，并明确区分当前诊断和会话连接。Tauri 原生确认、IPC 访问控制、权限封锁和真实 UI 未走通。
- 更新准入须同时观测 Actor 的诊断 worker 活动态和 `Store::has_unresolved_managed_mcp_diagnostic` 对应的持久边界。HTTP 或外部 Host 诊断须独立绑定接收方、凭据范围和 Host 授权，当前 Core 入口拒绝远端。
- 集成窗口复核并执行合并，任务窗口仅提交、验证和移交。

## 验证边界

Apple Silicon 上 15 项 MCP、4 项配置、9 项更新门槛通过。完整 Core 单元 467/468，会话 IPC 59/63，五项失败在干净认领基准上逐一复现。Linux ARM64 独立合成候选 15/15。产品、基准对照、独立 Linux 候选和各自原始日志均见 [EVIDENCE.json](EVIDENCE.json)。
