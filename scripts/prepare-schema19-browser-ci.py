#!/usr/bin/env python3
"""Build and freeze the common schema 19 Linux browser evidence candidate."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import subprocess
import time

SCHEMA = 19
TOOLS = ('batond', 'baton')
TOOL_LIMIT = 128 * 1024 * 1024
FRONTEND_LIMIT = 64 * 1024 * 1024
FILE_LIMIT = 16 * 1024 * 1024
RECORD_LIMIT = 1_048_576
CARGO = ['cargo', 'build', '--locked', '--bin', 'batond', '--bin', 'baton',
    '--features', 'agentbaton-core/preview-evidence']
NPM = ['npm', '--prefix', 'apps/desktop', 'run', 'build']
FEATURES = ['agentbaton-core/preview-evidence']
SCOPE = ('CI-built Linux dev evidence candidate. Only this tool build sets CARGO_PROFILE_DEV_DEBUG=0. '
    'Other profile settings and package overrides are unchanged. Preview evidence is non-default and release-prohibited.')
PUBLIC = Path('artifacts/ui-browser')
FRONTEND = Path('apps/desktop/dist')
CORE_TOOLS = Path('target/debug')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def decode(raw):
    def unique(pairs):
        value = {}
        for key, item in pairs:
            require(key not in value, 'Duplicate browser evidence key')
            value[key] = item
        return value

    def invalid_constant(_):
        raise ValueError('Non-finite browser evidence value')

    result = json.loads(raw, object_pairs_hook=unique, parse_constant=invalid_constant)
    require(type(result) is dict, 'Browser evidence record must be an object')
    return result


def write_bytes(path, raw):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb') as stream:
        os.fchmod(stream.fileno(), 0o600)
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def encode(value):
    return (json.dumps(value, indent=2, allow_nan=False) + '\n').encode()


def git_identity():
    def git(*argv):
        return subprocess.check_output(['git', *argv], text=True).strip()

    require(not git('status', '--porcelain=v1', '--untracked-files=no'), 'Browser build requires a clean tracked tree')
    head, tree = git('rev-parse', 'HEAD', 'HEAD^{tree}').splitlines()
    require(all(re.fullmatch('[0-9a-f]{40}', value) for value in (head, tree)), 'Invalid tracked source identity')
    return head, tree


def stamp(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink,
        info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def file_pin(path, limit, *, directory=None, executable=False, single_link=False, receive=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= limit
            and not before.st_mode & 0o022 and before.st_uid == os.geteuid(), 'Unsafe browser input file')
        if executable:
            require(before.st_mode & 0o500 == 0o500 and not before.st_mode & 0o7000,
                'Unsafe Core tool mode')
        if single_link:
            require(before.st_nlink == 1, 'Browser input must have a single link')
        digest = hashlib.sha256()
        remaining = before.st_size
        while block := stream.read(min(1_048_576, remaining + 1)):
            remaining -= len(block)
            require(remaining >= 0, 'Browser input grew during verification')
            digest.update(block)
            if receive is not None:
                receive(block)
        after = os.fstat(stream.fileno())
        current = os.stat(path, dir_fd=directory, follow_symlinks=False)
        require(remaining == 0 and stamp(before) == stamp(after) == stamp(current),
            'Browser input identity changed')
        return {'sha256': digest.hexdigest(), 'bytes': before.st_size, 'mode': stat.S_IMODE(before.st_mode),
            'identity': list(stamp(before))}


def read_record(path):
    raw = bytearray()
    file_pin(path, RECORD_LIMIT, single_link=True, receive=raw.extend)
    return bytes(raw)


def frontend_assets(frontend):
    assets = {}
    entries = 0
    remaining = FRONTEND_LIMIT

    def walk(path, prefix='', parent=None):
        nonlocal entries, remaining
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY, dir_fd=parent)
        try:
            before = os.fstat(fd)
            with os.scandir(fd) as children:
                for entry in children:
                    entries += 1
                    name = prefix + entry.name
                    require(entries <= 512 and re.fullmatch('[A-Za-z0-9._/-]{1,256}', name),
                        'Frontend asset path or count refused')
                    if entry.is_dir(follow_symlinks=False):
                        walk(entry.name, name + '/', fd)
                    else:
                        pin = file_pin(entry.name, min(remaining, FILE_LIMIT), directory=fd, single_link=True)
                        remaining -= pin['bytes']
                        assets[name] = pin['sha256']
            require(stamp(before) == stamp(os.fstat(fd)) == stamp(os.stat(path, dir_fd=parent, follow_symlinks=False)),
                'Frontend directory identity changed')
        finally:
            os.close(fd)

    walk(frontend)
    require('index.html' in assets, 'Frontend index is missing')
    return dict(sorted(assets.items()))


def core_config():
    return {name: file_pin(Path(name), RECORD_LIMIT, single_link=True)['sha256']
        for name in ('Cargo.toml', 'Cargo.lock')}


def core_pins(directory):
    return {name: file_pin(directory / name, TOOL_LIMIT, executable=True) for name in TOOLS}


def timed_build(kind, output):
    core = kind == 'core'
    command = CARGO if core else NPM
    head, tree = git_identity()
    config = core_config() if core else None
    if core:
        require(os.environ.get('CARGO_PROFILE_DEV_DEBUG') == '0', 'Expected preview-only dev debuginfo setting')
    start = time.monotonic()
    result = subprocess.run(command, check=False)
    seconds = time.monotonic() - start
    try:
        unchanged = git_identity() == (head, tree) and (not core or core_config() == config)
    except (ValueError, OSError, subprocess.SubprocessError):
        unchanged = False
    record = {'source': head, 'git_tree': tree, 'command': command, 'exit': result.returncode,
        'seconds': seconds, 'source_head_and_tracked_unchanged': unchanged}
    if core:
        record.update(cargo_profile_dev_debug='0', cargo_features=FEATURES, cargo_inputs_sha256=config)
    if result.returncode == 0 and unchanged:
        try:
            if core:
                record['tool_pins'] = core_pins(CORE_TOOLS)
            else:
                record['frontend_assets'] = frontend_assets(FRONTEND)
        except (ValueError, OSError):
            write_bytes(output, encode(record))
            raise
    write_bytes(output, encode(record))
    return result.returncode or (0 if unchanged else 1)


def verify_build(record, head, tree, command):
    elapsed = record.get('seconds')
    require(record.get('source') == head and record.get('git_tree') == tree
        and record.get('command') == command and type(record.get('exit')) is int and record['exit'] == 0
        and record.get('source_head_and_tracked_unchanged') is True
        and type(elapsed) in (int, float) and math.isfinite(elapsed) and elapsed >= 0,
        'Paired build source, command or completion refused')


def verify_pair(manifest_raw, frontend_raw, head, tree, frontend=FRONTEND):
    manifest, build = decode(manifest_raw), decode(frontend_raw)
    require(type(manifest.get('schema')) is int and manifest['schema'] == SCHEMA
        and all(manifest.get(name) == head for name in
            ('git_head', 'source_head', 'compiled_core_source_head', 'frontend_source_head'))
        and manifest.get('git_tree') == tree and manifest.get('dirty') is False and manifest.get('tracked_clean') is True
        and manifest.get('frontend_build_record_sha256') == sha(frontend_raw)
        and manifest.get('cargo_command') == CARGO and manifest.get('cargo_features') == FEATURES
        and manifest.get('cargo_profile_dev_debug') == '0', 'Paired candidate source or build configuration refused')
    verify_build(build, head, tree, NPM)
    require(manifest.get('frontend_assets') == build.get('frontend_assets') == frontend_assets(frontend),
        'Frontend assets do not match the paired build')
    tools = manifest.get('binaries')
    require(type(tools) is dict and set(tools) == set(TOOLS)
        and all(type(value) is str and re.fullmatch('[a-f0-9]{64}', value) for value in tools.values())
        and len(set(tools.values())) == len(TOOLS), 'Invalid paired Core tool digests')
    return manifest


def copy_core_tool(source, destination, expected):
    with os.fdopen(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb') as output:
        pin = file_pin(source, TOOL_LIMIT, executable=True, receive=output.write)
        require(pin == expected, 'Core build tool pin changed')
        output.flush()
        os.fsync(output.fileno())
        os.fchmod(output.fileno(), 0o500)
    copied = file_pin(destination, TOOL_LIMIT, executable=True, single_link=True)
    require(copied['sha256'] == pin['sha256'] and copied['bytes'] == pin['bytes'] and copied['mode'] == 0o500,
        'Frozen Core tool copy changed')
    return copied


def prepare_candidate(core_record, frontend_record, private):
    head, tree = git_identity()
    core_raw, frontend_raw = read_record(core_record), read_record(frontend_record)
    core, build = decode(core_raw), decode(frontend_raw)
    verify_build(core, head, tree, CARGO)
    verify_build(build, head, tree, NPM)
    require(core.get('cargo_profile_dev_debug') == '0' and core.get('cargo_features') == FEATURES
        and core.get('cargo_inputs_sha256') == core_config(), 'Core build configuration changed')
    require(build.get('frontend_assets') == frontend_assets(FRONTEND), 'Built frontend assets changed')
    original = core_pins(CORE_TOOLS)
    require(core.get('tool_pins') == original, 'Core tools do not match the measured build')
    private.mkdir(mode=0o700, exist_ok=False)
    tools = private / 'tools'
    tools.mkdir(mode=0o700)
    copied = {name: copy_core_tool(CORE_TOOLS / name, tools / name, original[name]) for name in TOOLS}
    candidate = {'schema': SCHEMA, 'git_head': head, 'git_tree': tree, 'source_head': head,
        'compiled_core_source_head': core['source'], 'frontend_source_head': build['source'],
        'dirty': False, 'tracked_clean': True, 'cargo_features': FEATURES, 'cargo_command': CARGO,
        'cargo_profile_dev_debug': '0', 'cargo_inputs_sha256': core['cargo_inputs_sha256'],
        'core_build_record_sha256': sha(core_raw), 'frontend_build_record_sha256': sha(frontend_raw),
        'binaries': {name: pin['sha256'] for name, pin in copied.items()}, 'frontend_assets': build['frontend_assets'],
        'scope': SCOPE}
    raw = encode(candidate)
    verify_pair(raw, frontend_raw, head, tree)
    require(core_pins(CORE_TOOLS) == original and read_record(core_record) == core_raw
        and read_record(frontend_record) == frontend_raw and git_identity() == (head, tree),
        'Browser candidate inputs changed during freezing')
    canonical, compatible = (PUBLIC / (name + '-candidate.json') for name in ('goal-execution', 'factory-child-planning'))
    write_bytes(canonical, raw)
    write_bytes(compatible, raw)
    require(read_record(canonical) == read_record(compatible) == raw, 'Compatibility candidate differs from canonical bytes')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stage', choices=('frontend', 'core', 'freeze'))
    parser.add_argument('--runner-temp', type=Path, required=True)
    args = parser.parse_args()
    runner = args.runner_temp.resolve(strict=True)
    PUBLIC.mkdir(parents=True, exist_ok=True)
    core = runner / 'agentbaton-browser-core-build.json'
    frontend = PUBLIC / 'frontend-build-record.json'
    if args.stage == 'freeze':
        prepare_candidate(core, frontend, runner / 'agentbaton-browser-candidate')
        return 0
    return timed_build(args.stage, core if args.stage == 'core' else frontend)


if __name__ == '__main__':
    raise SystemExit(main())
