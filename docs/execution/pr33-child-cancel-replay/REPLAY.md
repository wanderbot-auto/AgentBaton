# PR #33 Child Goal cancel 的 /proc 采样竞争

托管 Product UI 的 normal 和 lost-start 通过。cancel 完成 CG01/02 后在已发 Start 的节点等待中失败，驱动栈为 `2174 → 2157 → 1644 → 1810`。有限诊断 v5 把失败限定到 `started_records`、`factory_base:525`、errno 3。没有托管原始 IPC 或进程采样可用于识别具体 PID。 [托管作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44787136337/job/136464852658)

托管 checkout `0bef7594e7f44cf9cd2e9e1c946257b7556479da` 与工作树基准 `b3e274802672d5173c2a06e5badfe3bdafc19b9e` 共用树 `93c2f76419ee16b8d4326d166c19d86104cd20ec`。托管 ZIP 摘要与 GitHub 记录相同。在 `scripts/collab.sh run` 的任务外部私有目录准备新 checkout，用 Linux ARM64 Docker、网络禁用、Rust 1.91.1、Chromium 151.0.7922.34。执行工作流的 `prepare-schema19-browser-ci.py core`、`frontend` 与 `freeze`，然后用原驱动和投影器。三个原用例使用同一个冻结候选。

## 调度对照

调度探针只在真实 Start 回执使原驱动发布 `release-parent` 之后，短暂延后生成夹具父进程已有的 `mcp.wait(timeout=1)`。伴随进程关闭原 stdin 后自然退出。原始收集器先复验既有记录，包括文件、nonce、Core session/turn/attempt、旧绑定和身份，再走原有 late reaper 分支。探针在 **L525 对已绑定 MCP 的 /proc stat** 匹配该栈后才打开内核 FD。它在夹具目录放行原有 wait。收割前的 zombie PID、birth、PPID、PGID 和 UID 与记录一致。

父进程收割后，名字查询返回 ENOENT，已打开 FD 的真实 read 返回 `ProcessLookupError(ESRCH)`。原版随即生成与托管相同的失败投影。内存中的分类实验仅对 L525 吸收 ProcessLookupError，说明锚点足以恢复原流程。该实验保持真实 Core 与 Chromium、使用修改调度的生成夹具。独立权限对照在相同位置交付 EACCES，仍失败并投影 errno 13。该权限对照是主动注入的错误。 [有限证据](EVIDENCE.json)

## 修复认领

- `apps/desktop/tests/factory-real-ipc.py` 的 late bound companion stat。仅对该次 ESRCH 重新观察固定 PID 一次。确实消失则保留旧记录。具有不同 birth 的新进程不会获得旧进程身份，也不读新 argv、不等待、不发信号。同一身份仍在、未知错误和没有旧绑定都不能通过该路径。
- `tests/infrastructure/test_factory_bound_provider_exit.py` 增加一次重采样、权限拒绝、identity 不匹配及无绑定对照，保留既有 parent/PGID/收割和强拒绝。
- `apps/desktop/tests/factory-child-planning-real-ipc.py` 的当前整文件 pin 按新 Factory fixture 固定。 `tests/infrastructure/host19_child_goal_source_fixture.py` 追加测试专用精确逆转，在旧 Schema18/串行 history 逆转之前恢复原规划驱动和 Factory support pin `e81410e1…`。所有既有 SHA 和原 Goal 驱动、投影器、CI 都可逐字核对。
