#!/usr/bin/env bash
set -euo pipefail
umask 077

proof=$1
vite=$2
mkdir "$proof"
proof="$(cd "$proof" && pwd -P)"
source="$proof/source"
stage="$proof/image"
details="$proof/receipts"
app="$stage/AgentBaton.app/Contents"
project="$(pwd -P)"
mkdir -p "$source/apps/desktop/src-tauri/icons" "$source/docs/execution" "$source/src" \
    "$app/MacOS" "$app/Resources" "$details"
ln -s /Applications "$stage/Applications"
cat > "$source/apps/desktop/src-tauri/tauri.conf.json" <<'EOF'
{"productName":"AgentBaton","identifier":"dev.agentbaton","version":"0.0.1","bundle":{"externalBin":["binaries/batond","binaries/baton"]}}
EOF
printf 'private fixture icon\n' > "$source/apps/desktop/src-tauri/icons/icon.icns"
printf 'Private development fixture acceptance\n' > "$source/docs/execution/DEVELOPMENT_ACCEPTANCE.md"
cat > "$source/src/entry.js" <<'EOF'
import { announcement } from "./label.js";
document.body.textContent = announcement("receipt-fixture");
EOF
cat > "$source/src/label.js" <<'EOF'
export function announcement(name) { return "Built fixture: " + name; }
EOF
cat > "$source/src/native.c" <<'EOF'
#include <stddef.h>
#ifdef WEB_FIXTURE
__attribute__((used, section("__DATA,__fixtureweb")))
static const unsigned char fixture_web[] = {
#include "chunk.inc"
};
#endif
int main(void) {
#ifdef WEB_FIXTURE
    return fixture_web[0] == 0;
#else
    return 0;
#endif
}
EOF
cat > "$source/src/build-web.mjs" <<'EOF'
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { pathToFileURL } from "node:url";

const [root, out, vite] = process.argv.slice(2);
const metadata = JSON.parse(readFileSync(join(vite, "package.json")));
if (metadata.version !== "8.3.5") throw new Error("Unexpected Vite version");
const { build } = await import(pathToFileURL(join(vite, "dist/node/index.js")));
const captured = [];
await build({
  root,
  configFile: false,
  logLevel: "warn",
  build: {
    outDir: out,
    emptyOutDir: false,
    minify: false,
    rolldownOptions: {
      input: join(root, "src/entry.js"),
      output: { entryFileNames: "fixture-web.js" },
    },
  },
  plugins: [{
    name: "record-emitted-module-map",
    generateBundle(_options, bundle) {
      for (const item of Object.values(bundle)) {
        if (item.type === "chunk") captured.push({
          schema_version: 1,
          chunk: item.fileName,
          chunk_sha256: createHash("sha256").update(item.code).digest("hex"),
          modules: Object.entries(item.modules).map(([id, info]) => ({
            id: relative(root, id),
            rendered_length: info.renderedLength,
          })),
        });
      }
    },
  }],
});
if (captured.length !== 1) throw new Error("Unexpected generated chunks");
const map = captured[0];
const written = readFileSync(join(out, map.chunk));
if (createHash("sha256").update(written).digest("hex") !== map.chunk_sha256)
  throw new Error("Written chunk deviates from builder receipt");
writeFileSync(join(out, "web-record.json"), JSON.stringify(map, null, 2) + "\n");
console.log("Vite", metadata.version, "chunk", map.chunk, "modules", map.modules.length);
EOF
git -C "$source" init -q
git -C "$source" add .
GIT_AUTHOR_DATE=2026-11-19T00:00:00Z GIT_COMMITTER_DATE=2026-11-19T00:00:00Z \
    git -C "$source" -c user.name=Fixture -c user.email=fixture@example.invalid \
    -c commit.gpgsign=false commit -qm "Record generated native and Web fixture inputs"
commit=$(git -C "$source" rev-parse HEAD)
tree=$(git -C "$source" rev-parse HEAD^{tree})
node "$source/src/build-web.mjs" "$source" "$details" "$vite" | tee "$proof/vite.log"
python3 - "$details" <<'PY'
import pathlib, sys
data = pathlib.Path(sys.argv[1])
(data/"chunk.inc").write_text(", ".join(map(str, (data/"fixture-web.js").read_bytes())) + "\n")
PY
/usr/bin/clang -arch arm64 -Wl,-no_adhoc_codesign -DWEB_FIXTURE -I "$details" \
   "-Wl,-map,$details/desktop.map" "$source/src/native.c" -o "$app/MacOS/agentbaton-desktop"
