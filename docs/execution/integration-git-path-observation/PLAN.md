# Integration 同次 Git 路径观察

基线 `89f83e6bc2968f3d70cb449135f828e4ade2dafe`；源码 `0793d53eee3127c1b263411fb82a329c7c00a5a2`，仅测试访问修复 `3b2ac87bc5a511ef8fe78fdc64cd0ec8f079afd4`；分支 `codex/integration-git-path-observation`。这是既有完整设计的窄性能切片，不改需求状态或验收标准。

1. 每次原 recheck 内把三个路径 rev-parse 合成一次 fresh bounded observation；严格三完整字段、原实体 pin/路径规则；保留独立 bound HEAD、全部原重检点和真实原时钟计费。
2. 保留原生产测试，新增10项实际 Git/路径/退出/取消/超时/HEAD负向合同；整源码和测试做严格逆向证明。
3. 独立审查后在主树验证全部 Workspaces45与实际 Integration IPC33；保留首编译失败，不重复叠加10+35。
4. 准确提交的 macOS 双架构、原2s/5s预算、完整CI与桌面交付仍需主代理后续验证；第二 config batching 不在本单元。

路径比较保持原 Rust Path 组件语义，并非逐字节词法拒绝；没有跨 recheck/tick 缓存、预算扩张、IO折扣或未知结果提升。
