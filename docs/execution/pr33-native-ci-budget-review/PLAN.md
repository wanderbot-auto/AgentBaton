# AB-REL-001/002 · PR #33 原生 CI 时长取证

任务基线 `ae844ca33406aabd62f44ea73818f9127f211748`。OWNER-001 与 OWNER-002 已查阅。时长取证写入本目录。

- 读取两次 PR #33 macOS 作业的 GitHub 步骤、实际检出记录及终态日志。把执行失败、超时、跳过和通过分别记录。
- 继续观察 [当前作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44439619004/job/135276914703) 到终态。保留 14/15/16/18 迁移、Core、真实 WebView、审批、发布配置审计与开发包各自结果。
- 对照冷构建、测试执行、同一作业中重复调用和独立目标目录。只给实测时长，未执行的优化仅记为候选，说明一点式变更与来源护栏。
- 现有检查、期限、原生/历史模式、授权和产品源码均由各责任窗口管理。本轮仅通过读操作观察，不触发新运行或 GUI。
- 作业终态确认后，Leader 追加认领 `scripts/native-run.py`、`scripts/native-macos-smoke.sh`、`apps/desktop/src-tauri/tests/test_native_runner.py`、`tests/infrastructure/test_background_service_native_smoke_source.py` 与 `tests/infrastructure/host19_native_webview_source_fixture.py`。对当前证据只接受严格整数 Store19，保持旧源码反演、计数与区间、拒绝及回收校验，在同一原始 ZIP 成员上做红到绿离线回放。
- 组合复验发现历史构建器需要旧的 runner SHA 后，Leader 追加认领 `scripts/prepare-schema18-migration-fixture.py` 和 `tests/infrastructure/test_schema18_migration_preparation.py`。校验当前 runner 和反演文件完整字节，恢复历史 runner，再按原 HELPERS SHA 执行。单独重跑 schema18、schema19 定向套件和基础设施全量，并通过现有原生门验证真实历史构建与生成路径。
