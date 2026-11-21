# PR #33 子 Goal Start 排查计划

基准提交 `36a99a4fbcc58424b01815cdd44414317a1fbb3c`。本工作树只写本目录的排查与交接材料。用托管失败工件确认实际用例、失败窗口和有界诊断，再从托管 merge SHA 重建 Linux ARM64 Core 与 UI。对照 normal、lost-start 和 cancel 的原驱动，捕获正常的父记录、计划历史和子 Goal 刷新以及 Start 点击时的 DOM、IPC 及回执。

用一次受控的只读拒绝检查渲染行为，再在独立运行中让真实 Core 处理同 scope/task 的并发只读 Context，请求与响应经过原 fixture bridge。两条真实 CAS 拒绝、5 秒渲染就绪和 15 秒回执等待照常运行。复现时只允许原驱动触发一次显式 Start 点击，后台与对照操作均为读取。
