#!/usr/bin/env bash
# Actual native WebView/production IPC integration. Never used in shipping bundles.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'This lane requires a real macOS GUI session; Linux evidence is separate.' >&2
  exit 2
fi
output="${1:-artifacts/native-smoke}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
export AGENTBATON_SMOKE_COMMIT="$(git rev-parse HEAD)"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
# Caller builds actual frontend and sidecars before this lane, as for production.
test -f apps/desktop/dist/index.html
cargo build --locked --bin batond --bin baton
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --features native-smoke,custom-protocol --bin agentbaton-native-smoke
cargo build --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --features native-smoke,custom-protocol --bin agentbaton-native-smoke
root_target="$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')"
native_target="$(cargo metadata --offline --no-deps --format-version 1 --manifest-path apps/desktop/src-tauri/Cargo.toml | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"]+"/debug")')"
if [[ "$root_target/debug/batond" != "$native_target/batond" ]]; then
  cp "$root_target/debug/batond" "$native_target/batond"
fi
python3 scripts/native-run.py --output "$output" -- "$native_target/agentbaton-native-smoke" "$output"
python3 - "$output/native-smoke.json" <<'PY'
import json,sys
from pathlib import Path
record=json.loads(Path(sys.argv[1]).read_text())
assert record['status']=='passed',record
assert record['os']=='macos',record
assert record['first_webview']['realComposerSubmitted']
assert record['first_webview']['dataContractLists']
assert record['first_webview']['dataContractEmptyControls']
assert record['first_webview']['readonlyDataNoMutation']
assert record['first_webview']['commandContractLists'] is True
assert record['first_webview']['commandContractEmptyControls'] is True
assert record['first_webview']['readonlyCommandNoMutation'] is True
assert record['first_webview']['goalScopeLists'] is True
assert record['first_webview']['goalScopeEmptyControls'] is True
assert record['first_webview']['readonlyGoalNoMutation'] is True
assert record['reopened_webview']['restartPersisted']
assert record['window_close_daemon_survived']
assert record['second_writer_rejected']
assert record['owned_daemon_cleanup']
print('PASS actual macOS WebView, product goal form, restricted Tauri IPC, persisted restart and owned cleanup')
PY
