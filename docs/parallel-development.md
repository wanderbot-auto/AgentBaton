# AgentBaton 多窗口协作手册

在 Mac mini 上，一个任务窗口拥有一个分支和一棵 Git worktree；一个值班集成窗口负责合并顺序。常规任务并行开发、测试和开草稿 PR，共享契约由一个任务先定稿。以 GitHub PR 记录交付和检查结果，以本机任务登记表避开同时写同一路径。先从两个开发窗口加一个集成窗口开始。

## 开工与分工

| 角色 | 工作方式 |
| --- | --- |
| 集成窗口 | 维护任务依赖、共享文件归属和合并顺序；核对实际目标分支、PR、验证范围及 CI；一个时间只处理一个合并候选；维护全局 STATUS 和需求进度。 |
| 开发窗口 | 办理任务认领，在自己的 worktree 编辑、提交、验证并移交。PR 可由授权的任务窗口创建或更新；最终合并由集成窗口执行。 |
| 子代理 | 默认只做调查、测试设计或审阅。同一任务有明确独立文件时才委派写入，给出路径、接口、基准 SHA 和结果格式，主任务暂停在这些文件上的写入；涉及另一模块的独立实现用独立工作树。子代理不接管合并。 |

用户直接描述要新增、修改或优化的功能。开发窗口负责把需求转成任务卡，自行确认任务 ID、基准 ref 与 SHA、可写路径、依赖和适用的验证命令。私下进行时可放在该 worktree 的 `artifacts/collab/TASK.md`。跨机器进度可记录在草稿 PR 描述中。已暂停但仍可能修改的任务保留认领。

### 收到需求后

1. 先只读检查相关代码、测试和现有认领。对可逆的工程选择自行决定，按用户描述和已有行为确定需要验证的场景。遇到产品取舍、授权问题或关键需求无法判断时，向用户确认具体问题。
2. 同一个需求的试用与调整继续使用原来的已登记工作树。新任务自行取唯一 ID，默认从最新 `origin/main` 开始，用 `scripts/collab.sh new` 认领具体路径。依赖尚未合入时联系集成窗口核对上游基准。发现路径重叠时，先由集成窗口协调归属。
3. 从仓库根目录收到任务的本地会话也可以自行登记。登记成功后核对脚本返回的绝对路径、分支和起始 SHA，后续的编辑、构建和测试都在该工作树执行。把可试用的版本和当前验证结果告诉用户，收到调整意见后继续同一个任务。
4. 用户确认体验后运行 `scope`、`preview` 和适用的回归，向集成窗口交接基准、提交、改动范围、测试结果与未验证事项。最终合并由集成窗口执行。

前端任务认领实际组件、对应测试和 i18n；Rust 任务认领具体模块及测试。公共 IPC／wire 类型、schema 编号和迁移、锁文件、顶层路由、CI、全局状态账本都需指定单一写入者。前后端同改契约时，先合兼容接口或用小的依赖 PR；前端可先针对固定契约写测试，最终用真实 IPC 验证。如果必须同改才能维持兼容，做一个纵向任务和一个 PR。语义耦合即使不在同一文件，也要写进任务依赖。共享文件遇到冲突先暂停其中一处写入、明确由谁收口，再继续旁路任务。

## Mac mini 的日常命令

脚本支持 macOS Bash 3.2 和具有 `merge-tree --write-tree` 的 Git（2.38 起）；不依赖 GitHub CLI。第一次可从脚本所在的协作分支用绝对路径调用；合入后在任何仓库工作树调用该版本。以下 T101/T102 只是分工示例，路径按实际任务缩小。只有依赖尚未合入的任务才使用明确的上游分支，并记录相应 PR 依赖。

```sh
git fetch --no-tags origin
bash scripts/collab.sh list
bash scripts/collab.sh new t101-task-ui origin/main apps/desktop/src/ArtifactValidation.tsx apps/desktop/src/ArtifactValidation.test.tsx
bash scripts/collab.sh new t102-provider origin/main crates/providers
```

