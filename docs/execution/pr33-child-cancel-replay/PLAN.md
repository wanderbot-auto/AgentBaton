# PR #33 Child Goal cancel 排查

基准 `b3e274802672d5173c2a06e5badfe3bdafc19b9e`。首先用独立 Linux ARM64 容器从托管 merge checkout 编译 Core 和前端，运行原 normal、lost-start 和 cancel，以相同的有限投影器核对原证据。只在私有调度探针内移动已绑定伴随进程原有 wait 的时点，让采样打开真实 /proc 文件后才发生自然收割。对照原收集器、最小错误分类和权限失败。

保持原驱动的真实 Start、节点等待、显式 cancel、丢回执和显式同键恢复。原期限、保护、CI 留在既有入口。集成窗口后续认领四个 fixture 或源码守卫路径后，再提交精确修复和完整验证。
