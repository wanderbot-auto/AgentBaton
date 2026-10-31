#!/usr/bin/env python3
"""Build/provenance helpers for genuine Task schema15 migration inputs.

No product or compiler starts on import. The explicit builder is separate from
the generator and records compilation, not a claimed runtime probe.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import stat
import struct
import subprocess
import sys
import time
import types

SOURCE = 'baa72ac04944ccd1a1abc659084f5bc6cd148f33'
TREE = 'fabfe01e74189f4986ea8c133f32a5f1ed52006b'
REPOSITORY = 'https://github.com/wanderbot-auto/AgentBaton.git'
CONTRACT = 'agentbaton.schema15-source-build.v1'
FIXED_MANIFEST_SHA = '51949baa8e19abcd3832d8c2c2e63739055d04df2a0405ac09b0b0e05a1125ca'
FIXED_BUILD_SHA = '2dc019b3e49d111088f0d75cf431bec9142f030c76fa0641d03bb760b8a04f99'
FIXED_HASHES = {'batond':'5f6e31504ad7e94c4c29cbe550da20506432be4a1034c8059e6ddf4b2bea3984',
                'baton':'0c36b0d8d0eb01143e3a176e383ee15147443940a721712501dca146b083170f'}
FACTORY = ('factory_scopes','factory_runs','factory_task_lineage','factory_delegations',
           'factory_task_bindings','factory_attempts','factory_scoped_handles','factory_handoffs',
           'factory_budget_reservations','factory_child_plan_heads','factory_child_plan_revisions','factory_child_goal_runs')
EMPTY_AUTHORITY = ('approval_proposals','execution_grants','sessions','attempts','managed_workspaces',
                   'goal_scopes','goal_runs','goal_graph_capabilities','connections')

def require(value, message):
    if not value: raise ValueError(message)

def sha(raw): return hashlib.sha256(raw).hexdigest()
def digest(value): return type(value) is str and len(value)==64 and all(c in '0123456789abcdef' for c in value)

def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'Duplicate JSON key')
        result[key] = value
    return result

def strict_equal(a, b):
    if type(a) is not type(b): return False
    if type(a) is dict: return a.keys() == b.keys() and all(strict_equal(a[k],b[k]) for k in a)
    if type(a) is list: return len(a) == len(b) and all(strict_equal(x,y) for x,y in zip(a,b))
    return a == b

def read_regular(path, limit, mode=0o600):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and before.st_uid == os.geteuid() and stat.S_IMODE(before.st_mode) == mode
                and 0 < before.st_size <= limit, 'Private bounded regular file required')
        with os.fdopen(os.dup(fd), 'rb') as stream: raw = stream.read(limit + 1)
        after = os.fstat(fd)
        fields = lambda m:(m.st_dev,m.st_ino,m.st_size,m.st_mtime_ns,m.st_ctime_ns,m.st_mode,m.st_nlink,m.st_uid)
        require(fields(before) == fields(after) and len(raw) == before.st_size, 'File changed during bounded read')
        return raw
    finally: os.close(fd)

def common():
    path = Path(__file__).with_name('prepare-legacy-migration-fixture.py')
    raw = path.read_bytes()
    module = types.ModuleType('reviewed_legacy_helpers'); module.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module, sha(raw)

def binary_metadata(raw):
    arch = platform.machine()
    if sys.platform == 'darwin':
        require(len(raw) >= 8 and struct.unpack('<II', raw[:8]) ==
                (0xfeedfacf, {'arm64':0x0100000c,'x86_64':0x01000007}.get(arch)), 'Wrong native Mach-O architecture')
        return {'format':'mach-o-64','architecture':arch}
    require(sys.platform == 'linux' and len(raw) >= 20 and raw[:6] == b'\x7fELF\x02\x01'
            and int.from_bytes(raw[18:20],'little') == {'x86_64':62,'aarch64':183}.get(arch), 'Wrong native ELF architecture')
    return {'format':'elf-64','architecture':arch}

def tool_records(frozen):
    require(frozen.is_absolute() and frozen.resolve(strict=True) == frozen, 'Canonical frozen directory required')
    info = frozen.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700, 'Private frozen directory required')
    result = {}
    for name in ('batond','baton'):
        before = (frozen/name).lstat()
        raw = read_regular(frozen/name, 268_435_456, 0o500)
        after = (frozen/name).lstat()
        pin = lambda m:(m.st_dev,m.st_ino,m.st_size,m.st_mtime_ns,m.st_ctime_ns,m.st_nlink,m.st_mode,m.st_uid)
        require(pin(before)==pin(after),'Frozen path changed during byte verification')
        result[name] = {'sha256':sha(raw),'bytes':len(raw),'mode':'0o500','nlink':1,'uid':before.st_uid,
                        'device':str(before.st_dev),'inode':str(before.st_ino),'mtime_ns':str(before.st_mtime_ns),'ctime_ns':str(before.st_ctime_ns),**binary_metadata(raw)}
    return result

def validate_compiled(m):
    require(m['contract'] == CONTRACT and m['status'] == 'compiled' and type(m['schema']) is int and m['schema'] == 15, 'Wrong source-build contract')
    require(m['source']['commit'] == SOURCE and m['source']['tree'] == TREE
            and m['source']['repository'] == REPOSITORY and m['source']['compiled_source_clean'] is True, 'Wrong compiled source')
    files=m['source']['files_sha256']
    require(digest(m['source']['archive_sha256']) and type(files) is dict and 2<=len(files)<=4096
            and all(type(k) is str and not Path(k).is_absolute() and '..' not in Path(k).parts and digest(v) for k,v in files.items())
            and 'Cargo.toml'in files and 'Cargo.lock'in files and m['build']['lockfile_sha256']==files['Cargo.lock'],'Missing exact archived source/lock hashes')
    require(m['platform'] == sys.platform and m['architecture'] == platform.machine(), 'Build is not native to this producer')
    argv = m['build']['argv']
    require(type(argv) is list and len(argv) == 10 and type(argv[0]) is str and Path(argv[0]).is_absolute()
            and argv[1:] == ['build','--offline','--locked','-j','2','--bin','batond','--bin','baton'], 'Wrong locked default build command')
    require(type(m['build']['exit_code']) is int and m['build']['exit_code'] == 0 and m['build']['default_features'] is True
            and m['build']['features'] == [] and set(m['binaries']) == {'batond','baton'}, 'Build did not complete normally')
    require(all(type(m['toolchain'][n]) is str and 0 < len(m['toolchain'][n]) <= 512 for n in ('cargo','rustc')), 'Missing measured toolchain')

def provenance(frozen, build_manifest=None, build_evidence=None):
    records = tool_records(frozen)
    if build_manifest is None:
        raw = read_regular(frozen/'manifest.json', 4_194_304)
        require(sha(raw) == FIXED_MANIFEST_SHA, 'Unknown historical fixed-pair manifest')
        m = json.loads(raw, object_pairs_hook=unique)
        require(m['git_head'] == SOURCE and m['git_tree'] == TREE and m['schema'] == 15 and m['tracked_clean'] is True, 'Fixed pair source mismatch')
        require(m['binaries'] == FIXED_HASHES and {n:r['sha256'] for n,r in records.items()} == FIXED_HASHES, 'Fixed pair bytes mismatch')
        require(m['probe']['schema'] == 15 and m['probe']['query_only'] is True and m['probe']['sequence'] == 0
                and m['probe']['zero_tables'] == {n:0 for n in FACTORY}
                and m['probe']['original_daemon_SIGTERM_wait_exit'] == 0 and m['probe']['sigterm_sent'] is True
                and m['probe']['forced_kill'] is False, 'Fixed pair original probe mismatch')
        path = build_evidence or frozen.parent/'RESULTS.json'
        evidence = read_regular(path, 4_194_304)
        require(sha(evidence) == FIXED_BUILD_SHA, 'Unknown original build evidence')
        rows = json.loads(evidence, object_pairs_hook=unique)
        require(any(r['source'] == SOURCE and r['command'][1:] == ['build','--offline','--locked','--workspace','--bins','-vv']
                    and r['exit'] == 0 and r['interruption'] is None for r in rows), 'Original successful build absent')
        return {'kind':'immutable_frozen_pair','raw_manifest':raw,'raw_build_evidence':evidence,'records':records,
                'toolchain_limit':'Original immutable report has a real build command/result, no recorded toolchain version'}
    raw = read_regular(build_manifest, 4_194_304); m = json.loads(raw, object_pairs_hook=unique)
    validate_compiled(m)
    helper, common_sha = common()
    require(m['builder_sha256'] == sha(Path(__file__).read_bytes()) and m['common_helper_sha256'] == common_sha, 'Unreviewed build helper')
    require(strict_equal(m['binaries'],records) and m['frozen_directory'] == str(frozen), 'Measured build/tool bytes mismatch')
    return {'kind':'reviewed_source_build','raw_manifest':raw,'records':records}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('repository','output','target-dir','cargo','rustc'): p.add_argument('--'+name,type=Path,required=True)
    args = p.parse_args(); helper, helper_sha = common()
    output = args.output.absolute(); output.mkdir(mode=0o700,parents=True,exist_ok=False); output = output.resolve()
    frozen = output/'frozen'; frozen.mkdir(mode=0o700); source = output/'source'; source.mkdir(mode=0o700)
    report = {'contract':CONTRACT,'status':'failed_or_interrupted','schema':15,'platform':sys.platform,
              'architecture':platform.machine(),'os_release':platform.release(),'builder_sha256':sha(Path(__file__).read_bytes()),
              'common_helper_sha256':helper_sha,'frozen_directory':str(frozen),'not_claimed':['actual database probe','Native/Provider','product acceptance']}
    try:
        require(sys.platform in ('linux','darwin'),'Native Linux/macOS builders only')
        require(all(v.is_absolute() and v.is_file() for v in (args.cargo,args.rustc)), 'Explicit absolute compiler paths required')
        repository = args.repository.resolve(strict=True)
        args.target_dir.mkdir(parents=True,exist_ok=True);target = args.target_dir.resolve(strict=True)
        require(shutil.disk_usage(output).free >= 200*1024*1024,'Keep the coordinator free-space guard')
        def git(*argv):return subprocess.check_output(['git','-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','-C',str(repository),*argv])
        subprocess.run(['git','-C',str(repository),'fetch','--no-tags','--no-write-fetch-head',REPOSITORY,SOURCE],check=True,timeout=120)
        require(git('rev-parse',SOURCE+'^{commit}').decode().strip() == SOURCE and git('rev-parse',SOURCE+'^{tree}').decode().strip() == TREE,'Historical source mismatch')
        raw = git('archive','--format=tar',SOURCE,*helper.PREFIXES); helper.extract_source(raw,source); initial = helper.source_hashes(source)
        report['source'] = {'commit':SOURCE,'tree':TREE,'repository':REPOSITORY,'archive_sha256':sha(raw),'files_sha256':initial,'compiled_source_clean':False}
        env = os.environ.copy()
        for name in ('CARGO_BUILD_TARGET','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'):
            require(not env.get(name),'Clear cross-target/compiler overrides before normal historical build');env.pop(name,None)
        env.update(CARGO_TARGET_DIR=str(target),CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',RUSTC=str(args.rustc),CARGO_NET_OFFLINE='true')
        report['toolchain'] = {n:subprocess.check_output([str(v),'--version'],env=env,text=True).strip() for n,v in [('cargo',args.cargo),('rustc',args.rustc)]}
        argv = [str(args.cargo),'build','--offline','--locked','-j','2','--bin','batond','--bin','baton']
        report['build'] = {'argv':argv,'cwd':str(source),'target_dir':str(target),'default_features':True,'features':[],'lockfile_sha256':initial['Cargo.lock']}
        started=time.monotonic()
        with (output/'build.log').open('xb') as log: result = subprocess.run(argv,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
        report['build']['elapsed_seconds']=time.monotonic()-started;report['build']['log_sha256']=sha((output/'build.log').read_bytes())
        report['build']['exit_code'] = result.returncode;require(result.returncode == 0,'Historical build failed; retain build.log')
        require(helper.source_hashes(source) == initial,'Historical compiled source changed');report['source']['compiled_source_clean'] = True
        sizes = [(target/'debug'/name).lstat().st_size for name in ('batond','baton')]
        require(shutil.disk_usage(output).free >= 200*1024*1024 + sum(sizes),'Insufficient space to freeze both tools safely')
        for name in ('batond','baton'):
            path = target/'debug'/name;origin = path.lstat();mode = stat.S_IMODE(origin.st_mode)
            require(mode & 0o111 and not mode & 0o022,'Unsafe compiled executable mode')
            # Cargo's own deps aliases may hardlink an output; pin its read FD,
            # but frozen copies below must each be single-link.
            fd = os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
            try:
                before = os.fstat(fd)
                require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and 0 < before.st_size <= 268_435_456,'Unsafe compiled executable')
                require((before.st_dev,before.st_ino) == (origin.st_dev,origin.st_ino)
                        and stat.S_IMODE(before.st_mode) == mode,'Compiled executable path changed before pinning')
                with os.fdopen(os.dup(fd),'rb') as stream:raw = stream.read(268_435_457)
                after = os.fstat(fd)
                require((before.st_dev,before.st_ino,before.st_size,before.st_mtime_ns,before.st_ctime_ns) == (after.st_dev,after.st_ino,after.st_size,after.st_mtime_ns,after.st_ctime_ns)
                        and len(raw) == before.st_size,'Compiled executable changed while freezing')
            finally:os.close(fd)
            binary_metadata(raw)
            with (frozen/name).open('xb') as stream:stream.write(raw);stream.flush();os.fsync(stream.fileno())
            (frozen/name).chmod(0o500)
        report['binaries'] = tool_records(frozen);report['status'] = 'compiled';validate_compiled(report)
    except BaseException:
        report['status'] = 'failed_or_interrupted';raise
    finally:helper.publish_private_json(frozen/'source-build-manifest.json',report)

if __name__ == '__main__':main()
