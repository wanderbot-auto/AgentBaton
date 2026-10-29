# 调查证据（非新增产品测试）

## 来源与命令

只读基线 `8c178088d7213f625695a9d31a5b6d31cc342a42`。实际运行：`rg`/`rg --files`定位、`sed`读取、Python JSON解析需求/静态日志、`git show/rev-parse/diff`固定blob对比。新增独立docs worktree后始终保持该基线，没有对导入的用户项目执行Git操作。工作树自身四docs提交属于获准本仓库可逆文档工作，不是产品测试。

两次只读exec最初遇transport disconnected；之后 `git status --porcelain --untracked-files=no` exit0且clean，继续原worktree。最早搜索不存在的旧UI文件名后改读实际App.tsx/ManagedSessions.tsx，未将搜索退出码称作测试失败。没有新增Cargo/npm/Provider/浏览器命令。

## 需求与代码入口（本基线）

| 需求/链路 | 实际源码位置 | 本次能证明的范围 |
| --- | --- | --- |
| PRJ-002导入 | `crates/workspaces/src/lib.rs:401 inspect`；`registry.rs:292 import_project` | Git/Directory实际身份；不执行内容/初始化Git；local registry，不是远程host支持 |
| PRJ-003移动 | `lib.rs:449 reassociate`；`registry.rs:425 reassociate_project`；`Projects.tsx`重关联表单 | 同device/inode/kind、CAS、幂等及历史，不是跨FS/host或自动修复活跃Git worktree |
| Git隔离 | `lib.rs:470 create_isolated`；`registry.rs:462 create_workspace`；`ipc.rs`workspaces.create | exact commit、detached/private/outside-source、持久意图/unknown，不代表Task默认联通 |
| Directory隔离 | `data_workspaces.rs:396 propose_data_workspace`、453 finalize、697 finish_data_copy、1254 require_data_input_from；`workspaces/src/data_workspace.rs`；`Projects.tsx`内DataWorkspaces | 已批准选定数据的真实copy/witness/managed registry；不是普通Directory任意写授权 |
| Task入口 | `storage.rs:29 Task`、307 create_registered；`ipc.rs:1024 tasks.create`；`App.tsx:135 createTask` | 只建draft，未含默认workspace准备 |
| 执行绑定 | `execution.rs:14 ExecutionBinding`；`sessions.rs:364 create_session`、create_session_inner、1270 workspace；`policy.rs:338 validate_registered`；`ManagedSessions.tsx` | 现成workspace+连接+Grant人工选择，真实lease/fencing/身份检查；非完整任务Router |
| Router/Factory边界 | `routes.rs`；`factory_store.rs:129 limits`、560 start_contract；`factory_child_goal.rs:81`；`factory_child_plan.rs` | route是协议路由；Factory workspace数量固定1，所有new-workspace字段false，不能创建新权威 |

行号仅定位固定源，函数名为恢复锚点。FS device/inode均u64，UI不可通过JSON.stringify/浮点重构作精确授权比较。

## 已有真实测试证据：重新审阅，不重新执行

原件：`/workspace/.onboarding/pr17-c43-ci/rust-job.log`，209,838 bytes，SHA-256 `93f4ba238559c2c793b25e70d9124f216ed07933e62d7a79e0552d006df92548`。本次只读现存log，未下载/重跑。原log真实checkout `c69b3aad59d8379c1dbd2a2cd2ac9cef937cffce`、tree `061b5baac8b51051e57767c43ca10cf66cc03f24`；本地feature `c43ea68695039296be76efb9e8e2ac2d1052fd6e`的完整tree与它相同。c69临时merge Git对象本地已不可取，未fetch或假称直接读取该对象；以原checkout log与完整tree一致性绑定已存在feature blobs。

| 原log组/定位 | 实际历史结果 | 关键覆盖 |
| --- | --- | --- |
| project_ipc，约889–898行 | 3PASS/0ignored，0.86s | imported_projects_isolated_workspace_restart_and_archive_use_real_ipc；invalid_registry_requests_and_stale_head_do_not_create_workspaces；simultaneous_clients_reuse_identical_workspace_creation_key |
| registry，约900–914行 | 10PASS/0ignored，3.39s | Directory持久导入；move/replacement/CAS/archive；source HEAD/dirty保留；creating最终持久化失败unknown且不再checkout；lease重启/旧fence/目录替换/rollback |
| workspaces safety，约1240–1270行 | 23PASS/0ignored，3.22s | detached互不串写；无脚本导入；hooks/filters/隐藏index/sparse拒绝；移动与替换；路径/符号链接/大小限制 |

三个组共36个历史测试函数，不是本次新增36测试，更不是当前15完整集成结果。以下五文件以固定Git blob逐字核验：c43与本8c完全相等；不据此移签其余已变化的Core/Session/Factory行为。

| 文件 | 本基线SHA-256（与已测c43同字节） |
| --- | --- |
| `crates/core/src/registry.rs` | `2e364198355ad5c2bea1077c2c3b7aada842c5dee34826a093bd53817fa2fa45` |
| `crates/workspaces/src/lib.rs` | `352168f4fcd354fb3a34020b24c50cadec394f45b133ad9709a58cc17ef94e26` |
| `crates/core/tests/registry.rs` | `9355f135a16a3f4cb2a2523053f11bc3442d640101be7f5e2b04926891f60e19` |
| `crates/workspaces/tests/safety.rs` | `cc5fe6204309fc5a7a678e38830f63e24f30efc1a91faa449ffad6c82afa9b59` |
| `crates/core/tests/project_ipc.rs` | `9b38de8f283a706730edb49060c11124ad6341111ac0e76abdb07930884205f8` |

同一个旧官方log还明确通过 `actual_non_git_preview_owner_approval_copy_registry_and_same_key_are_durable`、`ready_copy_survives_deleted_original_and_restart_without_touching_model_budgets`、`non_git_real_grant_generated_acp_stop_and_data_json_contract_are_bound_to_actual_copy`。它们证明已有实际数据隔离链并非Mock，但其完整当前源未在本调查做同字节移签，不汇总到本次或36数目。历史保护集成/双Mac限制另见 `docs/execution/data-runtime-integration/DEVELOPMENT_DELIVERY_F9C6.md`；那里旧DMG、签名及OS/Provider限制不变，不能充当新工作流的Mac验收。

`requirements-progress.json`的三个条目仍partial/implementing，并有历史分片与残留旧措辞；本单元不修改它、不把“registry仍待做”的历史措辞覆盖现有源码，也不把已有分片当完整默认隔离验收。原specs specified保持设计快照语义。

## 本单元验证与未运行

本单元只做文档范围、引用和固定SHA/文件一致性检查，提交前 `git diff --check`；不写镜像单元测试。所有后续resolver/UI/default-task/并发/移动链的产品测试均NOT_RUN。真实Provider/远程Host/OS沙箱/实际新Mac安装与运行/最低OS/签名/最终许可证未验，也未增加资金或身份权限。下一实现的每条正常与失败路径列在PLAN，只有真实编码、执行测试、独审、准确CI后才能新增验证分片。
