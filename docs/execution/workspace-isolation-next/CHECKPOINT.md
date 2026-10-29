# Checkpoint

- 单元：workspace-isolation-next；调查分支 `codex/workspace-isolation-next`，worktree `/workspace/.worktrees/workspace-isolation-next`。
- 产品固定基线：`8c178088d7213f625695a9d31a5b6d31cc342a42`，tree `b926b5318208fdb33d321173f026e65b83e04329`。本记录提交只含PLAN/PROGRESS/CHECKPOINT/EVIDENCE；确切docs提交由交付回复与 `git log -1`提供，未虚构自指SHA。
- 授权：AB-START-001，完整Designv0.2/130项不缩减，OWNER-001/002 ACK，未发现003。真实Provider身份/调用、额外费用、生产发布、签名与最终许可证仍关闭。
- 已做：需求逐条阅读；Git/Directory/import/reassociate/create/workspace-copy、Task/Session、协议route与Factory完整选定workspace路径源码追踪；历史真实CI相关组复核及五文件Git字节一致性核验；最小后续合同/路径/失败测试建议。
- 未做：产品编码、Rust/Node/type/build/tests、Browser/真实IPC/Mac、新工作区产品操作、用户项目Git变更、Host/Provider调用、新DDL/schema16、PR/CI/merge/push。工作树创建仅为本仓库独立docs分支，不是运行产品的隔离功能测试。
- 关键结果：现有隔离原语和身份重关联应复用；缺口在Task默认准备/绑定/授权/路由联通、并行任务分配及移动后引用验证，非Git不是从零开始。当前Factory新workspace权限false不可被调查或客户端变成true。
- 依赖：Root当前15写入/测试与共同合同所有权；冻结根任务preparation与Task版本关联后即可编码本地单元，不必等远程主机、真实账户或签名。任何新增持久字段先做旧document兼容分析，不先命名下一schema。
- 下次恢复：核Root最新已集成基线/OWNER_INBOX及unknown副作用；读取本PLAN的调用链和边界；明确Core/UI不重叠路径；执行WSI-1/2与真实正常/失败测试，独立review后再PR/CI。旧记录与失败不删，公开产物仍只有限摘要/已审材料。

本单元没有后台执行器、未安排定时续跑；完成调查不代表PRJ需求或完整产品完成。
