# 已绑定 MCP 的 /proc 迟采样修复

基准 `b3e274802672d5173c2a06e5badfe3bdafc19b9e`。复现证据 `9fa8ba30`。候选源码 `150b5b59b805ea0ff3ad556d1c5a437b67e6aad0`，树 `4a372a7147f63bd8ce4d3c3ece9d49cc253fa2f1`。

修复位于既有的已绑定 companion、空或缺失 argv、late reaper 分支。该分支完成同一记录的二次 reaper 检查后，读取 stat 时遇到内核 ESRCH，按原绑定 PID 重开一次 stat。只使用可归因于该 PID 的观测。若路径确实不存在，则保留原绑定为历史。若该 PID 已被不同 birth 的进程复用，仅可观察到原 birth 消失。原采样期间仍能读到同一 birth、重采样再次发生 ESRCH、权限错误、未知 errno 或外来 PID 都通过原失败路径退出。

已存在的 PID/birth/PGID 检查、已采信 immutable 文件、已有 adopter wait 判定和最终清理规则保持原样。真实测试证明只有已经归属测试控制器的固定 zombie 可以被 wait，错 birth、PGID 和被改记录拒绝，重复 reap 为空操作。绑定缺失仍拒绝。读取竞态不生成 stop/join 凭证或 task acceptance。Core 的取消回执和确认依赖原驱动与实际 IPC。

Child Planning 更新当前整个 Factory support pin。第一版候选的 Child Planning 和 Factory cancel 实跑通过。Child Goal 对变化的 Planning 完整源码保守地在 preflight 拒绝，报告保留。最终候选在获批范围内只更新 Child Goal 的一行 `PLANNING_SHA256`。测试专用外层先恢复精确旧 Factory `e81410e1…`、Planning `64e27e2d…` 和 Goal `320af1df…`，再进入既有 Start、串行历史和 Schema18 逆转。历史 pin、Child Goal 其他字节、有限投影器和 CI 保持原样。 [候选验证](CANDIDATE_EVIDENCE.json)

## 实测结果

- 原版的两次强制调度均在已发 Start、尚未发 cancel 的节点等待中失败，实际内核 errno 3 和有限诊断完整路径均匹配托管工件。
- 冻结候选的两次强制调度均实际读取到旧 FD 的 errno 3，随后只对同一 PID 重开一次并取得实际 errno 2，然后完成 CG04、CG06。原取消请求的成功回应故意丢失，用户显式恢复重用完全相同的 key、参数和 CAS。Core 确认 cancelled，未产生 TaskAccepted。
- 权限负控在同一采样交付 EACCES，仍投影 errno 13，未重开 stat，未发出取消请求。该 EACCES 由探针注入。两条实际启动 CAS 拒绝和原等待窗口在所有重放中保留。
- 原 normal、lost-start、cancel 全过。实际 Child Planning 通过九项检查，实际 Factory cancel 通过五项检查。真实 Linux 收割对照和 Linux 目标回归通过。

基础设施 1096 项通过，16 项跳过。首次对最终源码跑全量时，台账草稿的证据链接尚未落盘，设计链接用例失败。生成正式证据后按相同命令完整复跑通过。失败日志及其 SHA 都保留。最终结果来自本机冻结候选。 [托管失败](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44787136337/job/136464852658)
