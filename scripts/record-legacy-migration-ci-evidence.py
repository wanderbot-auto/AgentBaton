#!/usr/bin/env python3
"""Publish a finite metadata projection, never private migration history."""
import argparse
import json
import math
import os
from pathlib import Path
import re
import stat

SOURCE = 'f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701'
TREE = '93ce75df286770a9f196994ed9f5c9a11528cc5e'
TABLES = ('goal_scopes', 'goal_runs', 'goal_graph_capabilities', 'factory_scopes',
          'factory_runs', 'factory_child_plan_revisions', 'factory_child_plan_heads')
SOURCE15 = 'baa72ac04944ccd1a1abc659084f5bc6cd148f33'
TREE15 = 'fabfe01e74189f4986ea8c133f32a5f1ed52006b'
FACTORY15 = ('factory_scopes', 'factory_runs', 'factory_task_lineage', 'factory_delegations',
             'factory_task_bindings', 'factory_attempts', 'factory_scoped_handles', 'factory_handoffs',
             'factory_budget_reservations', 'factory_child_plan_heads', 'factory_child_plan_revisions',
             'factory_child_goal_runs')
AUTHORITY15 = ('approval_proposals', 'execution_grants', 'sessions', 'attempts',
               'managed_workspaces', 'goal_scopes', 'goal_runs', 'goal_graph_capabilities', 'connections')


def require(condition):
    if not condition:
        raise ValueError('Legacy evidence metadata violates the finite public schema')


def digest(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None)
    return value


def known_status(record):
    require(type(record) is dict)
    status = record.get('status')
    require(type(status) is str and status in ('passed', 'failed_or_interrupted'))
    return status


def public_host(record, historical_fixture=False):
    platform = record.get('platform', 'linux' if historical_fixture else None)
    require(type(platform) is str and platform in ('linux','darwin'))
    result = {} if historical_fixture and 'platform' not in record else {'platform':platform}
    if 'architecture' in record or platform == 'darwin':
        architecture = record.get('architecture')
        require(type(architecture) is str and architecture in (('arm64','x86_64') if platform=='darwin' else ('x86_64','aarch64','arm64')))
        result['architecture'] = architecture
    if platform == 'darwin':
        release = record.get('os_release')
        require(type(release) is str and re.fullmatch(r'[0-9]+(?:\.[0-9]+){1,3}',release) is not None and len(release)<=32)
        result['os_release'] = release
    return result


def public_build(record):
    status = known_status(record)
    # Partial failure records can hold arbitrary diagnostics. Do not publish
    # any of their nested values, even if they share a successful field name.
    if status != 'passed':
        return {'status': status}
    require(record.get('contract') == 'agentbaton.schema14-source-build.v1')
    require(type(record.get('schema')) is int and record['schema'] == 14)
    host = public_host(record)
    source = record.get('source')
    require(type(source) is dict and source.get('commit') == SOURCE and source.get('tree') == TREE)
    require(source.get('compiled_source_clean') is True)
    binaries = record.get('binaries')
    require(type(binaries) is dict and set(binaries) == {'batond', 'baton'})
    safe_binaries = {}
    for name in ('batond', 'baton'):
        binary = binaries[name]
        require(type(binary) is dict)
        size = binary.get('bytes')
        require(type(size) is int and 0 < size <= 134217728)
        require(binary.get('mode') == '0o500' and type(binary.get('nlink')) is int and binary['nlink'] == 1)
        safe_binaries[name] = {'sha256': digest(binary.get('sha256')), 'bytes': size,
                               'mode': '0o500', 'nlink': 1}
        if host['platform'] == 'darwin':
            require(binary.get('format')=='mach-o-64' and binary.get('architecture')==host['architecture'])
            safe_binaries[name].update(format='mach-o-64',architecture=host['architecture'])
    toolchain = record.get('toolchain')
    require(type(toolchain) is dict)
    safe_toolchain = {}
    for name in ('cargo', 'rustc'):
        value = toolchain.get(name)
        require(type(value) is str and len(value) <= 160)
        require(re.fullmatch(name + r' [0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.]+)? \([0-9a-f]+ [0-9]{4}-[0-9]{2}-[0-9]{2}\)', value) is not None)
        safe_toolchain[name] = value
    build = record.get('build')
    require(type(build) is dict and type(build.get('exit_code')) is int and build['exit_code'] == 0)
    result = {'contract': 'agentbaton.schema14-source-build.v1', 'status': status,
            'schema': 14, **host, 'builder_sha256': digest(record.get('builder_sha256')),
            'source': {'commit': SOURCE, 'tree': TREE,
                       'archive_sha256': digest(source.get('archive_sha256')),
                       'compiled_source_clean': True},
            'binaries': safe_binaries, 'toolchain': safe_toolchain, 'build_exit': 0}
    if host['platform']=='darwin':
        result['native_helper_sha256']=digest(record.get('native_helper_sha256'))
        proof=record.get('probe')
        require(type(proof) is dict and proof.get('identity_method')=='darwin_owned_unreaped_group'
                and proof.get('leader_unreaped_until_signalling') is True
                and proof.get('group_quiescent_before_reaping') is True
                and proof.get('sigterm_sent') is True and proof.get('forced_kill') is False
                and type(proof.get('daemon_wait_exit')) is int and proof['daemon_wait_exit']==0
                and proof.get('identity_gone_after_join') is True and proof.get('cleanup_errors')==[])
        result['process_proof']={'method':'darwin_owned_unreaped_group','sigterm_sent':True,
            'group_quiescent_before_reaping':True,'wait_exit':0,'forced_kill':False}
    return result


