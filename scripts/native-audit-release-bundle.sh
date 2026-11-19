#!/usr/bin/env bash
# Inspect the three pinned executables in a default macOS bundle.
set -euo pipefail

if [[ "$#" != 4 || "$1" != --bundle || "$3" != --sha256-manifest ]]; then
  echo 'Usage: native-audit-release-bundle.sh --bundle APP --sha256-manifest EXPECTED_SHA256SUMS' >&2
  exit 2
fi
export LC_ALL=C
roles=(agentbaton-desktop baton batond)
role_markers=('__TAURI_INTERNALS__' 'Usage: baton connections list/propose' 'batond API v1 ready at ')

# The manifest supplies known expected digests, separately from the app being audited.
expected="$(python3 - "$2" "$4" "${roles[@]}" <<'PY'
from pathlib import Path
import plistlib
import re
import sys
from xml.parsers.expat import ExpatError

app, manifest = map(Path, sys.argv[1:3])
names = sys.argv[3:]
contents = app / "Contents"
macos = contents / "MacOS"

def fail(message):
    raise SystemExit(f"Native bundle inspection failed. {message}")

if any(path.is_symlink() or not path.is_dir() for path in (app, contents, macos)):
    fail("Expected a local app with real Contents and MacOS directories.")
if {path.name for path in macos.iterdir()} != set(names):
    fail("Contents/MacOS must contain exactly agentbaton-desktop, baton and batond.")
for name in names:
    binary = macos / name
    if binary.is_symlink() or not binary.is_file() or not binary.stat().st_mode & 0o111:
        fail(f"Invalid packaged executable: {name}.")
plist = contents / "Info.plist"
if plist.is_symlink() or not plist.is_file():
    fail("Expected a regular Info.plist.")
try:
    with plist.open("rb") as stream:
        metadata = plistlib.load(stream)
except (OSError, ValueError, plistlib.InvalidFileException, ExpatError):
    fail("Cannot read Info.plist.")
if not isinstance(metadata, dict) or metadata.get("CFBundleExecutable") != names[0]:
    fail("Info.plist declares an unexpected desktop executable.")

if not manifest.is_file():
    fail("Expected SHA-256 manifest is missing or irregular.")
try:
    with manifest.open(encoding="ascii") as stream:
        text = stream.read(1025)
except (OSError, UnicodeError):
    fail("Cannot read the expected SHA-256 manifest.")
pins = {}
for line in text.splitlines():
    entry = re.fullmatch(r"([0-9a-f]{64})  ([a-z-]+)", line)
    if not entry or entry[2] not in names or entry[2] in pins:
        fail("Expected one SHA-256 row per packaged executable.")
    pins[entry[2]] = entry[1]
if len(text) > 1024 or set(pins) != set(names) or len(set(pins.values())) != len(names):
    fail("Expected three distinct SHA-256 digests.")
print(" ".join(pins[name] for name in names))
PY
)"
read -r -a pins <<< "$expected"
inspection="$(mktemp)"
trap 'rm -f "$inspection"' EXIT
audit_directory="$(cd -- "$(dirname -- "$0")" && pwd -P)"
bundle_markers='agentbaton-native-(smoke|owner-dialog-smoke|background-service-smoke)'

verify_digest() {
  local digest
  digest="$(shasum -a 256 < "$1")"
  if [[ "${digest%% *}" != "$2" ]]; then
    printf 'Native bundle inspection failed. SHA-256 mismatch for %s.\n' "$3" >&2
    exit 1
  fi
}

for index in 0 1 2; do
  role="${roles[$index]}"
  binary="$2/Contents/MacOS/$role"
  verify_digest "$binary" "${pins[$index]}" "$role"
  format="$(file -b -- "$binary")"
  case "$format" in
    'Mach-O 64-bit executable arm64'|'Mach-O 64-bit executable arm64,'*|\
    'Mach-O 64-bit arm64 executable'|'Mach-O 64-bit arm64 executable,'*) ;;
    *)
      printf 'Native bundle inspection failed. Expected a thin ARM64 Mach-O executable for %s.\n' "$role" >&2
      exit 1
      ;;
  esac
  # A nonzero strings exit must stop the audit even if partial output looked clean.
  strings -a -- "$binary" > "$inspection"
  # Reuse the guarded legacy marker list on the complete printable-byte scan.
  legacy_status=0
  "$BASH" "$audit_directory/native-audit-release.sh" "$inspection" >/dev/null || legacy_status=$?
  if [[ "$legacy_status" != 0 ]]; then
    printf 'Native bundle inspection failed. Legacy native marker audit rejected %s.\n' "$role" >&2
    exit "$legacy_status"
  fi
  marker_status=0
  grep -Eo "$bundle_markers" "$inspection" || marker_status=$?
  case "$marker_status" in
    0)
      printf 'Test-only native harness content is present in %s.\n' "$role" >&2
      exit 1
      ;;
    1) ;;
    *)
      echo 'Native application inspection failed.' >&2
      exit "$marker_status"
      ;;
  esac
  for candidate in 0 1 2; do
    match=0
    grep -F -- "${role_markers[$candidate]}" "$inspection" >/dev/null || match=$?
    if [[ "$match" != 0 && "$match" != 1 ]]; then
      echo 'Native application inspection failed.' >&2
      exit "$match"
    fi
    if [[ ( "$index" == "$candidate" && "$match" != 0 ) || ( "$index" != "$candidate" && "$match" == 0 ) ]]; then
      printf 'Native bundle inspection failed. Executable role mismatch for %s.\n' "$role" >&2
      exit 1
    fi
  done
  verify_digest "$binary" "${pins[$index]}" "$role"
done
printf 'PASS no inspected native harness identifiers in pinned ARM64 bundle executables\n'
