#!/usr/bin/env python3
"""Bounded current-run companion transfer and two-stage Native CI receipts.

Not an artifact extractor, cache, authority grant, or production component.
Only fixed regular files are read/copied. GitHub downloads select the artifact
ID returned by this run's dependency; manifests additionally bind run/attempt.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TOOLS = {f'{profile}-{name}': f'target/{profile}/{name}'
         for profile in ('debug', 'release') for name in ('batond', 'baton')}
TOOL_LIMIT = 268435456
JSON_LIMIT = 1048576
TOTAL_LIMIT = 4 * TOOL_LIMIT + 2 * JSON_LIMIT
CONTRACT = 'agentbaton.native-ci-split.v1'
APP = 'apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop'


def require(value, message='Native CI evidence refused'):
    if not value:
        raise ValueError(message)


def unique(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, 'Duplicate JSON key')
        value[key] = item
    return value


def safe_path(path):
    path = Path(os.path.abspath(path))
    for parent in reversed((path, *path.parents)):
        require(not parent.is_symlink(), 'Symlink component refused')
    return path


def read(path, limit=JSON_LIMIT):
    path = safe_path(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and 0 < before.st_size <= limit, 'Regular bounded single-link file required')
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
        pin = lambda v: (v.st_dev, v.st_ino, v.st_size, v.st_mode, v.st_mtime_ns, v.st_ctime_ns, v.st_nlink)
        require(pin(before) == pin(after) and len(raw) == before.st_size, 'File changed during read')
    return raw


def load(path):
    return json.loads(read(path), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, 'Nonfinite JSON'))


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def sha(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value))
    return value


def write(path, value):
    path = safe_path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()


def identity():
    require(not git('status', '--porcelain', '--untracked-files=no'), 'Tracked source changed')
    commit = git('rev-parse', 'HEAD')
    require(commit == os.environ['GITHUB_SHA'], 'Checkout is not current test merge')
    require(re.fullmatch('[0-9a-f]{40}', os.environ['NATIVE_CI_PR_HEAD']))
    for name in ('GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT'):
        require(re.fullmatch('[1-9][0-9]{0,19}', os.environ[name]))
    require(os.environ['GITHUB_REPOSITORY'] == 'wanderbot-auto/AgentBaton')
    return {'repository': os.environ['GITHUB_REPOSITORY'], 'run': os.environ['GITHUB_RUN_ID'],
            'attempt': os.environ['GITHUB_RUN_ATTEMPT'], 'pr_head': os.environ['NATIVE_CI_PR_HEAD'],
            'test_merge': commit, 'tree': git('rev-parse', 'HEAD^{tree}')}


def arm_host():
    require(platform.system() == 'Darwin' and platform.machine() == 'arm64', 'Real Apple Silicon required')


def binary(path):
    raw = read(path, TOOL_LIMIT)
    # Thin ARM64 Mach-O executable, not a universal/x86 image or a declaration.
    require(len(raw) >= 32 and raw[:4] == b'\xcf\xfa\xed\xfe'
            and struct.unpack_from('<I', raw, 4)[0] == 0x0100000c
            and struct.unpack_from('<I', raw, 12)[0] == 2, 'ARM64 Mach-O executable required')
    return {'sha256': digest(raw), 'bytes': len(raw), 'architecture': 'arm64', 'format': 'mach-o-64'}


def tools():
    return {name: binary(ROOT / path) for name, path in TOOLS.items()}


def validate_tools(value):
    require(type(value) is dict and set(value) == set(TOOLS))
    for item in value.values():
        require(type(item) is dict and set(item) == {'sha256', 'bytes', 'architecture', 'format'})
        sha(item['sha256'])
        require(type(item['bytes']) is int and 0 < item['bytes'] <= TOOL_LIMIT
                and item['architecture'] == 'arm64' and item['format'] == 'mach-o-64')


def closed_directory(directory, names):
    directory = safe_path(directory)
    require(directory.is_dir() and set(p.name for p in directory.iterdir()) == set(names), 'Closed file whitelist required')
    total = 0
    for name in names:
        require('/' not in name and name not in ('.', '..'))
        total += len(read(directory / name, TOOL_LIMIT if name in TOOLS else JSON_LIMIT))
    require(total <= TOTAL_LIMIT)


def public_core():
    # The original finite projectors still run unmodified. Only their hashes
    # cross jobs; no DB, history, runtime, raw log, auth or arbitrary JSON data.
    paths = ('artifacts/native-smoke/legacy-migration/summary.json',
             'artifacts/native-smoke/factory-rework-progress.json')
    return {'contract': CONTRACT, 'identity': identity(), 'stage': 'core',
            'host': 'darwin-arm64', 'evidence': {p: digest(read(ROOT / p)) for p in paths}}


def pack(directory):
    arm_host()
    directory = safe_path(directory)
    directory.mkdir(mode=0o700, exist_ok=False)
    observed = tools()
    for name, path in TOOLS.items():
        raw = read(ROOT / path, TOOL_LIMIT)
        require(digest(raw) == observed[name]['sha256'])
        (directory / name).write_bytes(raw)
    write(directory / 'core.json', public_core())
    manifest = {'contract': CONTRACT, 'identity': identity(), 'stage': 'core',
                'host': 'darwin-arm64', 'tools': observed, 'core_sha256': digest(read(directory / 'core.json'))}
    write(directory / 'manifest.json', manifest)
    closed_directory(directory, (*TOOLS, 'core.json', 'manifest.json'))
    output('manifest_sha256', digest(read(directory / 'manifest.json')))


def validate_manifest(directory, expected):
    closed_directory(directory, (*TOOLS, 'core.json', 'manifest.json'))
    require(digest(read(directory / 'manifest.json')) == sha(expected), 'Manifest hash mismatch')
    value = load(directory / 'manifest.json')
    require(type(value) is dict and set(value) == {'contract', 'identity', 'stage', 'host', 'tools', 'core_sha256'})
    require(value['contract'] == CONTRACT and value['identity'] == identity()
            and value['stage'] == 'core' and value['host'] == 'darwin-arm64', 'Wrong source/run/attempt/host')
    validate_tools(value['tools'])
    for name in TOOLS:
        require(binary(directory / name) == value['tools'][name], 'Tool bytes/architecture changed')
    require(digest(read(directory / 'core.json')) == sha(value['core_sha256']))
    core = load(directory / 'core.json')
    paths = {'artifacts/native-smoke/legacy-migration/summary.json', 'artifacts/native-smoke/factory-rework-progress.json'}
    require(type(core) is dict and set(core) == {'contract', 'identity', 'stage', 'host', 'evidence'})
    require(core['contract'] == CONTRACT and core['identity'] == identity() and core['stage'] == 'core'
            and core['host'] == 'darwin-arm64' and type(core['evidence']) is dict and set(core['evidence']) == paths)
    for item in core['evidence'].values():
        sha(item)
    return value


def restore(directory, expected):
    arm_host()
    value = validate_manifest(directory, expected)
    for name, path in TOOLS.items():
        target = safe_path(ROOT / path)
        target.parent.mkdir(parents=True, exist_ok=True)
        require(not target.exists(), 'Fresh consumer destination required')
        fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o700)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(read(directory / name, TOOL_LIMIT))
        require(binary(target) == value['tools'][name])


def probe_actual(directory):
    arm_host()
    directory = safe_path(directory)
    directory.mkdir(mode=0o700, exist_ok=False)
    before = tools()
    spec = importlib.util.spec_from_file_location('normal_companion_probe', ROOT / 'scripts/prepare-factory-test-tools.py')
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    observations = {}
    for profile in ('debug', 'release'):
        evidence = directory / profile
        evidence.mkdir(mode=0o700)
        observations[profile] = helper.probe(ROOT / 'target' / profile, evidence)
    require(before == tools(), 'Actual companion bytes changed during probe')
    # Existing probe logs stay private in RUNNER_TEMP. Public receipt is closed.
    write(directory / 'actual.json', {'contract': CONTRACT, 'identity': identity(),
          'host': 'darwin-arm64', 'tools': before, 'empty_schema': 18,
          'debug_and_release_probe': True, 'sigterm_joined': True})


def check_actual(directory):
    arm_host()
    actual = load(directory / 'actual.json')
    require(type(actual) is dict and set(actual) == {'contract', 'identity', 'host', 'tools', 'empty_schema', 'debug_and_release_probe', 'sigterm_joined'})
    require(actual['contract'] == CONTRACT and actual['identity'] == identity() and actual['host'] == 'darwin-arm64'
            and actual['empty_schema'] == 18 and actual['debug_and_release_probe'] is True and actual['sigterm_joined'] is True)
    validate_tools(actual['tools'])
    require(actual['tools'] == tools(), 'Reconstructed tools changed: fresh provenance required')
    # native-macos-smoke copies the actual normal daemon beside its own driver.
    require(binary(ROOT / 'apps/desktop/src-tauri/target/debug/batond') == actual['tools']['debug-batond'])
    for name in ('baton', 'batond'):
        require(binary(ROOT / f'apps/desktop/src-tauri/binaries/{name}-aarch64-apple-darwin') == actual['tools']['release-' + name])
    return actual


def complete(directory, expected, actual_dir, receipt_dir):
    arm_host()
    validate_manifest(directory, expected)
    actual = check_actual(actual_dir)
    require(os.environ['NATIVE_CI_CANCELLED'] == 'false')
    image = binary(ROOT / APP)
    # The exact unmodified default build, release audit, package and successful
    # upload precede this step. Package content is checked here as real files.
    development = ROOT / 'artifacts/development'
    dmg = f'AgentBaton-arm64-{identity()["test_merge"]}-development.dmg'
    names = (dmg, 'build-record.txt', 'DEVELOPMENT_ACCEPTANCE.md', 'SHA256SUMS')
    require(set(p.name for p in safe_path(development).iterdir()) == set(names))
    package = read(development / dmg, TOOL_LIMIT)
    sums = read(development / 'SHA256SUMS').decode()
    require(sums == digest(package) + '  ./' + dmg + '\n', 'Package checksum mismatch')
    build = read(development / 'build-record.txt').decode()
    require(f'commit={identity()["test_merge"]}\narchitecture=arm64\n' in build)
    read(development / 'DEVELOPMENT_ACCEPTANCE.md')
    receipt_dir = safe_path(receipt_dir)
    receipt_dir.mkdir(mode=0o700, exist_ok=False)
    record = {'contract': CONTRACT, 'identity': identity(), 'host': 'darwin-arm64',
              'stage': 'native-default', 'producer_manifest_sha256': sha(expected),
              'actual_tools': actual['tools'], 'actual_probe': True,
              'default_app': image, 'profile': 'release', 'features': [],
              'package_sha256': digest(package), 'package_bytes': len(package),
              'native_success': True, 'default_build_success': True,
              'release_audit_success': True, 'package_success': True, 'development_upload_success': True}
    write(receipt_dir / 'native.json', record)
    output('receipt_sha256', digest(read(receipt_dir / 'native.json')))


def aggregate(directory, expected, receipt_dir, receipt_sha):
    require(os.environ['NATIVE_CI_CANCELLED'] == 'false', 'Workflow cancelled')
    needs = json.loads(os.environ['NATIVE_CI_NEEDS'], object_pairs_hook=unique)
    require(type(needs) is dict and set(needs) == {'desktop', 'native'})
    for name in needs:
        require(needs[name]['result'] == 'success', 'Both genuine ARM stages must succeed')
        require(re.fullmatch('[1-9][0-9]{0,19}', needs[name]['outputs']['artifact_id']))
    require(needs['desktop']['outputs']['manifest_sha256'] == expected
            and needs['native']['outputs']['receipt_sha256'] == receipt_sha)
    validate_manifest(directory, expected)
    closed_directory(receipt_dir, ('native.json',))
    require(digest(read(receipt_dir / 'native.json')) == sha(receipt_sha))
    value = load(receipt_dir / 'native.json')
    keys = {'contract', 'identity', 'host', 'stage', 'producer_manifest_sha256', 'actual_tools', 'actual_probe',
            'default_app', 'profile', 'features', 'package_sha256', 'package_bytes', 'native_success',
            'default_build_success', 'release_audit_success', 'package_success', 'development_upload_success'}
    require(type(value) is dict and set(value) == keys)
    require(value['contract'] == CONTRACT and value['identity'] == identity() and value['host'] == 'darwin-arm64'
            and value['stage'] == 'native-default' and value['producer_manifest_sha256'] == expected
            and value['profile'] == 'release' and value['features'] == [])
    validate_tools(value['actual_tools'])
    validate_tools({name: value['default_app'] for name in TOOLS})
    for key in ('actual_probe', 'native_success', 'default_build_success', 'release_audit_success', 'package_success', 'development_upload_success'):
        require(value[key] is True, 'Incomplete default/native acceptance')
    sha(value['package_sha256'])
    require(type(value['package_bytes']) is int and 0 < value['package_bytes'] <= TOOL_LIMIT)
    print('PASS both current-run ARM stages, actual tools, Native and default release audit/package/upload')


def output(name, value):
    with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
        stream.write(name + '=' + value + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pack', 'restore', 'probe', 'check', 'complete', 'aggregate'))
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--expected', default='')
    parser.add_argument('--actual', type=Path)
    parser.add_argument('--receipt', type=Path)
    parser.add_argument('--receipt-sha', default='')
    args = parser.parse_args()
    if args.mode == 'pack': pack(args.directory)
    elif args.mode == 'restore': restore(args.directory, args.expected)
    elif args.mode == 'probe': probe_actual(args.directory)
    elif args.mode == 'check': check_actual(args.directory)
    elif args.mode == 'complete': complete(args.directory, args.expected, args.actual, args.receipt)
    else: aggregate(args.directory, args.expected, args.receipt, args.receipt_sha)


if __name__ == '__main__':
    main()
