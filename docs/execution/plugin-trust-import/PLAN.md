# AB-EXT-003 · 插件静态导入与启用检查

固定基线 `cbac72707f7c075b202a63a658b46f4f0a2df9c8`，工作分支 `collab/plugin-trust-import`，功能集成目标 `origin/collab/host-registration-integration`。已核对任务卡、AB-START-001、OWNER_INBOX、AB-EXT-003 及四条认领。

1. 复用 Config `SourceRoot` 的只读、受选路径和私有文件检查。第一版接受封闭 manifest，固定本地 manifest 和可选单一代码入口。校验来源、版本、许可声明引用、权限及 Provider 兼容声明，依据实际选中入口和权限派生信任等级。
2. 将兼容声明与调用者核实的运行事实分开。启用检查重读固定文件并验证精确摘要。代码还要求独立的受审记录、匹配的实际约束与权限。普通插件盘点保持可见，来源 `enabled` 不构成有效启用。
3. 使用真实私有临时目录的合成 manifest 和入口测试静态读、拒绝、防变更重用和无执行。只交 Config 入口；运行、UI、持久审批和真实兼容性留后续。
4. 通过任务包装器运行 Config 定向与全包 locked/offline 测试、格式、全目标编译、严格 Clippy。记录固定 SHA、日志、scope 和功能集成合并预览，交集成窗口独审。
