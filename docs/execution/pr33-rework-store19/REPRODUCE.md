# Store 19 真实保留快照复验

素材是其他窗口在本机保留的既有真实生成夹具。保持原权限和绝对路径。收集器按原流程核验其拥有的主文件，再在私有副本上以只读、immutable 和 query_only 投影。托管工件只提供公开红色记录。

~~~sh
scripts/collab.sh run -- bash -euo pipefail -s <<'SH'
retained=/Users/wander/Documents/code/agentBaton-next/artifacts/collab-worktrees/schema19-default-early-migrations/artifacts/factory-child-goal-core
python3 scripts/record-factory-rework-ci-evidence.py --evidence-root "$retained"
python3 -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
SH
~~~

在基线 c7dffe2c 上，同一个入口退出 1，首个拒绝点为 project_db 的 Store 版本匹配。修复候选退出 0，公开记录声明 Store 19 与 not_a_product_or_test_pass=true。原真快照、路径和进程元数据只用于本机验收。提交证据仅含有限投影。

源码反演先验证 Store 19 的精确整文件摘要，只恢复批准的一行后继续校验 Schema18 和更早层的所有源摘要。错误版本、未知增量、移动或重复源码会被相关控制拒绝。
