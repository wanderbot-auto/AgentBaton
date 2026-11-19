#!/usr/bin/env bash
set -euo pipefail

output=$1
mkdir -p "$output"
output="$(cd "$output" && pwd -P)"
export CARGO_NET_OFFLINE=true
platform=aarch64-apple-darwin
metadata() {
    local directory=$1 manifest=$2 report=$3
    shift 3
    (cd "$directory" && cargo metadata --manifest-path "$manifest" --locked --offline \
        --format-version 1 "$@") > "$output/$report"
}
normal_tree() {
    local directory=$1 profile=$2 manifest=$3 target=$4 package=$5
    shift 5
    (cd "$directory" && cargo tree --manifest-path "$manifest" --locked --offline \
        --target "$target" --package "$package" --edges normal,no-proc-macro \
        --prefix none --format '{p}' "$@") > "$output/tree-$profile.log"
}
metadata . Cargo.toml metadata-..json --filter-platform "$platform"
metadata . apps/desktop/src-tauri/Cargo.toml metadata-desktop-release.json \
    --features custom-protocol --filter-platform "$platform"
metadata . crates/remote-host/Cargo.toml metadata-crates_remote-host.json --filter-platform "$platform"
normal_tree . macos_arm64_desktop apps/desktop/src-tauri/Cargo.toml "$platform" agentbaton-desktop --features custom-protocol
normal_tree . macos_arm64_daemon Cargo.toml "$platform" batond
normal_tree . macos_arm64_cli Cargo.toml "$platform" baton
normal_tree . macos_arm64_remote_companion crates/remote-host/Cargo.toml "$platform" agentbaton-remote-host
for target in aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu; do
    metadata . crates/remote-host/Cargo.toml "metadata-remote-$target.json" --filter-platform "$target"
done
normal_tree . linux_arm64_remote_companion crates/remote-host/Cargo.toml aarch64-unknown-linux-gnu agentbaton-remote-host
normal_tree . linux_x64_remote_companion crates/remote-host/Cargo.toml x86_64-unknown-linux-gnu agentbaton-remote-host
smoke=$(mktemp -d "$output/smoke.XXXXXX")
git archive HEAD tools/environment-smoke | tar -x -C "$smoke"
metadata "$smoke" tools/environment-smoke/Cargo.toml metadata-smoke.json --filter-platform "$platform"
normal_tree "$smoke" infra_smoke_tool tools/environment-smoke/Cargo.toml "$platform" agentbaton-environment-smoke

historical=df35231eab29aa2d4a043fd9be60888504507716
historical_source=$(mktemp -d "$output/historical.XXXXXX")
test "$(git rev-parse "$historical^{tree}")" = ddd5bddbeeb8011fa070d53e67f7978deae87214
git archive "$historical" Cargo.toml Cargo.lock crates apps/cli apps/daemon apps/desktop/src-tauri \
    | tar -x -C "$historical_source"

for mode in current historical; do
    source_root=.
    if [ "$mode" = historical ]; then
        source_root="$historical_source"
    fi
    (
        cd "$source_root"
        for dep in cssparser@0.37.0 dtoa-short@0.3.5 option-ext@0.2.0 selectors@0.38.0; do
            for edges in normal,no-proc-macro normal,build; do
                printf 'dependency=%s edges=%s\n' "$dep" "$edges"
                cargo tree --manifest-path apps/desktop/src-tauri/Cargo.toml \
                    --locked --offline --features custom-protocol --target "$platform" \
                    --edges "$edges" --invert "$dep" --charset ascii
            done
        done
    ) > "$output/mpl-$mode.log" 2>&1
done
