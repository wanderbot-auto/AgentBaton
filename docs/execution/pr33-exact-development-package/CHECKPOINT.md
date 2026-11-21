# AB-REL-003/004 精确开发包交接

构建对象为 `d70362ad1fc4b080537163be60cb830849ee4167`，树为 `19aba6e8f208f021bc84573dd8b496d29fce5633`。实际环境为 Apple Silicon、macOS 26.6.1 (25G76)、Xcode 27.2 (27B52)、Rust/Cargo 1.91.1、Node 26.7.0、npm 11.19.0、主构建 Python 3.15.2、Tauri CLI 2.12.4、Vite 8.3.5。三份锁文件字节与 Git 一致，构建后 checkout 干净。

| 本地保留产物 | SHA-256 |
| --- | --- |
| ARM64 开发 DMG | `7e7598774e7bc20bcf7ad0cf6e61d6096a21afadf1fa9543891225e928aa2055` |
| 四文件归档 `development.zip` | `2fa14871b9e9e715b25dd1f9bc5fcfbfaf67243e99ae8a25a484f4a187ef25c0` |

完整产物目录为 `/private/tmp/ab-collab-0dvd6o1q/pr33-package.SQF5QB/source/artifacts/development`。归档和验证报告在同一运行目录下的 `output`，原始编译与打包日志在 `build.log`。私有目录保留供集成复核。 [证据索引](EVIDENCE.json)

默认 App 单文件防护和 DMG 三二进制审计通过。重新读取归档后，provenance 读取器核对实际只读卷及三个原件的 SHA-256。三件可执行文件均为 thin ARM64。Cargo fingerprint 确认默认应用没有启用 `native-smoke`，Tauri 依赖启用 `custom-protocol`。已核对任务无残留映像挂载。 [只读 provenance](logs/provenance.json) [独立复核](logs/independent-review.txt)

本次执行明确使用 Tauri `--no-sign`。编译目录的独立桌面 Mach-O 严格签名检查返回 0，App 中的主程序路径和 App bundle 严格检查返回 1。系统诊断为 `code has no resources but signature indicates they must be present`。本次验证范围是锁定编译、开发 DMG 组装及静态读取。未运行 GUI、实际安装验收、较低 macOS 版本、托管 CI、Developer ID 签名、公证、上传或发布。 [构建记录](logs/build-record.txt)

## 复现

在已认领的工作树内以本机已有 Git 对象及缓存运行。脚本会保留新私有路径，输出对应构建目录。

```bash
bash scripts/collab.sh run --native -- /bin/bash \
  docs/execution/pr33-exact-development-package/replay.sh
```

将 `attempt` 指向上一条命令输出的私有目录，复核锁文件、DMG、ZIP、编译特性和签名状态。

```bash
bash scripts/collab.sh run --native -- /bin/bash \
  docs/execution/pr33-exact-development-package/verify-result.sh "$attempt"
```

若单独重读本次四文件归档，在原生锁中执行精确树上的消费者。

```bash
bash scripts/collab.sh run --native -- /bin/bash -ec '
  cd "$1/source"
  python3 scripts/record-development-bundle-provenance.py \
    --archive "$1/output/development.zip" \
    --expected-archive-sha256 2fa14871b9e9e715b25dd1f9bc5fcfbfaf67243e99ae8a25a484f4a187ef25c0 \
    --expected-commit d70362ad1fc4b080537163be60cb830849ee4167 --repo "$PWD"
' _ "$attempt"
```
