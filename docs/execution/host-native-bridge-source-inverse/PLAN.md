# Host 桌面接线的历史来源校验适配

窗口 B 认领 `collab/host-native-bridge-source-inverse`。预期候选 target `origin/collab/host-native-bridge-candidate`，固定基线 `8aa4483d7028f9e9cb65a709bf2659f5e6715efd`。前驱功能快照 `1f756ce201b8cf10f4b1798053674f4e960ac77d`。ACK OWNER-001。已核对本地 `origin/main` 的 OWNER-002，UI 由集成窗口另行安排。

1. 读取组合验证的原始失败日志，通过既有项目 venv 及 collab run 在精确基线复现同名失败。
2. 用 Git 对照确认唯一审查的新增是模块声明和七个固定生产 handler。固定原始输入及前驱整文件 SHA-256，按两个各自唯一的字面上下文删除这八行。在原后台测试的方法中，仅将还原输出交给原整文件逆变换。独立命令和启动摘要继续读取当前源文件。
3. 验证固定候选能逐字还原前驱，并以变更、重复、缺失、错位、BOM、CRLF 和其他源文件作负控。
4. 固定源码，依次用既有 venv/collab run 执行新增和原后台定向测试、C3 源检查和完整 infra discovery。归档原始日志和源码/候选绑定，确认 scope/diff/目标树预检，交 leader 独审。

本任务仅 Python 基础设施验证。Rust、原生和 CI 验证维度沿用此前根据实际运行记录的证据。

## 验证命令

在任务工作树内通过 `bash scripts/collab.sh run -- env PYTHONDONTWRITEBYTECODE=1` 调用既有 `/Users/wander/Documents/code/agentBaton-next/.venv/bin/python`。

```sh
python -m unittest -v tests.infrastructure.test_background_service_native_smoke_source.NativeBackgroundSourceBoundaries.test_complete_production_inverse_and_all_nineteen_original_tests
python -m unittest -v tests.infrastructure.test_host_native_bridge_source_fixture tests.infrastructure.test_background_service_native_smoke_source
python apps/desktop/src-tauri/tests/test_hosts_native_source.py
python -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
```

首条用于固定基线的红测复现。其余命令在固定实现提交上运行。
