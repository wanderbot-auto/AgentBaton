#!/usr/bin/env bash
set -euo pipefail
umask 077

commit=14530c62a0b532e2ba43ca4be4fbc75ccc41d856
tree=41b3e2fe076ee4ebcb6355dddc1505f6bb75e135
origin="$(pwd -P)"
runtime="$BATON_RUNTIME_DIR/ci-14530c62"
attempt="$runtime/locked-build"
mkdir "$attempt"
printf '%s\n' "$$" > "$attempt/build.pid"
printf 'pid=%s log=%s commit=%s tree=%s\n' "$$" "$attempt/build.log" "$commit" "$tree"
exec > >(tee "$attempt/build.log") 2>&1
date -u '+start=%Y-%m-%dT%H:%M:%SZ'
git clone --shared --no-checkout --quiet "$origin" "$attempt/source"
git -C "$attempt/source" -c advice.detachedHead=false checkout --detach --quiet "$commit"
test "$(git -C "$attempt/source" rev-parse HEAD HEAD^{tree})" = "$(printf '%s\n%s' "$commit" "$tree")"
test -z "$(git -C "$attempt/source" status --porcelain=v1 --untracked-files=all)"
install -d -m 700 "$attempt/output"
: > "$attempt/npm-users.config"
: > "$attempt/npm-global.config"
tool_path="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
set +e
env -i PATH="$tool_path" TMPDIR="$runtime/tmp/" \
    CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" CARGO_NET_OFFLINE=true \
    npm_config_cache="$HOME/.npm" npm_config_offline=true \
    npm_config_userconfig="$attempt/npm-users.config" npm_config_globalconfig="$attempt/npm-global.config" \
    LC_ALL=C /bin/bash -euo pipefail -s -- "$runtime" "$attempt" "$origin" "$commit" "$tree" <<'BUILD'
runtime=$1
attempt=$2
recording_root=$3
commit=$4
tree=$5
cd "$attempt/source"
test "$(uname -sm)" = "Darwin arm64"
sw_vers
rustc -Vv
node --version
npm --version
python3 --version
xcodebuild -version
printf 'stage=npm_ci utc=%s\n' "$(date -u +%FT%TZ)"
(cd apps/desktop && npm ci --ignore-scripts --no-audit --no-fund)
node -e 'const p=require("./apps/desktop/node_modules/@tauri-apps/cli/package.json"); console.log("Tauri CLI "+p.version); if(p.version!=="2.12.4")process.exit(1)'
(cd apps/desktop && npm run tauri -- build --help) > "$attempt/tauri-help.log"
awk '/^      --no-sign$/{ found=1; print; getline; print } END { exit !found }' "$attempt/tauri-help.log"
awk '/^Usage:.*\[ARGS\]/{ found=1; print } END { exit !found }' "$attempt/tauri-help.log"
shasum -a 256 apps/desktop/src-tauri/Cargo.lock > "$attempt/tauri-lock-before.sha256"
printf 'stage=root_release_sidecars utc=%s\n' "$(date -u +%FT%TZ)"
cargo build --release --locked --bin batond --bin baton
bash scripts/prepare-desktop-sidecars.sh
printf 'stage=tauri_default_bundle utc=%s\n' "$(date -u +%FT%TZ)"
(cd apps/desktop && npm run tauri -- build --bundles app --no-sign -- --locked)
printf 'stage=default_release_marker_audit utc=%s\n' "$(date -u +%FT%TZ)"
bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop
printf 'stage=development_dmg utc=%s\n' "$(date -u +%FT%TZ)"
bash scripts/package-development-macos.sh
test "$(git rev-parse HEAD HEAD^{tree})" = "$(printf '%s\n%s' "$commit" "$tree")"
shasum -a 256 apps/desktop/src-tauri/Cargo.lock > "$attempt/tauri-lock-after.sha256"
cmp "$attempt/tauri-lock-before.sha256" "$attempt/tauri-lock-after.sha256"
test -z "$(git status --porcelain=v1 --untracked-files=no)"
artifact="$PWD/artifacts/development"
(
    cd "$artifact"
    shasum -a 256 -c SHA256SUMS
)
printf 'stage=local_archive_and_readonly_report utc=%s\n' "$(date -u +%FT%TZ)"
python3 - "$artifact" "$attempt/output" "$commit" <<'PY'
import pathlib, sys, zipfile
artifact, out = map(pathlib.Path, sys.argv[1:3])
commit = sys.argv[3]
names = [
    f"AgentBaton-arm64-{commit}-development.dmg",
    "DEVELOPMENT_ACCEPTANCE.md", "build-record.txt", "SHA256SUMS",
]
assert {file.name for file in artifact.iterdir()} == set(names)
with zipfile.ZipFile(out/"development.zip", "x", compression=zipfile.ZIP_DEFLATED) as archive:
    for name in names:
        archive.write(artifact/name, name)
PY
archive_sha=$(shasum -a 256 "$attempt/output/development.zip" | awk '{print $1}')
printf '%s  development.zip\n' "$archive_sha" > "$attempt/output/archive.sha256"
python3 "$recording_root/scripts/record-development-bundle-provenance.py" \
    --archive "$attempt/output/development.zip" --expected-archive-sha256 "$archive_sha" \
    --expected-commit "$commit" --repo "$attempt/source" > "$attempt/output/provenance.json"
printf 'stage=complete utc=%s\n' "$(date -u +%FT%TZ)"
BUILD
status=$?
set -e
printf '%s\n' "$status" > "$attempt/build.exit"
printf 'build_exit=%s utc=%s\n' "$status" "$(date -u +%FT%TZ)"
exit "$status"
