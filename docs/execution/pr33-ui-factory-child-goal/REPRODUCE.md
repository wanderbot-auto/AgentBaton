# 复查原托管红灯与本地复跑

托管 Product / UI 作业 135276914307，merge eb2783beb2bee0cff165663b2d0da20f15d69e64。保留 ZIP 只在 11,818,643 字节且其 SHA-256 等于托管上传记录时读取。旧绿工件同样先核对固定摘要。原始入口、版本、摘要、各分支证据和私有日志位置见 [EVIDENCE.json](EVIDENCE.json)。

按原入口运行同源的真实 Linux Core、前端和已固定的 Chromium。生成式 ACP 与真实 IPC 限定在无网络容器，私人报告保存在任务路径。ARM64 Docker 复跑的结果仅用于本地对照。托管复验需要同一 PR 合成提交和托管 Linux x64 Runner。

~~~sh
python apps/desktop/tests/factory-child-goal-real-ipc.py --run --case CASE \
  --tools PRIVATE_TOOLS --frontend apps/desktop/dist --manifest PAIRED_MANIFEST \
  --chromium PINNED_CHROMIUM --output PRIVATE_CASE_DIRECTORY
python scripts/record-child-goal-ci-evidence.py --case CASE \
  --manifest PAIRED_MANIFEST --private-dir PRIVATE_CASE_DIRECTORY \
  --exit-code ORIGINAL_DRIVER_EXIT --output PUBLIC_SUMMARY_DIRECTORY
~~~

目标源码变动后先从该提交重新运行工作流中的 Core、前端和 freeze 步骤。使用新私有输出目录以及原始驱动返回值。测试诊断不重试点击、前置读取或写入。既有 5 秒 renderer readiness、15 秒回执、单写和两层 CAS 仍由原代码检查。只有失败的 Start 窗口发布 v6，成功继续发布 v4。

v6 的 zero 表示在这个有限区间内，测试 HTTP 桥未解码到绑定 scope/task 的 Start。one 加 unobserved 表示桥收到一条请求，仍未观察到 Core 回复。未观察到回复时写是否送达及是否持久化仍为未知。多个请求或采样截断不选择某一条回执。Context 拒绝只作时间关联。
