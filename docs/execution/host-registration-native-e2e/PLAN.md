# Host 登记的 Apple Silicon 原生验收

固定产品源码和候选目标为 `cde501db847d1bbfc27a298e16cd772afc1a41e9` (`origin/collab/host-registration-integration`)。本任务只写原生验收驱动、驱动测试和本目录的执行证据。

1. 在任务工作树锁定构建依赖，打包 ARM64 release App 和生产侧车。已有用户 App 正在使用，其 bundle ID 对应现有 WebKit 容器。以单项 bundle identifier overlay 生成隔离开发包，核对两个 Info.plist 和各二进制哈希。
2. 在同一个 `collab.sh run --native` 独占会话内创建私有短 runtime，启动真实 daemon 和 App，使用现有 macOS 授权操作指定 App PID 的设置窗口。登记虚构 `.invalid` Host，分别检查初始列表、回执、显式确认、设置重开、App 重开和只读 SQLite。回收本次启动的全部直接子进程。
3. 在独立私有 runtime 用生产 `batond` 验证真实 Unix socket 响应丢失。等待独立只读 SQLite 证明提交后关闭未读连接，再使用原请求键查询、重放，并在重启 daemon 后复查回执与唯一性。
4. 保存原始日志与小结，核对冻结的测试驱动、工作范围和目标合并预览，提交给 leader 独审。既有 App、默认 runtime、用户权限、登录服务、产品 RPC 与上游发布流程不在写范围。
