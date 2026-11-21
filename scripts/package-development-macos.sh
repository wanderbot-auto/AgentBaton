#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test "$(uname -s)" = Darwin
test "$(uname -m)" = arm64
architecture=arm64
commit="$(git rev-parse HEAD)"
artifact_dir="artifacts/development"
roles=(agentbaton-desktop baton batond)
name="AgentBaton-$architecture-$commit-development.dmg"

release_directory() {
  cargo metadata --manifest-path "$1" --offline --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"] + "/release")'
}

file_sha256() {
  local digest
  digest="$(shasum -a 256 < "$1")" || return
  printf '%s\n' "${digest%% *}"
}

resource_seal_sha256() {
  local directory="$1/Contents/_CodeSignature"
  if [[ ! -d "$directory" || -L "$directory" ||
        ! -f "$directory/CodeResources" || -L "$directory/CodeResources" ]]; then
    printf 'Missing or invalid development app resource seal.\n' >&2
    return 1
  fi
  file_sha256 "$directory/CodeResources"
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
staged_app="$private/stage/AgentBaton.app"
mounted_app="$mountpoint/AgentBaton.app"
bundle_audit=scripts/native-audit-release-bundle.sh
attach_attempted=false
retain_private=false

owned_device() {
  # A failed or interrupted attach can leave a device without a mounted volume.
  # Only the uniquely matching private image can supply a device to detach.
  hdiutil info -plist | python3 -c '
import plistlib, re, sys

images = plistlib.loads(sys.stdin.buffer.read())["images"]
if type(images) is not list or any(
    type(image) is not dict or type(image.get("image-path")) is not str or not image["image-path"]
    for image in images
):
    sys.exit("Cannot read development image attachments.")
matches = [image for image in images if image.get("image-path") == sys.argv[1]]
if len(matches) > 1:
    sys.exit("Ambiguous development image attachment.")
owned = matches[0] if matches else None
device = None
for image in images:
    entities = image.get("system-entities", [])
    if type(entities) is not list or any(type(entity) is not dict for entity in entities):
        sys.exit("Cannot read development image devices.")
    if image is owned:
        device = entities[0].get("dev-entry") if entities else None
        if type(device) is not str or not re.fullmatch(r"/dev/disk[0-9]+", device):
            sys.exit("Cannot identify development image backing device.")
    for entity in entities:
        mounted = entity.get("mount-point")
        if mounted and (mounted == sys.argv[2]) != (image is owned):
            sys.exit("Unexpected ownership of the development image mountpoint.")
if device is not None:
    for image in images:
        if image is not owned and any(
            type(entry) is str and re.fullmatch(device + r"(?:s[0-9]+)*", entry)
            for entry in (entity.get("dev-entry") for entity in image.get("system-entities", []))
        ):
            sys.exit("Development image device is also listed for another image.")
    print(device)
' "$image" "$mountpoint"
}

detach_candidate() {
  local device
  device="$(owned_device)" || return
  if [[ -n "$device" ]]; then
    hdiutil detach "$device" || return
  fi
  # Removing only the empty mountpoint prevents recursive cleanup across a mount.
  [[ ! -e "$mountpoint" ]] || rmdir "$mountpoint"
}

clear_failed_attachment() {
  local delay="$1" device
  detach_candidate || return
  if (( delay > 0 )); then
    sleep "$delay" || return
  fi
  device="$(owned_device)" || return
  if [[ -n "$device" || -e "$mountpoint" || -L "$mountpoint" ]]; then
    printf 'Development image still attached after failed mount cleanup.\n' >&2
    return 1
  fi
  printf 'Confirmed failed mount cleanup for the private development image.\n' >&2
}

attach_candidate() {
  local attempt status stderr delay
  for attempt in 1 2 3; do
    stderr="$private/attach-$attempt.stderr"
    attach_attempted=true
    if LC_ALL=C hdiutil attach -readonly -noautoopen -nobrowse -mountpoint "$mountpoint" "$image" 2>"$stderr"; then
      cat "$stderr" >&2
      return
    else
      status=$?
    fi
    cat "$stderr" >&2
    if [[ "$status" != 1 ]] ||
      ! printf '%s\n' 'hdiutil: attach failed - Resource temporarily unavailable' | cmp -s - "$stderr"; then
      return "$status"
    fi
    delay="$attempt"
    if [[ "$attempt" == 3 ]]; then
      delay=0
    fi
    if ! clear_failed_attachment "$delay"; then
      retain_private=true
      return "$status"
    fi
    attach_attempted=false
    if [[ "$attempt" == 3 ]]; then
      printf 'Development image mount retry limit reached.\n' >&2
      return "$status"
    fi
    if [[ ! -f "$image" || -L "$image" || "$(file_sha256 "$image")" != "$created_image_sha256" ]]; then
      printf 'Development image bytes changed before mount retry.\n' >&2
      return 1
    fi
    printf 'Retrying the same read-only development image mount after attempt %s.\n' "$attempt" >&2
    if ! mkdir "$mountpoint"; then
      retain_private=true
      return 1
    fi
  done
}

cleanup() {
  local status=$?
  trap - EXIT
  trap '' HUP INT TERM
  if "$retain_private" || { "$attach_attempted" && ! detach_candidate; }; then
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

# Bind staging to the compiler outputs before allowing a signature to change bytes.
for index in "${!roles[@]}"; do
  role="${roles[$index]}"
  source="$workspace_release/$role"
  if [[ "$role" == agentbaton-desktop ]]; then
    source="$desktop_release/$role"
  fi
  if [[ ! -f "$source" || -L "$source" || ! -x "$source" ]]; then
    printf 'Missing or invalid compiled development executable for %s.\n' "$role" >&2
    exit 1
  fi
  compiled_hashes[index]="$(file_sha256 "$source")"
  printf '%s  %s\n' "${compiled_hashes[$index]}" "$role"
done > "$private/compiled-sha256"

ditto "$application" "$staged_app"
"$BASH" "$bundle_audit" --bundle "$staged_app" --sha256-manifest "$private/compiled-sha256"
for helper in baton batond; do
  codesign --verify --strict "$staged_app/Contents/MacOS/$helper"
done
codesign --force --sign - --timestamp=none "$staged_app"
codesign --verify --deep --strict "$staged_app"
seal_sha256="$(resource_seal_sha256 "$staged_app")"

# Only the desktop executable can change during outer bundle signing.
for index in "${!roles[@]}"; do
  role="${roles[$index]}"
  shipped_hashes[index]="$(file_sha256 "$staged_app/Contents/MacOS/$role")"
  if [[ "$role" != agentbaton-desktop && "${compiled_hashes[$index]}" != "${shipped_hashes[$index]}" ]]; then
    printf 'Development bundle signing unexpectedly changed %s.\n' "$role" >&2
    exit 1
  fi
  printf '%s  %s\n' "${shipped_hashes[$index]}" "$role"
done > "$private/shipped-sha256"

ln -s /Applications "$private/stage/Applications"
hdiutil create -volname AgentBaton -srcfolder "$private/stage" -format UDZO "$image"
created_image_sha256="$(file_sha256 "$image")"
attach_candidate
python3 - "$mountpoint" <<'PY'
import os
import sys

volume = sys.argv[1]
if not os.path.ismount(volume) or not os.statvfs(volume).f_flag & os.ST_RDONLY:
    sys.exit("Development image must be mounted read-only.")
PY
"$BASH" "$bundle_audit" --bundle "$mounted_app" --sha256-manifest "$private/shipped-sha256"
mounted_seal_sha256="$(resource_seal_sha256 "$mounted_app")"
if [[ "$mounted_seal_sha256" != "$seal_sha256" ]]; then
  printf 'Development image resource seal SHA-256 mismatch.\n' >&2
  exit 1
fi
codesign --verify --deep --strict "$mounted_app"
signature="$(codesign --display --verbose=4 "$mounted_app" 2>&1)"
if ! grep -Fxq 'Signature=adhoc' <<< "$signature"; then
  printf 'Expected an ad hoc development app signature.\n' >&2
  exit 1
fi
detach_candidate
attach_attempted=false

dmg_sha256="$(file_sha256 "$image")"
if [[ "$dmg_sha256" != "$created_image_sha256" ]]; then
  printf 'Development image bytes changed during mount verification.\n' >&2
  exit 1
fi
{
  echo "commit=$commit"
  echo "architecture=$architecture"
  echo 'channel=development; no Developer ID signing or notarization configured'
  echo 'signing_mode=ad_hoc_app'
  echo "dmg_sha256=$dmg_sha256"
  echo "code_resources_sha256=$seal_sha256"
  for index in "${!roles[@]}"; do
    role="${roles[$index]}"
    printf 'compiled_sha256.%s=%s\n' "$role" "${compiled_hashes[$index]}"
    printf 'shipped_sha256.%s=%s\n' "$role" "${shipped_hashes[$index]}"
  done
  sw_vers
  rustc -vV
  node --version
  xcodebuild -version
  echo 'Signature inspection follows; ad hoc signatures do not establish a trusted publisher.'
  grep -E '^(Signature=|TeamIdentifier=|CDHash=|Sealed Resources version=)' <<< "$signature"
} > "$output/build-record.txt"

cp docs/execution/DEVELOPMENT_ACCEPTANCE.md "$output/"
printf '%s  ./%s\n' "$dmg_sha256" "$name" > "$output/SHA256SUMS"
# Publish all four files together. Preserve any already populated artifact directory.
python3 - "$output" "$artifact_dir" <<'PY'
import os
import sys

try:
    os.rename(sys.argv[1], sys.argv[2])
except OSError as error:
    sys.exit(f"Cannot publish development package to {sys.argv[2]}. {error.strerror}.")
PY
