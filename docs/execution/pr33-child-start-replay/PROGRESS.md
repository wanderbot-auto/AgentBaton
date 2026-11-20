# 排查进度

- 校验托管工件 ZIP 与 API 摘要。提取原 normal、lost-start、cancel 的有限投影。normal 失败窗口只覆盖 CG01/02 后的 Start 回执等待。
- 在隔离 Linux ARM64 容器从托管 merge SHA 编译 Core，再通过工作流脚本构建及冻结 UI。首次前端溯源检查因容器访问本机 clone 的松散 Git 对象失败。保留该次日志与构建记录，通过本地 packed fetch 准备同一提交后重跑成功。冻结候选使用成功构建记录。
- 用原驱动完成三案，再通过 Chromium 观察实际点击和渲染状态。受控交付失败对照只替换一次浏览器层 Context 回应，Core 的原始成功回应仍保留。独立轮次向真实 Core 发送三个并发 Context 只读请求，通过原 bridge 观察 click 前置 Context 的实际回传。
- 交付有限证据、失败边界和后续文件认领建议。完整生成夹具记录、浏览器跟踪和过程日志保存在任务私有目录。此分支不含产品、测试或 CI 修改。
- 候选让同 scope/task 面板的 Context 轮流派发。用组件测试验证在途历史、最新待办 generation、外部拒绝、父绑定变化和关面板。用真实浏览器和 Core 重放原版本与最终候选。预期失败和初次环境失败保留，交接给集成窗口。 [候选验证](CANDIDATE.md)
