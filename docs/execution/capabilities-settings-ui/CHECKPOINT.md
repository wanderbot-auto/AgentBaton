# 集成交接

- 目标为受保护的 `main`。本任务基准是 `145a5041e93f82778167de28ac8aebbc4b795cd5`，实现提交为 `93ef96c66e5dd80bb26106bf548f9b62d556aadc`，最终核验的前端修订为 `a554ebbe82278ae68f4e85bc00f1c7e1ddd14125`。交接提交只更新证据。
- 已在隔离工作树通过路径范围检查及对准确目标的 Git 冲突预检。测试明细见 EVIDENCE.json。全局 STATUS 的编辑权保留给集成窗口。
- 本机浏览器使用已有的兼容 ARM64 schema 19 daemon，来源与散列单独登记。PR 候选仍依赖准确提交的 CI、原生 WebView 和开发包验证。真实 Provider 联通、macOS 登录项写入及生产发布没有在此任务中执行。

## 最新导航调整

内嵌页面验收绑定到 `a957054e10adfb49ab158cc29975cb41350ade7a`。已从原隔离工作树重新启动开发实例，核验可见桌面窗口、前端源码及 schema 19 后台连接。此次浏览器验证覆盖返回、窄窗口布局和真实只读 IPC。PR 对应的原生确认和开发包检查仍由 CI 验证。

## 方案 B 交接

实现及测试对象为 `95ac1888789591b87afb16dbcb94bb6a8eec5ca2`，目标为 `145a5041e93f82778167de28ac8aebbc4b795cd5`。115 个渲染测试文件、1609 个用例、生产前端构建、真实 Core 浏览器流程的 40 项检查、13 项工作区夹具回归和设计校验通过。Apple Silicon 上实际开发窗口显示新版任务界面，连接本地后台。浏览器覆盖 1488、1440、1024 和 390px，原生窗口截图单列。全套原生操作与包验证等待对应候选 CI。详见 EVIDENCE.json。

## 2026-11-24 · Mantine

- 基线 `145a5041e93f82778167de28ac8aebbc4b795cd5`。已验证前端及原生候选 `80b932b9be1f732fc82d11877e80873abbcebdcb`。后续 `dc23d658b6135a56aefdefefddef62972c62b4a7` 只调整历史文件、执行计划和数据工作区浏览器驱动，前端制品相同。
- 本机 `bash scripts/collab.sh run -- npm --prefix apps/desktop run build`、前端 Vitest、双引擎真实 IPC 浏览器驱动、`scripts/native-macos-smoke.sh`、原生登录烟测及数据工作区驱动通过。报告及截图在当前工作树的 `artifacts/ui-checks/mantine-final/`。
- 集成窗口应在实际合并历史导出代码后重测项目页。远程必需检查由 PR #37 验证。
