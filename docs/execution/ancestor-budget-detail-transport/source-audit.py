#!/usr/bin/env python3
"""Source-only inverse audit; does not run Rust, SQL or product processes."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
BASE = "e874b123d3e7f329a551e21922baf1a7a643548f"
CHECKS = []

def source(path):
    return (ROOT / path).read_text()

def baseline(path):
    return subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT).decode()

def check(name, condition):
    if not condition:
        raise AssertionError(name)
    CHECKS.append(name)

def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()

factory_path = "crates/core/src/goal_factory.rs"
factory = source(factory_path)
old_factory = baseline(factory_path)
restored = factory
start = restored.index('const RECORDED_ADVANCE_PREFIX:')
end = restored.index('#[derive(Clone, Copy, Debug, Serialize, Deserialize)]', start)
restored = restored[:start] + restored[end:]
restored = restored.replace('    #[serde(deserialize_with = "required_admission_metric")]\n', '')
start = restored.index('\n#[derive(Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\nstruct RecordedAdmissionDetail')
end = restored.index('pub(super) fn ancestor_admission_diagnostic(', start)
restored = restored[:start] + restored[end:]
parser_start = 'pub(super) fn ancestor_admission_diagnostic('
parser_end = '\npub(super) fn reserve_ancestors('
old_parser = old_factory[old_factory.index(parser_start):old_factory.index(parser_end)]
start, end = restored.index(parser_start), restored.index(parser_end)
restored = restored[:start] + old_parser + restored[end:]
restored = restored.replace('    include!("goal_admission_detail_codec_tests.rs");\n', '')
check('full_factory_inverse_equals_fixed_base_including_all_admission_guards', restored == old_factory)

ipc_path = 'crates/core/src/ipc.rs'
ipc = source(ipc_path)
old_ipc = baseline(ipc_path)
old_line = next(line for line in old_ipc.splitlines(True) if 'recorded-goal-advance:{id}' in line)
new_line = next(line for line in ipc.splitlines(True) if 'Store::recorded_goal_advance_error' in line)
check('full_ipc_inverse_is_exact_original_one_line_wrapper', ipc.replace(new_line, old_line, 1) == old_ipc)
check('ipc_wrapper_uses_same_actual_receipt_selected_run_and_requested_revision',
      'Store::recorded_goal_advance_error(error,&receipt,&p.run_id,revision)' in new_line)

runtime_path = 'crates/core/src/goal_runtime.rs'
runtime = source(runtime_path)
check('full_runtime_inverse_is_only_existing_cfg_test_module_include',
      runtime.replace('    include!("goal_admission_detail_store_tests.rs");\n', '', 1) == baseline(runtime_path))
check('runtime_include_is_inside_existing_cfg_test_module',
      runtime.index('#[cfg(test)]\nmod tests {') < runtime.index('include!("goal_admission_detail_store_tests.rs")'))

pure = factory[factory.index('struct RecordedAdmissionDetail'):factory.index('pub(super) fn ancestor_admission_diagnostic(')]
for forbidden in ['.conn', 'Connection', 'SELECT ', 'INSERT ', 'UPDATE ', 'DELETE ', 'event(', 'now(', 'std::fs', 'std::process', 'thread::']:
    check(f'pure_codec_wrapper_has_no_{forbidden}', forbidden not in pure)
check('compound_cap_4096', 'MAX_RECORDED_ADMISSION_BYTES: usize = 4096' in factory)
check('inner_cap_1024_precedes_serde_decode', factory.index('if raw.len() > 1024', factory.index('fn original_admission_detail')) < factory.index('serde_json::from_str(raw).ok()', factory.index('fn original_admission_detail')))
check('strict_four_field_compound_schema', 'struct RecordedAdmissionDetail {\n    run_id: String,\n    run_revision: u64,\n    intent_id: String,\n    original_guard: String,\n}' in factory)
check('required_nullable_metrics_all_four', factory.count('#[serde(deserialize_with = "required_admission_metric")]') == 4)
codec_tests = source('crates/core/src/goal_admission_detail_codec_tests.rs')
store_tests = source('crates/core/src/goal_admission_detail_store_tests.rs')
check('seven_codec_test_cases_authored_not_executed', codec_tests.count('#[test]') == 7)
check('three_real_store_test_cases_authored_not_executed', store_tests.count('#[test]') == 3)
check('store_tests_call_production_admit_wrapper_dto_and_existing_block', all(x in store_tests for x in ['.admit_goal_advance(', 'Store::recorded_goal_advance_error(', 'serde_json::from_slice(&serde_json::to_vec(&wrapped)', '.block_goal_run(']))
print(json.dumps({'base': BASE, 'scope': 'source_only', 'checks_passed': len(CHECKS), 'checks': CHECKS,
    'source_sha256': {p: digest(source(p)) for p in [factory_path, ipc_path, runtime_path,
        'crates/core/src/goal_admission_detail_codec_tests.rs', 'crates/core/src/goal_admission_detail_store_tests.rs']},
    'not_run': ['Rust compilation and tests', 'SQL/Store test execution', 'product processes', 'Node', 'browser', 'network', 'macOS Intel or ARM', 'CI']}, indent=2))
