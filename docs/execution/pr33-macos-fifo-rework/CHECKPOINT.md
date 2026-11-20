# PR #33 macOS FIFO 复验交接

| 对象 | Git commit | Git tree |
| --- | --- | --- |
| 任务基准 | `53da4ce20fdfedae4a745e704394072ad6b742ae` | `9ce195d936d5798b077f50d1b7e25102318ce9c6` |
| 原生 Store 19 二进制对应源码 | `6c544406108c81046dcafc28f07b05e3b6983e93` | `b41a5d2c0eb83926d6b95c80a90e88c2558bc2e2` |
| 两个历史入口修正后的源码 | `a2352b87ddb1b6765e54db4442256ca730292ca5` | `a05fe04d84d2f3558fa60bf37659293f04f83be5` |

本地分支为 `collab/pr33-macos-fifo-rework`。可交付的源码仅涉及两份 Rust 测试夹具和一份 Python 源码守卫。守卫只从精确 Store 19 进入 19→18 逆转，继续验证已有 Store 18 和历史产物 pin。Store 18 collector 的 18→17 逆转和原阶段逆转保持逐字节校验。证据提交是源码修正的后续文档提交。

## 第一轮 preparation 的最后阶段标记

七个真实 FIFO 失败样本均来自 Apple Silicon 私有低优先级测试进程。六个是基准二进制的留存库样本，一个是新 collector 的真实失败投影。时间单位为毫秒，相对于原准备计时；“下界”来自收尾事件。

| 样本 | 最后一个已记录工作阶段 | 准备返回 | 观察到 join | 结算下界 | 证据里的错误 |
| --- | --- | ---: | ---: | ---: | --- |
| 基准 non-descendant | `control_isolate` 1690 | 2058 | 2107 | 2118 | `DEADLINE_EXCEEDED` / `INPUT_CHANGED` |
| 基准 owner cancel | `control_isolate` 1875 | 2000 | 2152 | 2167 | `DEADLINE_EXCEEDED` / `INPUT_CHANGED` |
| 基准 owner revoke | `integration_authorize` 1857 | 2009 | 2066 | 2078 | `DEADLINE_EXCEEDED` / `INPUT_CHANGED` |
| 基准 third advance | `candidate_prepare` 1705 | 2049 | 2069 | 2081 | `DEADLINE_EXCEEDED` / `INPUT_CHANGED` |
| 基准 three rounds | `integration_authorize` 1979 | 2029 | 2051 | 2068 | `DEADLINE_EXCEEDED` / `INPUT_CHANGED` |
| 基准 unexpected exit | `source_capture` 943 | 2001 | 2017 | 2042 | `RESOURCE_LIMIT` |
| Store 19 投影 non-descendant | `source_capture` 1001 | 2006 | 2151 | 2164 | `RESOURCE_LIMIT` |

所有这些样本的第一轮 preparation 均预留 2000 ms，结算记录的时间下界均已超过预留值。前五个基准样本明确记录 supervisor 的 `DEADLINE_EXCEEDED`。Store 19 投影在 preparation 返回时记录 Git 调用 29 次、完成 28 次，后续阶段采样槽为空。两个 `source_capture` 样本均以 `RESOURCE_LIMIT` 返回。对照 `crates/core/src/integration_io.rs` 的阶段标记和错误映射，该区间的 `capture_source_commit_controlled` 将 `WorkspaceError::ResourceLimit` 和 `WorkspaceError::TooLarge` 合并为同一个错误。记录可将这两个样本缩小到受控源捕获，仍无法区分内部限额、确定具体 Git 子命令或还原七个托管失败的阶段。[留存库摘要](background-retained-summary.json) · [真实失败投影](collector-real-projection.json)

原有 500 ms 命令、5 s FIFO 观察和 2 s preparation 预算均保留。托管 FIFO 根因与修复仍未验证。这个交付恢复了 Store 19 夹具的失败采集，托管重跑和候选组合的验收由集成窗口持有。[验证命令及原始证据散列](EVIDENCE.json)
