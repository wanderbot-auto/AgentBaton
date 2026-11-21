# Factory schema 19 浏览器验收

固定基线 `7e41a4834af2ee592c9bba94b00152a40eda9b7a`，独立分支 `collab/host19-factory-ci`。认领 Factory 主 driver、定向测试和本证据目录。已核对 AB-START-001、OWNER-001/002 和共享 workflow 的三个既有入口。

- 保留旧 schema 18 preflight 和 Child 系列调用的 helper。固定已验收 Graph 19 的读取 helper 源字节。现行 Factory 启动前核对清单的版本、Git 来源、两工具与全前端素材，启动后核对实际 IPC 和只读 SQLite。
- 验证清单及运行期间的漂移拒绝。使用已核验依赖，在独立容器从固定任务源码构建同源 ARM64 Core 候选，按顺序实跑 normal、cancel 和 unknown。保留原预算、原时限、128 MiB companion 限额、历史与进程检查。
- 记录三个场景、负控、原图、工具摘要、定向及全量结果，在精确目标做冲突预览。由集成窗口独审和合入。
