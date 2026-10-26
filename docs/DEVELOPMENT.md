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

UI tests mock the transport and do not prove native IPC. The separate process smoke test exercises actual daemon/CLI IPC and SQLite restart persistence. Native host tests and development DMG packaging run on both macOS architectures in Product validation CI. Existing Environment checks remain independent.

For an interactive native development session, start `target/debug/batond` from the repository root, then run `npm run tauri -- dev` from `apps/desktop`. The UI communicates only through host commands. Renderer input has no generic execute, SQL or filesystem API. Connection executable proposals require a separate native owner confirmation bound to the server snapshot before any version probe; session execution requires its own bounded grant. A missing daemon or unsupported capability is an explicit error; no demo backend substitutes for it.

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
