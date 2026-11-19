# 执行进展

本地 Git 对象校验通过，私有 checkout 初始状态干净。三份锁文件直接从精确 Git blob 封存，再与 checkout 比较。运行脚本沿用前次锁定构建的隔离工具环境，清除调用窗口的构建变量，只使用本机依赖缓存。

离线 npm ci 已完成，Tauri CLI 2.12.4 和 Vite 8.3.5 与锁文件一致。私有运行目录为 `/private/tmp/ab-collab-0dvd6o1q/pr33-package.SQF5QB`。完整过程和退出状态由 `build.log`、`build.exit` 保全。

同一轮的普通 ARM64 辅助程序、默认 Tauri App、旧单文件审计、实际 DMG 的三二进制审计及四文件 provenance 读取均已完成。检查确认实际包与本轮编译原件 SHA-256 一致，checkout 干净，自有映像均卸载。

独立复核初次对 Cargo fingerprint 的特性位置作了错误假设，按原样保留失败。复核改为分别读取应用 crate 与 Tauri 依赖的实际特性，原包和编译输入均未重建。应用 crate 未启用 native-smoke，Tauri 依赖启用了 custom-protocol。严格签名检查的已知限制随证据记录。
