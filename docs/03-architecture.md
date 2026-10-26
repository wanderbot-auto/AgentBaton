# 03 · 系统架构与运行边界

## 1. 推荐技术基线

Tauri 2 + React/TypeScript 承载桌面界面；Rust/Tokio 承载核心、进程与事件；SQLite 存本地元数据；普通文件存大产物；ACP 作为标准会话协议，原生 SDK/API 和 headless CLI 作为 Provider 补充。ACP 已有官方 Rust SDK，但采用的具体 release、扩展和功能须锁版验证，不默认依赖草案协议。[S03][S04]

前端可采用 Vite、Tailwind、可访问组件库、Zustand 和查询缓存；这些是建议而非同时引入的强制清单。表格、diff、图视图按实际需求选择依赖，禁止为未来可能用到的功能提前堆库。

## 2. 模块化单体，而不是微服务集群

```text
Desktop UI (React)
        │ 受限 Tauri commands / 批量事件
Desktop Host (Tauri/Rust)
        │ 经过认证的本地 IPC
batond (每用户单实例、业务权威)
  ├─ Project / Workspace Registry
  ├─ Session Supervisor + Attention
  ├─ Capability Resolver + Config Transactions
  ├─ Task Router + Workflow Engine + Validator
  ├─ Event Journal / Artifact Index / Recovery
  └─ Provider Adapters + Host Connectors
        ├─ ACP / 原生 structured API
        ├─ headless 子进程
        └─ 远程Host代理（macOS/Linux）

baton CLI ──────────────┘
Baton MCP bridge ───────┘（按会话发放窄权限委派入口）

SQLite（daemon 单写入） + 文件产物 + 原生凭据系统
```

进程分离是为了 UI 重开、MCP 桥接和 CLI 复用，不意味着每个模块单独部署。业务规则只在 Core 内实现，UI、CLI、MCP 不各自维护任务状态机。

## 3. 进程与生命周期

登录Mac后通过受支持服务管理机制启动每用户daemon，UI仅连接并协商API/schema；daemon拥有受管进程和记录，UI退出不终止任务。可撤销后台注册；不承诺系统未登录即执行。手动启动UI时也能诊断/恢复daemon连接，避免创建第二个写入者。

daemon 退出时按任务策略通知、取消并回收子进程；崩溃后仅凭 PID 不接管，因为 PID 可复用。记录启动时间、父关系、nonce 与连接标识；必须重新协商后恢复控制。对旧进程的取消要验证身份，否则标记需人工处理。

macOS 休眠使本地工作可能暂停或断网。唤醒后区分可重连、可恢复、失败和未知结果；不自动复制一份新任务。产品的远程 Host 才可能脱离本机持续执行，与研发使用 Codex Cloud 是不同能力。

## 4. 本地 IPC 与权限

macOS 优先采用用户私有目录中的 Unix domain socket。使用目录/文件权限、连接身份校验、随机会话凭证和版本握手，控制浏览器或其他本地进程的任意接入。不能声称这些机制能抵挡已完全控制同一用户账户的恶意程序。

若为开发调试启用 localhost HTTP/WebSocket，必须显式开启、认证、Origin 校验与 CSRF 防护；生产默认不监听公共 TCP。UI 不获得通用 shell、任意 SQL 或任意路径读写接口。Tauri capabilities 用于限制渲染端命令，不代表 Agent 子进程自动被安全隔离。[S14]

## 5. 状态、事件与大流量

SQLite 是结构化业务状态的事实来源；事件日志用于追踪、恢复和 UI 重连。建议 WAL、迁移版本和单 daemon 写入，但不把“SQLite 事务”错误延伸为外部系统事务。

状态变更和对应 domain event 在一个事务中落盘；派发命令用 outbox/intents 表，再由 worker 发送。外部执行结果可能处于不确定区间，恢复时需 reconcile，不能保证 exactly-once。

控制事件、审批与产物索引不可静默丢失；高频 token/日志独立分片、限流、批量推送、保留截断标记。客户端用 sequence cursor 重连；没有完整原生日志时明确标记 gap。大文件不进入数据库或 React 全局状态。

## 6. 工作区、执行器与验证器

运行前锁定 workspace、输入快照、capability snapshot、任务 policy。受管并发写入在 Baton 内用 lease 约束；外部编辑器和 CLI 不遵守 Baton lease，因此提交/采纳时仍检查文件与 Git 状态。

Validator 是独立组件，执行产品负责人批准的确定性检查。Agent 可以提出验证建议，但不能自己任意改掉验收程序。验证进程的目录和权限与被测对象分开定义；测试本身可执行代码，应在合适的隔离和凭据范围内运行。

## 7. 代码组织建议

```text
apps/desktop/{src,src-tauri}
apps/daemon/src
apps/cli/src
crates/core/          # 项目、会话、能力、任务、策略等领域模块
crates/providers/     # ACP/native/headless/host connectors
crates/storage/       # SQLite、事件、产物与迁移
packages/contracts/  # 从规范生成的前端类型/验证器
specs/               # 需求、schemas、provider 支持矩阵
 tests/              # 单元之外的契约、场景、安全、桌面测试
 docs/               # 设计、ADR、执行证据
```

不要求第一行代码就拆成几十个 crate。接口稳定后再按编译、依赖和测试边界拆分，禁止纯粹为了目录好看制造抽象。

## 8. 运行时依赖管理

Rust 是 Baton 自有核心；某些 Provider adapter 可能是 Node 程序，这不要求再建一个 Node 业务 daemon。可选择调用用户已安装的 adapter 或分发锁版运行时，需明确许可证、签名、供应链与更新责任。

GUI 启动环境常与终端 PATH 不同：发现器必须允许选择绝对路径并运行安全的 version probe，而不是无条件 source 用户 shell 启动文件。Homebrew 与 Intel/Apple Silicon 路径使用发现结果，不硬编码唯一位置。

## 9. 单点与故障处理

数据库损坏进入只读恢复/导出模式；磁盘满停止新写任务而不伪装提交成功；adapter 崩溃不拖垮 UI；schema 不兼容拒绝握手；升级先验证数据备份；通知失败保留 Attention 项；删除产物影响关联验证有效性；时间变化用单调时钟衡量超时，以 UTC 记录事件。

## 10. 已定架构边界与验证项

独立daemon、单主入口、双架构macOS桌面、个人远程macOS/Linux、统一托管与高级原生连接均已确认。技术基线为Tauri/React/TypeScript＋Rust/Tokio＋SQLite，精确依赖版本必须锁定测试。最低macOS由验证决定；具体开源许可证待正式审定。

新增领域对象：Connection连接程序与配置、身份；RuntimeEnvironment记录配置归属与隔离边界；HostAgent在远端保持有界执行与事件缓冲；Integration与Release分开跟踪验收和发布。参见数据契约，不用多个daemon分别实现互相冲突的授权规则。
