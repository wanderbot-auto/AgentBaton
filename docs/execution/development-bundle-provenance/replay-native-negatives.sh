#!/usr/bin/env bash
set -euo pipefail
umask 077
fixture=$1
history=$2
out=$3
mkdir "$out"
out="$(cd "$out" && pwd -P)"
cd "$(git rev-parse --show-toplevel)"
record="$PWD/scripts/record-development-bundle-provenance.py"
fixture_commit=$(git -C "$fixture/source" rev-parse HEAD)
history_commit=df35231eab29aa2d4a043fd9be60888504507716
history_sha=cbed165a62710d1d302f3ab28cc3e6e12165d270f9c14ff858d6e93ac00b99e0
fixture_sha=$(shasum -a 256 "$fixture/fixture.zip" | awk '{print $1}')
receipt_sha=$(shasum -a 256 "$fixture/receipts/receipt.json" | awk '{print $1}')
fixture_dmg=AgentBaton-arm64-$fixture_commit-development.dmg

expect_denial() {
    local name=$1 diagnostic=$2
    shift 2
    set +e
    python3 "$record" "$@" > "$out/$name.stdout" 2> "$out/$name.stderr"
    local status=$?
    set -e
    test "$status" = 1
    test ! -s "$out/$name.stdout"
    rg -Fx "development bundle provenance: $diagnostic" "$out/$name.stderr"
    printf '%s exit=%s report_bytes=0\n' "$name" "$status"
}

cp "$fixture/fixture.zip" "$out/corrupt.zip"
printf 'changed archive bytes\n' >> "$out/corrupt.zip"
expect_denial corrupt "archive SHA-256 mismatch" \
  --archive "$out/corrupt.zip" --expected-archive-sha256 "$fixture_sha" \
  --expected-commit "$fixture_commit" --repo "$fixture/source"

python3 - "$fixture/fixture.zip" "$out/mixed.zip" "$fixture_commit" "$history_commit" <<'PY'
import pathlib, sys, zipfile
old, new = map(pathlib.Path, sys.argv[1:3])
before, after = (item.encode() for item in sys.argv[3:])
with zipfile.ZipFile(old) as source, zipfile.ZipFile(new, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name in source.namelist():
        value = source.read(name)
        if name == "build-record.txt":
            value = value.replace(before, after)
        output.writestr(name, value)
PY
mixed_sha=$(shasum -a 256 "$out/mixed.zip" | awk '{print $1}')
expect_denial mixed_commit "build record commit mismatch" \
  --archive "$out/mixed.zip" --expected-archive-sha256 "$mixed_sha" \
  --expected-commit "$fixture_commit" --repo "$fixture/source"

expect_denial reused_receipt "receipt package identity mismatch" \
  --archive "$history" --expected-archive-sha256 "$history_sha" --expected-commit "$history_commit" \
  --receipt "$fixture/receipts/receipt.json" --receipt-sha256 "$receipt_sha"

ditto "$fixture/image" "$out/wrong-arch-volume"
/usr/bin/clang -arch x86_64 -Wl,-no_adhoc_codesign "$fixture/source/src/native.c" \
    -o "$out/wrong-arch-volume/AgentBaton.app/Contents/MacOS/baton"
file "$out/wrong-arch-volume/AgentBaton.app/Contents/MacOS/baton" | tee "$out/wrong-arch-file.log"
/usr/bin/hdiutil create -volname AgentBatonFixture -srcfolder "$out/wrong-arch-volume" \
    -format UDZO "$out/$fixture_dmg" > "$out/wrong-arch-dmg-build.log"
python3 - "$fixture/fixture.zip" "$out/wrong-arch.zip" "$out/$fixture_dmg" <<'PY'
import hashlib, pathlib, sys, zipfile
old, new, dmg = map(pathlib.Path, sys.argv[1:])
data = dmg.read_bytes()
sha = hashlib.sha256(data).hexdigest()
with zipfile.ZipFile(old) as source, zipfile.ZipFile(new, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name in source.namelist():
        value = source.read(name)
        if name == dmg.name:
            value = data
        elif name == "SHA256SUMS":
            value = f"{sha}  ./{dmg.name}\n".encode()
        output.writestr(name, value)
PY
wrong_sha=$(shasum -a 256 "$out/wrong-arch.zip" | awk '{print $1}')
expect_denial wrong_arch "expected a thin ARM64 Mach-O executable" \
  --archive "$out/wrong-arch.zip" --expected-archive-sha256 "$wrong_sha" \
  --expected-commit "$fixture_commit" --repo "$fixture/source"

shasum -a 256 "$out/"*.zip "$out/$fixture_dmg"
