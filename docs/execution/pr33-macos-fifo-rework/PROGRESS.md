# PR #33 macOS FIFO 复验进展

- 托管作业 135312028796 在 163.59 秒内通过 27/34 项。七项均报 `owned FIFO ready observation expired`，旧 collector 只记录 `actor_join=ok`、`status=unobserved_records` 和空行集。前一次同 Rust 源码、锁文件、Product workflow 和托管镜像的作业在 101.83 秒内通过 34/34 项。托管工件索引没有这七项的数据库，实际失败阶段仍未观测。旧诊断的定位参考提交为 `42dac8f4c58e1ea43f5d0b2c0602af842410af48`。[托管失败作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44450220780/job/135312028796) · [托管样本](hosted-observations.json)
- 旧诊断在私有 IPC 中继上实测扣留真实回复后的 5 秒客户端错误，并保留了正常对照。旧自然 IPC 断连只留下外层错误，本次七个托管失败的记录则到达 FIFO 观察断言。[旧 IPC 边界证据](https://github.com/wanderbot-auto/AgentBaton/blob/42dac8f4c58e1ea43f5d0b2c0602af842410af48/docs/execution/pr33-macos-integration-fifo/IPC_BOUNDARIES.md)
- 在 Apple Silicon 本机以原基准串行运行同一测试二进制，34/34 通过。对该私有测试进程应用 Darwin `taskpolicy -b` 后复跑七项，六项重现同一 FIFO 错误。另一个 block-policy 用例在 `Server::request` 失败，留存库没有 Integration run 或 attempt。原基准六个 FIFO 样本在受控复现中均只留下第一轮 preparation，未产生 validator 或 CAS 尝试。[留存库摘要](background-retained-summary.json)
- 同一严格只读 fixture collector 改为匹配 Store 19，并保留精确的源码逆转和历史散列。新二进制普通优先级串行 34/34 通过。对 non-descendant 用例再施加相同私有降优先级压力，原用例以 rc 101 报出 FIFO 错误，同时输出 `status=collected`、`schema=19` 和第一轮 preparation 行。收集器以所有者收尾后的状态为快照，投影保留 `measurement_not_authority=true` 和 `process_stop_proven=false`。[真实失败投影](collector-real-projection.json)
- 首次受控命令使用了错误的 `taskpolicy` 路径。第二次的临时目录过长，七项均在绑定私有 socket 时失败。这两次原始日志继续留在私有任务目录。首次探索性 SQLite `mode=ro` 查询使 block-policy 夹具出现空 WAL 与 SHM。随后留存库摘要只查询私有副本，并在查询前后比对所有源文件散列。
- 已核对原轮次的正常套件和真实失败均有完整退出记录。复查进程表时无本任务原有的测试二进制在运行。
- 本工作树的定向基础设施检查通过 22/22 项。全量三次各发现 1099 项、跳过 16 项。首轮有三处原生打包审计超时。第二轮的原生审计通过，但编写中的交接文档暂缺 `EVIDENCE.json` 链接目标。补齐后第三轮链接与历史逆转均通过，仍有两处原生打包审计超时。保留三轮原始记录，测试及其期限未作调整。集成窗口另报其组合串行通过 1091 项、跳过 16 项。[本工作树验证记录](EVIDENCE.json)
