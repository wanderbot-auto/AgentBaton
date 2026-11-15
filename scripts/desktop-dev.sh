#!/usr/bin/env bash
# Works from any directory; Python owns the bounded process lifecycle.
set -euo pipefail
exec python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/desktop-dev.py" "$@"
