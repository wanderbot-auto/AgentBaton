# PR #33 Factory 子目标检查点

两处托管失败都发生在原 Start 点击后的 15 秒成功回执等待。两案都完成 CG01/02，各有一次 Context Busy。原工件无法说明错误是否来自 Start 的前置读取，也无法确认 Start 是否送达 Core。相同 Git 对象在本地 Linux ARM64 的三案都通过，托管失败未自然复现。 [托管作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44439619004/job/135276914307)

受测诊断提交 bbddded63d74216427f2adc8cf44122d78e63206。新增 v6 只在原 Start 窗口失败时发布有限观察。独立故障注入验证了零条匹配桥接 Start、闭合 Context Busy、失败退出 1 和脱敏。未注入的三案及原成功投影继续报告 v4。旧源码反演的固定 SHA 和拒绝控制仍有效。

全量 Python 1,052 项通过、16 项跳过。托管新诊断和故障具体来源待集成窗口复验。 [完整证据](EVIDENCE.json)
