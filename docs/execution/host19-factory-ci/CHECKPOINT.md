# Factory schema 19 交接

固定基线 `7e41a4834af2ee592c9bba94b00152a40eda9b7a`。实跑源码 `29f6a14147880a2a1744e1044278165d8c11f4db`，源码树 `63023c96e04d28827d2d280112b79ac295c2fdc6`。交付提交只在实跑源码上增加本目录记录。保留三条路径认领等待独立验收。

- 本机 Linux ARM64 Docker 以真 procfs、Chromium、私有 Core、生成 ACP 和真实 Baton MCP 跑完三案，合计 15 项和 6 张原图。终态依次为 completed、cancelled、outcome_unknown，实际 `system.snapshot` 和只读 SQLite 都返回整数 19，已创建尝试及停止回执均为 3/1/1，声明产物 bytes 为 88/0/0，根与项目 active slots 都为 0。相应的 6/2/2 个 ACP/伴随程序原身份均在回收时已消失。
- 实际冻结候选和构建回执同源。九项恶意或错误候选输入在纯前置入口拒绝。浏览器关闭、HTTP 关闭和前端漂移三项注入在业务结束后退出 1，daemon 各 wait 0、HTTP join，各两个原身份已回收且无补发信号。首轮缺 strip 的红原件保留。
- 定向 13+84+18、全量基础设施 985 通过及 16 跳过、桌面 1597、root Node 4、preflight 和 smoke 通过。额外诊断自检在当前与逐字复原的历史文件均退出 1，原因相同。其余旧静态 63+13+28 和 Child 纯测试通过。
- 新鲜远端目标 `b5681ec4fa3ff2f48fe0c40bc24b436ce4e1c4ea` 与固定源码通过合并预览。共享 workflow 的候选仍是 schema 18，待集成窗口接入 schema 19 和前端素材清单。托管 Linux、macOS 原生 Owner UI 及 Child 浏览器运行尚待验证。
- [EVIDENCE.json](EVIDENCE.json) 保存构建、输入、版本、过程回收、输出和红记录的固定摘要。原报告、截图、候选及日志保留在任务私有 runtime。
