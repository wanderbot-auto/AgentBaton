# 交付检查点

基线 `53da4ce20fdfedae4a745e704394072ad6b742ae`，验证源码 `1fa62e391e4b0deaef6b5839538deff70fbc96f3`，分支 `collab/recorded-session-state-labels`。只提交认领的前端代码、定向测试与本目录。集成窗口负责独立验收。

七个测试文件共 117 项通过。TypeScript 类型检查与前端构建通过。只读复用 AB-EXT-005 的桥接脚本和同一 Core daemon，在独立 Chromium 中通过原有五项检查，并断言冷重开后完整文案和发送按钮禁用。仅渲染修复，任务状态、发送和审批代码与基线逐字核对一致。

中英文由组件回归覆盖。真实浏览器复验使用英文和生成 ACP。原生 WebView、原生审批、真实 Provider 与 CI 未测，截图中的恢复提示来自测试桥接不提供原生恢复入口。见 [证据](EVIDENCE.json) 和 [冷重开截图](logs/reopened.png)。
