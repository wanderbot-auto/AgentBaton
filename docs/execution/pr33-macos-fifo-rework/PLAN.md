# PR #33 macOS FIFO 复验

- 基准 `53da4ce20fdfedae4a745e704394072ad6b742ae`。托管新红灯为作业 135312028796，旧诊断为 `42dac8f4c58e1ea43f5d0b2c0602af842410af48`。
- 先比较托管原件及同源码旧通过样本，再串行原样复跑 Apple Silicon。保留原始失败及私有夹具，根据实际状态判断 validator 启动、写 marker 前置条件或准备阶段是否已达到。
- fixture 修改必须有实测依据。保留原用例、500ms 命令和 5s 观察、真实 CAS 及权限路径。产品范围问题向集成窗口提交。OWNER-001 已核对。
