# AB-REL-001/002 原生 CI 时长与验证边界

两次 PR #33 的 macOS 作业均在 60 分钟上限之前以失败结束。两次都未执行默认应用审计、开发 DMG 打包或上传。

| 阶段 | 旧作业 135252128867 | 当前作业 135276914703 |
| --- | --- | --- |
| 作业总耗时 | 34:42 | 53:13 |
| 前端依赖与验证 | 2:03 | 1:35 |
| 构建真实 schema14 / 15 / 16 输入 | 1:13 / 1:08 / 1:00 | 0:48 / 0:37 / 0:35 |
| Core 与宿主步骤 | 28:45，Integration IPC 31/34 | 37:06，通过，Integration IPC 34/34 |
| 从 Core 步骤开始到同一 Integration IPC 目标结束 | 28:45.469 | 21:54.237 |
| 真实 schema14 / 15 / 16 验收 | 跳过 | 2:10 / 1:55 / 2:00，分别通过 7、6、11 项与严格 Clippy |
| 真实 schema18 迁移与恢复 | 跳过 | 4:55，通过 |
| 实际 WebView 和 daemon | 跳过 | 1:03，外层证据拒绝 |
| Owner Grant、Factory、Native 特性库测试、后台注册、Integration 审批 | 跳过 | 跳过 |
| 默认应用构建、发布审计、DMG | 跳过 | 跳过 |

旧作业只跑了 Core/宿主步骤的前半部分，直接比较该步骤的总时长会混入不同的执行范围。两次均为 Apple Silicon。新作业检出合并 SHA `eb2783beb2bee0cff165663b2d0da20f15d69e64`，树 `6a55867536d7900bafecc1467590edf5b4eff046`。实际 GitHub 日志与时间戳见证据索引。 [旧作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44432204086/job/135252128867) · [当前作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44439619004/job/135276914703)

## 编译与执行

- 旧作业的根 workspace debug bins 和 workspace test 编译分别为 3:15、6:04。新作业同一位置为 2:30、4:33。新作业从 workspace test 编译完成到最后一个 doctest 结束为 23:48.694，其中 Core 库 468 项用了 460.94 秒，Integration IPC 34 项用了 101.83 秒。两轮之间的时间差不用于归因。
- 新作业继续冷编译 release sidecar，耗时 2:14。嵌套 Tauri workspace 默认测试编译耗时 3:42，实际执行 189 项通过，2 项按原设置忽略。根 debug/test 和 Tauri 默认测试两处日志共有 60 个相同名称与版本的 crate 输出了 `Compiling`。两份锁中精确相同的 registry 名称、版本、来源与校验值有 80 条。crate 重合并不保证编译指纹相同。
- 两次后续 debug sidecar build 实测仅 0.22 秒与 0.40 秒。现有 warm 调用保留，节省这部分无法改变主要预算。Native WebView 特性测试编译 33.88 秒，37 项运行 14.45 秒，实际驱动编译 2.90 秒。特性测试与驱动二进制有不同用途。后续后台 `--lib background_service` 与同特性全库测试有静态范围重叠，本轮二者均跳过，没取得节省时长。

## 真实原生结果与外层校验

当前 WebView 进入时距 60 分钟上限按时间戳计算还有 7:59。驱动返回 0，原生原始报告为 `passed`，记录 Composer 提交、强制销毁窗口后的实际 daemon 存活、重启持久化、第二写入者拒绝和自有 daemon 回收。它明确记录正常关闭到后台再恢复为 `not_run`。外层 runner 无超时且确认进程组回收，因 Goal Execution 报表采用 schema19 而自身要求 schema18 拒绝了证据。源层结果和外层失败分别保留。未执行的审批、后台注册、默认发布审计和打包保持 `not_run`。 [原始托管产物](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44439619004/artifacts/14493545935)

## 最小待审加速

在同一作业的当前源码构建中复用 Cargo 中间产物。候选只在桌面作业第 9 步设置 `CARGO_BUILD_BUILD_DIR="$GITHUB_WORKSPACE/target"`，并在该步成功后将同一值写入 `GITHUB_ENV` 供后续当前源码构建使用。最终产物目录仍使用各自的 workspace 目标目录。离线 metadata 已确认嵌套最终路径仍为 `apps/desktop/src-tauri/target`。14/15/16 的历史构建发生在这之前，fresh18 脚本的显式环境白名单不传递该变量。应保持历史构建和 schema18 生成目录独立。 [Cargo 中间产物与最终输出](https://doc.rust-lang.org/cargo/reference/build-cache.html)

建议认领的精确路径为 `.github/workflows/product.yml` 与 `tests/infrastructure/test_schema17_migration_workflow.py`。前者只调整中间目录的作用域，后者在既有历史逆转入口前验证并剥离这段精确变更，保留整文件父 SHA 和缺失、重复、移动及权限变更的负控。现有测试命令、Runner 配额、检查名和权限、真实审批、生命周期、默认发布 profile、审计及打包均保持原有执行。无需跨作业恢复缓存或共享最终二进制。候选尚未执行，也没有验证能节省多少时间或足以完成完整作业。
