# 兼容性矩阵交接

- 固定基线为 51b9d762b2acc6ffa0e1c31411bb154ed4326b99，本次审查修复验证头为 3221f26f03f805b2200f4c832f1ce2b27bb34234。交接提交在其后追加此目录的任务证据。
- 已与固定基线、采样时远端集成头 6d45b417210f892ec7d5f17dd992c05fc903773c、leader 控制分支 0db9c959ce56b80402146a3c375775dbca50a505 预演合并。Leader 负责独审与实际集成候选验收。
- 观察索引覆盖选定的历史存档。当前没有锁定到候选的真实 Provider 账户、最低系统整链或发布证明。AB-REL-009 仍待验证。

## 运行

~~~sh
scripts/collab.sh run -- python3 scripts/report-compatibility-matrix.py --format markdown
scripts/collab.sh run -- python3 scripts/report-compatibility-matrix.py --format json \
    --source 3221f26f03f805b2200f4c832f1ce2b27bb34234
~~~

默认用工作树 HEAD 比较记录。other_source 保留其他准确源码的结果。historical_intel 表示 macOS Intel 旧记录。unknown 表示缺少适用的判定。not_run 表示证据明确记载未执行。blocked、unsupported、not_applicable、failed 与 passed 保持独立。

输入路径均相对于脚本所在工作树。通过 --index 指定 docs/execution 下的备选 JSON 索引。索引为 schema_version 1，各源以路径、SHA-256 及 observations 登记。观察记录写明 dimension、origin、provider_execution、scope，使用 result 指定实际字段、期望的原始值和固定解码种类。fields 中用 JSON Pointer 指向准确值。历史构建记录用 path 和 line 读取精确键。context 仅用于已审上下文展示。登记实测证据时保持原始结果，新增记录为不同版本、构建或重跑保留独立标识。

## 源记录与账户回执

浏览器证据读取实际正常场景报告及独立快照核验。校验文件须以报告 SHA-256 绑定到同一次运行，并记录正整数的 schema 19 快照计数。source_dimension_label 逐字保留原历史维度标签。原标签只作为来源说明。

真实账户观察的原始 JSON 为 schema_version 1、kind real_provider_execution。build 使用候选构建字段，provider 使用 provider_id 加候选的四个 Provider 版本字段。account_execution 给出 mode real_account、provider_invoked true、认证与任务完成的布尔值，以及脱敏的 run_ref。status 为原始执行结果。passed 要求已经认证并完成任务。失败回执保留实际执行状态。索引固定原始回执摘要并登记观察，执行结果和身份字段由 CLI 在该回执的固定位置读取。

## 最低系统待验项

用 --candidate 指定 docs/execution 下的评估 JSON。文件顶层应提供 source_commit、product_version、os（macos）、os_version、os_build、architecture（arm64）和 artifact_sha256。providers 对规格的各正式 Provider 提供 provider_version、adapter、adapter_version、protocol_version。配置版本和锁文件、候选包及实际运行环境随证据绑定。

- build 需要该候选的依赖锁定、原生构建和产物摘要。
- native_app 需要同一候选在选择的 macOS 构建与硬件上的启动和核心操作。
- credential_store 需要系统凭据存储、托管配置与具名原生连接的隔离操作。
- background 需要界面退出、登录启动、休眠恢复、对账和停止的系统验收。
- install 需要实际安装、Gatekeeper 和安装后启动。
- update 需要更新来源、不中断活动尝试、迁移失败和回退验收。
- 每个正式 Provider 需要在候选环境绑定明确版本、适配器和协议的真实账户任务。

评估依据为同一候选文件与固定证据内的值。全部维度就绪时可进入发布评审，最低支持版本仍待正式定案。Tauri 10.15 是打包配置。已交付的开发 DMG 记录仅覆盖归档审核和其构建机版本。详见 [EVIDENCE.json](EVIDENCE.json)。

## Leader 独审与交接

Leader 接受 7ecee4c198d9d90acb11418d85d519509d770090 的兼容矩阵阶段。独立复核了固定原始版本、浏览器报告与 schema 19 快照的摘要绑定，以及真实 Provider 回执对生成记录误标的拒绝。独立复跑 15 项专项测试，核实 JSON 与 Markdown 各自逐字一致，并复核 7 份固定主证据与 13 条观察。Scope 检查及对 cbac72707f7c075b202a63a658b46f4f0a2df9c8 的合并预演通过。

真实 Provider 账户与最低 macOS 支持仍待验收。开发窗口保留分支、工作树和证据，按交接流程释放三条路径认领。
