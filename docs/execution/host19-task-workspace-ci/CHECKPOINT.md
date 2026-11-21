# Task Workspace schema 19 交接

固定开发基线为 9e897d6f6bf0b823e187930fbb84f897be51b89e。实施与实际验证提交为 5f6718778d06e415098ca81ee29c89aa86965480，后续提交只添加本任务证据。Leader 负责独审和最终集成。已与固定基线、远端采样目标 7de2112b9833fb5ec1b4f62b36ea9b5a3c88c603 和 leader 控制头 ce04c706856f00dcc455ad674c5ffd5820967132 做合并预演。

## 现行输入与输出

- 候选 manifest 的 schema 为整数 19，现有源码别名与期望的准确源码一致。当前工具字节和前端构建记录分别绑定到同一源。manifest 摘要取校验过的同一份原始字节。
- Driver 在创建测试数据前观察私有 Core 的 API 1 和整数 schema 19，并核对只读 SQLite 的完整普通表集合及空 Host 表。新增表只运行行存在性查询。原有数据库不变性、六场景、权限、图像及回收断言仍适用。
- 有限投影校验整数 schema 19、确切 64 表计数及整数 0 的 host_registrations。driver 错误和投影器退出分别保留。完整通过才发布 8 张原图。有效的失败报告可以结构化投影，失败状态与图像门槛仍生效。
- 共享 CI 的当前候选生产端需发整数 19。历史 schema 18 与 63 表集合由完整旧源的逆向回放验证。实际托管 Linux 仍待集成后验证。

## 重跑

用任务包装器运行当前定向测试。

~~~sh
scripts/collab.sh run -- python3 -m unittest discover -s apps/desktop/tests -p test_task_workspace_binding_real_ipc.py -v
scripts/collab.sh run -- python3 -m unittest tests.infrastructure.test_task_workspace_ci_evidence tests.infrastructure.test_task_workspace_failure_stage tests.infrastructure.test_host19_task_workspace_source_fixture tests.infrastructure.test_task_workspace_ci_launch_diagnostics tests.infrastructure.test_task_workspace_ci_launch_diagnostic_inverse -v
~~~

本机实测使用按同一源码重新构建的 ARM64 release 侧车和当前前端。Chromium 与独立 Playwright venv 复制自已核验的本机夹具。原始运行文件留在任务私有 runtime。重跑脚本从准确实施提交的工作树启动，脚本本身调用 collab run。核心探针、原报告、8 张图、投影摘要、负控及日志校验见 [EVIDENCE.json](EVIDENCE.json)。

## Leader 独审与交接

Leader 接受 cffdbfa5a12271c853af4b29bad986f96c506b7f 的 Task Workspace schema 19 切片。独立复跑 62 项 driver 测试和 81 项投影定向测试，核对 42 条证据索引及两个实际 ARM64 工具。独审核实六案、64 表、空 Host 注册表、进程真实退出 0、8 张原始 PNG 的逐字节投影，以及两份旧源码的完整还原。

共享候选生产端与实际托管 Linux CI 仍待验证。按交接流程释放 8 条认领，保留原分支、工作树和证据。集成由 leader 负责。
