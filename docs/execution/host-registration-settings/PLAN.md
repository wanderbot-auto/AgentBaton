# C4 · 设置中的 Host 登记与恢复

窗口 B 使用 `collab/host-registration-settings`。基线和目标 `origin/collab/host-registration-integration` 固定为 `02f2886285f21cf62ba616c8c1797728191677fe`。按任务卡和 C4 wire 合同，仅修改 App 设置接线、八个专有文件和此任务证据。ACK OWNER-001，已核对 OWNER-002。当前设置样式和语言入口继续使用。

1. 用现有 request 接七条固定命令。解析闭式 schema1 DTO、Host 声明元数据及回执关联。
2. 新增设置披露，原请求先读私有 pending，开设置后只读核对。已确认回执由用户明确确认后清理，核对成功缺失才提供显式原键重试。本机后台断开仍可读取 pending。库存和 pending 可并行，远端连接、执行和启动引导留待各自范围。
3. App 保存一个稳定的 Host session 引用。关闭设置只废弃当前视图，原始未决请求、键或已知回执保留。请求世代隔离重新连接和重开产生的迟到响应。
4. 在本任务 worktree 通过 collab run 和原 npm lock 验证新增定向和完整 Vitest、typecheck、Vite 构建、源与 infra。收集首失败与精确日志，验证范围及目标预检，交 leader 独审。

## 验证命令

命令在本任务工作树经 `bash scripts/collab.sh run --` 串行执行。

```sh
npm --prefix apps/desktop ci --no-audit --no-fund
npm --prefix apps/desktop test -- src/hosts-types.test.ts src/hosts-api.test.ts src/Hosts.test.tsx src/Hosts.entry.test.tsx src/QuietCanvas.test.tsx
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test
npm --prefix apps/desktop run build
env PYTHONDONTWRITEBYTECODE=1 /Users/wander/Documents/code/agentBaton-next/.venv/bin/python apps/desktop/src-tauri/tests/test_hosts_native_source.py
env PYTHONDONTWRITEBYTECODE=1 /Users/wander/Documents/code/agentBaton-next/.venv/bin/python -m unittest discover -s tests/infrastructure -p 'test_*.py'
```

验证分支自身 scope、固定基线 diff 和目标预检；App 与原生运行由集成窗口的后续组合验证覆盖。
