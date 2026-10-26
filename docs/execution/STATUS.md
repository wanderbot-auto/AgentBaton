# AgentBaton 执行状态

完整目标仍是 Design v0.2 的130项需求、27项已定决定与 WP01–WP14；不以阶段工作、Mock、浏览器或安装包代替完整验收。AB-START-001 有效；OWNER-001/002 已读取并 ACK。最近实际 fetch 的受保护 main 为 dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7，负责人收件箱没有后续序号。当前入口无原生 Goal 工具或 Cloud 任务 URL；正在使用真实原生子代理和独立审查。

PR #3 已合入1f34b44：精确222aafd的三项Environment与四项Product检查全部通过。PR #4 已合入dc5aea0：精确da28cd9的Product40395281447、Environment40395281545全部通过。原三个必需Environment名称及分支保护保留；无直接/强推main或绕过检查。两单元同一候选的ARM/Intel开发DMG均实际下载，ZIP摘要、DMG SHA256、构建记录与UDIF检查通过；见各单元EVIDENCE及PR描述。ARM为linker ad hoc/no TeamIdentifier，Intel未签名；无Developer ID、公证、原生GUI/人工安装或完整产品验收。

活动远程分支codex/provider-runtime-adapters，PR #8为叠加在专用UI PR #7上的draft；已推送f3eb83ebf9022544730e8ad6be578f5bfaf3efa1。它连接明确选择的custom ACP/native Codex/Qoder ACP/Claude ACP路线、受批准的私有默认RuntimePlan、按attempt持久化的实际配置证据/schema5备份与发起前校验；Native身份/配置加载/隐藏继承/OS强制隔离仍未知，严格不支持请求拒绝。默认计划以外的Skills/MCP/model配置、完整路由/委派/计划/工作流、验收采纳、留存导出、远程主机和登录/睡眠生命周期仍未完成。

f3实际CI：Product40401378109的Rust和UI（含官方schema与真实浏览器IPC）通过；Environment40401383150原三项通过。ARM完成核心/宿主编译后在原生夹具启动失败，runner清理EPERM又掩盖记录；下载核对的artifact12689352960只有两份失败日志，无窗口/报告成功。Intel前端47通过/1失败，初始Loading时未找到ready option。这些失败原样保留。当前本地整合了独立审查通过的显式0700原生夹具、保留失败JSON的安全runner、原生夹具单元CI步骤，以及await React act初始化的测试修复；原时限/断言保留，新增延迟scope测试后49项UI通过。15项Python runner/audit与1项真实Daemon准入单元独立通过，尚未在新候选上运行MacGUI。

最近完整高级分支Rust为250项（core78/config67/providers81/workspaces22/真实daemon信号2，嵌套FIFO子测试不重复计数）；fmt/Clippy/真实重启、22项真实浏览器/Unix IPC及8图、Linux宿主7项通过。其后include防护增量已独立24项workspace通过，新的完整计数须实际重跑后登记；UI49与生产UI源码不变。所有维度均是部分证据，不代表真实Provider或macOS原生验收。精确源码、旧检查点、失败日志和图片哈希见provider-adapters各REVIEW/EVIDENCE。

PR #9集中修复Git输入边界，远程head32ca76：禁止隐式fetch/协议、拒绝promisor/partialClone及include/includeIf、读取raw对象/祖先，同时保留用户文件/索引/replace/graft配置。实际控制是受控直接Git语义观测，不是旧Baton API执行；REVIEW_OBSERVATIONS明确原证据范围。32ca实际Product40403211132 ARM在既有真实daemon重启测试遇到WORKSPACE_BUSY，Environment40403211122原三项通过。后续本地34dc5f0/源码93ca7ea显式释放退役daemon的flock，复制FD控制旧FAIL/新PASS，全部131 Rust/36Core及独立审查通过；历史ARM原因未因此断言。主代理正在整合到有Supervisor的分支，必须先回收owned workers再释放authority。PR9和高级分支均须新精确CI，未合入。

Session PR #6已ready/main，远程3b94d1c仍是已失败旧候选；本地334d71e已保留FIFO修复并整合32ca防护，199Rust/35UI/24workspace/真实重启与独立FINAL_GUARD_REVIEW通过。待PR9合入后同步main并推送新精确CI。Quiet Canvas PR #7为专用draft，远程4871ae4较旧；本地6111037/源码bf042ec已整合session/include防护及异步测试修复，199Rust/47UI/22真实浏览器IPC+8新图通过，等待独立最终审查与新MacCI。保留OWNER-002两列卡片/聚焦任务/折叠能力界面，未导入原型成功数据。

另外隔离分支codex/validation-integration基于30f466d，已本地整合独立审查通过的validation/adoption库c856e46：40库测试通过，文件完整性和本地CAS机制不等于代码正确或全局TaskAccepted。主代理正在连接schema6、独立原生批准策略、真实IPC和Artifacts UI；14迁移/存储与6核心策略测试已实际通过，尚未完整集成/审查/推送。已实现内容不会据此标全需求完成。

真实Provider身份/模型测试、未批准远程主机、额外费用、正式签名/公证、最终许可证与生产发布继续单独关闭；无私人凭据/认证目录复制或真实模型调用。缺项只阻塞相关验证。继续修复精确CI、按保护合入已验证单元并接通后续获批功能；不承诺无活动执行器的后台续跑。可恢复分支、命令、限制和下一步见各CHECKPOINT。

当前组合修复已实际完成完整回归：254顶层Rust（core80/config67/providers81/workspaces24/真实daemon信号2），49UI/typecheck，15runner/audit，1真实Daemon私有夹具，当前Linux宿主7项，以及当前新daemon的22浏览器IPC/8图；fmt/Clippy/真实重启/设计检查通过。Actor版明确先取消并join owned workers/probes，再unlink旧socket并释放自身flock，错误/展开路径亦不提前交接authority；独立33项目标测试通过。源码与证据正保存到PR8功能分支并将启动新的精确提交MacCI。它仍不含另一个分支的schema6/ArtifactValidation，且不是MacGUI或全产品通过。
