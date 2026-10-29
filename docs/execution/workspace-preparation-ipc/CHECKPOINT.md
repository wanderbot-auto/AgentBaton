# 检查点

- Tree `/workspace/.worktrees/workspace-preparation-ipc`，branch `codex/workspace-preparation-ipc`；原树workspace-isolation-next/f38保持冻结。
- 固定源码 `12d0f71300f8baf14af71dd5441559bfce97ce8c`，parent `f38d3f838021bb346581e4decf3130bfa5e31d67`。当前四docs另提交，交付回复给准确docs SHA，不伪造自指。
- 8源码：workspace_resolution wire转换；ipc固定Owner arm/注释；专属wire unit和真实IPC test；Native lib固定wrapper/frameunit/handler；api.ts/api.test.ts；新workspace-preparation-wire.ts。源已经冻结，文档阶段不再修改。
- 合同：4params、非null envelope写字段拒；Owner local only；Factory/MCP allowlist无新增；opaque FS十进制文本、safeint revisions、所有权限false；无创建/事件/预算/DDL。
- 已实际：格式/有限插入逆改、旧API和allowlist/旧10tests/lib/权限/锁保持检查0。NOT_COMPILED/NOT_RUN：新4IPC+1wire+1Native Rust函数及TS用例、类型/build/Clippy/真实IPC/GUI/Mac。
- 下一步：Root独立review，按lease执行针对性真实命令，保留所有失败并绑定精确集成SHA；Actor/Factory其它工作不混验。WSI2b/3/default UI尚未实施，本端点不能作为执行批准或工作区assignment。
- 权限：AB-START-001；OWNER001/002；真实Provider/spend/生产/签名/许可证未扩大。没有新Host/模型/API账户或后台执行器。

## 最新限定检查点

- Source12d生产冻结；新增test-only `e7f976b3aaaf9a97d5a64dca3d46e736d98547e0`。api.test SHA `2b8c33fb96dc01c82c562e9bf989c3589ea83e4f99a5d0363f68fbd76a25b294`，其它7源码仍上表12d SHA。当前fourdocs再单独提交。
- 正则P2已撤回：双方内建V8有限原pattern反证，无旧FAIL/无生产fix；新增JSON终止符源码覆盖尚未执行TS suite。
- 继承f38由Root实际Clippy0+targeted10PASS，精确log见EVIDENCE，不移签WSI2a。WSI2a新源码仍NOT_COMPILED/NOT_RUN，独立config正在审查。保持原树/head给Root，不占Cargo/Node/Actor。

## 实测后检查点（Root执行c2bef0bf，当前仅docs追加）

- Root Core11PASS/test3.43s/22.662015s、真实Unix IPC4PASS/test0.97s/15.442594s；Node873/51、typecheck/build均0。原件目录分别workspace-preparation-ipc-core-units1、workspace-preparation-ipc-real-ipc1、workspace-preparation-ipc-ui-regression1。
- Production仍12d，testonly e7f；Root测试head c2bef0bf前后源不变。当前四docs提交后准确新head由交付给出，不能伪造自指。
- 独审SOURCE_READY_FOR_ROOT_SERIAL_TESTS：`.onboarding/workspace-preparation-ipc-12d0-independent-review` REVIEW11623c5a27ef7f060f1c12a0432e4d080da4a5ab6340f08872b53511435d1d47 / MANIFESTdd35bb01bdcf0b73c252b5a54f037d4c8d0fffdf3665608d5018765070ad8e8f。25静态/64内建V8控不是额外产品测试；e7追加源保持已核。
- 尚未运行：本次新增Native frame unit/Native编译、OS GUI、新Mac、最新Factory组合/完整CI；Root后续严格lint绑定待实际结果。
- 下一动作仅只读研究WSI3持久Task→Git workspace绑定/unknown恢复，提一个最小Store合同待Root确认。不得把候选metadata、相同project或registry旧ready replay当Task绑定，不写公共接口/新DDL/Grant权限。
