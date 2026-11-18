# AB-CFG-009 首次项目配置导入

认领基线 `b5681ec4fa3ff2f48fe0c40bc24b436ce4e1c4ea`，任务分支 `collab/project-config-import`。依据 AB-START-001、D21、OWNER-003 和集成窗口任务合同。OWNER-003 延后的是启动引导，保留底层导入及批准能力。

1. 对注册项目根与固定版本的显式项目配置定位做只读检查。复用 NativeSourceRoot 和 InstructionOnlySkill 展示来源、冲突、未知或不支持信息。
2. 将项目身份、修订、观察输入、当前策略和选定配置绑定为可持久化的确认对象。仅在可信确认后生成私有托管快照。可入选的内容为非秘密纯文本 Skill 及独立批准的托管 MCP 定义。Hooks 保留事件检查结果。
3. 复用 ManagedRoot 的持久事务与锁。首次确认比较当前观察，重复确认返回原结果。管理层重启时按可信批准、日志和文件摘要对账，保留未应用、已应用和需对账的区别。
4. 用私有临时项目、实际 Git、合成配置和真实文件失败验证正常及负向边界。运行 Config 定向和全量测试、fmt、check、严格 Clippy、scope 与 preview。Core 登记和确认、IPC、原生 UI、Hooks 激活由后续独立接线。
