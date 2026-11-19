# AB-CFG-010 项目配置变化的审批与新版本

Design v0.2 的要求是在确认首批项目能力及更新规则后，按已批准规则接受后续项目源变化。新增高风险能力、扩权、不可解冲突进入明确审阅。已独立配置的活跃尝试继续使用其固定快照；原生外部配置只读检查。依据 [能力与事务设计](../../05-capabilities.md) 和 `specs/requirements.json` 的 AB-CFG-009、AB-CFG-010、AB-CFG-013。

当前功能集成 `af6d7e89` 已有项目源只读检查、首次确认和托管快照。`crates/config/src/project_import.rs` 的 `apply_project_import` 只接受可信调用者给出的确切首次审批绑定，并以文件不存在作为首次事务前置条件。`read_imported_project_plan` 可以按相同绑定重开已批准快照。现有静态 MCP 和 Hook 分类不会执行仓库内容。Core 和 App 尚未调用该首次导入入口，能力台账已记录 AB-CFG-009 的这一缺口。

第一阶段从将来最新确认功能集成开独立 worktree。初始仅认领 `crates/config/src/project_import.rs`、`crates/config/tests/project_import.rs` 和该任务证据目录。在现有可信、固定 descriptor 和事务接口上实现项目配置后续更新。用既有已批准的托管快照与显式更新授权作为基准，从同一项目、Provider 和政策版本的源快照计算此次结果。确有已批准的变化才建立可读取的新托管版本，重复扫描同一状态保持无操作。对新增或扩大的可执行能力、未解决的重复声明、未知来源或无法核实的变更，输出可审阅原因，保持已有状态。旧快照引用、正在运行的任务、项目目录和原生全局配置的写入语义按设计保持独立。编写真实文件事务及回归，检查批准范围内的更改、审批需要、伪造授权、并发源或目标替换、重启对账和隐私边界。

第二阶段在同款项目配置正常入口接通可信 Core 审批与 App 项目配置，以实际 UI/IPC 证明首批确认、自动生成新版本、必须再次审核及活跃尝试保持固定版本。首阶段如发现调用方不能为内部批准提供必要信息，应先报告接口草案和字面冲突。按协作规则分配共享入口，保留最终适用验收；阶段性 Core 证明在台账仅计已验证的实际覆盖。

项目导入基线在准确 `af6d7e89` 的 Apple Silicon 上实跑 7 项全过。命令为 `cargo test --locked --offline -p agentbaton-config --test project_import -- --test-threads=1`。本机原始日志 `/tmp/ab-project-config-baseline.j3aKSFg4/cargo.log`，SHA-256 `d2bfc295f7c7b3dce841d8c5a2657e6b465b4299098befef944db5aa259fcb08`。此结果覆盖现有首次导入，不改变 AB-CFG-010 的台账状态。

## Core 项目设置入口

库代码准入后，独立实现 Core 的项目配置生命周期。接通现有 `projects.*` 与 Owner 审批路径，可信端从已登记项目、限定的配置目录及已确认版本生成快照、更新规则和私有版本引用。清楚区分只读预览、首次明确批准、已有规则内自动生成、以及必须再审。版本、项目、权限或作用域变化使用现有修订与回执语义，事务未确定前保留安全恢复路径。新任务从已确认的版本取配置，已有尝试继续引用其原先快照。

初始范围是新 `crates/core/src/project_config.rs`、`crates/core/src/lib.rs`、`crates/core/src/ipc.rs`、新 `crates/core/tests/project_config_ipc.rs` 和任务证据。先定位可信审批持久化及正常运行绑定边界。需要登记记录、存储、权限契约或其他写路径时先明确字面范围和依赖，由集成窗口接续分配。App 项目设置、Tauri 的明确允许入口和实际桌面验证继续保留在后续阶段。真实 UnixIPC 验证首次批准、自动后续、重新审阅、掉线恢复、旧准入修订、无执行启动和原有项目入口回归。

实际 Core 接线工作树 `project-config-core-ipc`（基线 `7b04514c`）追加了 `storage.rs`、`sessions.rs` 和 `runtime_config.rs` 的字面认领。Task 创建、绑定记录和读回需要覆盖已有库与工作中尝试。独立检查发现 Factory 委派还从 `factory_runtime.rs` 直接插入 Task，需核实冻结父版本／已审批作用域的行为。如需该文件和对应回归，窗口 A 在动手前另行确认。Core 首阶段采用 Owner 已批准的精确非秘密摘要，向 Renderer 明确要求再次审阅或沿已批准规则执行。客户端不能声明自己已经完成可信分类。