def public_fixture(record):
    status = known_status(record)
    if status != 'passed':
        return {'status': status}
    require(type(record.get('schema')) is int and record['schema'] == 14)
    require(record.get('origin') == 'actual_frozen_schema14_core_rpc')
    require(record.get('source_commit') == SOURCE)
    require(record.get('provenance_kind') == 'reviewed_source_build')
    require(type(record.get('daemon_exit')) is int and record['daemon_exit'] == 0)
    require(record.get('forced_kill') is False)
    require(record.get('runtime_cleanup') == 'removed_after_successful_owned_join_and_backup')
    counts = record.get('counts')
    require(type(counts) is dict and set(counts) == set(TABLES))
    require(all(type(value) is int and 0 < value <= 100000 for value in counts.values()))
    host=public_host(record,historical_fixture=True)
    result = {'status': status, 'schema': 14, **host, 'origin': 'actual_frozen_schema14_core_rpc',
            'source_commit': SOURCE, 'provenance_kind': 'reviewed_source_build',
            'daemon_exit': 0, 'forced_kill': False,
            'runtime_cleanup': 'removed_after_successful_owned_join_and_backup',
            'database_sha256': digest(record.get('database_sha256')),
            'counts': {name: counts[name] for name in TABLES}}
    if host.get('platform')=='darwin':
        require(record.get('daemon_identity_method')=='darwin_owned_unreaped_group'
                and record.get('daemon_sigterm_sent') is True
                and record.get('daemon_group_quiescent_before_reaping') is True
                and record.get('daemon_leader_unreaped_until_signalling') is True
                and record.get('daemon_cleanup_errors')==[])
        proof=record.get('provider_stop_proof')
        require(type(proof) is dict and proof.get('method')=='core_stop_confirmed_plus_pid_absence'
                and type(proof.get('observed')) is int and 2<=proof['observed']<=256
                and proof.get('core_join_confirmed') is True and proof.get('pid_absence_confirmed') is True)
        result['provider_stop_proof']={'method':'core_stop_confirmed_plus_pid_absence',
            'observed':proof['observed'],'core_join_confirmed':True,'pid_absence_confirmed':True}
    return result


def public15_host(record):
    host = public_host(record)
    require(host.get('architecture') in
            (('arm64', 'x86_64') if host['platform'] == 'darwin' else ('x86_64', 'aarch64')))
    return host


def bounded_int(value, low, high):
    require(type(value) is int and low <= value <= high)
    return value


