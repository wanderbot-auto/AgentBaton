# Integration 浏览器投影交接

固定基线为 6feaa39ae60ed4094ff9220fc5edb79c895eb627。已验证实施提交为 2f983e0166345c9957df4ae162878d1f1aeb563b。后续提交只添加任务记录。Leader 负责独立复核、共享上游和实际集成候选。本机现行两案与合成投影合同已验证，Hosted Linux 尚待实际运行。

## 上游探针记录

私有 schema-probe.json 只允许下列八个键。原 manifest、build record、探针与场景报告均由现有路径和私有文件守卫读取。

| 键 | 严格值及来源 |
| --- | --- |
| source | 同一准确源码的 40 位小写 Git SHA，须和阶段状态、候选、构建及场景报告一致 |
| api_version | 实际私有 system.snapshot 的整数 1 |
| database_schema_version | 同一次实际私有 system.snapshot 的整数 19 |
| daemon_exit | 对该探针进程真实等待返回的整数 0 |
| joined | 真实等待并回收后记录 JSON 布尔 true |
| manifest_sha256 | 投影目录内同份 candidate.json 原始字节的 SHA-256，64 位小写 |
| build_record_sha256 | 投影目录内同份 frontend-build-record.json 原始字节的 SHA-256，64 位小写 |
| tools_sha256 | 只含 batond 与 baton 的 JSON 对象，每个值取该次实际使用的私有二进制字节的 64 位小写 SHA-256 |

上游在启动前核对私有工具归属和实际字节，使用候选对应的 batond 进程，正常停止后核对回收与摘要并写出探针。Probe 失败、强杀或真实等待结果非零时，阶段状态保持未通过。两份原件按原始字节校验，JSON 重新序列化会改变摘要。投影同时比较候选工具摘要、探针工具摘要与两个实际报告的工具归属。旧探针格式及 schema 18 保留在历史回放。

## 复核与重跑

在任务工作树运行定向投影验证。

~~~sh
scripts/collab.sh run -- python3 -m unittest tests.infrastructure.test_host19_browser_ci_projector_source_fixture tests.infrastructure.test_integration_browser_ci_evidence tests.infrastructure.test_integration_browser_ci_evidence_runtime -v
~~~

应用构建使用锁定 release 侧车与真实前端构建命令。本机重跑脚本、原始日志、报告、私有 transcript、截图和合成投影件在该任务 runtime 内保存，路径及摘要见 [EVIDENCE.json](EVIDENCE.json)。重跑脚本从准确实施提交的工作树外层启动，脚本本身调用 collab run。原始测试数据留在私有 runtime，交接文件记录有限摘要与哈希。

标准两案使用生成 ACP，第一案确认本地引用，第二案保留预期的命令退出 7 和零次 CAS。Linux 投影器对真实 macOS 报告返回失败摘要。可发布的 Linux 投影当时仅通过合成正反例；共享 schema 19 生产端和通知扩展需在实际 Linux 候选上统一验证。

## Leader 独审与交接

Leader 接受 76d7aaf442f921c235d39f3106c5b7bb8af51639 的浏览器 schema 19 driver 与投影阶段。独立复跑 66 项，核对 20 条证据索引、两份 ARM64 工具和八键探针与原件的配对。独审核实正常与命令失败场景分别有 12 和 10 次真实 schema 19 快照，合计 12 张原图，并确认两份历史源文件均逐字节还原。

实际托管 Linux CI 仍待验收。按交接流程释放任务认领，保留原分支、工作树和证据。集成由 leader 负责。
