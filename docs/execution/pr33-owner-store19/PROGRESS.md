# 执行进展

- Hosted Linux Rust 作业 135276914618 在 Owner 段记录 47 项、3 个失败和 1 个错误。同一报错来自 pending Grant schema 检查。
- 用基准的普通 Cargo 工具在 Apple Silicon 重现四处错误。真实 API 1 和只读 SQLite 均为 Store19。Host 表存在且为空，旧观察仅计 39 张表。原生探针退出后自有 daemon 已回收。
- Grant 和 Factory 在同一个只读事务中核对精确 Store19、实际表类型和 40 张表计数。拒绝旧版、未来版、非整数版本、缺表、同名视图和 Host 行污染。原权限范围、事件序列、非委派约束和零副作用检查通过。
- 当前 runner 先反演到原 Integration Owner 输入，再执行原五文件反演和 Native Acceptance 历史反演。原固定值与补丁锚点逐字核对。
- 首次本地基准命令只构建 daemon，Factory 对照需要同源 companion。补齐后用完整工具对重跑并重现 hosted 四处错误。Python 3.14 试跑触发 SQLite 连接警告后，为变更的证据路径增加明确关闭，正式结果用 CI 同系列的 Python 3.12 重跑。
