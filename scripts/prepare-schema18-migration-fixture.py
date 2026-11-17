#!/usr/bin/env python3
"""Freeze normal accepted18 tools. Import is inert; no fetch/config/feature changes."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import types

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'a4e03303b54bd5a5544131e95ad39662f8310e29'
TREE = 'ddd5bddbeeb8011fa070d53e67f7978deae87214'
REPOSITORY = 'https://github.com/wanderbot-auto/AgentBaton.git'
CONTRACT = 'agentbaton.schema18-source-build.v1'
ARCHIVE_SHA256 = '1995df43686db331d61e5d7bdbfdb0146de7eeaa1dd1a784be42597b73f58bb3'
SOURCE_FILES_SHA256 = '6f064dafeed10bdd60470301262f57472531a935f8f9fc4e849d021ca2412db4'
LOCK_SHA256 = '539b5d31c06ece04833a6d4e17b676c1afd827b9e12cb4938aa3a8ff6fa38d38'
PREFIXES = ('Cargo.toml', 'Cargo.lock', 'crates', 'apps/daemon', 'apps/cli')
HELPERS = {
    'prepare-schema16-migration-fixture.py': '5e814e979115d15a9696b78d586f63a30c01df3548b323dd99f8f5acbd48713a',
    'prepare-legacy-migration-fixture.py': 'b69f07713271d07992150db523b9344d9afc7db39cc0d31921ec49e643004b90',
    'native-run.py': '205d76e38721760744bd4d889e34ab3d2cf436ebc94583fee40fa17029ffac0a',
}
BUILD_ARGS = ['build', '--offline', '--locked', '-j', '2', '--bin', 'batond', '--bin', 'baton']


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def load_helper(name):
    path = ROOT / 'scripts' / name
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and not info.st_mode & 0o022, 'Unsafe helper source')
    raw = path.read_bytes()
    require(sha(raw) == HELPERS[name], 'Changed immutable helper')
    module = types.ModuleType('pinned18_' + name.replace('-', '_'))
    module.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


# Reuse the reviewed secure read/tool metadata and process lifecycle functions.
# No old builder/main/probe is invoked and no old schema contract is changed.
OLD = load_helper('prepare-schema16-migration-fixture.py')
COMMON = load_helper('prepare-legacy-migration-fixture.py')
NATIVE = load_helper('native-run.py')
read_regular = OLD.read_regular
unique = OLD.unique
strict_equal = OLD.strict_equal
tool_records = OLD.tool_records
SCHEMA_TABLES = tuple(sorted(OLD.SCHEMA_TABLES + (
    'integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences',
    'conversations', 'conversation_dispatches', 'conversation_turns', 'conversation_budgets')))
EMPTY_AUTHORITY = tuple(sorted(set(OLD.EMPTY_AUTHORITY + OLD.FACTORY + (
    'integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences',
    'conversations', 'conversation_dispatches', 'conversation_turns', 'conversation_budgets'))))


def parse_json(raw):
    def bad_constant(_):
        raise ValueError('Non-finite JSON')
    return json.loads(raw, object_pairs_hook=unique, parse_constant=bad_constant)


def native_platform():
    require((sys.platform == 'darwin' and platform.machine() == 'arm64') or
            (sys.platform == 'linux' and platform.machine() in ('x86_64', 'aarch64')),
            'Supported native platform required; macOS is ARM64 only')


def safe_name(name):
    path = PurePosixPath(name)
    return (bool(name) and not path.is_absolute() and '..' not in path.parts
            and str(path) == name and any(name == p or name.startswith(p + '/') for p in PREFIXES))


def archive_files(raw):
    require(0 < len(raw) <= 67_108_864, 'Archive bound exceeded')
    files, seen, total = {}, set(), 0
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:') as archive:
        for entry in archive:
            require(len(seen) < 4096 and entry.name not in seen, 'Duplicate/too many archive paths')
            seen.add(entry.name)
            name = entry.name.rstrip('/') if entry.isdir() else entry.name
            require((safe_name(name) or (entry.isdir() and name == 'apps'))
                    and (entry.isdir() or entry.isfile()) and not entry.mode & 0o7000,
                    'Unsafe archive path/link/type/mode')
            if entry.isfile():
                require(0 <= entry.size <= 16_777_216, 'Oversized archive member')
                total += entry.size
                require(total <= 67_108_864, 'Extracted source bound exceeded')
                data = archive.extractfile(entry).read(16_777_217)
                require(len(data) == entry.size, 'Short archive member')
                files[name] = sha(data)
    return files


def validate_index(files):
    require(type(files) is dict and len(files) == 244 and all(
        type(k) is str and safe_name(k) and OLD.digest(v) for k, v in files.items())
        and sha(canonical(files)) == SOURCE_FILES_SHA256
        and files.get('Cargo.lock') == LOCK_SHA256, 'Wrong complete fixed-source index/lock')


def extract_source(raw, destination):
    require(sha(raw) == ARCHIVE_SHA256, 'Wrong fixed source archive')
    files = archive_files(raw)
    validate_index(files)
    require(destination.is_dir() and not any(destination.iterdir()), 'Empty source destination required')
    # This reviewed extractor only receives the already fully pinned archive.
    COMMON.extract_source(raw, destination)
    require(source_hashes(destination) == files, 'Extracted source differs')
    return files


def source_hashes(source):
    require(source.is_absolute() and source.resolve(strict=True) == source, 'Canonical source required')
    files, count = {}, 0
    for path in sorted(source.rglob('*')):
        count += 1
        require(count <= 4096 and not path.is_symlink(), 'Unexpected source link/path bound')
        info = path.lstat()
        require(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), 'Unexpected source type')
        if stat.S_ISREG(info.st_mode):
            name = path.relative_to(source).as_posix()
            require(safe_name(name) and info.st_size <= 16_777_216 and info.st_nlink == 1
                    and info.st_uid == os.geteuid() and not info.st_mode & 0o077,
                    'Unexpected source path/size/owner/link/mode')
            raw = read_regular(path, 16_777_216, stat.S_IMODE(info.st_mode)) if info.st_size else b''
            files[name] = sha(raw)
    return files


def validate_compiled(m):
    native_platform()
    require(type(m) is dict and m.get('contract') == CONTRACT and m.get('status') == 'compiled'
            and type(m.get('schema')) is int and m['schema'] == 18, 'Wrong compiled18 contract')
    s, b = m['source'], m['build']
    require(s['commit'] == SOURCE and s['tree'] == TREE and s['repository'] == REPOSITORY
            and s['archive_sha256'] == ARCHIVE_SHA256 and s['compiled_source_clean'] is True,
            'Wrong compiled source/archive')
    validate_index(s['files_sha256'])
    require(m['builder_sha256'] == sha(Path(__file__).read_bytes())
            and strict_equal(m['helpers_sha256'], HELPERS), 'Wrong builder/helper bytes')
    require(m['platform'] == sys.platform and m['architecture'] == platform.machine(), 'Wrong native platform')
    require(type(b['argv']) is list and len(b['argv']) == 10 and Path(b['argv'][0]).is_absolute()
            and b['argv'][1:] == BUILD_ARGS and b['features'] == [] and b['default_features'] is True
            and type(b['exit_code']) is int and b['exit_code'] == 0
            and b['lockfile_sha256'] == LOCK_SHA256 and b['group_quiescent_before_reap'] is True,
            'Not a normal joined locked default build')
    require(b['cwd'] == s['directory'] and Path(b['target_dir']).is_absolute()
            and b['cargo_config_paths_checked'] == cargo_config_paths(Path(s['directory']))
            and b['cargo_config_absent_before'] is True and b['cargo_config_absent_after'] is True,
            'Ambient Cargo config absence not bound to complete ancestor chain')
    require(set(m['binaries']) == {'batond', 'baton'}, 'Missing normal tool pair')
    for item in m['binaries'].values():
        require(OLD.digest(item['sha256']) and type(item['bytes']) is int
                and 0 < item['bytes'] <= 268_435_456 and item['mode'] == '0o500'
                and type(item['nlink']) is int and item['nlink'] == 1
                and item['architecture'] == platform.machine(), 'Invalid frozen tool record')
    for name in ('cargo', 'rustc'):
        tool = m['toolchain'][name]
        require(type(tool['path']) is str and Path(tool['path']).is_absolute()
                and OLD.digest(tool['sha256']) and type(tool['version']) is str
                and 0 < len(tool['version']) <= 512, 'Missing compiler provenance')


def provenance(frozen, manifest):
    require(frozen.is_absolute() and frozen.resolve(strict=True) == frozen, 'Canonical frozen path required')
    require(manifest is not None and manifest.absolute() == frozen / 'source-build-manifest.json',
            'Exact adjacent compiled source-build manifest required; no unbound tool fallback')
    raw = read_regular(manifest, 4_194_304)
    record = parse_json(raw)
    validate_compiled(record)
    require(sha(read_regular(frozen.parent / 'source.tar', 67_108_864)) == ARCHIVE_SHA256,
            'Changed saved source archive')
    measured = tool_records(frozen)
    require(record['frozen_directory'] == str(frozen) and strict_equal(measured, record['binaries']),
            'Wrong frozen tool/path/source binding')
    source = Path(record['source']['directory'])
    parent = source.parent
    info = parent.lstat()
    require(source.name == 'source' and parent.name.startswith('ab18-build-') and parent.resolve() == parent
            and stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
            and stat.S_IMODE(info.st_mode) == 0o700, 'Wrong private source root')
    require(source_hashes(source) == record['source']['files_sha256'], 'Source changed after normal build')
    return record, raw, measured


def git_archive(repository):
    env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null', GIT_NO_REPLACE_OBJECTS='1')
    prefix = ['git', '--no-replace-objects', '-c', 'core.fsmonitor=false', '-c',
              'core.hooksPath=/dev/null', '-c', 'core.attributesFile=/dev/null', '-c', 'tar.umask=0002',
              '-C', str(repository)]
    for suffix, expected in [('^{commit}', SOURCE), ('^{tree}', TREE)]:
        require(subprocess.check_output(prefix + ['rev-parse', SOURCE + suffix], env=env,
                                        timeout=10).decode().strip() == expected, 'Wrong fixed Git object')
    raw = subprocess.check_output(prefix + ['archive', '--format=tar', SOURCE, *PREFIXES],
                                  env=env, timeout=20)
    require(sha(raw) == ARCHIVE_SHA256, 'Wrong archived source bytes')
    return raw


def cargo_config_paths(source):
    return [str(parent / '.cargo' / name) for parent in (source, *source.parents)
            for name in ('config', 'config.toml')]


def assert_no_ambient_config(source):
    paths = cargo_config_paths(source)
    for name in paths:
        path = Path(name)
        require(not path.exists() and not path.is_symlink(), 'Ambient Cargo configuration refused')
    return paths


def run_build(argv, source, env, log, report):
    child, owned, joined = None, None, False
    try:
        child = subprocess.Popen(argv, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        report['owned_identity'] = None
        owned = COMMON.darwin_owned_identity(child)
        report['owned_identity'] = owned
        deadline = time.monotonic() + 1800
        while True:
            exited = NATIVE.owned_child_exited(child.pid)
            require(type(exited) is bool, 'Unknown compiler exit observation')
            if exited:
                break
            require(time.monotonic() < deadline, 'Normal18 compile deadline expired')
            time.sleep(.5)
        require(NATIVE.owned_group_is_quiescent(child.pid) is True, 'Compiler group still active')
        report['group_quiescent_before_reap'] = True
        report['exit_code'] = child.wait(timeout=5)
        joined = True
    finally:
        if child is not None and not joined:
            cleanup = dict(owned_identity=owned, sigterm_sent=False, forced_kill=False)
            COMMON.stop_darwin_owned(child, NATIVE, cleanup)
            report['failed_build_cleanup'] = cleanup
    require(report['exit_code'] == 0, 'Normal18 build failed; keep first build.log')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('repository', 'output', 'target-dir', 'cargo', 'rustc'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    native_platform()
    output = args.output.absolute()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    output = output.resolve()
    frozen = output / 'frozen'
    frozen.mkdir(mode=0o700)
    report = dict(contract=CONTRACT, status='failed_or_interrupted', schema=18,
                  platform=sys.platform, architecture=platform.machine(), os_release=platform.release(),
                  builder_sha256=sha(Path(__file__).read_bytes()), helpers_sha256=HELPERS,
                  frozen_directory=str(frozen), not_claimed=['runtime probe (separate generator)',
                  'Store19 activation/backup/restore', 'native GUI/Provider/SSH'])
    try:
        require(shutil.disk_usage(output).free >= 200 * 1024 * 1024, 'Insufficient private build space')
        root = Path(tempfile.mkdtemp(prefix='ab18-build-', dir='/tmp')).resolve()
        root.chmod(0o700)
        source = root / 'source'
        source.mkdir(mode=0o700)
        raw = git_archive(args.repository.resolve(strict=True))
        with (output / 'source.tar').open('xb') as stream:
            stream.write(raw)
        (output / 'source.tar').chmod(0o600)
        files = extract_source(raw, source)
        report['source'] = dict(commit=SOURCE, tree=TREE, repository=REPOSITORY, directory=str(source),
                                archive_sha256=sha(raw), files_sha256=files, compiled_source_clean=False)
        # A private Cargo home shares only the public crate cache, not auth/config.
        cargo_home = root / 'cargo-cache'
        cargo_home.mkdir(mode=0o700)
        registry = (Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'registry').resolve(strict=True)
        (cargo_home / 'registry').symlink_to(registry, target_is_directory=True)
        target_root = Path(os.environ['CARGO_TARGET_DIR']).resolve()
        target = args.target_dir.resolve()
        require(args.target_dir.is_absolute() and args.target_dir == target, 'Canonical target path required')
        require(target.is_relative_to(target_root) and target != target_root, 'Task-private target subdirectory required')
        target.mkdir(mode=0o700, parents=True, exist_ok=False)
        target = target.resolve()
        require(target.is_relative_to(target_root), 'Target symlink escapes task')
        cargo, rustc = (p.resolve(strict=True) for p in (args.cargo, args.rustc))
        env = {k: v for k, v in os.environ.items() if k in ('PATH', 'LANG', 'LC_ALL', 'TMPDIR')}
        env.update(CARGO_HOME=str(cargo_home), CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE='true',
                   CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2', CARGO_PROFILE_DEV_DEBUG='0', RUSTC=str(rustc))
        report['toolchain'] = {}
        for name, tool in [('cargo', cargo), ('rustc', rustc)]:
            data = tool.read_bytes()
            OLD.binary_metadata(data)
            report['toolchain'][name] = dict(path=str(tool), sha256=sha(data),
                version=subprocess.check_output([str(tool), '--version'], env=env, timeout=10).decode().strip())
        argv = [str(cargo), *BUILD_ARGS]
        report['build'] = dict(argv=argv, cwd=str(source), target_dir=str(target), features=[],
                               default_features=True, lockfile_sha256=files['Cargo.lock'],
                               cargo_home=str(cargo_home), cache_scope='registry only; no config/auth copied',
                               cargo_config_paths_checked=assert_no_ambient_config(source),
                               cargo_config_absent_before=True, cargo_config_absent_after=False)
        started = time.monotonic()
        with (output / 'build.log').open('xb') as log:
            os.fchmod(log.fileno(), 0o600)
            run_build(argv, source, env, log, report['build'])
        assert_no_ambient_config(source)
        report['build']['cargo_config_absent_after'] = True
        report['build'].update(elapsed_seconds=time.monotonic() - started,
                               log_sha256=sha((output / 'build.log').read_bytes()))
        require(source_hashes(source) == files, 'Compiled source/lock changed')
        report['source']['compiled_source_clean'] = True
        for name in ('batond', 'baton'):
            path = target / 'debug' / name
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                info = os.fstat(fd)
                require(stat.S_ISREG(info.st_mode) and 0 < info.st_size <= 268_435_456
                        and info.st_uid == os.geteuid() and not info.st_mode & 0o022, 'Unsafe compiled tool')
                with os.fdopen(os.dup(fd), 'rb') as stream:
                    data = stream.read(268_435_457)
                after = os.fstat(fd)
                require((info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
                        == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
                        and len(data) == info.st_size, 'Compiled tool changed')
            finally:
                os.close(fd)
            OLD.binary_metadata(data)
            with (frozen / name).open('xb') as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            (frozen / name).chmod(0o500)
        report['binaries'] = tool_records(frozen)
        report['status'] = 'compiled'
        validate_compiled(report)
    except BaseException as error:
        report['status'] = 'failed_or_interrupted'
        report['failure_kind'] = type(error).__name__
        raise
    finally:
        COMMON.publish_private_json(frozen / 'source-build-manifest.json', report)
    print(json.dumps(dict(status=report['status'], source=SOURCE,
                         binaries={k: v['sha256'] for k, v in report['binaries'].items()})))


if __name__ == '__main__':
    main()
