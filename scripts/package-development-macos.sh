#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test "$(uname -s)" = Darwin
architecture="$(uname -m)"
commit="$(git rev-parse HEAD)"
artifact_dir="artifacts/development"
mkdir -p "$artifact_dir"
application="apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app"
test -d "$application"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
ditto "$application" "$stage/AgentBaton.app"
ln -s /Applications "$stage/Applications"
hdiutil create -volname AgentBaton -srcfolder "$stage" -ov -format UDZO "$artifact_dir/AgentBaton-$architecture-$commit-development.dmg"
{
  echo "commit=$commit"
  echo "architecture=$architecture"
  echo 'channel=development; no Developer ID signing or notarization configured'
  sw_vers
  rustc -vV
  node --version
  xcodebuild -version
  echo 'Signature inspection follows; ad hoc signatures do not establish a trusted publisher.'
  codesign -dv --verbose=4 "$application" 2>&1 || true
} > "$artifact_dir/build-record.txt"
cp docs/execution/DEVELOPMENT_ACCEPTANCE.md "$artifact_dir/"
(cd "$artifact_dir" && shasum -a 256 ./*.dmg > SHA256SUMS)
