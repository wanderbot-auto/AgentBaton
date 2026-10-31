#!/usr/bin/env bash
# Actual native WebView/production IPC integration. Never used in shipping bundles.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'This lane requires a real macOS GUI session; Linux evidence is separate.' >&2
  exit 2
fi
output="${1:-artifacts/native-smoke}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
export AGENTBATON_SMOKE_COMMIT="$(git rev-parse HEAD)"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
# Caller builds actual frontend and sidecars before this lane, as for production.
test -f apps/desktop/dist/index.html
cargo build --locked --bin batond --bin baton
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --features native-smoke,custom-protocol --bin agentbaton-native-smoke
cargo build --locked --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --features native-smoke,custom-protocol --bin agentbaton-native-smoke
root_target="$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')"
native_target="$(cargo metadata --offline --no-deps --format-version 1 --manifest-path apps/desktop/src-tauri/Cargo.toml | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"]+"/debug")')"
if [[ "$root_target/debug/batond" != "$native_target/batond" ]]; then
  cp "$root_target/debug/batond" "$native_target/batond"
fi
python3 scripts/native-run.py --output "$output" -- "$native_target/agentbaton-native-smoke" "$output"
python3 - "$output/native-smoke.json" <<'PY'
import importlib.util,json,sys
from pathlib import Path
record=json.loads(Path(sys.argv[1]).read_text())
native_spec=importlib.util.spec_from_file_location('native_schema15_contract','scripts/native-run.py')
native_runner=importlib.util.module_from_spec(native_spec)
native_spec.loader.exec_module(native_runner)
factory_error=native_runner.factory_report_error(record)
assert factory_error is None,factory_error
for phase in ['before', 'after']:
    factory_counts = record['factory_readonly_database'][phase]
    assert type(factory_counts['schema_version']) is int and factory_counts['schema_version'] == 15
    assert all(type(factory_counts[field]) is int and factory_counts[field] == 0 for field in ['child_plan_heads', 'child_plan_revisions', 'child_goal_runs'])
assert record['status']=='passed',record
assert record['os']=='macos',record
assert record['first_webview']['realComposerSubmitted']
assert record['first_webview']['dataContractLists']
assert record['first_webview']['dataContractEmptyControls']
assert record['first_webview']['readonlyDataNoMutation']
assert record['first_webview']['commandContractLists'] is True
assert record['first_webview']['commandContractEmptyControls'] is True
assert record['first_webview']['readonlyCommandNoMutation'] is True
assert record['first_webview']['goalScopeLists'] is True
assert record['first_webview']['goalScopeEmptyControls'] is True
assert record['first_webview']['readonlyGoalNoMutation'] is True
assert record['first_webview']['goalExecutionLists'] is True
assert record['first_webview']['goalExecutionMissingRunDenied'] is True
assert record['first_webview']['readonlyGoalExecutionNoMutation'] is True
assert record['goal_execution_readonly_database']['read_only'] is True
assert record['goal_execution_readonly_database']['unchanged'] is True
assert record['goal_execution_readonly_database']['before'] == record['goal_execution_readonly_database']['after']
assert type(record['goal_execution_readonly_database']['before']['schema_version']) is int and record['goal_execution_readonly_database']['before']['schema_version'] == 15
assert all(record['goal_execution_readonly_database']['before'][field] == 0 for field in ['run_count','node_count','reservation_count'])
assert record['first_webview']['goalExecutionReadonlySequence'] == record['first_webview']['goalReadonlySequence']
assert record['first_webview']['goalExecutionReadonlySequence']['before'] == record['goal_execution_readonly_database']['before']['sequence']
assert record['first_webview']['goalExecutionMissingRun'] == {'run_id':'native-readonly-missing-goal-run','code':'NOT_FOUND','side_effect_state':'not_started'}
assert record['first_webview']['goalGraphLists'] is True
assert record['first_webview']['goalGraphMissingRunDenied'] is True
assert record['first_webview']['readonlyGoalGraphNoMutation'] is True
assert record['goal_graph_readonly_database']['read_only'] is True
assert record['goal_graph_readonly_database']['unchanged'] is True
assert record['goal_graph_readonly_database']['before'] == record['goal_graph_readonly_database']['after']
assert type(record['goal_graph_readonly_database']['before']['schema_version']) is int and record['goal_graph_readonly_database']['before']['schema_version'] == 15
assert all(record['goal_graph_readonly_database']['before'][field] == 0 for field in ['capability_count','job_count','candidate_count','validation_count','owner_decision_count','acceptance_count'])
assert record['first_webview']['goalGraphReadonlySequence'] == record['first_webview']['goalExecutionReadonlySequence'] == record['first_webview']['goalReadonlySequence']
assert record['first_webview']['goalGraphReadonlySequence']['before'] == record['goal_graph_readonly_database']['before']['sequence']
assert record['first_webview']['goalGraphMissingRun'] == {'run_id':'native-readonly-missing-goal-graph-run','code':'NOT_FOUND','side_effect_state':'not_started'}
assert record['reopened_webview']['restartPersisted']
assert record['window_close_daemon_survived']
assert record['second_writer_rejected']
assert record['owned_daemon_cleanup']
print('PASS actual macOS WebView, product goal form, restricted Tauri IPC, persisted restart and owned cleanup')
PY
