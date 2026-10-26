#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/check_gate.py --scope preparation
command -v node >/dev/null || { echo 'Node.js is required.' >&2; exit 2; }
command -v rustup >/dev/null || { echo 'Rustup is missing. Install through the official Rust toolchain setup, then rerun. No unreviewed download is executed.' >&2; exit 2; }
python3 -m venv .venv
.venv/bin/python -m pip install --disable-pip-version-check -r requirements-infra.txt
# Dependency-free Node infrastructure tests; this does not install product dependencies.
npm ci --ignore-scripts --no-audit --no-fund
cargo fetch --locked --manifest-path tools/environment-smoke/Cargo.toml
.venv/bin/python scripts/doctor.py --strict
.venv/bin/python scripts/validate_design.py
.venv/bin/python scripts/validate_infrastructure.py
.venv/bin/python -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
npm test
cargo test --locked --manifest-path tools/environment-smoke/Cargo.toml
printf '\nInfrastructure checks finished. Current product authority is recorded in infra/execution-gate.json and its approval reference.\n'
