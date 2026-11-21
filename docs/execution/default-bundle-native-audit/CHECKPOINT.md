# 默认 ARM64 开发包原生入口审计

现役 CI 调用保持为 `bash scripts/native-audit-release.sh <executable>`。新增整包入口如下。

```bash
bash scripts/native-audit-release-bundle.sh --bundle "/path/to/AgentBaton.app" \
  --sha256-manifest docs/execution/default-bundle-native-audit/logs/historical-executables.sha256
```

示例检查的是构建提交 `df35231eab29aa2d4a043fd9be60888504507716` 的指定历史产物。摘要文件来自已独立核对的包检查记录。其他工件应使用其可信记录里的摘要。格式为三个小写 SHA-256，每行跟两个空格及对应的 `agentbaton-desktop`、`baton` 或 `batond` 名称。

- 脚本核对已知三个可执行槽和 Info.plist 指向的桌面入口。检查实际文件的 thin ARM64 Mach-O 格式、角色识别字符串及摘要，识别角色互换和同角色跨构建替换。检查过程只读取输入和运行检查工具。
- 禁用标记来自桌面 `native-smoke` 的三个 Cargo 目标及其原生测试代码，以及 Core `preview-evidence` 下的 preview/copy 调试入口。对所有三个可执行文件扫描同一标记集合并在检查前后核对摘要。CLI 和 daemon 的角色标记取自各自真实主入口，桌面标记取自历史包的 Tauri WebView 绑定。
- 固定历史 DMG 工件 `14350183579` 在 Apple Silicon 只读挂载通过。真实副本缺 `batond` 与替换 `baton` 均退出 1。恢复副本后通过，原包前后快照相同且成功卸载。初轮 macOS 定向 21/21、现役 CI 接口 18/18，禁网 Linux ARM64 定向 21/21。
- 本结论限于该历史工件的三份已绑定可执行文件和已核测试入口标记。原生运行时、监听行为、当前发布产物验证各有独立验收证据。

[源码、命令、日志与 SHA-256](EVIDENCE.json)

## 单文件守卫回归复验

`scripts/native-audit-release.sh` 已恢复为基线 `c7dffe2c` 的完整源文件，SHA-256 为 `372ebc9860d2ecc1f529f5f089b877cf3bc0d8d68459ebf1d663af09933b99a5`。整包脚本把三份可执行文件的完整可打印字符串快照交给旧审计，另行核对已绑定的摘要、角色和独立测试目标名。旧守卫和 CI 调用原样保留。

实测源码 `bb9703e120a14954f7fafff38d66cd65d66967fe`。原摘要失败已复现并保留。修复后源守卫 12/12、macOS 与 Linux ARM64 定向各 22/22、CI 接口 18/18。本分支完整 Python 套件运行 1,048 项，1,032 项通过，16 项跳过。历史 DMG 再次只读通过并正常卸载，实际副本缺失和错换均拒绝。结论限于已核历史可执行文件及其已知测试入口。[拆分复验证据](SPLIT_REPLAY.json)
