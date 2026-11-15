# WSI2a：固定只读工作区准备接口

需求AB-PRJ-002/003/005，Design v0.2 / AB-START-001；保留完整130项/27决定，OWNER-001/002 ACK，无新增授权。工作树 `/workspace/.worktrees/workspace-preparation-ipc`，分支 `codex/workspace-preparation-ipc`，固定基线 `f38d3f838021bb346581e4decf3130bfa5e31d67`。原WSI1树保持冻结，Factory正在开发的其它源不导入。当前源码 `12d0f71300f8baf14af71dd5441559bfce97ce8c`；只有8允许源码文件，不修改DDL/锁/CI/STATUS/权限文件/Factory/UI入口。

## 合同与权限

- Owner Unix IPC固定 `workspaces.resolve`，params仅task_id、expected_task_revision、expected_project_revision、host_id，serde deny_unknown_fields。host只local；expected_revision/idempotency_key envelope非null一律INVALID_REQUEST，null/省略不产生写意图。
- 复用WSI1的只读Deferred snapshot、关系pin、实际inspect、CAS、归档与有界candidate读取。不创建工作区、不记事件/预算/幂等，不从goal推断权限。source观察不替代未来创建/批准时fresh验证。
- 返回原WSI1结构，唯source.device/inode使用专用Core wire类型从原Rust u64直接to_string；不经JSON/JS number，不修改旧Project/ManagedWorkspace wire或其旧消费者。
- Native仅 `workspaces_resolve` typed selection → 固定method/frame → call(false)，加入既有generate_handler；不自动启动daemon、不确认Owner、不开放generic方法或文件/插件权限。
- 两处Factory scoped固定工具allowlist原字节保留，均不含该端点；companion/MCP凭据不得取得Owner source-read。真实socket负控测试method拒绝先于handle认证，不制造测试凭据来假称live授权已验。
- TS新专用wireguard：严格字段集合/状态关系、revision safe integer、个人project期待0允许；FS u64为规范十进制文本0或非零无前导零且<=18446744073709551615，仅opaque。无Number/parseInt等转换。全部authorization/creation/assignment/revalidation flags必须false。候选即使ready也只是metadata，expired非空lease不被释放。

## 依赖与执行顺序

1. 已完成源码与有意义测试源，作者只做选择性rustfmt及静态保持核验，不占Cargo/Node/Actor窗口。
2. 独立review原接口/新wire、角色、false flags与测试，不以作者自查代替审查。
3. Root串行真实运行Core serializer/WSI1/Unix IPC、严格Clippy、Native桥测试、TS api/type/build；准确绑定新集成源。现尚NOT_COMPILED/NOT_RUN。
4. 通过后另独立PR/准确CI；WSI2b默认Task准备/创建审批接线、WSI3分配/并发/移动链仍后续，不因一个只读接口宣称AB-PRJ-005完成。

## 验证计划

Core新1 serializer unit覆盖0、>2^53和u64MAX，完整JSON相等但仅两FS字段变成文本；这不是实际FS身份receipt。新4 integration函数真正建立TempDir Git/Directory/个人Task与Unix daemon，重复读取比所有表/sequence、源及managed文件/空目录不变；负控含foreign task引用错配、stale、archive、foreign host、额外字段/envelope、类型/大小、Project关系pin腐坏与wrong owner/scoped role。Owner token原本是整个本机Owner权限，不虚构逐Task ACL；已知另Task的有效请求仍按其真实Task解析。

Native新1frame unit证明固定四字段、无write envelope/自动daemon/审批；TS API源码测试保留旧3用例并追加传输/语义、canonical u64、unsafe revision、状态/候选/权限负控。上述测试尚未执行，不是browser、OSNative、Mac或真实Provider证据。

建议Root命令（作者未运行）：Core `cargo test --offline --locked -p agentbaton-core --lib workspace_resolution:: -- --nocapture --test-threads=1`；`cargo test --offline --locked -p agentbaton-core --test workspace_preparation_ipc -- --nocapture --test-threads=1`；严格相应Clippy。Native需其独立workspace与target窗口；Node对api.test.ts、typecheck/build按现package脚本。后续整合最新Factory基线后完整all-targets CI仍必需，旧base其它诊断问题不得冒成本单元新失败。

## 可逆ADR

使用独立只读Owner方法与专用lossless wire类型，保持既有registry API字节兼容。新接口不授予执行权、不扩Factory角色、不增加表或通用管理层；FS身份在JS只展示/保留为opaque文本，真正身份/授权仍Core fresh比较。接默认UI前先完成该真实边界；不借读取接口自动复制目录、初始化Git、创建Grant或绕Native确认。真实Provider/账户、Host、额外费用、生产、签名与许可证仍关闭。

## 源冻结后有限审查澄清

当前追加test-only叶 `e7f976b3aaaf9a97d5a64dca3d46e736d98547e0`；生产8文件中所有production字节仍12d，唯一差异是api.test.ts新增1个测试声明，覆盖JSON解码LF/CR/U+2028/U+2029在device/inode/head/key后的拒绝。独审最初静态认为JS无m的$可在终止符前匹配，作者与独审分别在内建V8实际反证并撤回；没有旧失败，未修改合法production正则来迎合误报。TS测试仍NOT_RUN。

Root已真实复验精确基线f38的Core lib strictClippy0与WSI1 targeted10PASS；这只闭合继承的Path借用修复，不是新端点/API/角色测试通过。新WSI2a仍需上述全部独审/实际测试。

## Root实际验证分片（精确c2bef0bf）

Root已串行执行Core11选定lib测试（旧WSI1十项+新serializer一项）、四项真实Unix IPC、Node全873/51files、typecheck/build全部通过。新Native frame unit仍未编译/运行；没有Native GUI、完整Factory集成、新Mac或远程CI通过声明。原source-time NOT_RUN记录保持历史。下一依赖为Root绑定严格lint与Native测试，再独立默认Git工作区持久Task绑定合同；不在此leaf编码WSI3接口或扩Factory权限。
