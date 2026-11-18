# Schema 19 · Factory 运行时测试交接

产品测试源码 `531a5f14986d023cdadc65ca5a555af3a020214c`，认领基准 `577b922eb6df9f133c14508ffa26aa4b3bbaf290`。与 A 的 `17252ce69f1189a904ab24ff40385f9dca99e265` 组合测试的 Git 树为 `ecc29d1ea251d8f6f1b2d3c9cbc3fa3dcf63365e`，本机组合提交 `1e1c83184ed620d476e51a9d863876b11b383c29`。

- 现行 Factory Data 与 Child Goal 在原生命周期位置通过只读 SQLite 核对真实 Store19 和空 Host 注册。Data 另外匹配真实 IPC 的 API 1 和版本。已固定的有效工具、原审批、负控、读门槛、时限和历史显式目标照常参与。
- Apple Silicon 原基准三项在真实 19/旧 18 守卫退出，修复后同三项通过。精确组合 Mac 全组先 34/36，后 36/36。原 wrong-CAS 拒绝仍命中，后继实际子会话创建失败并回收。另一例在原期限未到 prompt，去掉新增 snapshot 的单因素对照也重现。后继创建失败的底层错误尚未分辨，短组成功没有覆盖第一次原件。
- Linux ARM64 在禁网容器执行同一组合，39/39，含原 3 个 read gate。所有实际冻结工具 prepare/verify 成对一致，原始报告分别记录直接回收和既有边界。
- Mac 组合 workspace 468 个 Core 单元通过，随后自然退出 101，首红 `admission_performance::real_admission_profile_and_post_reservation_input_drift_remain_guarded` 的旧 schema 18 断言。Factory 完整证据来自同组合的独立默认目标。合并预演和已执行组合的各自身份见 [EVIDENCE.json](EVIDENCE.json)。
