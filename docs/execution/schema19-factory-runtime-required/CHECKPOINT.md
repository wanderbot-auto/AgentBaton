# Schema 19 · Factory 运行时测试交接

产品测试源码 `531a5f14986d023cdadc65ca5a555af3a020214c`，认领基准 `577b922eb6df9f133c14508ffa26aa4b3bbaf290`。与 A 的 `17252ce69f1189a904ab24ff40385f9dca99e265` 组合测试的 Git 树为 `ecc29d1ea251d8f6f1b2d3c9cbc3fa3dcf63365e`，本机组合提交 `1e1c83184ed620d476e51a9d863876b11b383c29`。

- 现行 Factory Data 与 Child Goal 在原生命周期位置通过只读 SQLite 核对真实 Store19 和空 Host 注册。Data 另外匹配真实 IPC 的 API 1 和版本。已固定的有效工具、原审批、负控、读门槛、时限和历史显式目标照常参与。
- Apple Silicon 原基准三项在真实 19/旧 18 守卫退出，修复后同三项通过。精确组合 Mac 全组先 34/36，后 36/36。原 wrong-CAS 拒绝仍命中，后继实际子会话创建失败并回收。另一例在原期限未到 prompt，去掉新增 snapshot 的单因素对照也重现。后继创建失败的底层错误尚未分辨，短组成功没有覆盖第一次原件。
- Linux ARM64 在禁网容器执行同一组合，39/39，含原 3 个 read gate。所有实际冻结工具 prepare/verify 成对一致，原始报告分别记录直接回收和既有边界。
- Mac 组合 workspace 468 个 Core 单元通过，随后自然退出 101，首红 `admission_performance::real_admission_profile_and_post_reservation_input_drift_remain_guarded` 的旧 schema 18 断言。Factory 完整证据来自同组合的独立默认目标。合并预演和已执行组合的各自身份见 [EVIDENCE.json](EVIDENCE.json)。
## Config 库更新后的 Factory 补充复验

目标 `7b04514caaf7807b46018547a8b10f2a5e7fc3d6` 合成原交付 `76916962fbef91abd4144e4fad82ed4e705f6e47`。实际测试提交 `a72d35d01067fde5adae16f05a930870f58b1571` 的树与合并预演均为 `aed04f79ea4e8ae2657bee2c055c46b062fcede9`。两份 Factory 测试源码与 `531a5f14986d023cdadc65ca5a555af3a020214c` 逐字一致。

- 两平台重新构建了同一组合的普通 `baton` 和 `batond`。真实准备器观察到 API 1、Store 19、空 Host 注册和 daemon 正常回收。Apple Silicon 36/36（69.10s），Linux ARM64 39/39（58.92s）。两端的用例清单与各自原通过组一致。Linux 三个原 read gate 的实读量均为 65,536 字节，直接 daemon 均已回收。
- Linux 容器停启后首次 verify 报文件身份漂移，Factory 未启动，容器退出 1。校验原冻结和原构建文件的 SHA-256 均与初次准备相同。复用原构建，在一次容器运行内用新私有目录完成准备、前置校验、Factory 和后置校验，三份回执逐字一致，Cargo 与容器均退出 0。macOS 的准备和前后校验也逐字一致。
- 本轮范围为受编译后的 Config 库变化影响的 Factory 目标。默认 workspace 未重跑，原通过组、失败和诊断原件完整保留。[补充复验清单](CONFIG_REVALIDATION.json) · [原验收清单](EVIDENCE.json)
