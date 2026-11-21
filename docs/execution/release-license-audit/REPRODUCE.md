# 复验说明

在审计分支的仓库根目录执行。工作基线为 97803b16276903ffe4b6945966f21adeaad11eeb。审计脚本用 Cargo 1.91.1、Python 3.14.0a7、macOS ARM64 26.6.1 执行。Python 标准库使用 tomllib，运行需 Python 3.11 或更新版本。图文件的本地目录被替换成 current 或 historical 标签，版本和边保持原样。

## 核对已有证据

~~~sh
audit_dir=docs/execution/release-license-audit
python3 "$audit_dir/inventory.py" verify "$audit_dir/LOCK-INVENTORY.json"
git diff --check
scripts/collab.sh scope
~~~

LOCK-INVENTORY.json 逐项记录来源、版本、校验和、原始清单声明以及摘要。OFFICIAL-NOTICES.tar.gz 按原文 SHA-256 保存 LICENSE、COPYRIGHT、NOTICE 等文本。按 inventory 中的原始路径和摘要建立映射，可用下列命令读取单项。对 11 个发行归档缺少独立声明文本的运行候选，另保存精确上游提交的 Dropbox 或 objc2 文件。原始许可表达式原样记录。

~~~sh
notice_sha=填入_LOCK_INVENTORY_中某一_original_notices_SHA256
tar -xOf "$audit_dir/OFFICIAL-NOTICES.tar.gz" "sha256/$notice_sha"
~~~

upstream/UPSTREAM-SOURCES.json 记录每个外部请求的公开 URL、成功摘要或访问错误。官方服务网页保留 URL、当次可取得的校验和和审阅结论。外部协议服务条款可能变化，分发前应重新确认账户地区、产品条款和适用合同。

## 重建锁清单和图

先取得历史构建合成提交。Cargo 与 npm 缓存路径应指向本机可读的实际 registry 缓存目录。缓存缺失时，收集器通过官方公开 registry 读取精确归档并核对锁摘要。此过程无需安装依赖，也不会运行包脚本。

~~~sh
git fetch --no-tags origin df35231eab29aa2d4a043fd9be60888504507716
scripts/collab.sh run -- bash -euo pipefail -s <<'SH'
audit_dir=docs/execution/release-license-audit
proof="$BATON_RUNTIME_DIR/license-replay"
cargo_cache=$(find "$HOME/.cargo/registry/cache" -maxdepth 1 -type d -name 'index.crates.io-*' -print | head -1)
bash "$audit_dir/replay-graphs.sh" "$proof"
python3 "$audit_dir/inventory.py" collect \
  --commit 97803b16276903ffe4b6945966f21adeaad11eeb \
  --runtime "$proof" --cargo-cache "$cargo_cache" --npm-cache "$HOME/.npm/_cacache" \
  --output "$proof/inventory"
python3 "$audit_dir/inventory.py" verify "$proof/inventory/LOCK-INVENTORY.json"
SH
~~~

在初次取证中，系统 Python 的证书信任设置让部分公开 registry 请求失败。使用保持 TLS 验证的 curl 后，所有锁中可识别的第三方发布包都通过摘要验证。环境烟测从准确 Git 归档解析，因为直接在嵌套工作树调用时 Cargo 错把外层仓库识别成工作区。

## 核对历史开发工件

以 GitHub 的固定 artifact ID 下载 ZIP，核对 ZIP 摘要和内部 SHA256SUMS。只读挂载可核对目录、Mach-O 架构和系统动态依赖。构建日志完整 SHA-256 为 a578b7229fc2cba4712768711dedeee18ba532e0d79a28841cce534738ab1bdb。仓库中保留使用行号的实际构建摘录，原始 ZIP、DMG 和日志保存在本任务私有运行目录。 [Actions 工件](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44122458362/artifacts/14350183579)

~~~sh
scripts/collab.sh run -- bash -euo pipefail -s <<'SH'
proof=$(mktemp -d "$BATON_RUNTIME_DIR/dmg.XXXXXX")
mkdir "$proof/unpacked" "$proof/volume"
gh api repos/wanderbot-auto/AgentBaton/actions/artifacts/14350183579/zip > "$proof/development.zip"
(cd "$proof" && printf '%s  %s\n' cbed165a62710d1d302f3ab28cc3e6e12165d270f9c14ff858d6e93ac00b99e0 development.zip | shasum -a 256 -c)
unzip -q "$proof/development.zip" -d "$proof/unpacked"
(cd "$proof/unpacked" && shasum -a 256 -c SHA256SUMS)
dmg=$(find "$proof/unpacked" -type f -name '*.dmg' -print)
hdiutil attach -readonly -noautoopen -nobrowse -mountpoint "$proof/volume" "$dmg"
trap 'hdiutil detach "$proof/volume"' EXIT
(cd "$proof/volume" && find -s . -type f -exec shasum -a 256 {} \;)
for bin in "$proof/volume"/AgentBaton.app/Contents/MacOS/*; do file "$bin"; otool -L "$bin"; done
SH
~~~
