#!/usr/bin/env python3
"""Build and freeze the exact public schema14 source for the Linux migration gate.

This creates no approval or product fixture. The separately reviewed generator
uses the frozen normal pair to create actual private Core history. Never upload
its private SQLite files, runtime tokens or capabilities as public artifacts.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import signal
import socket
import sqlite3
import stat
import subprocess
import sys
import tarfile
import tempfile
import time

SOURCE = 'f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701'
TREE = '93ce75df286770a9f196994ed9f5c9a11528cc5e'
PUBLIC_REPOSITORY = 'https://github.com/wanderbot-auto/AgentBaton.git'
CONTRACT = 'agentbaton.schema14-source-build.v1'
PREFIXES = ('Cargo.toml', 'Cargo.lock', 'crates', 'apps/daemon', 'apps/cli')
TABLES = ('goal_scopes', 'goal_runs', 'goal_graph_capabilities', 'factory_scopes',
          'factory_runs', 'factory_task_lineage', 'factory_delegations',
          'factory_task_bindings', 'factory_attempts', 'factory_scoped_handles',
          'factory_handoffs', 'factory_budget_reservations',
          'factory_child_plan_heads', 'factory_child_plan_revisions')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(value):
    return isinstance(value, str) and len(value) == 64 and all(c in '0123456789abcdef' for c in value)


def publish_private_json(path, value):
    candidate = path.with_name(path.name + '.pending')
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
    with os.fdopen(os.open(candidate, flags, 0o600), 'wb') as stream:
        os.fchmod(stream.fileno(), 0o600)
        stream.write((json.dumps(value,indent=2)+'\n').encode())
        stream.flush()
        os.fsync(stream.fileno())
    # No fallible permission/close operation follows publication. A failed
    # prepare/flush/close/rename retains the private candidate, not a passed
    # final-name record consumable by the next gate.
    os.replace(candidate, path)


def validate_build_manifest(m):
    """Finite metadata contract; never a substitute for an actual build/probe."""
    require(type(m) is dict and m.get('contract') == CONTRACT and m.get('status') == 'passed', 'Incomplete source-build record')
    require(digest(m.get('builder_sha256')), 'Builder source hash missing')
    require(type(m.get('schema')) is int and m['schema'] == 14 and m.get('platform') == 'linux', 'Only actual Linux14 evidence is supported')
    s, b, p = m.get('source', {}), m.get('build', {}), m.get('probe', {})
    require(s.get('commit') == SOURCE and s.get('tree') == TREE and s.get('repository') == PUBLIC_REPOSITORY, 'Wrong historical source')
    require(s.get('compiled_source_clean') is True and digest(s.get('archive_sha256')), 'Compiled source changed or archive missing')
    require(type(s.get('files_sha256')) is dict and all(digest(s['files_sha256'].get(k)) for k in ['Cargo.toml','Cargo.lock','crates/core/Cargo.toml','apps/daemon/Cargo.toml','apps/cli/Cargo.toml']), 'Incomplete archived workspace')
    require(type(b.get('exit_code')) is int and b['exit_code'] == 0 and b.get('packages') == ['batond','baton'] and b.get('features') == [] and b.get('default_features') is True, 'Not a normal successful product pair build')
    require(b.get('lockfile_sha256') == s['files_sha256']['Cargo.lock'], 'Lockfile source mismatch')
    argv = b.get('argv', [])
    require(isinstance(argv, list) and len(argv) == 9 and isinstance(argv[0],str) and Path(argv[0]).is_absolute() and argv[1:] == ['build','--locked','-j','2','--bin','batond','--bin','baton'], 'Unexpected build command')
    # Kept separately from runtime limits: this bounded subprocess probe must
    # actually reap, and forced SIGKILL never qualifies as normal shutdown.
    require(type(p.get('schema')) is int and p['schema'] == 14 and p.get('query_only') is True and p.get('foreign_key_errors') == [], 'Invalid actual14 database probe')
    require(type(p.get('counts')) is dict and set(p['counts'])==set(TABLES) and all(type(v) is int and v==0 for v in p['counts'].values()), 'Nonempty or incomplete initial source-build probe')
    require(all(type(p.get(k)) is int and p[k]==v for k,v in [('snapshot_api_version',1),('snapshot_sequence',0),('cli_exit',0)]), 'Pair CLI/Core probe failed')
    require(p.get('sigterm_sent') is True and type(p.get('daemon_wait_exit')) is int and p['daemon_wait_exit']==0 and p.get('forced_kill') is False and p.get('identity_gone_after_join') is True, 'No clean bounded owned daemon join')
    require(all(isinstance(m.get('toolchain', {}).get(k),str) and m['toolchain'][k] for k in ['cargo','rustc']), 'Toolchain not recorded')
    binaries = m.get('binaries', {})
    require(set(binaries) == {'batond','baton'}, 'Missing product pair')
    for value in binaries.values():
        require(type(value) is dict and digest(value.get('sha256')) and type(value.get('bytes')) is int and 0 < value['bytes'] <= 134_217_728 and value.get('mode') == '0o500' and type(value.get('nlink')) is int and value['nlink'] == 1, 'Unsafe or missing frozen binary')
    return {n: binaries[n]['sha256'] for n in ['batond','baton']}


def extract_source(raw, destination):
    require(0 < len(raw) <= 67_108_864, 'Historical archive exceeds bounded workspace')
    seen = set()
    total = 0
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:') as archive:
        for entry in archive:
            path = PurePosixPath(entry.name)
            require(not path.is_absolute() and '..' not in path.parts and entry.name not in seen, 'Unsafe duplicate/archive path')
            require(any(entry.name == p or entry.name.startswith(p+'/') or entry.name.rstrip('/') in ('apps',) for p in PREFIXES), 'Unexpected archive root')
            require(entry.isdir() or entry.isfile(), 'Archive links/devices are not source files')
            seen.add(entry.name)
            target = destination.joinpath(*path.parts)
            if entry.isdir():
                target.mkdir(mode=0o700,parents=True,exist_ok=True)
            else:
                require(entry.size <= 16_777_216, 'Oversized archived source file')
                total += entry.size
                require(total <= 67_108_864, 'Oversized extracted source workspace')
                target.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
                with target.open('xb') as stream:
                    stream.write(archive.extractfile(entry).read())
                target.chmod(0o700 if entry.mode & 0o111 else 0o600)


def source_hashes(source):
    return {str(p.relative_to(source)): sha(p.read_bytes()) for p in sorted(source.rglob('*')) if p.is_file()}


def identity(pid):
    return Path('/proc', str(pid), 'stat').read_text().rsplit(')',1)[1].split()[19]


def probe_pair(frozen, output):
    runtime = Path(tempfile.mkdtemp(prefix='ab14-build-',dir='/tmp')).resolve()
    runtime.chmod(0o700)
    require(len(str(runtime/'batond.sock').encode()) <= 100, 'Probe socket path exceeds Unix bound')
    result = {'runtime': str(runtime), 'runtime_cleanup': 'retained_on_failure', 'forced_kill': False, 'sigterm_sent': False}
    env = {k:v for k,v in os.environ.items() if k in ('PATH','HOME','LANG','LC_ALL','TMPDIR')}
    env['BATON_RUNTIME_DIR'] = str(runtime)
    process = None
    try:
        with (output/'probe-daemon.log').open('xb') as log:
            process = subprocess.Popen([str(frozen/'batond')],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            result['pid'] = process.pid
            start = None
            try:
                start = identity(process.pid)
                result['start_ticks'] = start
                until = time.monotonic()+15
                while not ((runtime/'session.token').is_file() and (runtime/'batond.sock').exists()):
                    require(process.poll() is None and time.monotonic()<until, 'Bounded real14 daemon start failed')
                    time.sleep(.02)
                require(stat.S_IMODE((runtime/'session.token').stat().st_mode)==0o600, 'Owner token file is not private')
                with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True) as db:
                    db.execute('PRAGMA query_only=ON')
                    result.update(schema=db.execute('PRAGMA user_version').fetchone()[0],query_only=True,foreign_key_errors=db.execute('PRAGMA foreign_key_check').fetchall(),counts={n:db.execute('SELECT count(*) FROM '+n).fetchone()[0] for n in TABLES})
                reply = subprocess.run([str(frozen/'baton'),'snapshot'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
                result['cli_exit'] = reply.returncode
                require(len(reply.stdout)<=4_194_304 and len(reply.stderr)<=262_144 and reply.returncode==0, 'Real companion snapshot failed')
                snapshot = json.loads(reply.stdout)
                require(not snapshot.get('error'), 'Actual companion snapshot refused')
                result.update(snapshot_api_version=snapshot['result']['api_version'],snapshot_sequence=snapshot['result']['sequence'])
            finally:
                if process.poll() is None:
                    result['sigterm_sent'] = True
                    process.send_signal(signal.SIGTERM)
                try: result['daemon_wait_exit'] = process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    result['forced_kill'] = True
                    process.kill(); result['daemon_wait_exit'] = process.wait(timeout=5)
                try: result['identity_gone_after_join'] = start is not None and identity(process.pid) != start
                except FileNotFoundError: result['identity_gone_after_join'] = start is not None
        require(result.get('daemon_wait_exit')==0 and result['sigterm_sent'] and not result['forced_kill'], 'Real daemon did not exit cleanly')
        require(result.get('schema')==14 and result.get('foreign_key_errors')==[] and result.get('counts')=={n:0 for n in TABLES}, 'Actual database is not empty coherent schema14')
        shutil.rmtree(runtime)
        result['runtime_cleanup'] = 'removed_after_successful_owned_join'
        return result
    finally:
        (output/'probe-record.json').write_text(json.dumps(result,indent=2)+'\n')
        (output/'probe-record.json').chmod(0o600)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--repository',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--target-dir',type=Path,required=True)
    p.add_argument('--cargo',type=Path,required=True)
    p.add_argument('--rustc',type=Path,required=True)
    args=p.parse_args()
    require(sys.platform=='linux','Linux /proc evidence only; macOS migration is a separate NOT_RUN dimension')
    require(args.cargo.is_absolute() and args.cargo.is_file() and args.rustc.is_absolute() and args.rustc.is_file(),'Explicit existing absolute cargo/rustc paths are required')
    repository=args.repository.resolve(strict=True)
    output=args.output.absolute();output.mkdir(mode=0o700,parents=True,exist_ok=False);output=output.resolve()
    source=output/'source';source.mkdir(mode=0o700)
    frozen=output/'frozen';frozen.mkdir(mode=0o700)
    target=args.target_dir.absolute()
    require(not target.is_relative_to(source),'Build outputs cannot modify the archived source tree')
    require(shutil.disk_usage(output).free>=209_715_200,'Preserve the 200MiB workspace guard')
    report={'contract':CONTRACT,'status':'failed_or_interrupted','schema':14,'platform':'linux','builder_sha256':sha(Path(__file__).read_bytes())}
    try:
        subprocess.run(['git','-C',str(repository),'fetch','--no-tags','--no-write-fetch-head',PUBLIC_REPOSITORY,SOURCE],check=True,timeout=120)
        commit=subprocess.check_output(['git','-C',str(repository),'rev-parse','--verify',SOURCE+'^{commit}'],text=True).strip()
        tree=subprocess.check_output(['git','-C',str(repository),'rev-parse',SOURCE+'^{tree}'],text=True).strip()
        require(commit==SOURCE and tree==TREE,'Fetched public historical source does not match its fixed commit/tree')
        raw=subprocess.check_output(['git','-C',str(repository),'archive','--format=tar',SOURCE,*PREFIXES])
        extract_source(raw,source)
        initial=source_hashes(source)
        report['source']={'commit':commit,'tree':tree,'repository':PUBLIC_REPOSITORY,'archive_sha256':sha(raw),'files_sha256':initial,'compiled_source_clean':False}
        env=os.environ.copy()
        require(not env.get('CARGO_BUILD_TARGET'),'This is a native Linux build/probe, not a cross target')
        for name in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'):
            require(not env.get(name),'Explicitly clear compiler flags/wrappers before the normal source build')
            env.pop(name,None)
        env.update(CARGO_TARGET_DIR=str(target),CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',RUSTC=str(args.rustc))
        report['toolchain']={n:subprocess.check_output([str(path),'--version'],env=env,text=True).strip() for n,path in [('cargo',args.cargo),('rustc',args.rustc)]}
        argv=[str(args.cargo),'build','--locked','-j','2','--bin','batond','--bin','baton']
        report['build']={'argv':argv,'cwd':str(source),'target_dir':str(target),'packages':['batond','baton'],'features':[],'default_features':True,'lockfile_sha256':initial['Cargo.lock'],'environment':{k:env[k] for k in ('CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_PROFILE_DEV_DEBUG')}}
        with (output/'build.log').open('xb') as log:
            built=subprocess.run(argv,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
        report['build']['exit_code']=built.returncode
        require(built.returncode==0,'Historical normal product pair build failed; retain build.log')
        require(source_hashes(source)==initial,'Compiled source changed during the locked build')
        report['source']['compiled_source_clean']=True
        report['binaries']={}
        for name in ('batond','baton'):
            origin=target/'debug'/name
            info=origin.lstat();require(stat.S_ISREG(info.st_mode) and not info.st_mode&0o022,'Unsafe compiled product binary')
            raw=origin.read_bytes();destination=frozen/name
            with destination.open('xb') as stream: stream.write(raw);stream.flush();os.fsync(stream.fileno())
            destination.chmod(0o500);info=destination.lstat()
            require(info.st_nlink==1 and sha(destination.read_bytes())==sha(raw),'Frozen pair did not retain exact original bytes')
            report['binaries'][name]={'sha256':sha(raw),'bytes':len(raw),'mode':oct(stat.S_IMODE(info.st_mode)),'nlink':info.st_nlink}
        report['probe']=probe_pair(frozen,output)
        for name in report['binaries']:
            require(sha((frozen/name).read_bytes())==report['binaries'][name]['sha256'],'Frozen tool changed during probe')
            info=(frozen/name).lstat()
            require(info.st_nlink==1 and stat.S_IMODE(info.st_mode)==0o500,'Frozen tool metadata changed during probe')
        require(source_hashes(source)==initial,'Source changed after actual probe')
        require(sha(Path(__file__).read_bytes())==report['builder_sha256'],'Builder source changed during execution')
        report['status']='passed'
        validate_build_manifest(report)
    except BaseException:
        report['status']='failed_or_interrupted'
        raise
    finally:
        publish_private_json(frozen/'source-build-manifest.json',report)


if __name__=='__main__':
    main()
