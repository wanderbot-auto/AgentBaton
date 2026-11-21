#!/usr/bin/env bash
set -euo pipefail
umask 077

commit=d70362ad1fc4b080537163be60cb830849ee4167
tree=19aba6e8f208f021bc84573dd8b496d29fce5633
origin="$(pwd -P)"
test "$(uname -sm)" = 'Darwin arm64'
test "$(git rev-parse "$commit" "$commit^{tree}")" = "$(printf '%s\n%s' "$commit" "$tree")"
attempt="$(mktemp -d "$BATON_RUNTIME_DIR/pr33-package.XXXXXX")"
mkdir "$attempt/tmp" "$attempt/output" "$attempt/locks" "$attempt/npm-logs"
: > "$attempt/npm-user.config"
: > "$attempt/npm-global.config"
printf 'attempt=%s\ncommit=%s\ntree=%s\n' "$attempt" "$commit" "$tree"

status=0
env -i PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"     TMPDIR="$attempt/tmp/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"     CARGO_NET_OFFLINE=true GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null     npm_config_cache="$HOME/.npm" npm_config_offline=true     npm_config_userconfig="$attempt/npm-user.config" npm_config_globalconfig="$attempt/npm-global.config"     npm_config_logs_dir="$attempt/npm-logs" LC_ALL=C     /bin/bash -euo pipefail -s -- "$attempt" "$origin" "$commit" "$tree"     > "$attempt/build.log" 2>&1 <<'BUILD' || status=$?
attempt=$1
origin=$2
commit=$3
tree=$4
printf 'stage=checkout utc=%s\n' "$(date -u +%FT%TZ)"
git -c core.hooksPath=/dev/null clone --shared --no-checkout --quiet "$origin" "$attempt/source"
cd "$attempt/source"
git -c core.hooksPath=/dev/null checkout --detach --quiet "$commit"
test "$(git rev-parse HEAD 'HEAD^{tree}')" = "$(printf '%s\n%s' "$commit" "$tree")"
test -z "$(git status --porcelain=v1 --untracked-files=all)"
locks=(Cargo.lock apps/desktop/src-tauri/Cargo.lock apps/desktop/package-lock.json)
for path in "${locks[@]}"; do
  mkdir -p "$attempt/locks/$(dirname "$path")"
  git cat-file blob "$commit:$path" > "$attempt/locks/$path"
  cmp "$attempt/locks/$path" "$path"
done
shasum -a 256 "${locks[@]}" > "$attempt/locks-before.sha256"

check_detached() {
  hdiutil info -plist | python3 -c '
import plistlib
import sys

images = plistlib.loads(sys.stdin.buffer.read())["images"]
owned = [row.get("image-path", "") for row in images
         if row.get("image-path", "").startswith(sys.argv[1] + "/")]
if owned:
    sys.exit("Task-owned image still attached: " + ", ".join(owned))
print("PASS no task-owned disk image attachment")
' "$attempt"
}

finish() {
  local status=$? checks=0
  trap - EXIT
  shasum -a 256 "${locks[@]}" > "$attempt/locks-after.sha256" || checks=1
  git diff --no-ext-diff "$commit" -- "${locks[@]}" > "$attempt/lock-drift.patch" || checks=1
  git status --porcelain=v1 --untracked-files=all > "$attempt/source-status.txt" || checks=1
  cmp "$attempt/locks-before.sha256" "$attempt/locks-after.sha256" || checks=1
  test ! -s "$attempt/lock-drift.patch" || checks=1
  test ! -s "$attempt/source-status.txt" || checks=1
  check_detached || checks=1
  printf 'stage=final_checks status=%s utc=%s\n' "$checks" "$(date -u +%FT%TZ)"
  if [[ "$checks" == 0 ]]; then
    echo 'PASS clean checkout and unchanged exact Git lockfile bytes'
  fi
  if [[ "$status" == 0 ]]; then
    status="$checks"
  fi
  exit "$status"
}
trap finish EXIT

printf 'commit=%s\ntree=%s\n' "$commit" "$tree"
uname -sm
sw_vers
rustc -vV
cargo -V
node --version
npm --version
python3 --version
xcodebuild -version

printf 'stage=npm_ci utc=%s\n' "$(date -u +%FT%TZ)"
(cd apps/desktop && npm ci --ignore-scripts --no-audit --no-fund)
node - <<'JS'
const lock = require("./apps/desktop/package-lock.json");
for (const name of ["@tauri-apps/cli", "vite"]) {
  const version = require("./apps/desktop/node_modules/" + name + "/package.json").version;
  console.log(name + "=" + version);
  if (version !== lock.packages["node_modules/" + name].version) process.exitCode = 1;
}
JS
(cd apps/desktop && npm run tauri -- build --help) > "$attempt/tauri-help.log"
awk '/^      --no-sign$/{ found=1; print; getline; print } END { exit !found }' "$attempt/tauri-help.log"
awk '/^Usage:.*\[ARGS\]/{ found=1; print } END { exit !found }' "$attempt/tauri-help.log"