/usr/bin/clang -arch arm64 -Wl,-no_adhoc_codesign "$source/src/native.c" -o "$app/MacOS/baton"
/usr/bin/clang -arch arm64 -Wl,-no_adhoc_codesign "$source/src/native.c" -o "$app/MacOS/batond"
cp "$app/MacOS/agentbaton-desktop" "$details/agentbaton-desktop"
cp "$source/apps/desktop/src-tauri/icons/icon.icns" "$app/Resources/icon.icns"
python3 - "$app" <<'PY'
import pathlib, plistlib, sys
p = pathlib.Path(sys.argv[1])
(p/"Info.plist").write_bytes(plistlib.dumps({
    "CFBundleIdentifier":"dev.agentbaton",
    "CFBundleExecutable":"agentbaton-desktop",
    "CFBundleShortVersionString":"0.0.1",
    "CFBundleVersion":"0.0.1",
}))
PY
dmg="AgentBaton-arm64-$commit-development.dmg"
/usr/bin/hdiutil create -volname AgentBatonFixture -srcfolder "$stage" -format UDZO "$proof/$dmg" \
    > "$proof/dmg-build.log"
python3 - "$proof" "$dmg" "$commit" "$tree" <<'PY'
import hashlib, json, pathlib, sys, zipfile
root = pathlib.Path(sys.argv[1])
name, commit, tree = sys.argv[2:]
receipts = root / "receipts"
image = root / name
archive = root / "fixture.zip"
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
    output.write(image, name)
    output.write(root/"source/docs/execution/DEVELOPMENT_ACCEPTANCE.md", "DEVELOPMENT_ACCEPTANCE.md")
    output.writestr("SHA256SUMS", f"{sha(image)}  ./{name}\n")
    output.writestr("build-record.txt", f"commit={commit}\narchitecture=arm64\n"
                     "channel=development; no Developer ID signing or notarization configured\n")
binary = receipts/"agentbaton-desktop"
chunk = receipts/"fixture-web.js"
bind = {"file": "AgentBaton.app/Contents/MacOS/agentbaton-desktop", "file_sha256": sha(binary)}
identity = {
    "schema_version": 1,
    "source_commit": commit,
    "source_tree": tree,
    "target": "aarch64-apple-darwin",
    "archive_sha256": sha(archive),
    "dmg_sha256": sha(image),
    "bindings": [
        {**bind, "kind": "linker", "record": "desktop.map",
         "record_sha256": sha(receipts/"desktop.map"), "linked_output": binary.name},
        {**bind, "kind": "web", "record": "web-record.json",
         "record_sha256": sha(receipts/"web-record.json"), "asset": chunk.name,
         "asset_sha256": sha(chunk), "offset": binary.read_bytes().index(chunk.read_bytes())},
    ],
}
(receipts/"receipt.json").write_text(json.dumps(identity, indent=2) + "\n")
(root/"fixture-digests.json").write_text(json.dumps({
    "fixture_commit":commit, "fixture_tree":tree, "archive_sha256":sha(archive),
    "dmg_sha256":sha(image), "external_receipt_sha256":sha(receipts/"receipt.json"),
}, indent=2) + "\n")
PY
archive_sha=$(shasum -a 256 "$proof/fixture.zip" | awk '{print $1}')
receipt_sha=$(shasum -a 256 "$details/receipt.json" | awk '{print $1}')
python3 "$project/scripts/record-development-bundle-provenance.py" \
    --archive "$proof/fixture.zip" --expected-archive-sha256 "$archive_sha" \
    --expected-commit "$commit" --repo "$source" \
    --receipt "$details/receipt.json" --receipt-sha256 "$receipt_sha" \
    > "$proof/fixture-report.json"
python3 - "$proof/fixture-report.json" <<'PY'
import json, sys
report = json.load(open(sys.argv[1]))
desktop = next(row for row in report["files"] if row["role"] == "desktop")
assert desktop["static_linkage"] == "bound_receipt"
assert desktop["embedded_web"] == "bound_receipt"
assert report["attribution_completeness"] == "unknown"
print("real fixture linker and Web evidence bound to the exact shipped carrier")
PY
