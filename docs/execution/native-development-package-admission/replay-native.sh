#!/usr/bin/env bash
# Run through scripts/collab.sh run --native with the locked build checkout
# and the verified historical development DMG as read-only inputs.
set -euo pipefail
umask 077
test "$(uname -sm)" = "Darwin arm64"
origin="$(pwd -P)"
fixture="$1"
historical="$2"
tested="$(git rev-parse HEAD)"
printf 'package_source=%s\npackage_tree=%s\n' "$tested" "$(git rev-parse 'HEAD^{tree}')"
sw_vers
rustc -vV
node --version
xcodebuild -version

test "$(git -C "$fixture" rev-parse HEAD 'HEAD^{tree}')" = "$(printf '%s\n%s' \
  14530c62a0b532e2ba43ca4be4fbc75ccc41d856 41b3e2fe076ee4ebcb6355dddc1505f6bb75e135)"
desktop="$fixture/apps/desktop/src-tauri/target/release"
test "$(shasum -a 256 < "$historical" | cut -d ' ' -f 1)" = e6373b07b5d76cb1d329b455b6a1dfc41595735be02416b2aeea974e740df03f

scratch="$(mktemp -d "$BATON_RUNTIME_DIR/package-replay.XXXXXX")"
printf 'replay_dir=%s\n' "$scratch"
mkdir "$scratch/reference-volume"
reference_volume_attached=false
detach_reference() {
  if "$reference_volume_attached"; then
    hdiutil detach "$scratch/reference-volume"
  fi
}
trap detach_reference EXIT
reference_volume_attached=true
hdiutil attach -readonly -noautoopen -nobrowse -mountpoint "$scratch/reference-volume" "$historical"
cp "$scratch/reference-volume/AgentBaton.app/Contents/MacOS/batond" "$scratch/historical-batond"
test "$(shasum -a 256 < "$scratch/historical-batond" | cut -d ' ' -f 1)" = a3ad712dfceb6fa9ce4cce3f1266e74d4663faa1e33af690ab9278a57792e109
detach_reference
reference_volume_attached=false
rmdir "$scratch/reference-volume"

git clone --quiet --shared --no-checkout "$origin" "$scratch/source"
cd "$scratch/source"
git checkout --quiet --detach "$tested"
# Use the workflow's default Cargo target layout inside this private checkout.
unset CARGO_TARGET_DIR
release="apps/desktop/src-tauri/target/release"
app="$release/bundle/macos/AgentBaton.app"
mkdir -p target/release "$release/bundle/macos"
install "$fixture/target/release/baton" "$fixture/target/release/batond" target/release/
install "$desktop/agentbaton-desktop" "$release/"
ditto "$desktop/bundle/macos/AgentBaton.app" "$app"
cat > "$scratch/originals.sha256" <<'PINS'
198293e8d1b87b3fec72d2f4b0945b30d5af6c0f873a46d45e14d1b217c2d62a  apps/desktop/src-tauri/target/release/agentbaton-desktop
96811560a548a4dfd10576c43e6d2408d53dad371536e6a4e9b243977866d30b  target/release/baton
0b18add53fc794e6684690a90b74542e487f18409216b2724c8d3d438785a1a1  target/release/batond
PINS
shasum -a 256 -c "$scratch/originals.sha256"
cmp "$release/agentbaton-desktop" "$app/Contents/MacOS/agentbaton-desktop"
cmp target/release/baton "$app/Contents/MacOS/baton"
cmp target/release/batond "$app/Contents/MacOS/batond"
assert_detached() {
  hdiutil info -plist | python3 -c '
import plistlib, sys
assert all(not row.get("image-path", "").startswith(sys.argv[1] + "/")
           for row in plistlib.loads(sys.stdin.buffer.read())["images"])
' "$scratch"
  test -z "$(find artifacts -maxdepth 1 -name '.development.*' -print)"
  echo 'PASS no owned candidate attachment or private packaging directory remains'
}

echo 'case=positive'
bash scripts/native-audit-release.sh "$app/Contents/MacOS/agentbaton-desktop"
bash scripts/package-development-macos.sh
assert_detached
(cd artifacts/development && shasum -a 256 -c SHA256SUMS)
mv artifacts/development "$scratch/accepted"
python3 - "$scratch" "$tested" <<'PY'
from pathlib import Path
import sys
from zipfile import ZipFile

scratch = Path(sys.argv[1])
files = {f"AgentBaton-arm64-{sys.argv[2]}-development.dmg", "SHA256SUMS",
         "DEVELOPMENT_ACCEPTANCE.md", "build-record.txt"}
accepted = scratch / "accepted"
assert {entry.name for entry in accepted.iterdir()} == files
with ZipFile(scratch / "development.zip", "x") as archive:
    for name in sorted(files):
        archive.write(accepted / name, name)
PY
archive="$scratch/development.zip"
archive_sha="$(shasum -a 256 < "$archive" | cut -d ' ' -f 1)"
python3 scripts/record-development-bundle-provenance.py --archive "$archive" \
  --expected-archive-sha256 "$archive_sha" --expected-commit "$tested" --repo "$PWD" > "$scratch/provenance.json"
python3 - "$scratch/provenance.json" <<'PY'
import json
import sys

with open(sys.argv[1]) as stream:
    data = json.load(stream)
print("consumer=" + data["inspection"])
print("archive_sha256=" + data["identity"]["archive_sha256"])
print("candidate_sha256=" + data["artifact"]["sha256"])
PY

expect_rejection() {
  local label="$1" message="$2" status=0
  printf 'case=%s\n' "$label"
  bash scripts/package-development-macos.sh > "$scratch/$label.log" 2>&1 || status=$?
  cat "$scratch/$label.log"
  printf 'package_exit=%s\n' "$status"
  test "$status" -ne 0
  grep -F "$message" "$scratch/$label.log"
  test ! -e artifacts/development
  assert_detached
}

install "$scratch/historical-batond" "$app/Contents/MacOS/batond"
expect_rejection mixed 'SHA-256 mismatch for batond.'
install target/release/batond "$app/Contents/MacOS/batond"
mv "$app/Contents/MacOS/baton" "$scratch/omitted-baton"
expect_rejection missing 'Contents/MacOS must contain exactly agentbaton-desktop, baton and batond.'
shasum -a 256 -c "$scratch/originals.sha256"
test "$(shasum -a 256 < "$historical" | cut -d ' ' -f 1)" = e6373b07b5d76cb1d329b455b6a1dfc41595735be02416b2aeea974e740df03f
