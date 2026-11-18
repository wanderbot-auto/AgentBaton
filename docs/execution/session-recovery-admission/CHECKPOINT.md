# AB-SES-006 交接检查点

| 项目 | 固定引用 |
|---|---|
| 任务分支 | `collab/session-recovery-admission` |
| 认领基线 | `315902e2c7c9e0c6dbaad8a22a5285009630d8ff` |
| 已测实现 | `ff13dd587b8b69001700245f7d2edd28d8ad0d31` |
| 预检目标 | `collab/host-registration-integration` @ `7e41a4834af2ee592c9bba94b00152a40eda9b7a` |
| 接续 | `leader-control` 集成窗口独立审阅并验证固定合成树 |

准入以持久化 session、attempt、intent 和任务为起点。旧进程、消息、执行结果或写权未结清时进入待对账。确认停止后，使用既有 grant 验证工作区与固定输入，验证具名连接、身份、原始规格修订、版本证据和磁盘上的运行快照。依据可信调用者提供的原生观察，返回原生候选资格，或要求新尝试并携带明确选定的同任务交接引用。旧运行快照及证据缺失有独立原因。

当前实现是只读 Rust API。调用方后续需要对获准的加载操作重新核对原生身份、加载能力与所有版本化输入。新尝试仍走正常审批和预算；此次没有更改 IPC、持久化结构、写权或运行时控制。已向集成窗口报告实际原生取证和加载接线依赖。

Apple Silicon 上的验证使用实际 Core、私有 Git 和 SQLite、合成 ACP 子进程。定向测试观察原始 turn 已完成，并比较评估前后的协议流量和持久记录。实际 Provider 原生恢复尚未验证。见 [EVIDENCE.json](EVIDENCE.json) 和 [范围与冲突预检](logs/scope-preview.txt)。
