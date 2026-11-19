# AB-REL-006 GitHub Actions 观测交接

基准 `53da4ce20fdfedae4a745e704394072ad6b742ae`，实测源码 `708f64fd537ecd4ccf8b2dce773729ffb9abbb45`。Core 入口为 `integration::ci_repair::github::map_observation`。受信 host actor 提供现有 `ApprovedCiSource`、当前候选及三份独立来源的 REST 读取上下文。映射产生现有 `CiObservation`，由原决策判断配置和授权。

## 读取契约

| 读取 | 绑定 |
| --- | --- |
| 选中的 endpoint | `GET .../actions/runs/{run_id}/attempts/{attempt}`。请求路径与回包分别匹配相同 run 和 attempt。 |
| 完整作业列表 | `GET .../actions/runs/{run_id}/attempts/{attempt}/jobs`。每个作业与该 attempt、候选 SHA 和分支一致。 |
| 独立最新查询 | `GET .../actions/workflows/{workflow_id}/runs?branch={branch}&head_sha={sha}`。从获批 repo 和 workflow 获取全状态结果，记录完整分支 ref 和实际查询 SHA。 |

服务固定为 GitHub.com Actions。调用端提供成功读取结果的主机接收时间，最后独立读取最新列表。列表须完整，达到 GitHub 搜索上限即视为无法验证。仓库 ID、head 仓库 ID、工作流 ID、提交和分支逐项核对。以最大 run_number 选定最新 run 并要求同一 attempt 与状态。较早或不同记录、rerun、错绑及过期均无可触发的失败观测。 [GitHub 工作流 run REST](https://docs.github.com/en/rest/actions/workflow-runs#list-workflow-runs-for-a-workflow) [GitHub run 编号](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#github-context)

候选分支执行接受 `push` 或 `workflow_dispatch`。PR 及 merge queue 的执行上下文单独处理。该映射拒绝这些事件及 PR merge ref，即使 REST 的 head 字段与候选相同。 [GitHub 事件与执行 ref](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request)

仅 `completed/failure` 的 run 加上同 attempt 内至少一项有效失败作业可生成失败报告引用。完整作业列表提供实际完成时间。run 的 updated_at 只约束完成时间上界。由已完成作业 ID 生成有限引用，失败作业自身也需在新鲜度窗口内。成功、取消、运行中和未知状态都不会提供修复报告。 [GitHub 作业 REST](https://docs.github.com/en/rest/actions/workflow-jobs#list-jobs-for-a-workflow-run-attempt)

## 真实只读对照

仓库 ID `1434422493`、工作流 ID `397305968`。选中 `43592293854/1`，随后以 `dev/macmini-agentbaton` 与 `e8abd6bbc939bc24cf07ba5793aadfd173313b13` 独立查询。列表返回同一 run，作业列表为 4 项。实际完成时间为 2026-11-14T12:18:57Z，报告指向作业 `132439465437`。另对照 PR run `44432204086` 和 job `135252128867`。其 REST run/job SHA 与关联 PR 当前 head 不同，离线测试覆盖两种绑定拒绝。 [真实有限字段](github-observed.json) [原 workflow_dispatch](https://github.com/wanderbot-auto/AgentBaton/actions/runs/43592293854) [原 PR 作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44432204086/job/135252128867)

离线测试用历史窗口重放真实格式及合成负控。这次代码交付覆盖只读映射，运行时获取响应及其接入由后续集成处理。本次没有发起或重跑任何工作流。 [证据索引](EVIDENCE.json)

```bash
bash scripts/collab.sh run -- cargo test --offline --locked -p agentbaton-core \
  --test ci_repair_github --test ci_repair_decision
bash scripts/collab.sh run -- cargo clippy --offline --locked -p agentbaton-core \
  --test ci_repair_github --test ci_repair_decision -- -D warnings
```
