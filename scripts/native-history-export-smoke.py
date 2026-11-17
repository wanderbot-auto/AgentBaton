#!/usr/bin/env python3
"""Own a real Apple Silicon desktop session for CUA-driven product acceptance.

Run through collab.sh run --native. This runner never drives UI or calls export
IPC: use the production project details UI to preview/publish/reveal, then enter
'finish'. Own daemon/app children are terminated and joined on every exit.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('history_export_smoke', ROOT / 'scripts/history-export-smoke.py')
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT / 'artifacts/history-export/native')
    args = parser.parse_args()
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        raise RuntimeError('This acceptance requires real Apple Silicon macOS')
    args.output.mkdir(parents=True, exist_ok=True)
    runtime = Path(tempfile.mkdtemp(prefix='ab-hexp-native-')).resolve()
    runtime.chmod(0o700)
    process = app = None
    try:
        with (args.output / 'daemon.log').open('w') as daemon_log, (args.output / 'app.log').open('w') as app_log:
            project = smoke.seed(runtime, daemon_log)
            before = smoke.business_hash(runtime)
            process = smoke.start(runtime, daemon_log)
            binary = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT/'target'))) / 'debug/agentbaton-desktop'
            app = subprocess.Popen([str(binary)], env=dict(os.environ, BATON_RUNTIME_DIR=str(runtime)), stdout=app_log, stderr=app_log)
            session = dict(status='awaiting_real_ui', runtime=str(runtime), project_id=project['id'], daemon_pid=process.pid, app_pid=app.pid, architecture=platform.machine(), os=platform.mac_ver()[0], binary=str(binary), provider_calls='not_run')
            (args.output / 'session.json').write_text(json.dumps(session, indent=2)+'\n')
            print(json.dumps(session, indent=2), flush=True)
            print('Use the real product UI: project details -> text/events selection -> preview -> generate -> Finder. Enter finish after observation; abort stops children.', flush=True)
            command = sys.stdin.readline().strip()
            if command != 'finish':
                raise RuntimeError('Native acceptance was not confirmed by the operator')
            packages = list((runtime/'history-exports').glob('*.history.json'))
            if len(packages) != 1:
                raise AssertionError('Expected exactly one UI-generated published package')
            package = json.loads(packages[0].read_bytes())
            reference = dict(preview_id=package['manifest']['export_id'], selection_digest=package['manifest']['selection_digest'], package_sha256=smoke.digest(packages[0].read_bytes()))
            data = smoke.validate_package(packages[0], reference)
            assert package['manifest']['selection']['project_id'] == project['id']
            assert package['manifest']['selection']['messages'] is True
            assert before == smoke.business_hash(runtime)
            destination = args.output/'sample.history.json'
            destination.write_bytes(data)
            destination.chmod(0o600)
            session.update(status='passed', dimension='real_macos_arm64_product_webview_tauri_daemon_sqlite_with_cua_observation', reference=reference, artifact=str(destination.resolve()), sha256=smoke.digest(data), bytes=len(data), business_history_unchanged=True, ui_automation='cua_repl_actual_app', native_reveal='operator_observation_required', fixture_retention='retained_private_no_automatic_deletion', runtime=str(runtime))
            (args.output/'report.json').write_text(json.dumps(session, indent=2)+'\n')
            print(json.dumps(session, indent=2), flush=True)
    finally:
        if app is not None:
            smoke.stop(app)
        if process is not None:
            smoke.stop(process)
        (args.output/'cleanup.json').write_text(json.dumps(dict(app_reaped=app is None or app.poll() is not None, daemon_reaped=process is None or process.poll() is not None, retained_runtime=str(runtime)), indent=2)+'\n')


if __name__ == '__main__':
    main()