`new` 会打印绝对路径。已登记的任务可以在 Codex 中用 **Local** 打开该目录，复用原工作树。新需求也可以从项目根目录的 **Local** 会话开始，由开发窗口自行登记。开工前由开发窗口用 `git rev-parse --show-toplevel` 和 `git branch --show-current` 确认目录与分支。若任务基准早于手册与脚本入主分支，使用协作分支中脚本的绝对路径。每棵工作树只分配一个写入窗口。Codex 的执行环境见 [Worktrees 文档](https://learn.chatgpt.com/docs/environments/git-worktrees)。

每棵 worktree 使用自己的 `.venv`、`node_modules`、构建目录和测试证据。可共用 Cargo registry 和 npm 下载缓存；避免共用 `CARGO_TARGET_DIR`、运行时 SQLite、IPC 路径和 dev-server 端口。启动桌面测试时按 [桌面 README](../apps/desktop/README.md) 使用 task 专属的短路径（例如新建 `mktemp -d /tmp/ab-t101.XXXXXX`，按测试脚本约定设置 `BATON_RUNTIME_DIR`）；关闭自己启动的进程。并行跑针对性测试，完整 Rust/原生桌面重测试按需错峰，防止本机负载干扰时限测试。

### 任务运行入口

在任务 worktree 中通过以下入口运行命令（需要 Python 3；macOS/Linux）：

```sh
bash scripts/collab.sh run -- cargo test --workspace --locked
# 原生桌面开发占用同一 checkout 的原生独占锁：
bash scripts/collab.sh run --native -- bash
```

`run` 为每个任务保留一个独立、权限为 0700 的短 runtime，并强制设置 `BATON_RUNTIME_DIR` 和本 worktree 的 `CARGO_TARGET_DIR`；不会继承调用窗口里指向另一任务的这两个值，也不改变 HOME、身份或认证目录。runtime 路径登记在共用 Git 目录的 `agentbaton-collab/runtime-<id>`，正常退出后保留，便于同任务重连。缺失、符号链接或权限漂移均报错，不自动修复或另建 runtime。任务彻底结束、确认全部关联进程停止后，按登记路径人工清理测试数据；`finish` 不删除这些数据。

同任务同时只允许一个 `run`；`finish` 在它运行期间拒绝释放认领。`--native` 另持有整个 checkout 的原生独占锁，防止默认 1420 端口与原生交互测试相撞；其他任务的普通检查仍可运行。锁由操作系统随持有进程退出释放，不要删除 `.lock` 文件。同一任务需要 daemon 和 UI 时，在同一个受锁前台命令内启动和回收两者；退出前停止并等待自己启动的后台子进程。包装器转发 INT/TERM 给命令进程组，保留退出码；它不是后台进程管理器，不保证回收主动脱离进程组的程序。

保持 Vite、Tauri 和原生来源校验的固定开发端口，不通过放宽信任列表实现并行。普通产品登录后台管理使用用户级 LaunchAgent，隔离 runtime 下会明确拒绝；这部分仅由集成窗口串行验证，优先使用已有私有 native-smoke 夹具，不能为了测试改 HOME 或操作其他窗口的服务。手动绕过 `run`、另一 clone/机器和同一 worktree 的编辑器仍不受这些锁保护；一工作树一写窗口及 scope/preview 交付检查继续有效。

```sh
git fetch --no-tags origin
bash scripts/collab.sh scope
bash scripts/collab.sh preview origin/main
# 仅单人、从未发布且无人基于它开发的当前任务分支：
bash scripts/collab.sh rebase --private
```

`scope` 比较已提交差异、暂存和未暂存的改动，以及未忽略的新文件；`preview` 只对两个提交快照做 Git 合并演算，不动分支、索引或工作区（Git 可能写入临时树对象）。返回 1 表示有冲突或越界，需由任务窗口处理；无冲突仍需跑测试，逐提交 rebase 也可能另有冲突。其余非零视为预检没完成。对候选 PR 一律用实际 target ref，别机械用示例的 main。两个任务都领先共同祖先时也可 `preview origin/main collab/<id>` 单独检查，或 `preview collab/<id1> collab/<id2>` 检查彼此；这不能替代精确基准上的集成测试。

脚本用共用 Git 目录下的本地登记表及短锁避免认领竞态，父目录与子路径也视为重叠。它只覆盖从同一 checkout 通过脚本登记的任务；既有或其他机器的分支必须靠任务卡和 PR 同步归属。认领是约定，不是安全隔离。申请超出范围时让集成窗口确认归属，再做新任务或书面交接；不要擅改他人的认领。工作创建异常时用 `collab.sh list` 核对任务；如果只留下孤立登记，经集成者核对 Git worktree 和分支、确认没有活跃写入后，在 `git rev-parse --git-common-dir` 下归档 `agentbaton-collab/active/<id>` 到 `closed/`。同理，异常退出残留的 `agentbaton-collab/lock` 仅在确认无注册或释放进程时用 `rmdir` 去除。

## 同步、冲突与合并

1. **私有分支**：在干净的任务 worktree 中显式运行 `rebase --private`。脚本拒绝脏工作区和检测到已发布的分支，保留旧 head 到本地备份 ref。冲突用 `git status` 和 `git diff --name-only --diff-filter=U` 找文件，结合两边意图及相关测试编辑；逐个暂存后 `git rebase --continue`，不能安全解决可 `git rebase --abort`。复核补丁序列可用 `git range-diff <old-base>..<backup> <new-base>..HEAD`，再运行相关验证。禁止整文件盲选 ours/theirs、跳过提交或把改动搬进别的任务工作树。
2. **已发布、被依赖或进入审核的分支**：通常在它自己的干净 worktree 普通合入更新后的实际 base（`git merge <base-ref>`），解决冲突和验证后正常更新 PR。不同窗口不得并发写同一个 head；禁止为更新共享 PR 强推。堆叠 PR 父分支若 squash/rebase 合并，旧父 SHA 可能不在新基准历史中；保留旧 head，新建接续分支，仅移植子任务范围（明确旧父/子 SHA，用 `git rebase --onto` 于新私有分支，或逐一挑选子提交）。比对旧新补丁、PR diff、测试结果后更新 PR 合并计划。
3. **交付**：提交前后运行 scope，更新并记录实际 base SHA、head SHA、变更范围、验收命令/结果、已知缺口和依赖。合并前集成窗口核对 GitHub 显示的 base/head 与验证对象、独立审查和未解决反馈；确认预期的环境、产品和平台测试均通过。base 或 head 变动后重新验证候选；同一 PR 的陈旧绿灯不能沿用。一个候选受阻则回交给原作者，其余无依赖 PR 继续。
4. **最终合并**：独立 PR 优先 squash，方便按任务回退。已有堆叠 PR 由集成窗口逐层验证，允许用保留祖先的 merge commit（需符合仓库规则和已授权策略）；默认不得同时把父和子当两个独立 PR 合入 main。GitHub CLI 可使用 `gh pr merge <PR> --squash --match-head-commit <reviewed-head>` 作为独立 PR 的执行命令；它只钉住 head，base 由集成者复查并依赖现有严格更新规则，仍需通过保护。不得绕过产品检查或用 admin 绕过保护。合并确认后，或已记录负责人与接续位置的交接后，用 `collab.sh finish --merged` 或 `--handoff` 释放路径。它只登记释放、不验证远端合并、不删 worktree。之后仅用正常的 `git worktree remove <path>` 清理已确认无须保留的工作树；有未追踪证据时先归档。

## AgentBaton 当前接入路径

当前主分支是 `main`。阶段成果 PR #26 已合入；后续独立任务以最新 `origin/main` 为起点。依赖未合入的成果可从明确的上游分支开发，最终合并受父链约束；不得把相互重叠的 PR 未经核对分别合入同一个目标。当前的 Product workflow 默认只在面向 main 的 PR 自动跑；堆叠分支若用 workflow_dispatch 验证，必须区分被测组合与将合入的精确目标。

已核对 main 的生效规则：Linux 与 Apple Silicon 的两个 Environment 检查必需，要求分支更新，并要求解决审查对话；PR #27 已确认实际 check 名称与 GitHub Actions 来源，并在 2026-11-15 将 `Product / Rust`、`Product / UI`、`Product / macos-15` 追加为必需，保留两个 Environment 检查、严格更新、PR 与审查对话要求、无 bypass。独立审阅仍按任务证据执行；规则不要求额外 GitHub 审批人数。当前是个人名下公开仓库，根据 [GitHub 官方的 Merge Queue 资格](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)，目前采用单集成窗口。以后若迁到符合资格的组织且 PR 吞吐有需要，先让两份 required-check workflow 支持 `merge_group`，验证合成分支能跑完整检查，再启用队列。

给新任务窗口的一句话示例：

> 把设置里的更新状态移到后台服务下面，刷新按钮保留在原面板里。
