# Checkpoint

- 单元：workspace-isolation-next；调查分支 `codex/workspace-isolation-next`，worktree `/workspace/.worktrees/workspace-isolation-next`。
- 产品固定基线：`8c178088d7213f625695a9d31a5b6d31cc342a42`，tree `b926b5318208fdb33d321173f026e65b83e04329`。调查docs提交eb0312f；WSI-1实现源提交`dedee373bd9f587de87ebe545ecaa88916a7d2b9`，仅三Rust文件。后续本记录提交只含四docs，确切docs SHA由交付回复与 `git log -1`提供，不虚构自指SHA。
- 授权：AB-START-001，完整Designv0.2/130项不缩减，OWNER-001/002 ACK，未发现003。真实Provider身份/调用、额外费用、生产发布、签名与最终许可证仍关闭。
- 已做：原需求调查与历史CI核验；Root确认合同后实现只读Rust resolver、DTO和9个专属真实Store/Git/Directory测试函数源；选择性rustfmt applied/check、源码路径/false flags/read markers核对。此为作者自查，不冒独立审查。
- 未做：Rust编译/Clippy/测试、Node/type/build、Browser/真实IPC/Mac、产品新工作区操作、用户项目Git变更、Host/Provider调用、新DDL/schema16、PR/CI/merge/push。9测试的临时Git/SQLite代码未执行；该功能还不能计为verified。工作树创建仅为本仓库隔离开发分支，不是产品隔离验收。
- 关键结果：现有隔离原语和身份重关联应复用；缺口在Task默认准备/绑定/授权/路由联通、并行任务分配及移动后引用验证，非Git不是从零开始。当前Factory新workspace权限false不可被调查或客户端变成true。
- 依赖：WSI-1源冻结待Root/Native独审及Root独占Cargo真实运行；不必等远程主机、账户或签名。WSI-2根任务默认准备与持久关联尚未实现；任何新增持久字段先做旧document兼容分析，不先命名下一schema。
- 下次恢复：核Root基线/OWNER_INBOX及unknown副作用；先读源dedee373的独审finding，不抢Cargo；Root执行`workspace_resolution::tests`及适当Clippy后据真实结果修复。再分配WSI-2/UI路径与完整默认流程，不将内部resolver完成当需求完成。旧记录与失败不删，公开产物只有限摘要/已审材料。

本单元没有后台执行器、未安排定时续跑；完成调查不代表PRJ需求或完整产品完成。
