# Product development

Read the current execution gate, approval and [status](execution/STATUS.md) first. The complete goal remains Design v0.2; current code is partial implementation. Provider model calls, personal identities, production release, formal signing and paid resources remain separately gated.

Use Node 24, Python 3.12 and Rust stable with rustfmt/clippy. Dependency versions actually resolved are in the Rust/Node lockfiles. No final project license is selected.

From the repository root:

```sh
python3 scripts/check_gate.py --scope product
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --locked --bin batond --bin baton
python3 scripts/smoke-local-product.py
```

From `apps/desktop`:

```sh
npm ci --ignore-scripts --no-audit --no-fund
npm run typecheck
npm test
npm run build
```

UI tests mock the transport and do not prove native IPC. The separate process smoke test exercises actual daemon/CLI IPC and SQLite restart persistence. Native host tests and development DMG packaging run on Apple Silicon (ARM64) in Product validation CI. Intel compatibility is deferred. Existing Environment checks remain independent.

## Desktop development shortcuts

On Apple Silicon macOS with Rust, Node, Python 3 and Xcode command-line tools installed, run from the repository root (the script can also be invoked by absolute path from any directory):

```sh
bash scripts/desktop-dev.sh start    # 后台启动：安装依赖（如需要）、构建、启动真实桌面预览
bash scripts/desktop-dev.sh status   # 当前阶段、预览地址、独立数据目录和日志位置
bash scripts/desktop-dev.sh logs     # 查看末尾 80 行并持续跟踪；Ctrl-C 只退出日志
bash scripts/desktop-dev.sh stop     # 停止本次开发 UI、watcher、Vite 和开发 daemon
bash scripts/desktop-dev.sh restart  # 停止后重新构建并启动，保留数据和编译缓存
bash scripts/desktop-dev.sh dev      # 前台开发模式；Ctrl-C 清理本次会话
bash scripts/desktop-dev.sh setup    # 提前准备锁定的前端依赖
```

`start` returns after the supervisor responds; the first Rust compilation continues in the background and may take minutes. The reported phase `dev (building or running; see log)` is not proof that the window has opened. Follow `logs` for Cargo/Vite errors. Repeated `start` is idempotent. An occupied port fails visibly; the helper never kills its owner. Each checkout/worktree gets its own default port, runtime and target directory. Set `BATON_DEV_PORT=1420` for a fixed port. Closing the window follows product behavior and may leave the daemon running; `stop` explicitly stops the helper's complete development session and can interrupt active development tasks. It does not stop an installed application's daemon.

Logs are appended to ignored `artifacts/desktop-dev/dev.log`. The private runtime lives under `/tmp/ab-dev-<uid>-<checkout-hash>/runtime` (canonical `/private/tmp` on macOS); it survives stop/restart but OS temporary-file cleanup can remove it. For persistent development data, explicitly set `BATON_RUNTIME_DIR` to a short, private, canonical absolute path, separate from an installed application's runtime. Keep this setting consistent across launches. The helper refuses a second daemon when that runtime is already owned rather than stopping another process. No global native config or authentication directory is changed or copied.

### Faster edit/preview cycles

