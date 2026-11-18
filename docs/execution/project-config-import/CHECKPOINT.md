# AB-CFG-009 交接检查点

| 项目 | 固定引用 |
|---|---|
| 任务分支 | `collab/project-config-import` |
| 认领基线 | `b5681ec4fa3ff2f48fe0c40bc24b436ce4e1c4ea` |
| 初次已测实现 | `c537182e7a723eed8562f8fe90cffe643f0213ad` |
| 审阅后已测实现 | `d601689a0dc85c47ef3b483bccef867de2d5d747` |
| 预检目标 | `collab/host-registration-integration` @ `1240b1ce7109590f1c61584a6132e14a45ae901b` |
| 接续 | `leader-control` 独立审阅、验证精确合成树并决定合并 |

调用方提供注册项目物理根和版本修订、精确 Provider 版本、选定项目内定位及规则修订。可依次调用 `inspect_project_config`、`prepare_project_import`、`apply_project_import`。检查和准备仅返回审阅数据。确认提交前，可信登记方持久化原始 `ProjectImportBinding`，核准并解析确认引用。快照中的规范化配置、完整来源状态、取舍和运行时计划均受同一摘要约束。

生成的 `project-config.json` 是对象所属私有托管根中的一次性配置快照。目录身份通过设备和 inode 固定。托管文件与日志用于 `recover_project_import`，可通过 `read_imported_project_plan` 读取再次验证过的计划。已有确认没有自动重放路径。确认时源内容必须匹配，在批准后的正常读取中维持当时固定的计划。其他配置写入须经后续独立策略。

当前支持指定 Codex 与 Claude 项目内发现表面。纯文本 Skill 需确切非秘密分类。MCP 仅引入另行批准的同名同传输注册定义。Hooks 的事件和不支持激活状态保留于检查与快照。源内的命令、参数、地址和凭据均不进入托管配置。

Core 需接线可信项目及当前 Git 输入修订、专属托管目录、审阅绑定、权威确认记录和具名 MCP 定义。共享 IPC、原生 App 首次确认与取消、真实 Provider 加载及 Hook 激活尚未交付。本阶段保留 D21 全量验收和 AB-CFG-010 后续风险规则。

Apple Silicon 通过定向、全量 Config、格式、编译和严格 Clippy。Git 与配置文件均为本地夹具。真实文件系统故障用例在私有目录中运行。精确命令见 [EVIDENCE.json](EVIDENCE.json)，基线和目标预检见 [scope-preview.txt](logs/scope-preview.txt)。

独立审阅发现项目配置检查在核对项目目录后仍按路径重新打开来源。修订后，存在性检查、原生配置与 Skill 内容读取共用该目录描述符。原原生来源和 Skill 公开 API 继续执行原有守卫。实际目录换位测试验证了被替换路径拒绝、已打开目录仍提供原内容以及原路径复原后的正常读取。修订后 7 项集成、2 项定向单元、99 项全量 Config、fmt、check 与严格 Clippy 通过。初次 `c537182e` / `f4cf1523` 的证据继续保留。最新预检见 [scope-preview-fd.txt](logs/scope-preview-fd.txt)。
