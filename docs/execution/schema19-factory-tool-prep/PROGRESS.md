# Schema 19 · 准备器进度

- 基准 `375a054e69d3d411f32266c41b03ba2f6c682460` 在 Apple Silicon 实跑普通 workspace 预构建。原设施 18 项全过，真正 prepare 退出 1，RPC 已返回 API 1、schema 19 和零序列。原 daemon 经 SIGTERM 退出 0，准备器因旧 18 守卫拒绝发布清单。原组日志与合成夹具的绿结果均保留。
- 源码 `02fd3cba1218b7f7882015f5fa4af556a9ac35a3` 令现行默认工具准备走固定 19。只读库校验 16 张历史表、新增 Host 表及空 tasks/events，RPC 平行核对版本和序列。发布清单需要已确认同一 daemon 的正常停止。verify 需要完整 19 回执、精确源码、两套一致的前后摘要和两份物理工具身份。历史 18 只由明确的历史夹具输入路径调用。
- 设施定向增加七项并保持原 18 项。实际数据库替换、同字节预构建／冻结文件换位、CLI 与 SQLite 不符、第二次 wait 超时都有拒绝证据。Apple Silicon 当前真工具 prepare 和 verify 均返回 0，两份退出输出字节相同。Linux ARM64 隔离运行及 Mac workspace 下游实测在进行。
- Linux ARM64 以同一精确源码和同一默认 workspace 命令完成真实 prepare/verify，均返回 0 且输出逐字相同。25 项定向通过。leader-control 独立核对了两平台原件，并另行复跑准备器、校验器与 25 项定向。
- Apple Silicon 使用已冻结工具与规定的三个环境变量实际执行 workspace。Core 完成 467 项，旧 Task Workspace 迁移测试仍在 schema 19 断言为 18。Cargo 自然退出 101，随后手动 verify 仍返回 0，冻结收据逐字一致。自然失败阶段由集成窗口另行处理。
- 系统 Python 全设施首轮因独立 design-discovery 用例缺少 jsonschema 失败。保留该环境失败并复用仓库虚拟环境重新验证。
- 以仓库 Python 3.12.12 虚拟环境复跑设施全组，共 1010 项，994 通过、16 跳过。设计校验同时通过。
