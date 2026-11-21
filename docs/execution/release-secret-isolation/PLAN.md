# AB-REL-005 普通 CI 凭据隔离计划

从 f912214b0a415f36b4bb4e4d949552d450ad67de 开发。认领 scripts/validate_infrastructure.py、tests/infrastructure/test_pr_workflow_secret_isolation.py 和本目录。集成窗口负责固定摘要和全局证据。

在现有 Environment 作业调用的检查器中解析两种 YAML 扩展名。保留原有 gate、Action 固定版本、时限、宿主范围和必需检查名。覆盖普通作业权限覆盖、环境绑定、复用凭据、Action 凭据和表达式变体。验证对象为现有工作流及使用占位符的临时工作流。

从原命令验收现有路径、负控和完整基础设施测试。当前工作只覆盖普通 CI 的静态隔离，签名和真实账户的保护环境另行验收。
