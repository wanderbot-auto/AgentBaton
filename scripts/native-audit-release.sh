#!/usr/bin/env bash
# Inspect the default application executable or the three pinned executables in a macOS bundle.
set -euo pipefail
native_markers='native_background_service_fixture_report|native_background_fixture_alarm|abg-registration-fixture-v1|native_smoke_report|native-smoke-create|AGENTBATON_SMOKE_COMMIT|owner_dialog_report|native_factory_planning_os_dialog_fixture|native_integration_os_dialog_fixture|agentbaton_preview_evidence_gate_v1|\.agentbaton-preview-evidence-v1\.json|\.agentbaton-preview-evidence-ready-v1\.json|agentbaton_copy_evidence_gate_v1|\.agentbaton-copy-evidence-v1\.json|\.agentbaton-copy-evidence-ready-v1\.json'

inspect_markers() {
  local status
  if grep "$1" "$2" "$inspection"; then
    printf '%s\n' "$3" >&2
    exit 1
  else
    status=$?
    if [[ "$status" != 1 ]]; then
      echo 'Native application inspection failed.' >&2
      exit "$status"
    fi
  fi
}

if [[ "${1:-}" != --bundle ]]; then
  binary="${1:?Pass the default application executable path}"
  test -f "$binary"
  command -v strings >/dev/null
  inspection="$(mktemp)"
  trap 'rm -f "$inspection"' EXIT
  strings -- "$binary" > "$inspection"
  inspect_markers -E "$native_markers" 'Test-only native harness content is present in the application executable.'
  printf 'PASS no native harness identifiers in default application executable\n'
  exit 0
fi

if [[ "$#" != 4 || "$3" != --sha256-manifest ]]; then
  echo 'Usage: native-audit-release.sh --bundle APP --sha256-manifest EXPECTED_SHA256SUMS' >&2
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
bundle_markers="$native_markers|agentbaton-native-(smoke|owner-dialog-smoke|background-service-smoke)"

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
  inspect_markers -Eo "$bundle_markers" "Test-only native harness content is present in $role."
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
