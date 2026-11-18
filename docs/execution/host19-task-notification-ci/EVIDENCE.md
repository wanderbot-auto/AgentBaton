# Task Notification schema 19 验证证据

验证源码 `aa9de9a96621cdabfad4097572365ae0693fa682`，源码树 `3e9c3566e51d169235dbdd73c2f49cf3f42a3803`。固定起点 `f4b1e5217085fe20c9122b3fae12bff535d24930`。环境为 macOS 26.6.1 ARM64、Chromium 149.0.7827.55、Playwright 1.61.0 和浏览器 Python 3.13.2。独立基础设施环境为 Python 3.12.12。

## 源码与对照

- 基线定向测试 13 项，9 通过、3 断言失败、1 异常。失败名称与 leader 独立复现一致。保留的 `initial-driver.log` SHA-256 为 `2989284b3b378b0307c7a043ccab2755a20b9109756cab4b4744581b8833fba2`。
- 现行探针 SHA-256 `d639c1d83c3c5c604fe8a13bbf2ca6dae9cb309d5573534686758ac3209d929a`。命令失败路径隔离、权限边界、15 秒固定时限和清理仍由既有 driver 及探针检查。Integration driver SHA-256 `f730621d77cb75f94e214794615b1125ba0f33cc454431a3432a8b43e8ddd0bb`。
- 定向 15 项通过，覆盖假来源、旧版、新版、非整型 schema 和不足、重复、乱序的日志。定向有限投影与历史 preflight 32 项通过。现行 driver 通过现有固定完整字节逆向恢复，原有十项权限 AST 固定值和原 run_case AST 固定值均通过。
- `cargo build --release --offline --locked --bin baton --bin batond` 和 `npm --prefix apps/desktop run build` 在同一干净源码上成功。ARM64 `batond` SHA-256 `b55d14b69beb3d906dccf75760e1983d4d6e1252ef18f20b2c137ca8c99f8ad4`，ARM64 `baton` SHA-256 `f7a523845043c5743d4b612163bb28cc74d39a2d7e8c5c3e55a61920b9eedc80`。候选清单与前端编译记录 SHA-256 分别为 `3953cadfc5d73edaa467ef08517646e96f9ef715507c52774ea5d77194f21717` 和 `f6e79eff580747e9d9e1cc220cdcda0fe275ce7f2146193c0c665634d4d9c4ad`。

## 实际回放

用只读 `system.snapshot` 及只读 SQLite `PRAGMA user_version` 独立验证候选 Core 的 schema 均为整数 19。候选 daemon 退出 0 后已 join，私有 socket 与 runtime 已移除。回执 SHA-256 `dac2060db7d46317f4cd2b9803bc02a866bd85b6f8fb99c02f7c7d00cef7a922`。

两次运行 `integration-real-ipc.py --run --case all --notification-probe`，两案均通过且 driver 退出 0。下表为第二次的结果。

| 路径 | 实际结果 | 报告 SHA-256 |
|---|---|---|
| 正常 Integration | 5 检查，命令退出 0，确认本地 ref，6 张原 PNG | `a8fc3d0939852185f6211bbdce75919c1ec277994c4e560d23435767246a249e` |
| 命令失败 | 4 检查，命令退出 7，零 CAS，6 张原 PNG，无通知探针目录 | `d584a543d3ac2ec00955fa0186b334b3f75343835af79c86f2b62ff35b5772ff` |
| 正常路径通知 | 6 检查，1 条任务通知，12 条实际映射事件，按显示顺序核对首 10 条 | `70000efc5429e795b6cac546f10e0c836e8ddbcb7b947a0d43472c913feca3ca` |

重新从私有 Core 转录定位相符的 snapshot、sessions 与 journal 回复。快照中只有该已验收任务，`task.accepted` 来自 `baton_goal_acceptance`，`session.stopped` 来自 `baton_supervisor`。核对通知显示的序列、来源和元数据，原始历史链接定位到真实验收条目且 payload 一致。验证同任务导航、四个快捷键的模态隔离、刷新后免打扰状态和手动查看、390 像素移动页面无水平溢出。bridge 与私有审计记录均仅包含已有有界读取。两案 daemon 退出 0 并 join，HTTP 线程已 join，Chromium 已关闭，候选工具、前端资产和源码未变，私有运行夹具已移除。

| 通知原图 | 像素 | 原始字节数 | SHA-256 |
|---|---:|---:|---|
| accepted-task-desktop.png | 1440×1100 | 136809 | `b43998b9cc40f2619ed60f9007c86a2123631b9ee47c03344924dcf8672ffcd9` |
| real-source-desktop.png | 1440×1100 | 152989 | `40c70be5b5b3ba2fc105f588eabe8f4e0b422daa76aabaeeb23fd6ba165e6a94` |
| original-journal-desktop.png | 1440×1100 | 190468 | `15c69a2bb12f2a8736d30481029127f39ed2e09535eab3b5dc77f1e8d65a7f61` |
| dnd-mobile.png | 390×844 | 49677 | `f92485da471ca8a709626123e198e7dd447e7717e51af673101e2ab3630ab8ee` |

两次均验证 16 张原图的 PNG 结构、尺寸和摘要，通知焦点框在相应视窗内。首轮另人工查看四张对应截图，确认任务条目、展开来源、原日志和移动免打扰状态可辨认。定位观察与截图未做像素同步。首轮的私有汇总误将通知 observation 写入独立 schema 证据字段。第二轮更正私有回放工具并重新生成完整证据，首轮原报告、schema 回执和截图保留。

本机第二轮有限核验记录 SHA-256 `146f6afc6b28bd4ce753d582df19697c4be6b7e562d3110a7095e3062439dcce`。证据目录为任务私有 runtime 中的 `notification-local.1BzFUc`，原始转录、截图和 fixture 详情保留在本机。Linux 有限投影在完整的合成输入上通过，真实本机输入因平台维度被拒绝，退出 1，仅写出 `summary.json`，SHA-256 `4b8c0ed943b9d454977f21fe5b496fdde6a1ee455d0e12e4f989a38b940c72d0`。托管 Linux 检查、原生 macOS 通知和后台系统通知未运行。回放使用生成 ACP、本机私有 Core 和 fixture，无真实 Provider 请求或用户数据。

## 全量本机检查

| 命令 | 结果 |
|---|---|
| `python -m unittest discover -s apps/desktop/tests -p test_task_notifications_real_ipc_probe.py -v` | 15 通过 |
| `python -m unittest tests.infrastructure.test_task_notification_ci tests.infrastructure.test_integration_driver_preflight_diagnostics -v` | 32 通过 |
| `python -m unittest discover -s tests/infrastructure -p 'test_*.py' -v` | 996 项，980 通过、16 跳过，退出 0 |
| `npm test` | 4 通过 |
| `npm --prefix apps/desktop test` | 112 文件、1580 测试通过 |
| `verify_bootstrap_manifest.py`、`validate_design.py`、`validate_infrastructure.py`、`check_gate.py --scope preparation`、`doctor.py --strict` | 均退出 0 |
| 锁定的 `tools/environment-smoke` cargo test 与全目标严格 Clippy | 2 测试通过，Clippy 退出 0。用固定源码归档到独立目录运行，逐个 `cmp` 验证三个文件。 |

全量 Python 日志 SHA-256 `293e8b8e063cad4c55517ec339af2153bee8a66e13d2485e1982139834b792d2`。桌面 Vitest 日志 SHA-256 `c066c01cf6dd813a76ac4bedb726024844a294c97b27631c254712a57acd7c98`。
