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

## 恢复后最新检查点：F1已修，Rust待运行

- 当前源leaf：`7a04d67a8a12bb3cc74088470560679346230ddc`；parent `e87fa8cd57a3d42f8a1918d394f1e2578c8b5c13`。原dedee/e87与独审BLOCKED保留。当前四docs另commit，准确docs head由交付回复给出。
- 仅resolver/tests两源文件修改：同Deferred snapshot inspect前核关系root/device/inode，拒绝同ID/revision document-only重定向和三pin漂移；无DB修复/FS写/授权改变。新test1fn/4case，旧9fn保持，总10fn。
- 独审新revision source-ready：`.onboarding/workspace-isolation-wsi1-independent-review/revision-7a04d67a`；REVIEW `3606846a596cc831c55fe7825b604e8e120caa52a098c21563612af0def7c73d`；MANIFEST `d352bafe651642bdb2d2f7736b3f8328994f9eb3658590f0d1883f2fe03bef46`。仅源审与16有限SQLite控，不是Rust测试。
- NOT_COMPILED/NOT_RUN：10个Rust函数、Clippy、真实IPC/UI/Mac。Root可受控运行workspace_resolution::tests；只用测试TempDir Git/SQLite，无bins/Native/真实用户Git操作，作者不占Cargo/共享target。若Root得到真实结果，另记准确集成SHA/命令，不能回写旧dedee为通过。
- 恢复实际git status确认clean且已有7a04，不重复实现/重建工作树；本轮仅补四记录。下一依赖仍WSI-2/3默认隔离接线，完整130项范围/27决定与权限限制不变。

## Root实际运行后检查点

- 已验证head：`5c70c314d22f55396c6d5905a89742b0f54bec50`（源仍7a04）；Root targeted lib 10PASS/0skip，exit0，test3.54s，wrapper25.261956s；log SHA `0bd8fe867596acde75ea463cd772dec8a7288f86aa44534cdca3f6e473e831af`。
- 原件：`/workspace/.onboarding/workspace-isolation-wsi1-targeted1/{RESULTS.json,command-0.log,SOURCE_BEFORE.json,SOURCE_AFTER.json,TOOLS_BEFORE_AFTER.json}`。作者只读复核source清单前后全同与四工具SHA前后全同；没有运行bins/Native/用户Git或产品。
- 源7a04和四字段内部合同不变；本次仅追加四docs。历史NOT_RUN/旧BLOCKED不抹去。严格Clippy、完整回归、真实IPC/TS/Native与Mac仍未运行；WSI-2/3/default task行为尚未完成。下一步先向Root给出一个固定只读IPC与FS-u64十进制字符串wire建议，待其确认后另leaf实现。
