# 证据

当前仅只读源码调查与小合同，无新产品测试。baseline aa489595，原WSI2a已测结果及限制留原记录，不移签本单元。

实际已读：storage.rs Task/Store::open版本15上限/备份/迁移/恢复顺序；registry.rs create_workspace已原子creating reservation→FS→ready/unknown，但ready replay不fresh检查；workspace lease过期只数据库时钟不等于writer stop；create_isolated实际git worktree add会登记Git元数据但不覆盖用户源内容。新绑定需单独fresh gate，不篡改旧API行为来假称已有Task绑定。

所有source/body/types/SQLite/Git/Mac的新增验证均NOT_RUN；当前没有真实15→16迁移证据，不使用篡改version的库冒genuine旧应用fixture。源码freeze及有限静态命令后追加准确SHA/结果。
