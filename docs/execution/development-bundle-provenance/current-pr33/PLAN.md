# PR #33 当前 ARM64 开发包验证计划

固定 PR #33 合成提交 14530c62a0b532e2ba43ca4be4fbc75ccc41d856 及树 41b3e2fe076ee4ebcb6355dddc1505f6bb75e135。使用本任务普通 collab run 中的独立 detached checkout。记录基线工具 c4accb5dc0901aa219a31dfe9fbe31cc0c241450，各自保留 exact SHA。

沿用该提交的 release sidecar、Tauri 默认 release 应用、测试标记扫描和开发 DMG 打包命令。本机构建加 --no-sign，并在无正式签名和公证变量的独立环境运行。验证包的 SHA-256、Git 来源、实际 ARM64 架构、签名显示和逐文件状态。构建输出只保留本机，应用运行与人工验收另行安排。
