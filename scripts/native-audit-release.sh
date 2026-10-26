#!/usr/bin/env bash
# Run on the actual default-feature application executable, not the smoke binary.
set -euo pipefail
binary="${1:?Pass the default application executable path}"
test -f "$binary"
if strings "$binary" | rg 'native_smoke_report|native-smoke-create|AGENTBATON_SMOKE_COMMIT'; then
  echo 'Test-only native harness content is present in the application executable.' >&2
  exit 1
fi
printf 'PASS no native harness identifiers in default application executable\n'
