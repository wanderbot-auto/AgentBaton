# Goal 原进程单次完成后停止

基线 `bd7fa954e545d8aa7d5a8037b15ab951bbf4ce53`；分支 `codex/goal-owned-turn-stop`；当前源码 `8367a663017c1e2c6b9c00b172cc7272fc050ab4`。该单元支持既有需求，不改变130项需求状态或验收标准。

目标是消除一次 Goal Work 已完成后仍等待 Actor 发停信号的可避免执行，同时保留原4秒、全部真实启动/等待/终止计费、真实停止证明、未知输出、Grant/输入/选择/CAS和后续验收。

1. 以原Actor已提交turn身份构造非序列化stop-only ticket，只有真实相关completed可提前结束原owned process。
2. 真实joined receipt保留terminal proof；原stop/lease/费用流水失败时保留，并防重复非幂等释放。
3. 完成12项基本回归后，针对独审发现的update+endturn/full64补第13项双顺序及伪造/缺失历史拒绝。
4. 独立源码审查、精确源码focused、旧hint/原sessions、strictClippy/infra/Core/Factory、真实IPC及双Mac/CI分维度验证。
5. Supervisor生命周期hooks另提交，不把本次结果移给Core18/Native。
