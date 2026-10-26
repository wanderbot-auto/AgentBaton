# AgentBaton 环境准备清单与实测结果

**状态：GitHub 仓库和三平台基础设施 CI 已验证；Codex Cloud 环境尚未创建/发布。产品开发未授权。**

仓库：`wanderbot-auto/AgentBaton`（私有，`main`）。
已测试基线：`02508d4eb1c32d35cc9deda546f334938abdd357`。

## 已完成

| 项目 | 结果 | 证据与范围 |
|---|---|---|
| 远端仓库初始化与写入 | 已完成 | 完整 Design v0.2、27题决定、130项需求及准备脚本已提交 |
| 上传完整性 | 已验证 | 80个文件的本地与远端 Git tree 一致；CI 校验 manifest |
| Linux 环境预检 | 通过 | Ubuntu 24.04，job `121619083247` |
| macOS Apple Silicon 环境预检 | 通过 | macos-15 / arm64，job `121619083685` |
| macOS Intel 环境预检 | 通过 | macos-15-intel / x86_64，job `121619083747` |
| Python / Node / Rust | 通过 | 每平台11项Python、4项Node、2项Rust fixture测试及clippy |
| 设计与Schema | 通过 | 130项需求、7组正例、10个负例；仍全部是未实现的产品需求 |
| macOS编译基础工具 | 通过 | Xcode、clang和SDK探测；不是Tauri应用编译或签名验证 |
| 产品开发批准检查 | 保持关闭 | 未调用真实Agent，未启动功能实现或发布 |

实际CI：[Environment preflight #1](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40336413916)。
三个job均为`completed / success`，日志与产物在该运行中。当前清单更新不修改执行代码或工作流；远端通过证据只绑定上述SHA，不冒充后续提交也运行过CI。

## 仍未完成

| 项目 | 状态 | 下一步 |
|---|---|---|
| Codex Cloud环境 | 未创建/发布 | 由用户在Cloud环境设置中选择仓库，执行准备检查并发布；见下方说明 |
| 分支保护 | 未配置 | 当前连接没有规则写入操作；功能开发前单独核定主分支保护，不声称已启用 |
| 全部产品依赖锁定及原生App测试 | 未执行 | 后续获批实施中建立，不能以环境fixture替代 |
| 真实Provider测试、签名与公证 | 未执行 | 后续在明确授权及合适测试身份下处理，不需要现在提供私人凭据 |

已知非阻塞诊断问题：`doctor.py`使用`codesign --version`，macOS该命令不支持这个参数；此字段不应作为签名能力证据，基础设施CI不验证签名。

## 用户需要完成的Cloud操作

打开Codex：`Work in → Cloud → Select environment → Create environment`；也可通过`Settings → Codex Cloud → Environments`创建。选择本仓库，按`codex/ENVIRONMENT_SETUP.md`只准备环境。实际检查通过后选择Publish，直到界面显示Environment published。不要启动产品长程任务。

官方流程参考：https://learn.chatgpt.com/docs/environments/cloud-environments

当前对话没有可用的Cloud环境创建/发布操作。仓库脚本与GitHub CI已完成，不等于用户账号下的Cloud环境已经发布。完成发布后需核验环境身份、绑定仓库、安装与检查记录，再报告整体准备状态；功能开发仍等待用户另行确认。