def public15_tools(record, host):
    require(type(record) is dict and set(record) == {'baton', 'batond'})
    result = {}
    for name in ('batond', 'baton'):
        binary = record[name]
        require(type(binary) is dict and binary.get('mode') == '0o500')
        require(binary.get('format') == ('mach-o-64' if host['platform'] == 'darwin' else 'elf-64')
                and binary.get('architecture') == host['architecture'])
        result[name] = {'sha256': digest(binary.get('sha256')),
                        'bytes': bounded_int(binary.get('bytes'), 1, 268435456),
                        'mode': '0o500', 'nlink': bounded_int(binary.get('nlink'), 1, 1),
                        'format': binary['format'], 'architecture': host['architecture']}
    return result


def public15_status(record, success):
    require(type(record) is dict)
    status = record.get('status')
    require(type(status) is str and status in (success, 'failed_or_interrupted'))
    return status


def public15_build(record):
    status = public15_status(record, 'compiled')
    if status != 'compiled':
        return {'status': status}
    require(record.get('contract') == 'agentbaton.schema15-source-build.v1')
    bounded_int(record.get('schema'), 15, 15)
    host = public15_host(record)
    source = record.get('source')
    require(type(source) is dict and source.get('commit') == SOURCE15 and source.get('tree') == TREE15
            and source.get('repository') == 'https://github.com/wanderbot-auto/AgentBaton.git'
            and source.get('compiled_source_clean') is True)
    build = record.get('build')
    require(type(build) is dict and build.get('default_features') is True and build.get('features') == [])
    bounded_int(build.get('exit_code'), 0, 0)
    argv = build.get('argv')
    require(type(argv) is list and len(argv) == 10 and type(argv[0]) is str and Path(argv[0]).is_absolute()
            and argv[1:] == ['build', '--offline', '--locked', '-j', '2', '--bin', 'batond', '--bin', 'baton'])
    toolchain = record.get('toolchain')
    require(type(toolchain) is dict)
    safe_toolchain = {}
    for name in ('cargo', 'rustc'):
        value = toolchain.get(name)
        require(type(value) is str and len(value) <= 160)
        require(re.fullmatch(name + r' [0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.]+)? \([0-9a-f]+ [0-9]{4}-[0-9]{2}-[0-9]{2}\)', value) is not None)
        safe_toolchain[name] = value
    return {'contract': 'agentbaton.schema15-source-build.v1', 'status': 'compiled', 'schema': 15,
            **host, 'builder_sha256': digest(record.get('builder_sha256')),
            'common_helper_sha256': digest(record.get('common_helper_sha256')),
            'source': {'commit': SOURCE15, 'tree': TREE15,
                       'archive_sha256': digest(source.get('archive_sha256')), 'compiled_source_clean': True},
            'binaries': public15_tools(record.get('binaries'), host),
            'toolchain': safe_toolchain, 'build_exit': 0}


