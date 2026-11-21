# 复现检查点

托管错误形状由真实 Linux Core/Chromium 及自然退出的已绑定夹具进程重现。hold 期间的进程是原绑定 PID/PGID/birth/UID 的 zombie。原父进程正常 wait 后，原打开的 stat FD 读取失败，且捕获点与托管一致。原驱动通过修复方向的分类对照完成真实取消。托管具体调度未保留。 [重放说明](REPLAY.md)

最终源码 `150b5b59b805ea0ff3ad556d1c5a437b67e6aad0` 通过两次真实强制竞争和原三案。权限负控保留，原控制器 reaper 与历史源码逆转全绿。真实 Child Planning、Factory cancel 及基础设施也通过。本地失败证据、源码和工作树交给集成窗口。 [候选验收](CANDIDATE.md)
