# AB-DOM-006 · 交接检查点

- 分支为 collab/requirements-coverage。固定基线为 f3ee057c874582427e77129e21014574c57484ae，已验证代码头为 79c2a892d94189f09a3f46c04e4c3d0a6d43c3d5。终点在该代码头之后的本任务证据提交。
- 集成分支在验证时已推进至 46e38016256dfe3ff2646eb546cad5b8179e27ef。固定开发基线、该远端头和 leader 输入提交 7f371feb5d063c018af8f65ae974a726adb9b826 与代码头的合并演算均通过。
- 交接给 leader-control，负责独立复核、需求台账和实际集成目标的验收。开发窗口已自审；独立复核、远端 CI 和实际合并候选验证待 leader 执行。当前验证范围是报告 CLI 和本地基础设施，Provider、原生桌面产品及远程 companion 运行验证未执行。
- 默认读取脚本所在 checkout 的规格与台账。对标准路径的其他 worktree 台账，引用根目录自动取该 worktree。用临时台账副本时可通过 --evidence-root 明确引用根目录。

## 重跑报告

~~~sh
scripts/collab.sh run -- python3 scripts/report-requirements-coverage.py --format json
scripts/collab.sh run -- python3 scripts/report-requirements-coverage.py --format markdown
leader_root=$(cd ../leader-control && pwd -P)
scripts/collab.sh run -- python3 scripts/report-requirements-coverage.py --format json --ledger "$leader_root/docs/execution/requirements-progress.json"
scripts/collab.sh run -- python3 scripts/report-requirements-coverage.py --format markdown --ledger "$leader_root/docs/execution/requirements-progress.json"
~~~

历史台账用 leader 提交 520aa9dd926d80330168331d2cdaedb80cf9ce07 中的 docs/execution/requirements-progress.json 字节写入任务 runtime，再以 --ledger 指定该文件，以 --evidence-root 指定实际 leader 工作树。详见 [EVIDENCE.json](EVIDENCE.json)。
