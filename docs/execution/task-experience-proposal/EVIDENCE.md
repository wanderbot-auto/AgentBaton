# AB-VAL-008 验证证据

实现固定在 `da7e3ebaeddeca8dd4be183f7a592d8d61b3eb40`。基础设施复跑使用 `1bbeaaadc65034158f0ff92ed3f12f23fdd55ce0`，两次提交的 `apps/desktop` 内容相同。Apple Silicon 验证机为 Darwin arm64、macOS 26.6.1（25G76）、Node 26.7.0、npm 11.19.0、Python 3.12.12。见 [环境原始记录](logs/environment.txt)。

| 命令（通过 `bash scripts/collab.sh run --` 执行） | 结果 |
|---|---|
| `npm --prefix apps/desktop test` | 114 文件，1,597 项通过。见 [Vitest](logs/vitest.log.gz)。 |
| `npm --prefix apps/desktop run typecheck` | 退出码 0。见 [TypeScript](logs/typecheck.log.gz)。 |
| `npm --prefix apps/desktop run build` | 退出码 0。Vite 报告单个 JS bundle 超过 500 kB 的提示。见 [构建](logs/build.log.gz)。 |
| `env PYTHONDONTWRITEBYTECODE=1 /Users/wander/Documents/code/agentBaton-next/.venv/bin/python -m unittest discover -s tests/infrastructure -p 'test_*.py' -v` | 共运行 991 项，975 通过、16 跳过。见 [基础设施复跑](logs/infrastructure.log.gz)。 |

正常 App 入口通过 jsdom 渲染完整 `App` 与生产的 Task 快照解析、真实项目选择组件及任务详情，Tauri IPC 由测试替身响应。覆盖候选来源、未验收或缺失验收引用、修订和验收标识变化、切换项目、错误长度、编辑、取消、键盘焦点和原样预览文本。IPC 仅观测到 App 原有读取，候选文本未写入浏览器存储。另外运行中文组件测试，覆盖 Unicode 边界和不同的候选类型。本次证据覆盖前端与基础设施测试，原生 Tauri/WebView 与 CI 由独立验证跟踪。

首次定向测试的两处断言错误与首次基础设施运行的文档链接错误均已修复。原始记录见 [首次定向测试](logs/vitest-first-attempt.log.gz) 和 [首次基础设施](logs/infrastructure-first-attempt.log.gz)。

构建产物的 SHA-256 见 [build.sha256](logs/build.sha256)。冲突预检只确认 Git 合成，测试对应上文的固定任务提交。见 [范围与冲突预检](logs/scope-preview.txt)。
