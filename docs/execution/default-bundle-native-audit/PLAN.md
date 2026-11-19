# AB-REL-003 · 默认 ARM64 开发包原生入口审计

固定基线 `c7dffe2ce7f3544e41fb4e2d93f816e536448754`，树 `41b3e2fe076ee4ebcb6355dddc1505f6bb75e135`。已核对 AB-START-001 及 OWNER-001 / OWNER-002。只认领 `scripts/native-audit-release.sh`、`tests/infrastructure/test_native_release_audit.py` 和本证据目录。

1. 核对现役单文件审计调用。在固定历史 ARM64 DMG 上确认三份实际文件与两套既有摘要，保留原工具的行为证据。
2. 增加只读整包入口。结合可信期望文件摘要核对默认角色和 ARM64 格式，再检测由真实 Cargo feature 和原生测试代码指定的标记。缺项、混配与检查失败返回非零。
3. 通过真实历史 DMG 的只读挂载、实际字节副本负控和定向测试验证。保留现役 CI 调用，记录精确源码、命令、环境和结论边界。
