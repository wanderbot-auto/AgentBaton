# 证据与未运行范围

本单元source-only，所有测试只是源码，尚未编译/执行。作者没有运行Cargo/Node/产品/Git fixture；没有把历史Root WSI1通过结果改名成本接口通过。

## 固定源

源commit `12d0f71300f8baf14af71dd5441559bfce97ce8c`，parent f38d3f838021bb346581e4decf3130bfa5e31d67。

| 文件 | SHA-256 |
| --- | --- |
| crates/core/src/workspace_resolution.rs | e65cf241f76524639b7f0a2ece05315933a7795ee5205d4db09594b4f8b7db76 |
| crates/core/src/ipc.rs | c8f45242cbc5840982bef6a6215725dfdda43d3e5bd476ecf6bcec1652c1d449 |
| crates/core/tests/workspace_preparation_ipc.rs | 070c6665ff63e014a49526825bbca759105584b4636ba26eb0761ca318abfd1a |
| crates/core/src/workspace_preparation_wire_tests.rs | 27582ffa9efe3b18d699b2d1dd27c15c818665ecbc94cdb310873c32e9db9a07 |
| apps/desktop/src-tauri/src/lib.rs | f2ed4aa37085a78fa4226f69138a19b417264188c9e0fbe7448f6ded7529a767 |
| apps/desktop/src/api.ts | 4ac8252b6f9c532d82a4111211542d466c654981dbbed6beee80e71ab0d85177 |
| apps/desktop/src/api.test.ts | 8a72b1e5e96945c3a133b32eb5c8f7f548c0a6dba938699be1e8cdebd736f595 |
| apps/desktop/src/workspace-preparation-wire.ts | 2711af2cd310ed985767cd6c58ffe695257d82497695b00472ce9547a82946c7 |

## 实际有限检查

`/workspace/.toolchains/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2021 --config skip_children=true` applied及同参数--check，限定resolver、wire unit、ipc、IPC tests、Native lib五文件，exit0。没有递归格式化其它模块。git diff/cached --check均0。

Python只读有限静态核验exit0：移除新增wire块与测试登记后整个resolver等parent；移除固定arm/说明注释后整个ipc等parent（包含两处Factory restricted allowlist）；移除新frame/wrapper/handler/test模块后整个Native lib等parent；移除新import/API方法后整个api.ts等parent；api.test.ts保留完整旧文件前缀。Core lib.rs、旧workspace_resolution_tests.rs、capabilities/main.json、三个相关lock均逐Git byte相等。FS decimal helper未包含Number/parseInt/parseFloat/BigInt转换；规范十进制长度与max字串上界固定。上述只是源码保持检查，不是复制实现的产品测试或运行计数。最初checker空行边界assert失败保留于执行历史，修正checker后0。

## 新测试源码（全部NOT_RUN）

- Core serializer1fn：u64零、9007199254740993、u64MAX实际Rust值 → string；完整serialized Value只替换两个FS字段等原值。构造身份是unit，不谎称实际FS。
- Core Unix IPC4fn：real_git_directory_and_personal_reads_are_metadata_only_and_repeatable；stale_foreign_malformed_and_write_envelopes_cannot_change_any_rows；corrupt_project_pin_foreign_task_document_and_oversized_document_fail_closed；companion_role_and_wrong_owner_token_cannot_obtain_source_reads。
- 每个Owner read前后比较SQLite所有表值/sequence和source/managed文件及空目录；负控corruption由测试显式准备，非产品写或合成执行成功。角色负控经真实socket发送不在Factory工具名单的方法，未伪造live handle成功；实际有效scoped凭据的全产品链仍属原Factory测试。
- Native1fn检查固定四字段完整frame及未知字段拒绝；源码断言callfalse/no确认/no自动daemon。该unit尚未运行，不等于OS GUI。
- TS API新增传输/正向个人GitDirectoryblocked/candidate；unsafe revision、canonical FS decimal/type/max、额外字段、选择错配、授权false/state/candidate负控。测试是mock invoke的adapter合同，未来运行也不能当真实IPC/Mac。

真实Unix IPC/类型/Clippy/build/Native/Mac与默认Task UI仍待Root资源窗口及独立review。当前只可记录source implemented，不能记录产品slice verified、所有130需求完成或新增开发包。旧WSI1 dedee BLOCKED/7a源审/Root10PASS/00f Clippy FAIL/3e借用修复证据继续保留在原四记录，不重复执行或移签。

## e7f追加覆盖与被撤回的静态归因

独审曾推断12d正则$可接受末尾行分隔符。作者内建functions.exec V8实际执行exact旧decimal/head两个pattern与LF/CR/U+2028/U+2029（8项）的test均false；独审config独立执行旧decimal/head/preparation_key三个pattern×4终止符（12项）也均false，并撤回P2。无m的ECMAScript行为与该静态推断不同。没有old runtime FAIL、没有Node/TS产品执行；不把该有限实验扩大为完整guard证明。曾在WIP替换三regex，反证后全部逐字恢复12d，未提交任何生产regex改动。

仅test-only leaf `e7f976b3aaaf9a97d5a64dca3d46e736d98547e0`（parentf146四docs）增加api.test.ts的JSON.parse真实分隔符负控，覆盖device/inode/head/key16个变体的同一测试声明。其它原测试和全部production不变，测试尚未执行。api.test当前SHA `2b8c33fb96dc01c82c562e9bf989c3589ea83e4f99a5d0363f68fbd76a25b294`；原12d SHA表保留历史，其余7文件SHA仍相同。

## 继承基线f38的Root实际复验（不是WSI2a验收）

Root source `f38d3f838021bb346581e4decf3130bfa5e31d67`：

| 原件目录 | 实际命令/结果 | 原log SHA-256 |
| --- | --- | --- |
| `.onboarding/workspace-isolation-wsi1-lib-clippy2` | cargo clippy --offline --locked -p agentbaton-core --lib -- -D warnings；exit0，wrapper7.916620766s | 3a9c11275f4c6bea4fb4ff4029d6542f57b00b0d328f2b1efffd85ea6f01df32 |
| `.onboarding/workspace-isolation-wsi1-targeted2` | cargo test --offline --locked -p agentbaton-core --lib workspace_resolution::tests -- --nocapture --test-threads=1；10PASS/0failed/0ignored/128filtered，test3.41s，wrapper19.945066626s，exit0 | 5c72509755afb7af6d6f51ed019a2fc8c947b0b21bb04efc38cdf19e4a45425d |

作者本轮Python只读核各RESULTS source/exit与command-0.log SHA全部匹配，exit0，没有重跑。该证据闭合旧WSI1 Path借用修复的准确运行绑定；原00f真实Clippy失败不抹，不能移签成12d/e7f新IPC/Native/TS已通过。原树未改，Root其它Factory运行仍独立。