def public15_fixture(record):
    status = public15_status(record, 'passed')
    if status != 'passed':
        return {'status': status}
    require(record.get('contract') == 'agentbaton.actual-task-schema15.v1'
            and record.get('origin') == 'actual_source15_core_rpc'
            and record.get('source_commit') == SOURCE15 and record.get('source_tree') == TREE15
            and record.get('provenance_kind') == 'reviewed_source_build')
    bounded_int(record.get('schema'), 15, 15)
    host = public15_host(record)
    tools = public15_tools(record.get('tool_metadata'), host)
    require(public15_tools(record.get('tool_metadata_after'), host) == tools)
    hashes = record.get('binaries')
    require(type(hashes) is dict and set(hashes) == {'baton', 'batond'})
    require(all(digest(hashes[name]) == tools[name]['sha256'] for name in tools))
    daemon = record.get('daemon')
    require(type(daemon) is dict and daemon.get('sigterm_sent') is True
            and daemon.get('forced_kill') is False and daemon.get('identity_gone_after_join') is True
            and daemon.get('leader_unreaped_until_signalling') is True
            and daemon.get('group_quiescent_before_reaping') is True and daemon.get('cleanup_errors') == [])
    bounded_int(daemon.get('daemon_wait_exit'), 0, 0)
    require(record.get('backup_logical_exact') is True and record.get('runtime_mode') == '0o700')
    bounded_int(record.get('socket_utf8_bytes'), 1, 100)
    cli = record.get('actual_cli_probe')
    require(type(cli) is dict)
    for field, expected in (('exit', 0), ('api_version', 1), ('sequence', 0)):
        bounded_int(cli.get(field), expected, expected)
    probes = {}
    for label in ('initial_probe', 'final_probe'):
        probe = record.get(label)
        require(type(probe) is dict and probe.get('query_only') is True)
        bounded_int(probe.get('schema'), 15, 15)
        bounded_int(probe.get('foreign_key_errors'), 0, 0)
        counts = probe.get('counts')
        require(type(counts) is dict and len(counts) <= 256)
        for count in counts.values():
            bounded_int(count, 0, 100000)
        names = ('tasks', 'events', 'idempotency') + FACTORY15 + AUTHORITY15
        require(set(names) <= set(counts))
        for name in FACTORY15 + AUTHORITY15:
            bounded_int(counts[name], 0, 0)
        if label == 'initial_probe':
            for name in ('tasks', 'events', 'idempotency'):
                bounded_int(counts[name], 0, 0)
            bounded_int(probe.get('sequence'), 0, 0)
        else:
            bounded_int(counts['tasks'], 3, 3)
            bounded_int(counts['events'], 5, 100000)
            bounded_int(counts['idempotency'], 4, 100000)
            bounded_int(probe.get('sequence'), 5, 1000000)
        probes[label] = {'counts': {name: counts[name] for name in names}, 'sequence': probe['sequence']}
    require(bounded_int(record.get('final_api_sequence'), 5, 1000000) == probes['final_probe']['sequence'])
    result = {'contract': 'agentbaton.actual-task-schema15.v1', 'status': 'passed', 'schema': 15,
              **host, 'origin': 'actual_source15_core_rpc', 'source_commit': SOURCE15, 'source_tree': TREE15,
              'provenance_kind': 'reviewed_source_build', 'database_sha256': digest(record.get('database_sha256')),
              'database_bytes': bounded_int(record.get('database_bytes'), 1, 16777216), 'binaries': tools,
              'initial_probe': probes['initial_probe'], 'final_probe': probes['final_probe'],
              'backup_logical_exact': True, 'daemon_stop': {'sigterm_sent': True, 'wait_exit': 0,
              'forced_kill': False, 'identity_gone_after_join': True, 'leader_unreaped_until_signalling': True,
              'group_quiescent_before_reaping': True, 'cleanup_error_count': 0}}
    for field in ('generator_sha256', 'helper_sha256', 'common_helper_sha256',
                  'native_helper_sha256', 'tool_provenance_sha256'):
        result[field] = digest(record.get(field))
    return result


def unique_json(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def finite_float(value):
    result = float(value)
    require(math.isfinite(result))
    return result


def load(path):
    if not path.exists():
        require(not path.is_symlink())
        return None
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= 4194304)
        raw = stream.read(4194305)
        after = os.fstat(stream.fileno())
        pin = lambda m: (m.st_dev, m.st_ino, m.st_size, m.st_mtime_ns, m.st_ctime_ns, m.st_mode)
        require(pin(before) == pin(after) and len(raw) == before.st_size)
    return json.loads(raw, object_pairs_hook=unique_json,
                      parse_float=finite_float, parse_constant=lambda _: require(False))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--temporary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    summary = {'dimension': 'Genuine14-to-16 and genuine15-to-16 source/fixture metadata; platform-specific supplied evidence only',
               'metadata_only': True, 'private_history_uploaded': False}
    invalid = False
    for label, path, project in [
        ('source_build', args.temporary / 'agentbaton-legacy14-build/frozen/source-build-manifest.json', public_build),
        ('fixture', args.temporary / 'agentbaton-legacy14-fixture/manifest.json', public_fixture),
        ('schema15_source_build', args.temporary / 'agentbaton-legacy15-build/frozen/source-build-manifest.json', public15_build),
        ('schema15_fixture', args.temporary / 'agentbaton-legacy15-fixture/manifest.json', public15_fixture),
    ]:
        try:
            record = load(path)
            summary[label] = project(record) if record is not None else {'status': 'missing_or_unpublished'}
        except (ValueError, OSError, TypeError, KeyError, RecursionError):
            summary[label] = {'status': 'invalid_private_metadata_not_published'}
            invalid = True
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    return 1 if invalid else 0


if __name__ == '__main__':
    raise SystemExit(main())
