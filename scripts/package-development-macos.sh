#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test "$(uname -s)" = Darwin
test "$(uname -m)" = arm64
architecture=arm64
commit="$(git rev-parse HEAD)"
artifact_dir="artifacts/development"
name="AgentBaton-$architecture-$commit-development.dmg"

release_directory() {
  cargo metadata --manifest-path "$1" --offline --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"] + "/release")'
}

desktop_release="$(release_directory apps/desktop/src-tauri/Cargo.toml)"
workspace_release="$(release_directory Cargo.toml)"
application="$desktop_release/bundle/macos/AgentBaton.app"
test -d "$application"

mkdir -p artifacts
private="$(mktemp -d "$(pwd -P)/artifacts/.development.XXXXXX")"
mountpoint="$private/volume"
output="$private/output"
image="$output/$name"
attach_attempted=false

detach_candidate() {
  local device
  # A failed or interrupted attach can leave a device without a mounted volume.
  # Match the owned image's private path and detach its backing device.
  device="$(hdiutil info -plist | python3 -c '
import plistlib, re, sys

images = [image for image in plistlib.loads(sys.stdin.buffer.read())["images"]
          if image.get("image-path") == sys.argv[1]]
if images:
    if len(images) != 1:
        sys.exit("Ambiguous development image attachment.")
    device = images[0]["system-entities"][0]["dev-entry"]
    if not re.fullmatch(r"/dev/disk[0-9]+", device):
        sys.exit("Cannot identify development image backing device.")
    print(device)
' "$image")" || return
  if [[ -n "$device" ]]; then
    hdiutil detach "$device" || return
  fi
  # Removing only the empty mountpoint prevents recursive cleanup across a mount.
  [[ ! -e "$mountpoint" ]] || rmdir "$mountpoint"
}

cleanup() {
  local status=$?
  trap - EXIT
  trap '' HUP INT TERM
  if "$attach_attempted" && ! detach_candidate; then
    printf 'Development image cleanup failed. Private files remain at %s.\n' "$private" >&2
    exit 1
  fi
  rm -rf "$private" || exit 1
  exit "$status"
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir "$private/stage" "$mountpoint" "$output"

# Pin the compiled originals before copying the assembled app into the image.
for role in agentbaton-desktop baton batond; do
  source="$workspace_release/$role"
  if [[ "$role" == agentbaton-desktop ]]; then
    source="$desktop_release/$role"
  fi
  if [[ ! -f "$source" || -L "$source" || ! -x "$source" ]]; then
    printf 'Missing or invalid compiled development executable for %s.\n' "$role" >&2
    exit 1
  fi
  digest="$(shasum -a 256 < "$source")"
  printf '%s  %s\n' "${digest%% *}" "$role"
done > "$private/expected"

ditto "$application" "$private/stage/AgentBaton.app"
ln -s /Applications "$private/stage/Applications"
hdiutil create -volname AgentBaton -srcfolder "$private/stage" -format UDZO "$image"
attach_attempted=true
hdiutil attach -readonly -noautoopen -nobrowse -mountpoint "$mountpoint" "$image"
python3 - "$mountpoint" <<'PY'
import os
import sys

volume = sys.argv[1]
if not os.path.ismount(volume) or not os.statvfs(volume).f_flag & os.ST_RDONLY:
    sys.exit("Development image must be mounted read-only.")
PY
"$BASH" scripts/native-audit-release-bundle.sh --bundle "$mountpoint/AgentBaton.app" \
  --sha256-manifest "$private/expected"
{
  echo "commit=$commit"
  echo "architecture=$architecture"
  echo 'channel=development; no Developer ID signing or notarization configured'
  sw_vers
  rustc -vV
  node --version
  xcodebuild -version
  echo 'Signature inspection follows; ad hoc signatures do not establish a trusted publisher.'
  codesign -dv --verbose=4 "$mountpoint/AgentBaton.app" 2>&1 || true
} > "$output/build-record.txt"
detach_candidate
attach_attempted=false

cp docs/execution/DEVELOPMENT_ACCEPTANCE.md "$output/"
(cd "$output" && shasum -a 256 "./$name" > SHA256SUMS)
# Publish all four files together. Preserve any already populated artifact directory.
python3 - "$output" "$artifact_dir" <<'PY'
import os
import sys

try:
    os.rename(sys.argv[1], sys.argv[2])
except OSError as error:
    sys.exit(f"Cannot publish development package to {sys.argv[2]}. {error.strerror}.")
PY
