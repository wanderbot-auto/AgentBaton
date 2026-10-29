#!/usr/bin/env python3
"""Publish a finite metadata projection, never private migration history."""
import argparse
import json
from pathlib import Path
import re

SOURCE = 'f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701'
TREE = '93ce75df286770a9f196994ed9f5c9a11528cc5e'
TABLES = ('goal_scopes', 'goal_runs', 'goal_graph_capabilities', 'factory_scopes',
          'factory_runs', 'factory_child_plan_revisions', 'factory_child_plan_heads')


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


def public_build(record):
    status = known_status(record)
    # Partial failure records can hold arbitrary diagnostics. Do not publish
    # any of their nested values, even if they share a successful field name.
    if status != 'passed':
        return {'status': status}
    require(record.get('contract') == 'agentbaton.schema14-source-build.v1')
    require(type(record.get('schema')) is int and record['schema'] == 14)
    require(record.get('platform') == 'linux')
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
    return {'contract': 'agentbaton.schema14-source-build.v1', 'status': status,
            'schema': 14, 'platform': 'linux', 'builder_sha256': digest(record.get('builder_sha256')),
            'source': {'commit': SOURCE, 'tree': TREE,
                       'archive_sha256': digest(source.get('archive_sha256')),
                       'compiled_source_clean': True},
            'binaries': safe_binaries, 'toolchain': safe_toolchain, 'build_exit': 0}


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
    return {'status': status, 'schema': 14, 'origin': 'actual_frozen_schema14_core_rpc',
            'source_commit': SOURCE, 'provenance_kind': 'reviewed_source_build',
            'daemon_exit': 0, 'forced_kill': False,
            'runtime_cleanup': 'removed_after_successful_owned_join_and_backup',
            'database_sha256': digest(record.get('database_sha256')),
            'counts': {name: counts[name] for name in TABLES}}


def load(path):
    if not path.exists():
        return None
    require(not path.is_symlink() and path.is_file() and path.stat().st_size <= 4194304)
    return json.loads(path.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--temporary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    summary = {'dimension': 'Linux genuine14 source/history and explicit15 migration; Mac legacy migration NOT_RUN',
               'private_history_uploaded': False}
    invalid = False
    for label, path, project in [
        ('source_build', args.temporary / 'agentbaton-legacy14-build/frozen/source-build-manifest.json', public_build),
        ('fixture', args.temporary / 'agentbaton-legacy14-fixture/manifest.json', public_fixture),
    ]:
        try:
            record = load(path)
            summary[label] = project(record) if record is not None else {'status': 'missing_or_unpublished'}
        except (ValueError, OSError, TypeError, KeyError):
            summary[label] = {'status': 'invalid_private_metadata_not_published'}
            invalid = True
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    return 1 if invalid else 0


if __name__ == '__main__':
    raise SystemExit(main())
