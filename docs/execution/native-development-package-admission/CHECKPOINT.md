# AB-REL-003 交接

| 项目 | 结果 |
| --- | --- |
| 基准和预检目标 | `ae844ca33406aabd62f44ea73818f9127f211748` |
| 已实测打包源码 | `37adc84966e6748fdd942651f3971e9bc6d850fc` |
| Apple Silicon 环境 | macOS 26.6.1（25G76），Xcode 27.2 |
| 本地候选 DMG | `209c1c8846cbdf8bc2581427113ab28eafd84d4becaafa2e2fdabd7db9b66188` |
| 实际消费 | 四文件归档、SHA256SUMS、既有 provenance 读取器均通过 |
| 拒绝 | 同角色旧版 daemon 混配、缺失 CLI 都在候选挂载后失败，两次退出 1 |
| 卷 | 三次打包及独立 provenance 检查均卸载成功，三条打包路径均无私有工作目录残留 |

打包回放复用 `14530c62a0b532e2ba43ca4be4fbc75ccc41d856` 的真实锁定构建原件和对应 App。混配注入来自 `df35231eab29aa2d4a043fd9be60888504507716` 的 ARM64 daemon；三份预期原件的 SHA-256 在各场景间保持固定。回放范围为本地安装包接入，既有构建证据见 [locked build](../development-bundle-provenance/current-pr33/REPRODUCE.md)。GitHub Actions 的精确集成候选检查由集成窗口执行。

复现使用任务私有目录。向脚本传入锁定构建 checkout 和历史 DMG 的本地路径，脚本会独立校验输入提交与哈希。

```bash
bash scripts/collab.sh run --native -- env LC_ALL=C /bin/bash \
  docs/execution/native-development-package-admission/replay-native.sh \
  "$LOCKED_BUILD_CHECKOUT" "$HISTORICAL_DEVELOPMENT_DMG"
```

范围以 `collab.sh scope` 和针对基准的 Git 预检为准。交付分支为 `collab/native-development-package-admission`，全局状态和 PR #33 交付给集成窗口。签名、上传及发布未执行。
