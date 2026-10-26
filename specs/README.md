# 机器可读规范 · Design v0.2

`decisions.json`记录已批准产品行为，`questionnaire.json`追踪27题；批准日期未知不伪造精确时间。`requirements.json`为当前完整需求，软件状态仍为specified。`retired-requirements.json`保留外部TUI观察等已退出范围的原ID，不把它们当待做功能。

`work-packages.json`记录依赖，不是短期目标。`authorization=not_granted`指尚未向实际执行器授权真实仓库/账户/资金，不表示产品决策未批准。`provider-compatibility.json`的正式目标不等于真实集成测试通过。

Schemas是Baton内部设计契约，不是原生CLI参数；实例全部为合成，不含真实账号。原四组v1示例保留，增加连接、执行绑定、项目策略三组。Schema不能代替权限/路径/数据/版本的语义检查。默认与样例数值不可混用，90天留存批准粒度见D04。
