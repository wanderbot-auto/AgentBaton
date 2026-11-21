# 真实 schema 18 迁移验收

任务基线 1060ff834f170de39e51cf14336525f53c2eb475。两平台实测的任务提交为 53e1fc9fcabd08453579715ea6119fd0cd432117，tree 为 72214cd7de45d85dd628335983bdb00b77393037。集成候选基于 7b04514caaf7807b46018547a8b10f2a5e7fc3d6，在 Apple Silicon 上实测的 detached head 为 7ac30c9a68bc24d45cce5e51dc40b5c608e105d1，tree 为 b2cc756289da39dd5d7f6f91b8560779720af2f9。

新增 Cargo 目标 `host_registry_fresh18_migration` 由特性 `fresh-schema18-migration-tests` 显式启用。原 `host_registry_actual18_migration` 继续使用 `actual-schema18-migration-tests` 特性和固定历史工件。Cargo 的原目标与特性配置、`Cargo.lock`、原生成器和工作流保持原字节。直接请求新目标而未启用该特性时，Cargo 退出 101。

新目标从历史固定 Git 对象，以原命令构建普通 batond 和 baton，再经真实私有 Unix IPC 建立 schema 18 样本。同一测试进程封存源码归档、构建清单、两件工具及夹具文件。相互匹配的替换工具与两份清单可通过原夹具 provenance 校验，但新验证的证明和恢复阶段均在启动旧工具前由独立封存拒绝。

固定候选在 Apple Silicon 上通过归档 2 项和新入口 1 项，在 Linux ARM64 上通过新入口 1 项。精确集成候选再次在 Apple Silicon 上通过同样 3 项。归档 8 个控制和新入口 14 个控制均得到预期拒绝。首次 Linux 容器缺少运行 UID 的 passwd 记录，退出 101。修正仅在本任务容器提供同一 UID 的账户映射，保留原始失败及退出记录。

实际 Store19 保留 1 个项目、3 个任务、5 个事件和 4 个回执的原有行，事件序号仍为 5，Host 表为空。重开保持一致。旧 Core 和 CLI 从备份恢复 schema 18、原有任务状态和事件，并成功收尾。原始平台证据、绝对路径与摘要见 [执行证据](EVIDENCE.json)。

新入口要求当前 checkout 的固定旧 Git 对象、规范的 `CARGO_TARGET_DIR`、Python 3 和原生工具链。实际验证由 Git 对象和构建收据确定来源，运行中只使用离线公共 crate 缓存及独立临时运行目录。托管工作流的接线和验收由集成任务处理。
