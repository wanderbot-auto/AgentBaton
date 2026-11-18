# Provider 认证状态第一阶段证据

## 固定上游行为

审阅 OpenAI Codex 提交 970111f6cb62cc02fe36a82ea7d71b37a2889c97。该源码的 Cargo 工作区版本为 0.0.0，CLI 包名为 codex-cli。测试用该开发版本标签验证版本门槛，随后仍核对选定可执行文件的 SHA-256。发行物尚需可信证据绑定到固定源码和实际可执行文件。CLI 入口的校验 SHA-256 为 ac51c153916132c0be2c37ea1136562685b832957bc2e43e53d71608d0b28a4a，和仓库现有协议证据中的固定文件一致。 [固定工作区清单](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/Cargo.toml) · [CLI 包清单](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/Cargo.toml) · [CLI 入口](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/src/main.rs)

- CLI 的固定 argv 为 **login status**。启动会读取 CODEX_HOME 的环境文件，并按宿主条件尝试维护辅助目录。命令会加载配置并按当前凭据模式读状态。保存的 PAT 可调用 whoami，工作负载身份和访问令牌路径也需要独立核实访问范围。因此未开放默认执行。 [CLI 启动](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/arg0/src/lib.rs#L134) · [状态逻辑](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/src/login.rs#L451) · [认证加载](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/login/src/auth/manager.rs#L1526) · [PAT 请求](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/login/src/auth/personal_access_token.rs#L40)
- 仅匹配空 stdout、固定 stderr 和相应退出码。缓存 ChatGPT 与掩码 API key 对应退出码 0，本地无登录对应退出码 1。API key 的脱敏片段及提示语只参与匹配，证据保留枚举、时间、摘要、字节数和直接子进程退出及回收结果。当前身份验证始终为 unknown，投影无法判定登录过期。 [CLI 状态和退出码](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/src/login.rs#L451) · [掩码实现](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/src/login.rs#L617)
- App Server 文档的 account/read 加 refreshToken=false 禁用请求显式强制刷新。固定实现对保存的 ChatGPT 工作区在启动时发现路由，读取可能重试，并在 accounts/check 返回 401 时触发认证恢复。已授权的运行会话也是单独待验证的场景。旧 GetAuthStatus 即使关闭显式刷新，仍可能调用可主动刷新的 auth()。内部持久化读取代码依赖凭据格式和后端，未发现符合当前权限的独立公开只读接口。 [公开账户接口](https://developers.openai.com/codex/app-server) · [固定路由协议](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/README.md#selected-workspace-routing) · [路由与恢复](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/src/request_processors/account_processor/workspace_routing.rs#L385) · [旧状态读取](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/src/request_processors/account_processor.rs#L1076) · [主动刷新](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/login/src/auth/manager.rs#L2476) · [内部存储](https://github.com/openai/codex/blob/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/login/src/auth/storage.rs)

## 夹具范围

- 公开 API 拒绝缺少验证上下文、错误路由、错误固定提交、不同摘要、未审阅版本和未回收的版本探测。用生成的可执行文件和落盘哨兵证明未启动。生产构建中的验证上下文没有创建或反序列化入口。
- 私有测试夹具用生成的可执行脚本验证 argv、空 stdin、最小环境和有效工作目录。未知状态负控涵盖退出错误、信号、乱码、额外 stderr、错误输出通道、未掩码值、超量 stdout/stderr、超时和执行前后摘要漂移。直接子进程用 signal 0 确认已消失，证据序列化、Debug 和错误文本均不含相应哨兵。
- 测试仅运行生成程序及仓库现有夹具。环境清理和受控目录是启动参数，实际 OS 隔离能力仍待验证。生产身份、凭据过期及 Core 诊断接线属后续验收。

## 验收环境与结果

2026-11-19，Apple Silicon，arm64 Darwin 25.6.0，macOS 26.6.1 (25G76)，Rust 1.91.1 (aarch64-apple-darwin)。实际测试代码为 8eb02900d9f4ef9a22dd45031e638c997761aa2e，树 829bf2af978f073d623a2ea9b37d586174c2aa83。验收会话使用本机 /usr/bin/python3 3.9.6、RUST_TEST_THREADS=1、CARGO_NET_OFFLINE=true，通过 scripts/collab.sh run 执行。

| 命令 | 结果 |
| --- | --- |
| cargo test --locked -p agentbaton-providers | 102 通过，0 失败。现有安装测试 10 通过，新增认证测试 6 通过。 |
| cargo clippy --locked -p agentbaton-providers --all-targets -- -D warnings | 退出 0。 |
| cargo fmt --all -- --check | 退出 0。 |
| git diff --check | 退出 0。 |

测试命令日志为任务运行目录中的 accept-8eb02900.log，SHA-256 a005d95493f2f8a3804833f3196bb3388b4a59adfdf28a1807a03c45576292fe。合并范围以远端功能目标 a6a9ba9b4cb11e9531e9b0b9f7f39ef9be9cb505 为基线。代码范围检查及向该准确提交的预演通过。最终文档提交仅增加本目录。

### 环境反例

初跑使用默认 Python 3.14.0a7 并行执行。六个新增测试通过，原有安装套件中 3 通过、7 失败，观察包括意外 TimedOut、错误的退出分类，以及 500ms 内未生成 PID 哨兵。单独从准确基线导出源码并在独立 Cargo target 重编，同环境下原有安装套件 6 通过、4 失败。基线失败覆盖版本失败分类、小管道排空、执行中变化与环境输出。改用 Homebrew 3.15.2 但继续并行时仍有原有安装测试失败。

使用系统 Python 串行时，独立基线的 10 个原有安装测试全部通过，两次运行候选的 Provider 全套共各 102 个测试通过。测试内容及原有 500ms 期限保持原样。当前证据复现了失败并提供可通过的验收环境，尚未定位并行超时的根因。

| 任务运行目录中的日志 | SHA-256 |
| --- | --- |
| providers-first.log | 1191eb78b03770eb94d78c288e82793fe50abbad1c47ceab87020cc3121574f3 |
| version-baseline-separate-target.log | 6ef65415a58dc2754d8082a4af2376c5a382e50bf8c1084f5c52d0821dd15f9c |
| providers-homebrew.log | 0e2048940b4cdd7ee98b8c1772677b89cd4fd60df394bcdffed5c31a6a56fd50 |
