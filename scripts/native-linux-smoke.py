#!/usr/bin/env python3
"""Linux evidence lane; never substituted for actual macOS native acceptance."""
import argparse
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--sysroot', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not sys.platform.startswith('linux'):
        parser.error('this display/package-overlay lane is Linux only')
    binary = args.binary.resolve(strict=True)
    root = args.sysroot.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    libraries = root / 'usr/lib/x86_64-linux-gnu'
    xvfb = root / 'usr/bin/Xvfb'
    with tempfile.TemporaryDirectory(prefix='baton-native-display-') as private:
        private = Path(private)
        auth = private / 'Xauthority'
        auth.touch(mode=0o600)
        number = next(n for n in range(171, 230)
                      if not Path(f'/tmp/.X11-unix/X{n}').exists()
                      and not Path(f'/tmp/.X{n}-lock').exists())
        display = f':{number}'
        subprocess.run(['xauth', '-f', str(auth), 'add', display,
                        'MIT-MAGIC-COOKIE-1', secrets.token_hex(16)], check=True)
        env = dict(os.environ, DISPLAY=display, XAUTHORITY=str(auth))
        env['LD_LIBRARY_PATH'] = str(libraries) + (':' + env['LD_LIBRARY_PATH'] if env.get('LD_LIBRARY_PATH') else '')
        env['WEBKIT_EXEC_PATH'] = str(libraries / 'webkit2gtk-4.1')
        env['WEBKIT_INJECTED_BUNDLE_PATH'] = str(libraries / 'webkit2gtk-4.1/injected-bundle')
        for variable in ['XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_RUNTIME_DIR']:
            path = private / variable
            path.mkdir(mode=0o700)
            env[variable] = str(path)
        with (output / 'xvfb.log').open('w') as log:
            display_child = subprocess.Popen([str(xvfb), display, '-screen', '0',
                                              '1488x981x24', '-nolisten', 'tcp',
                                              '-auth', str(auth)], env=env,
                                             stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 5
                while not Path(f'/tmp/.X11-unix/X{number}').exists():
                    if display_child.poll() is not None:
                        raise RuntimeError('private Xvfb exited before readiness')
                    if time.monotonic() > deadline:
                        raise TimeoutError('private Xvfb readiness deadline')
                    time.sleep(0.05)
                runner = Path(__file__).with_name('native-run.py')
                result = subprocess.run([sys.executable, str(runner), '--output',
                                         str(output), '--', 'dbus-run-session', '--',
                                         str(binary), str(output)], env=env, timeout=90)
                (output / 'linux-lane.json').write_text(json.dumps({
                    'platform': 'linux', 'runner_exit_code': result.returncode,
                    'private_authenticated_display': True,
                    'sandbox_disabled_for_test': False,
                    'webkit_helper_override': str(libraries / 'webkit2gtk-4.1'),
                    'native_macos': 'not_run'}, indent=2) + '\n')
                return result.returncode
            finally:
                display_child.terminate()
                display_child.wait(timeout=5)


if __name__ == '__main__':
    sys.exit(main())
