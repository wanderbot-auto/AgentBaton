#!/usr/bin/env python3
"""Actual macOS generated-tree IPC/Native sheets; no login/service activation.

The binary owns fixed private fixture paths and children. This runner supplies
only closed case names and evidence destinations, never HOME/runtime overrides.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import stat
import subprocess
import sys

CASES = ('normal', 'race', 'conflict')
REPORT_KEYS = {'conflict_mutation_path', 'registration_runtime_socket_bounded', 'fixture_retention', 'owned_daemon_cleanup_reaped', 'effective_login_start', 'completed_operations', 'receipt_before_plist', 'production_custom_runtime_refused_readonly', 'status', 'capture_cleanup_reaped', 'contract', 'root_private_and_bound', 'installed_layout', 'generated_fixture_root', 'os', 'architecture', 'provider_model_calls', 'real_home_disjoint', 'actual_os_login', 'failed', 'core_runtime_socket_bounded', 'case', 'owned_daemon_unchanged_and_alive_before_cleanup', 'renderer_observations_untrusted', 'core_runtime_bound', 'receipts', 'dimension', 'screenshots', 'actual_darwin_home_abi_agrees', 'current_daemon_action'}
WIRE_KEYS = {'api_version', 'platform', 'mechanism', 'login_registration', 'outcome', 'code',
             'side_effect_state', 'current_daemon_action', 'effective_login_start'}
REQUIRED_FLAGS = ('root_private_and_bound', 'core_runtime_bound', 'registration_runtime_socket_bounded',
                  'core_runtime_socket_bounded', 'real_home_disjoint', 'actual_darwin_home_abi_agrees',
                  'production_custom_runtime_refused_readonly', 'owned_daemon_unchanged_and_alive_before_cleanup',
                  'owned_daemon_cleanup_reaped', 'capture_cleanup_reaped')

def unique(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('duplicate JSON object key')
        value[key] = item
    return value


def expected_receipt(case, index):
    registration, outcome, code, effect = 'disabled', 'observed', 'ok', 'not_started'
    if case == 'conflict':
        registration, outcome, code = 'conflict', 'refused', 'content_conflict'
    elif case == 'race' and index:
        registration, outcome, code = 'unknown', 'refused', 'state_changed'
    elif case == 'normal' and index in (1, 5):
        registration, outcome, code = 'unknown', 'cancelled', 'cancelled'
    elif case == 'normal':
        registration = 'enabled_next_login' if 3 <= index <= 6 else 'disabled'
        if index in (3, 7):
            outcome, effect = 'applied', 'confirmed'
    return dict(api_version=1, platform='macos', mechanism='per_user_launchagent',
                login_registration=registration, outcome=outcome, code=code, side_effect_state=effect,
                current_daemon_action='none', effective_login_start='not_observed')


def validate_report(record, case, architecture):
    if case not in CASES or architecture not in ('aarch64', 'x86_64'):
        raise ValueError('unknown case or architecture')
    count = {'normal': 9, 'race': 3, 'conflict': 2}[case]
    if type(record) is not dict or record.get('status') != 'passed' or record.get('case') != case or set(record) != REPORT_KEYS:
        raise ValueError('Native fixture has no exact successful case report')
    if record.get('dimension') != 'real_macos_webview_ipc_native_sheet_generated_paths':
        raise ValueError('wrong Native evidence dimension')
    if any(record.get(name) is not True for name in REQUIRED_FLAGS):
        raise ValueError('missing binding, safety, ABI or actual owned cleanup observation')
    for name in ('completed_operations', 'renderer_observations_untrusted'):
        if type(record.get(name)) is not int or record[name] != count:
            raise ValueError('incomplete or replayed Rust phase/renderer observation sequence')
    receipts = record.get('receipts')
    if type(receipts) is not list or len(receipts) != count:
        raise ValueError('missing actual Native receipt')
    for index, receipt in enumerate(receipts):
        if (type(receipt) is not dict or set(receipt) != WIRE_KEYS
                or type(receipt.get('api_version')) is not int
                or receipt != expected_receipt(case, index)):
            raise ValueError('wrong or widened original nine-key receipt')
    screens = record.get('screenshots')
    if type(screens) is not int or not ({'normal': 6, 'race': 2, 'conflict': 0}[case] <= screens <= 64):
        raise ValueError('missing or unbounded own-sheet capture')
    if case == 'conflict' and screens != 0:
        raise ValueError('refused conflict must not claim a successful Native sheet')
    if (record.get('os') != 'macos' or record.get('architecture') != architecture
            or record.get('actual_os_login') != 'not_run'
            or record.get('installed_layout') != 'generated_not_supported_installation_proof'
            or record.get('receipt_before_plist') != 'source_order_only_not_terminal_chronology'
            or record.get('effective_login_start') != 'not_observed'
            or record.get('current_daemon_action') != 'none'
            or record.get('fixture_retention') != 'retained_private_no_unknown_data_deleted'
            or record.get('contract') != 'abg-registration-fixture-v1'
            or record.get('conflict_mutation_path') != ('explicit_bridge_only_ui_remains_disabled' if case == 'conflict' else 'production_dom_buttons')
            or record.get('provider_model_calls') != 'not_run' or record.get('failed') is not None):
        raise ValueError('unknown, failed or overstated lifecycle/chronology evidence')
    root = record.get('generated_fixture_root')
    if type(root) is not str or re.fullmatch(r'/private/tmp/abg-[0-9]{1,10}-[0-9]{1,20}', root) is None:
        raise ValueError('fixture root is not the fixed generated private namespace')


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def executable(path):
    path = Path(path).absolute()
    m = path.lstat()
    if (not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_uid not in (0, os.geteuid())
            or m.st_mode & 0o6022 or not m.st_mode & 0o111 or not 0 < m.st_size <= 128 * 1024 * 1024):
        raise ValueError('unsafe exact built executable')
    return path.resolve(strict=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--binary', required=True); p.add_argument('--daemon', required=True)
    p.add_argument('--output', required=True)
    a = p.parse_args()
    if platform.system() != 'Darwin':
        raise SystemExit('NOT_RUN: requires actual macOS GUI; Linux is a separate dimension')
    binary, daemon = executable(a.binary), executable(a.daemon)
    sibling = executable(binary.parent / 'batond')
    if digest(sibling) != digest(daemon):
        raise ValueError('fixed sibling daemon differs from exact declared build; do not overwrite unknown files')
    output = Path(a.output).absolute()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    architecture = {'arm64': 'aarch64', 'x86_64': 'x86_64'}.get(platform.machine())
    if architecture is None:
        raise ValueError('unknown Mac architecture')
    root = Path(__file__).resolve().parents[1]
    source_paths = ['apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/src/lib.rs',
                    'apps/desktop/src-tauri/src/background_service.rs', 'apps/desktop/src-tauri/src/native_parent.rs',
                    'apps/desktop/src-tauri/src/bin/native_background_service_smoke.rs',
                    'apps/desktop/src-tauri/src/native_background_service_test/mod.rs',
                    'apps/desktop/src-tauri/src/native_background_service_test/fixture.rs',
                    'apps/desktop/src-tauri/src/native_background_service_test/sheet.rs',
                    'apps/desktop/src-tauri/src/native_background_service_test/presentation.rs',
                    'apps/desktop/src/App.tsx', 'apps/desktop/src/BackgroundServiceSettings.tsx',
                    'apps/desktop/src/background-service-api.ts', 'scripts/native-background-service-smoke.py',
                    '.github/workflows/product.yml']
    source_hashes = {name: digest(root/name) for name in source_paths}
    git_head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    if re.fullmatch(r'[0-9a-f]{40}', git_head) is None:
        raise ValueError('exact checkout commit unavailable')
    frontend_root = root / 'apps/desktop/dist'
    frontend = sorted(frontend_root.rglob('*'))
    files = [f for f in frontend if not f.is_dir()]
    if not (frontend_root/'index.html').is_file() or not 0 < len(files) <= 256:
        raise ValueError('built real frontend missing/over bound')
    if any(f.is_symlink() or not f.is_file() for f in files) or sum(f.stat().st_size for f in files) > 32*1024*1024:
        raise ValueError('unsafe/overbound actual frontend')
    frontend_hashes = {str(f.relative_to(frontend_root)): digest(f) for f in files}
    before = {'binary': digest(binary), 'daemon': digest(daemon), 'sibling': digest(sibling)}
    manifest = {'status': 'running', 'dimension': 'generated_real_native_ipc_not_os_login',
                'architecture': architecture, 'inputs': before, 'git_head': git_head,
                'source_sha256': source_hashes, 'frontend_sha256': frontend_hashes, 'cases': {}, 'actual_os_login': 'not_run'}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    try:
        for case in CASES:
            directory = output / case; directory.mkdir(mode=0o700)
            with (directory / 'stdout.log').open('wb') as stdout, (directory / 'stderr.log').open('wb') as stderr:
                child = subprocess.Popen([str(binary), case, str(directory)], stdin=subprocess.DEVNULL,
                                         stdout=stdout, stderr=stderr, start_new_session=True)
                try:
                    code = child.wait(timeout=90)
                except subprocess.TimeoutExpired:
                    # Still-unreaped owned session leader pins this exact PGID.
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
                    raise RuntimeError('Native fixture alarm; no replay or permission fallback')
            report = directory / 'background-service.json'
            if code != 0 or report.stat().st_size > 65536:
                raise ValueError('Native fixture failed or report exceeds finite bound')
            record = json.loads(report.read_text(), object_pairs_hook=unique)
            validate_report(record, case, architecture)
            screenshots = sorted(directory.glob('step-*-page-*.png'))
            if len(screenshots) != record['screenshots']:
                raise ValueError('actual captured PNG count does not match Rust observations')
            for screen in screenshots:
                m = screen.lstat()
                if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or not 8 <= m.st_size <= 16 * 1024 * 1024:
                    raise ValueError('unsafe captured own-sheet evidence')
                with screen.open('rb') as stream:
                    if stream.read(8) != b'\x89PNG\r\n\x1a\n':
                        raise ValueError('capture is not PNG')
            manifest['cases'][case] = {'report_sha256': digest(report), 'screenshots': [digest(s) for s in screenshots]}
        if before != {'binary': digest(binary), 'daemon': digest(daemon), 'sibling': digest(sibling)}:
            raise ValueError('executable changed during Native sequence')
        if source_hashes != {name: digest(root/name) for name in source_paths} or frontend_hashes != {str(f.relative_to(frontend_root)): digest(f) for f in files}:
            raise ValueError('source/frontend changed during actual Native sequence')
        manifest['status'] = 'passed'
    finally:
        if manifest['status'] != 'passed':
            manifest['status'] = 'failed_or_interrupted'
        (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print('PASS generated Native registration IPC/real sheets; OS login and full lifecycle NOT_RUN')


if __name__ == '__main__':
    main()
