#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
triple="$(rustc -vV | sed -n 's/^host: //p')"
test -n "$triple"
mkdir -p apps/desktop/src-tauri/binaries
for binary in batond baton; do
  test -x "target/release/$binary"
  cp "target/release/$binary" "apps/desktop/src-tauri/binaries/$binary-$triple"
done
