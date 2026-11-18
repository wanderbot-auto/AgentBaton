# Task Workspace 投影进度

开工时固定基线、远端跟踪引用和独立认领一致，工作树干净。Leader 的基线证据显示实际 schema 19 相对旧枚举只增加 host_registrations，初始计数为 0。现行 driver 和投影器仍要求 18。

前一个浏览器任务的受测 Rust 源与本任务基线相比包含不同的 Config 源文件。本次重新锁定构建 ARM64 工具，避免沿用不同源码的二进制。

首次本机 driver 测试为 61 通过、1 失败。原 preflight 测试用未解析的 macOS /var 路径对比 driver 返回的规范路径。将夹具根目录规范化，保留源归属与实际路径相等的断言。

固定实施提交 5f6718778d06e415098ca81ee29c89aa86965480。已核对两份完整历史源的还原字节与基线 Git blob 一致。历史 63 表 schema 18 数据库的 driver 观察与投影回放通过。现行 driver 测试 62 项、投影及历史定向 81 项通过。全量基础设施 Python 990 项中 974 通过，16 项现有跳过。Node、Rust 环境夹具、Clippy、设计、基础设施和环境检查通过。

固定提交上重建了 release ARM64 工具和当前前端。本机真实私有 Core IPC 与只读 SQLite 都得到整数 19，共 64 张普通表且 host_registrations 为空。独立探针返回 0 并回收。正常 Chromium 入口的 WSI-B01 至 WSI-B06 全部通过，daemon 正常回收，HTTP 线程已 join。有限投影输出摘要及 8 张逐字节相同的原图。

独立副本上的四项 driver CLI 错误 schema、类型和来源控制在运行前拒绝。五项投影 CLI 缺表、额外表、非空 Host 表、旧 schema 或跨来源控制返回 invalid，仅输出有限摘要。实际原报告摘要保持一致。实际托管 Linux、共享生产端及集成候选仍待验证。