- React/CSS changes use Vite HMR in the running native window. Leave the session running while editing; do not restart for every UI change. `npm run dev` alone provides a browser view, but product IPC requires the native window; it has no demo/HTTP backend.
- The helper uses one `CARGO_TARGET_DIR=<checkout>/target` for the otherwise separate daemon and Tauri Cargo workspaces. Matching native dev overrides retain the core's runtime-sensitive optimization levels while allowing compatible dependencies to be reused. Keep build directories separate across worktrees. Two lockfiles/features can still cause some distinct builds.
- The helper defaults to `CARGO_PROFILE_DEV_DEBUG=1` and incremental builds. This retains file/line backtraces with less debug information. It leaves release profiles and safety assertions unchanged. For full debugger variable inspection, use `CARGO_PROFILE_DEV_DEBUG=2 bash scripts/desktop-dev.sh restart`; changing build flags causes a one-time rebuild. Explicit environment settings are respected. See [Cargo build performance](https://doc.rust-lang.org/cargo/guide/build-performance.html).
- Frontend dependencies are installed only when missing or when the lockfile hash changes. Sidecars are replaced only when bytes change, avoiding needless file watcher rebuilds. Ordinary restart does not run `cargo clean`, a release build, frontend production build, packaging or signing.
- If other work competes for CPU/RAM, try `CARGO_BUILD_JOBS=4 bash scripts/desktop-dev.sh restart`. Choose the value for your machine; more jobs are not always faster. To diagnose build stages, use `cargo build --locked --bin batond --bin baton --timings` with the same Cargo environment. Cold builds and cache hits should be measured separately; no fixed speedup is promised.

The original `npm run tauri -- dev` in `apps/desktop` is also supported: `beforeDevCommand` now builds and prepares **debug** sidecars, fixing `resource path binaries/batond-aarch64-apple-darwin doesn't exist`. This lower-level command uses its normal Cargo configuration, fixed Vite port 1420 and platform runtime; the script is the recommended way to manage all development processes. The prepare step supplies a sibling daemon so the native host can start it when needed; a separately started daemon must use the same runtime. The lower-level flow retains normal product background lifetime behavior on UI exit.

Verify helper lifecycle regressions with `python3 scripts/test-desktop-dev.py`. These tests use fake build/UI commands and do not prove native product behavior.

The UI communicates only through host commands. Renderer input has no generic execute, SQL or filesystem API. Connection executable proposals require a separate native owner confirmation bound to the server snapshot before any version probe; session execution requires its own bounded grant. A missing daemon or unsupported capability is an explicit error; no demo backend substitutes for it.

Both processes must use the same host-selected runtime. Default: macOS `~/Library/Application Support/dev.agentbaton/runtime`, Linux `${XDG_DATA_HOME:-$HOME/.local/share}/agentbaton/runtime`. A trusted developer can set `BATON_RUNTIME_DIR` to a new private canonical absolute path. Existing permissive directories, symlinks and path traversal are rejected rather than chmodded. The daemon owns an exclusive writer lock and private rotating local token. Do not copy credentials or commit runtime state.

For a macOS development bundle, from the repository root run:

```sh
cargo build --release --locked --bin batond --bin baton
bash scripts/prepare-desktop-sidecars.sh
cd apps/desktop
npm run tauri -- build --bundles app
cd ../..
bash scripts/package-development-macos.sh
```

Build records, signature inspection, DMG and SHA-256 are written under ignored `artifacts/development`. No Developer ID or notarization is configured. See the [manual acceptance checklist](execution/DEVELOPMENT_ACCEPTANCE.md); a build is not a passed desktop acceptance test. The configured macOS deployment target is provisional, not evidence of the minimum supported system.

For Linux compile-only native checks, install the distribution GTK3/WebKitGTK4.1, AppIndicator, SVG, xdo and OpenSSL development packages. In the current Debian13 managed Cloud (no root/sudo), verified packages were downloaded with private APT state/cache and the system Debian archive keyring, then extracted using dpkg-deb into `/workspace/.toolchains/linux-ui/sysroot` without maintainer scripts or system package writes. This is a local build overlay, not an OS sandbox.

Set `PKG_CONFIG_SYSROOT_DIR` to that private sysroot and `PKG_CONFIG_LIBDIR` to its `usr/lib/x86_64-linux-gnu/pkgconfig`, `usr/share/pkgconfig` and the existing system pkgconfig directories. Use its library directory in task-local `LD_LIBRARY_PATH`/`LIBRARY_PATH`. Private dev-library symlinks whose runtime target was already installed required links to those existing system runtimes; the initial missing-target linker failure is retained in session-supervision evidence. Build real sidecars and frontend assets first, then run `cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml` and `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml`. Both passed here; seven host tests do not open a native window and do not establish macOS acceptance. Official package signatures/TLS/proxy trust were preserved.
