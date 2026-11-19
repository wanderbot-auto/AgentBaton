# 默认 ARM64 开发包原生入口审计

现役 CI 调用保持为 `bash scripts/native-audit-release.sh <executable>`。新增整包入口如下。

```bash
bash scripts/native-audit-release.sh --bundle "/path/to/AgentBaton.app" \
  --sha256-manifest docs/execution/default-bundle-native-audit/logs/historical-executables.sha256
```

示例检查的是构建提交 `df35231eab29aa2d4a043fd9be60888504507716` 的指定历史产物。摘要文件来自已独立核对的包检查记录。其他工件应使用其可信记录里的摘要。格式为三个小写 SHA-256，每行跟两个空格及对应的 `agentbaton-desktop`、`baton` 或 `batond` 名称。

- 脚本核对已知三个可执行槽和 Info.plist 指向的桌面入口。检查实际文件的 thin ARM64 Mach-O 格式、角色识别字符串及摘要，识别角色互换和同角色跨构建替换。检查过程只读取输入和运行检查工具。
- 禁用标记来自桌面 `native-smoke` 的三个 Cargo 目标及其原生测试代码，以及 Core `preview-evidence` 下的 preview/copy 调试入口。对所有三个可执行文件扫描同一标记集合并在检查前后核对摘要。CLI 和 daemon 的角色标记取自各自真实主入口，桌面标记取自历史包的 Tauri WebView 绑定。
- 固定历史 DMG 工件 `14350183579` 在 Apple Silicon 只读挂载通过。真实副本缺 `batond` 与替换 `baton` 均退出 1。恢复副本后通过，原包前后快照相同且成功卸载。macOS 定向 21/21、现役 CI 接口 18/18，禁网 Linux ARM64 定向 21/21。
- 本结论限于该历史工件的三份已绑定可执行文件和已核测试入口标记。原生运行时、监听行为、当前发布产物验证各有独立验收证据。

[源码、命令、日志与 SHA-256](EVIDENCE.json)
