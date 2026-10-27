#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
triple="$(rustc -vV | sed -n 's/^host: //p')"
test -n "$triple"
target_directory="$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p apps/desktop/src-tauri/binaries
for binary in batond baton; do
  test -x "$target_directory/release/$binary"
  cp "$target_directory/release/$binary" "apps/desktop/src-tauri/binaries/$binary-$triple"
done
