# Schema 19 浏览器工作流验收检查点

固定任务基线 `bd97dcf2adbe2a439aa2253ed4d118cc069cc850`。实际构建及运行源码 `f32a5911c475db2b38b423b02e31823f52c55dd6`，tree `ccaa32b3ec29cae12b901741492a5cd31c6c939b`。

本地 Linux ARM64 以同源候选 `6709f424e865706b8e8676af42384489e5d89ae502a87f4af74e7d1e910fb05b` 运行现行 workflow 的浏览器命令。前端 1,597 项及预览合同通过。完整浏览器入口退出 0（141.80 秒）。独立 Child Planning 与 Child Goal 三案也退出 0。

Child Planning CP01–CP09 通过。CP07 关闭 root 面板后实际父状态读取从 4 次推进到 5 次，历史读始终为 9 次，未完成读取为 0。原单次 CAS 准确返回 `STALE_REVISION/not_started`，全库指纹未变。CP08 的丢应答、同键恢复及最终原件均通过。

Integration normal 与 command-failure、12 张 Integration 和 4 张通知有限投影均通过。真实私有探针回执为 API 1、Store 19、自然退出 0 和成功回收，绑定原始候选、前端构建记录及两件工具摘要。四个改件对照由原 workflow 守卫拒绝。

设施回归运行 1,012 项，跳过 16 项。Node 4 项、Child Goal wire 14 项与 392 项驱动自检、七段故障注入、三份历史原字节逆向均通过。此前运行拒绝和设施注入失败原件的摘要见 [执行证据](EVIDENCE.json) 与 [早期执行证据](EARLIER_WORKFLOW_EVIDENCE.json)。

在集成目标 `af6d7e89576c367ada46f6135c384b94cf1984b2` 上的无冲突模拟 tree 为 `f177e7f1f9437c8dd28e984e088dfdaccc9c93b1`。运行证据绑定上述任务源码。合成源码的复核及 GitHub 托管检查属于集成准入。前置归档和原件保存在任务私有运行目录。

## 集成源重放

实际重建的 detached 合成提交 a96acc79060c3509157e14748b54ba7f7d1365b8 绑定目标 af6d7e89576c367ada46f6135c384b94cf1984b2 与原交付头 0c83ee6de8b158df52353c00c3e40dde6ab19578，tree 精确为 06be1ac098d62c6c5841c5579027e18bace8011b。两个工具、前端和完整浏览器链均以该源码重建并验证。浏览器入口退出 0（142.60 秒），Integration、通知投影以及四个改件拒绝均通过。本地环境、原件与哈希见 [合成树执行证据](INTEGRATED_REPLAY.json)。
