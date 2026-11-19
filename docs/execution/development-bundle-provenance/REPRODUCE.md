# ARM64 开发包归属复验

## 历史开发包

使用 Apple Silicon macOS 和 Python 3.11 或更新版本。工件摘要须由独立的 GitHub Actions 或经批准的构建记录提供。先取回历史精确 Git 合成对象。检查过程中挂载为只读，程序只读包内文件及本机签名工具的检查结果。

~~~sh
git fetch --no-tags origin df35231eab29aa2d4a043fd9be60888504507716
scripts/collab.sh run -- bash -euo pipefail -s <<'SH'
work=$(mktemp -d "$BATON_RUNTIME_DIR/historical.XXXXXX")
gh api repos/wanderbot-auto/AgentBaton/actions/artifacts/14350183579/zip > "$work/development.zip"
python3 scripts/record-development-bundle-provenance.py \
    --archive "$work/development.zip" \
    --expected-archive-sha256 cbed165a62710d1d302f3ab28cc3e6e12165d270f9c14ff858d6e93ac00b99e0 \
    --expected-commit df35231eab29aa2d4a043fd9be60888504507716 > "$work/report.json"
SH
~~~

程序验证 ZIP 内固定成员、DMG SHA-256、与 Git 提交一致的验收清单、Bundle ID 和版本，以及三个薄 ARM64 Mach-O。报告列出每个普通文件的 SHA-256 和来源证据。图标直接按 Git 原件字节匹配。应用工具链或第三方组件的完整归属仍需实际构建收据。签名类型来自 codesign 显示，严格校验的退出结果单列。发布者信任、Gatekeeper、实际运行测试及公证各有独立证据范围。 [GitHub 历史工件](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44122458362/artifacts/14350183579) · [Apple 签名工具说明](https://developer.apple.com/documentation/Xcode/using-the-latest-code-signature-format)

## 收据格式及边界

使用额外的 --receipt 和 --receipt-sha256 参数。预期收据摘要必须来自可信、独立保存的构建记录。报告的 bound_receipt 只表示传入的特定回执经过摘要、目标身份和文件字节核对，整体归属完整性保留 unknown。

- 收据版本 1 固定 ZIP、DMG、源码提交、源码树和 aarch64-apple-darwin。每条绑定还固定包内路径、成品 SHA-256、旁证文件与 SHA-256。
- linker 项读取 Apple ARM64 链接映射并确认映射的输出名称、对象与符号记录。linked_output 的原件字节必须与包内成品相同。
- web 项读取构建器的最终 chunk 模块记录，并确认模块记录中的 chunk 摘要。独立 Web 文件须完整匹配。内嵌 Web 当前要求在经检验的包内文件偏移处按原始字节匹配。历史 Tauri 工件缺这类收据，归属仍为 unknown。
- 对象路径只投影文件名。模块名是构建器记录的相对标识。压缩的内嵌资产、死代码、专有组件许可证义务和完整静态链接分配由后续独立收据覆盖。 [Apple Mach-O 记录](https://github.com/apple-oss-distributions/xnu/blob/main/EXTERNAL_HEADERS/mach-o/loader.h) · [构建器模块证据](https://rollupjs.org/plugin-development/#generatebundle)

## 原生正反例

以下命令用已安装的锁定 Vite 8.3.5 和本机 Apple Clang。示例中的 vite_dir 为安装目录，history_zip 为上述历史档案。夹具以私有生成 Git 仓库作为来源，只有其单独构建出的原生与 Web 数据被匹配。夹具脚本不启动实际应用。

~~~sh
python3 -m unittest discover -s tests/infrastructure -p test_record_development_bundle_provenance.py -v
scripts/collab.sh run -- bash -euo pipefail -s -- "$vite_dir" "$history_zip" <<'SH'
dir=docs/execution/development-bundle-provenance
base=$(mktemp -d "$BATON_RUNTIME_DIR/bundle.XXXXXX")
bash "$dir/replay-receipts.sh" "$base/fixture" "$1"
bash "$dir/replay-native-negatives.sh" "$base/fixture" "$2" "$base/negative"
SH
~~~

反例涵盖原 ZIP 损坏、另一个提交的构建记录、真实夹具收据套用到历史包，以及 ARM64 包中夹入一个实际 x86_64 Mach-O。该 x86_64 文件只是拒绝夹具，例程仅在 Apple Silicon 执行。 [历史报告](observations/historical-report.json) · [真实夹具报告](observations/fixture-report.json) · [原生反例](observations/native-negatives.log)
