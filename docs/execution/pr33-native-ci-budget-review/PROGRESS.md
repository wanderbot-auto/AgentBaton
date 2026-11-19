# 进展

- 核对工作树与当前 PR head `ae844ca33406aabd62f44ea73818f9127f211748`。GitHub 已返回旧作业终态与当前作业实时时间戳。
- 旧 macOS 作业在 Core 进入 Integration IPC 后失败，Core/宿主步骤 28 分 45 秒，工作流跳过 WebView、原生审批和开发包。当前同名步骤实际运行 37 分 06 秒并通过，内含旧作业未执行到的编译与宿主步骤。
- 当前 schema14/15/16 已由真实迁移门验证。真实 schema18 于 02:40:12Z 通过，耗时 4 分 55 秒。WebView 进入时作业已用 52 分 01 秒，按 60 分钟配置计算余 7 分 59 秒。
- 初步识别独立源码边界。Rust Native WebView 生产者要求 schema19，外层 Python/Shell 仍固定接受 schema18。待当前终态验证实际触发位置。
- 离线 Cargo metadata 验证 `CARGO_BUILD_BUILD_DIR` 可与嵌套 Tauri 最终产物路径分开。两个锁文件有 80 条版本和校验值相同的 registry 记录。该计数属于可复用候选，尚无相同中间产物统计或加速实测。
- 当前作业在 53 分 13 秒终止，WebView 的原始驱动通过 37 项 fixture 测试后运行实际 App 与 daemon，驱动退出 0。四个独立当前只读证据域均为整数 19 且状态未变。外层拒绝 Goal Execution 的 schema19，后续审批、后台、发布审计和打包跳过。
- 新增范围内完成严格 Store19 修复和两份完整历史原文的逆转。精确原始 ZIP 和报告哈希已核对。旧 runner 与 Shell 回放均退出 1，改后均退出 0，接受的报告字节与托管原件完全一致。
- 格式语法与 53 项来源护栏通过。Linux arm64 38 项 runner 合同通过。第一次精简镜像缺 `strings` 和 `ps` 的失败已保留。macOS 36 项通过、1 项原负控失败、1 项 Linux-only 跳过；在精确基线复现同一回收断言失败。旧负控及进程回收源码未变。
- 已拆分比较 Core 公共执行前缀、冷/暖构建、真实迁移和原生终态。在两个现有 Guard 路径中提出同作业中间产物复用，保持现有检查、超时、权限、审批、打包与包审计。未实施或测量提速。
- 在前一交付 `e8060878` 复现历史构建器 `Changed immutable helper`。修复先读并校验当前 runner 与反演文件，再验证恢复字节符合原 HELPERS SHA。对直接提供旧文件、未知 runner、未知或不安全反演输入以及错误恢复结果的负控均通过。
- 隔离 Python 模式下 builder 与 generator 的 CLI 加载通过。schema18、schema19 定向分别通过 14 项、8 项。首次全量唯一失败为缺少 `jsonschema`。默认和官方镜像的 TLS 失败已保留，随后使用已安装且固定版本匹配的解释器，全量 1065 项中 1049 通过、16 跳过、零失败。
- Apple Silicon 原生 `fresh18_trusted_build_migrates_and_restores_before_accepting` 通过。实际重建固定 schema18 工具，经隔离 Python 生成真实历史，在 Store19 迁移并用旧工具恢复。14 项原拒绝检查通过，8 个原地封存对象再次验证。