printf 'stage=root_release_sidecars utc=%s\n' "$(date -u +%FT%TZ)"
cargo build --offline --release --locked --bin batond --bin baton
bash scripts/prepare-desktop-sidecars.sh

printf 'stage=tauri_default_bundle utc=%s\n' "$(date -u +%FT%TZ)"
(cd apps/desktop && npm run tauri -- build --bundles app --no-sign -- --locked --offline)
test "$(git rev-parse HEAD 'HEAD^{tree}')" = "$(printf '%s\n%s' "$commit" "$tree")"
for path in "${locks[@]}"; do
  cmp "$attempt/locks/$path" "$path"
done
test -z "$(git status --porcelain=v1 --untracked-files=all)"

desktop_release=apps/desktop/src-tauri/target/release
app="$desktop_release/bundle/macos/AgentBaton.app"
for role in agentbaton-desktop baton batond; do
  original="target/release/$role"
  if [[ "$role" == agentbaton-desktop ]]; then
    original="$desktop_release/$role"
  fi
  test -f "$original" && test -x "$original" && test ! -L "$original"
  cmp "$original" "$app/Contents/MacOS/$role"
  digest="$(shasum -a 256 < "$original")"
  printf '%s  %s\n' "${digest%% *}" "$role"
done > "$attempt/output/compiled-sha256"
cat "$attempt/output/compiled-sha256"

printf 'stage=development_dmg utc=%s\n' "$(date -u +%FT%TZ)"
bash scripts/native-audit-release.sh "$app/Contents/MacOS/agentbaton-desktop"
bash scripts/package-development-macos.sh
check_detached
test -z "$(find artifacts -maxdepth 1 -name '.development.*' -print)"
artifact="$PWD/artifacts/development"
(cd "$artifact" && shasum -a 256 -c SHA256SUMS)

printf 'stage=archive_and_readonly_consumer utc=%s\n' "$(date -u +%FT%TZ)"
python3 - "$artifact" "$attempt/output" "$commit" <<'PY'
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
import sys

artifact, output = map(Path, sys.argv[1:3])
dmg = f"AgentBaton-arm64-{sys.argv[3]}-development.dmg"
members = [dmg, "build-record.txt", "DEVELOPMENT_ACCEPTANCE.md", "SHA256SUMS"]
assert set(members) == {path.name for path in artifact.iterdir()}
with ZipFile(output / "development.zip", "x", compression=ZIP_DEFLATED) as archive:
    for name in members:
        path = artifact / name
        assert path.is_file() and not path.is_symlink()
        archive.write(path, name)
PY
archive_sha="$(shasum -a 256 < "$attempt/output/development.zip" | cut -d ' ' -f 1)"
printf '%s  development.zip\n' "$archive_sha" | tee "$attempt/output/archive.sha256"
python3 scripts/record-development-bundle-provenance.py \
    --archive "$attempt/output/development.zip" --expected-archive-sha256 "$archive_sha" \
    --expected-commit "$commit" --repo "$PWD" > "$attempt/output/provenance.json"

python3 - "$attempt/output" "$commit" "$tree" <<'PY'
from pathlib import Path
import json
import sys

output = Path(sys.argv[1])
report = json.loads((output / "provenance.json").read_bytes())
expected = {role: digest for digest, role in (
    line.split("  ") for line in (output / "compiled-sha256").read_text().splitlines())}
observed = {Path(row["path"]).name: row["sha256"] for row in report["files"]
            if row["path"].startswith("AgentBaton.app/Contents/MacOS/")}
assert observed == expected
assert report["identity"]["source_commit"] == sys.argv[2]
assert report["identity"]["source_tree"] == sys.argv[3]
assert report["inspection"] == "macos_arm64_read_only_dmg_detached"
print("PASS mounted archive executables match three original compiled SHA-256s")
print("dmg_sha256=" + report["artifact"]["sha256"])
PY
check_detached
for role in agentbaton-desktop baton batond; do
  original="target/release/$role"
  if [[ "$role" == agentbaton-desktop ]]; then
    original="$desktop_release/$role"
  fi
  digest="$(shasum -a 256 < "$original")"
  printf '%s  %s\n' "${digest%% *}" "$role"
done | cmp "$attempt/output/compiled-sha256" -
printf 'stage=complete utc=%s\n' "$(date -u +%FT%TZ)"
BUILD
printf '%s\n' "$status" > "$attempt/build.exit"
printf 'build_exit=%s log=%s\n' "$status" "$attempt/build.log"
tail -n 24 "$attempt/build.log"
exit "$status"
