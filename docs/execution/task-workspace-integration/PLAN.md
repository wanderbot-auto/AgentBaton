# Task 工作区集成计划

基线 main653679d，依赖 PR18 的 schema15 Goal/Factory；本分支独立schema16，不修改不可变设计。范围对应 AB-PRJ-002/003/005 与 AB-SEC-007 的局部切片。

1. 整合已审绑定、fixed IPC、真正旧14/15迁移、strict inverse和Native版本消费者。
2. 接实际Task详情与只读执行对话框默认绑定，保留旧手动选择，不自动创建Grant/session。
3. 原正常/丢回复/FS漂移/拒写四场景后新增强制第五实际对话框场景；同source构建前端和Core。
4. 本地回归、独审、依赖合入后精确schema16七CI和双Mac开发包；通过保护规则后集成。

真实Provider/远程身份/签名等缺失仅阻塞对应验证。完整130持续，不据此声明全部需求完成。
