# 检查点

- docs-only branch codex/task-workspace-binding-ipc-contract，base461e250；只允许本目录四记录，无产品修改。
- 唯一新方法建议workspaces.prepare_task_git；6body params+envelope TaskCAS/key，源pin canonical decimal FS；Native typed workspaces_prepare_task_git/callfalse；无新permissions/generic/MCP权限。
- Root未确认该公共wire最终命名/映射，尚未编码；WSI3当前功能修复source2f0b9b3/docs0d6a034已在另树freeze待Root实际重验。
- 所有新接口/Rust桥/TS/真实IPC/Native/Mac NOT_RUN。下一步Root快速合同判断→独立受限source实现，不在当前文档阶段擅改public接口。

最新原功能source4d4631842ac8aa298f7717a89669160f97d9a690/docs2026620e0cb65e367e048f564bc8da1e33896570；0d实际13+1PASS、Clippy101，4d源码审ready但未真实lint。Native拟两固定commands：新write workspaces_prepare_task_git+既有Core read tasks_get；不新增Core读endpoint，不改权限。

## 实现阶段最新冻结

- Tree `/workspace/.worktrees/task-workspace-binding-ipc`，branch codex/task-workspace-binding-ipc，base95c693d，docs导入d43b9a8，source `3362cbad7f62feeb45b7efdf5b03c5c0ccf5eeb9`。当前四docs另提交，交付给准确新head。
- 8源码hash见EVIDENCE；后端Task绑定/schema16/core Task ctor/registry/Session原码、权限和锁无改。源已freeze，独审data_core_review_resume进行中，不把误报撤回当整体review完成。
- 新4真实Unix IPC/2Core/1Native与Node/type/build全部未运行；busy用真实待join I/O worker（非模型Actor），unknown为明确控制标记（非真实崩溃证据），socket丢响应测试设计真实关闭未读reply。
- 下一步Root独审→实际精准source测试/编译/Clippy/Node/Native，所有失败保留；不改后端来隐藏wire错误，不自动换key/重建/放行Scope。
