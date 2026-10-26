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

For an interactive native development session, start `target/debug/batond` from the repository root, then run `npm run tauri -- dev` from `apps/desktop`. The UI communicates only through host commands. Renderer input does not select an executable, arbitrary SQL or file paths. A missing daemon or unsupported capability is an explicit error; no demo backend substitutes for it.

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
