> 执行授权更新：AB-START-001 已批准完整 v0.2 研发。此目录是保留的设计基线；work-packages 中的 not_granted 与 requirements 中的 specified 反映交付当时状态，不否定当前授权。当前权限从 infra/execution-gate.json 读取，实时验证进度记录在 docs/execution/。不要修改基线或删除需求来制造完成。

# 机器可读规范 · Design v0.2

`decisions.json`记录已批准产品行为，`questionnaire.json`追踪27题；批准日期未知不伪造精确时间。`requirements.json`为当前完整需求，软件状态仍为specified。`retired-requirements.json`保留外部TUI观察等已退出范围的原ID，不把它们当待做功能。

`work-packages.json`记录依赖，不是短期目标。`authorization=not_granted`保留设计交付时的授权快照；当前实际执行范围由 `infra/execution-gate.json` 覆盖，不能从产品行为或历史字段推导额外账户/资金授权。`provider-compatibility.json`的正式目标不等于真实集成测试通过。

Schemas是Baton内部设计契约，不是原生CLI参数；实例全部为合成，不含真实账号。原四组v1示例保留，增加连接、执行绑定、项目策略三组。Schema不能代替权限/路径/数据/版本的语义检查。默认与样例数值不可混用，90天留存批准粒度见D04。
