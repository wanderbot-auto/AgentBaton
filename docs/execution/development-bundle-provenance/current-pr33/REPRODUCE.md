# PR #33 本机复验

构建对象是合成提交 14530c62a0b532e2ba43ca4be4fbc75ccc41d856，树 41b3e2fe076ee4ebcb6355dddc1505f6bb75e135。来源工具固定于 c4accb5dc0901aa219a31dfe9fbe31cc0c241450。采用本机 ARM64、macOS 26.6.1 (25G76)、Xcode 27.2 (27B52)、Rust 1.91.1、Node 26.7.0、npm 11.19.0、Python 3.15.2、Tauri CLI 2.12.4 和 Vite 8.3.5。

现行托管 Product / macos-15 使用 Node 24 和滚动 Rust stable。取证时同一 PR 的托管 macOS 作业仍在 Core/native host 测试，实时日志 API 返回 404。该作业的确切补丁版本留待其日志核对。历史已完成作业 134177203395 的记录为 Node 24.22.2、Rust 1.100.0、Xcode 16.4。 [本次托管作业](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44432204086/job/135252128867) · [历史构建记录](../../release-license-audit/artifact/build-record.txt)

下列主步骤在独立 checkout 和普通 collab run 中执行。构建子进程由 env -i 启动，只注入工具路径、任务临时目录、离线缓存配置及固定 locale。npm 使用两份不同的私有空配置。Tauri 显式采用 --no-sign，-- 后的 --locked 交给 Cargo runner。Tauri Cargo.lock 构建前后字节一致。 [执行脚本原件](logs/build-locked.sh)

~~~sh
(cd apps/desktop && npm ci --ignore-scripts --no-audit --no-fund)
cargo build --release --locked --bin batond --bin baton
bash scripts/prepare-desktop-sidecars.sh
(cd apps/desktop && npm run tauri -- build --bundles app --no-sign -- --locked)
bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop
bash scripts/package-development-macos.sh
~~~

本机归档保留原 DMG、验收清单、build record 和 SHA256SUMS。使用独立记录的 ZIP SHA-256 运行只读复验。程序不会启动包内 App。

~~~sh
python3 scripts/record-development-bundle-provenance.py \
  --archive /private/tmp/ab-collab-0dgnp8ti/ci-14530c62/locked-build/output/development.zip \
  --expected-archive-sha256 112323195860008afb10912ed6fb92ee4f9e32c5b549eef36ed7a76d44e56649 \
  --expected-commit 14530c62a0b532e2ba43ca4be4fbc75ccc41d856 \
  --repo /private/tmp/ab-collab-0dgnp8ti/ci-14530c62/locked-build/source
~~~

[锁定复跑原日志](logs/locked-build.log.gz) · [五文件来源报告](logs/provenance.json) · [未锁定轮次原始锁差异](logs/tauri-lock-drift.patch.gz)
