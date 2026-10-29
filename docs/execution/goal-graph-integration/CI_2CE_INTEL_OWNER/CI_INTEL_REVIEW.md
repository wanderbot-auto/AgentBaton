# 2ce Intel 失败原件独立审查

封存 2026-10-29T04:32:47.144978+00:00。feature `2ceaa608d87b20e20e72068e4a143ab8e4126e48`，实际 checkout `a5814094549314187c46ffbe05601fd3945a0c72`，tree `cf8692c159a635d0d147d170c3c62f3e90f65d21`，Job 123254765347 于 2026-10-29T04:23:49Z FAILURE。原日志 checkout 与 API parents/tree、GRAPH_BUILD_SOURCES 的 sessions_ipc.rs SHA 相互核对；这不是旧 a220/7f83 的 native 结果。

失败发生在 Core and native host tests。原日志1983行 exact Owner proposal persisted、1984行 private Owner decision committed；1985行 approvals.finalize 返回 PERMISSION_DENIED，原文 `Execution grant or an ancestor is revoked or expired`，side_effect_state=not_started。1987行测试 actual_graph_owner_decision_unlocks_dependent_summary_with_exact_candidate_input 在 crates/core/tests/sessions_ipc.rs:65:5 panic。1995行 54 passed / 1 failed / 0 ignored，572.72s；1997行 exit101。INTEL_FAILURE.json 保存精确原行、时间和源哈希。这里只确认原错误，不推断为何撤销/过期、时钟或预算因果，不提出放宽权限/超时。

后续 Actual macOS WebView、Native Owner、默认 build、release audit、package 和开发包上传均 skipped。原 native artifact upload 动作本身 success 不代表有文件；terminal artifact API 无 Intel native/development ZIP，仅见 ARM 与其他 Linux 产物。因此本轮 Intel readonly/Owner/WebCrypto/19表0/DB permit/截图/开发包签名均未运行或未产出，不能借旧运行通过。没有 Intel PNG 可看；本轮 Intel 目视和 hash 图片数都是0。

| 实际 step | Conclusion |
| --- | --- |
| Set up job | success |
| Run actions/checkout@11d5960a326750d5838078e36cf38b85af677262 | success |
| Run actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 | success |
| Toolchain | success |
| Frontend dependencies and validation | success |
| Core and native host tests | failure |
| Actual macOS WebView and daemon lifecycle acceptance | skipped |
| Actual native Owner Grant dialog and private database acceptance | skipped |
| Run actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 | success |
| Build desktop UI and application | skipped |
| Audit the actual default release application for test controls | skipped |
| Package development application and evidence | skipped |
| Run actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 | skipped |
| Post Run actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 | skipped |
| Post Run actions/checkout@11d5960a326750d5838078e36cf38b85af677262 | success |
| Complete job | success |

原 Intel job log 仅 GET 一次，SHA `1be4907c5cef11be606e6354ae950610091a391eda77b5785b45bef9e81c5c72`。没有可下载的 Intel native/development ZIP；没有重复获取旧包。未本地 Cargo/GUI/Provider 或重现产品流程，未改源码/STATUS/权限/超时，未触发重试取消CI或任何远端写/正式Approve。真实 Core 修复另由 Root 依原件分配。
