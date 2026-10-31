#!/usr/bin/env bash
# Run on the actual default-feature application executable, not the smoke binary.
set -euo pipefail
binary="${1:?Pass the default application executable path}"
test -f "$binary"
command -v strings >/dev/null
inspection="$(mktemp)"
trap 'rm -f "$inspection"' EXIT
# A failed inspection must fail closed, rather than becoming the false branch
# of a pipeline inside `if` and printing a successful audit.
strings "$binary" > "$inspection"
if grep -E 'native_smoke_report|native-smoke-create|AGENTBATON_SMOKE_COMMIT|owner_dialog_report|native_factory_planning_os_dialog_fixture|agentbaton_preview_evidence_gate_v1|\.agentbaton-preview-evidence-v1\.json|\.agentbaton-preview-evidence-ready-v1\.json|agentbaton_copy_evidence_gate_v1|\.agentbaton-copy-evidence-v1\.json|\.agentbaton-copy-evidence-ready-v1\.json' "$inspection"; then
  echo 'Test-only native harness content is present in the application executable.' >&2
  exit 1
else
  status=$?
  if [[ "$status" != 1 ]]; then
    echo 'Native application inspection failed.' >&2
    exit "$status"
  fi
fi
printf 'PASS no native harness identifiers in default application executable\n'
