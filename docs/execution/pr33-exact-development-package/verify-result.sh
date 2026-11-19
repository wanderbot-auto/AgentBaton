#!/usr/bin/env bash
set -euo pipefail

attempt="$(cd "$1" && pwd -P)"
case "$attempt" in
  "$BATON_RUNTIME_DIR"/pr33-package.*) ;;
  *) echo 'Use this task’s private build attempt.' >&2; exit 2 ;;
esac
cd "$attempt/source"
test "$(cat "$attempt/build.exit")" = 0
test "$(git rev-parse HEAD)" = d70362ad1fc4b080537163be60cb830849ee4167
test "$(git rev-parse 'HEAD^{tree}')" = 19aba6e8f208f021bc84573dd8b496d29fce5633
shasum -a 256 -c "$attempt/locks-before.sha256"
test -z "$(git status --porcelain --untracked-files=all)"
(cd artifacts/development && shasum -a 256 -c SHA256SUMS)
(cd "$attempt/output" && shasum -a 256 -c archive.sha256)
python3 - "$attempt/output" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

output = Path(sys.argv[1])
report = json.loads((output / 'provenance.json').read_bytes())
desktop = Path('apps/desktop/src-tauri/target/release')
bundled = desktop / 'bundle/macos/AgentBaton.app/Contents/MacOS'
pins = {
    row['path'].split('/')[-1]: row['sha256'] for row in report['files']
    if row.get('architecture') == 'arm64'
}
assert set(pins) == {'agentbaton-desktop', 'baton', 'batond'}
assert {p.name for p in bundled.iterdir()} == set(pins)
assert not list(desktop.glob('agentbaton-native-*'))

for role in ('agentbaton-desktop', 'baton', 'batond'):
    release = desktop if role == 'agentbaton-desktop' else Path('target/release')
    [fingerprint] = (release / '.fingerprint').glob(f'{role}-*/bin-{role}.json')
    entry = json.loads(fingerprint.read_bytes())
    features = json.loads(entry['features'])
    assert 'native-smoke' not in features
    if role == 'agentbaton-desktop':
        assert 'native-smoke' in json.loads(entry['declared_features'])
    for path in (release / role, bundled / role):
        with path.open('rb') as handle:
            assert hashlib.file_digest(handle, 'sha256').hexdigest() == pins[role]
    print(f'{role} Cargo release features={json.dumps(features)}')

[tauri] = (desktop / '.fingerprint').glob('tauri-*/lib-tauri.json')
tauri_features = json.loads(json.loads(tauri.read_bytes())['features'])
assert 'custom-protocol' in tauri_features
print(f'tauri dependency Cargo release features={json.dumps(tauri_features)}')
print('PASS original and assembled executables match read-only provenance digests')
PY

desktop=apps/desktop/src-tauri/target/release
for path in "$desktop/agentbaton-desktop" \
    "$desktop/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop" \
    "$desktop/bundle/macos/AgentBaton.app"; do
  status=0
  printf 'codesign_inspect_path=%s\n' "$path"
  /usr/bin/codesign --verify --strict --verbose=4 "$path" || status=$?
  printf 'codesign_verify_exit=%s\n' "$status"
done
hdiutil info -plist | python3 -c '
import plistlib
import sys

owned = [row for row in plistlib.loads(sys.stdin.buffer.read())["images"]
         if row.get("image-path", "").startswith(sys.argv[1] + "/")]
assert not owned
print("PASS no attachment for the task private build or provenance")
' "$attempt"
