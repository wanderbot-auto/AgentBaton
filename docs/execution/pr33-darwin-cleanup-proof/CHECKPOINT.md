# PR #33 Darwin 清理修复交接

基准 `53da4ce20fdfedae4a745e704394072ad6b742ae`，实测源码 `6e0e4f775762a224b89be338aedb49f725899647`。在原直属子进程仍未回收时，EPERM 分支调用既有 `owned_group_is_quiescent`，以当前证明确认原组长及组成员状态。初始 observation_error、整次 failed、原信号目标和时限继续由原条件约束。

| ARM64 真实条件 | 基准 | 修复 |
| --- | --- | --- |
| 观测错误后真实 SIGKILL | EPERM(1) | EPERM(1) |
| wait 前的 runner 组证明 | 短路 | 实时证明原组仅有 zombie |
| wait 及二次 waitpid | -15，ECHILD(10) | -15，ECHILD(10) |
| 真实回收、整体状态 | PID 与组均消失，failed | PID 与组均消失，failed |
| 报告的 owned_group_cleanup | false | true |
| 忽略 TERM 的一秒控制 | 1.177 秒，SIGKILL 回收 | 1.171 秒，SIGKILL 回收 |

Apple Silicon 上直接套件由 38 项中唯一原失败转为 39 项中 38 项通过、1 项 Linux 专属跳过。历史迁移及来源护栏 28 项通过。完整基础设施 1,099 项中 1,083 项通过、16 项跳过。设计验证、基础设施静态检查及 bootstrap 完整性检查通过。 [脱敏测试摘要](logs/test-summary.txt) [脱敏真实对照](logs/real-syscalls.json)

当前 runner 先逐字恢复 `53da4ce2` 的 runner，再逐字恢复 `ae844ca3` 的历史 runner。迁移 loader 输出与后者完全相同。原 Store19 反演条目、Store18 固定值和历史 shell 原件均经比较。 [原件和 SHA](logs/source-bytes.json)

回放从证据目录读取原样 `diagnose.py`，先校验固定 SHA-256，再加载 tracer。基准与待测 runner 来自分支历史中已有的两个 Git 提交。 [固定 tracer](diagnose.py)

Apple Silicon 上使用 `git clone --no-local --single-branch --depth 8` 从本地分支建了独立浅克隆。克隆无 alternates，回放前后均确认缺少原诊断提交 `5ecb480b` 的对象。真实回放退出 0，基准与修复的清理证据差异和实际回收结果均复现。私有副本追加一个换行后，tracer 加载被拒绝。 [独立检出护栏](logs/clean-checkout-proof.txt) [独立检出真实对照](logs/clean-checkout-syscalls.json)

在已认领的证据工作树中设置 `PYTHON312` 为实际 Python 3.12 的绝对路径，然后串行重跑直接套件和精确源码对照。

```bash
bash scripts/collab.sh run --native -- "$PYTHON312" -B \
  apps/desktop/src-tauri/tests/test_native_runner.py -v
bash scripts/collab.sh run --native -- "$PYTHON312" -B \
  docs/execution/pr33-darwin-cleanup-proof/replay-comparison.py \
  6e0e4f775762a224b89be338aedb49f725899647
bash scripts/collab.sh run -- "$PYTHON312" -B -m unittest \
  tests.infrastructure.test_schema18_migration_preparation \
  tests.infrastructure.test_background_service_native_smoke_source -v
bash scripts/collab.sh run -- "$PYTHON312" -B -m unittest discover \
  -s tests/infrastructure -p 'test_*.py' -v
```

本次实测范围是 Apple Silicon 私有子进程及历史护栏。GUI、Linux 复跑、完整历史编译迁移和托管 CI 未运行。交付分支留在本机。 [证据索引](EVIDENCE.json)
