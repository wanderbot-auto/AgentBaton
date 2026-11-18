"""Exact inverse of the current driver label and schema 19 projector changes."""
import hashlib

DRIVER_PATH = "apps/desktop/tests/integration-real-ipc.py"
PROJECTOR_PATH = "scripts/record-integration-browser-ci-evidence.py"
DRIVER_BEFORE_SHA256 = "d7d163f945431abe328922c3f6e378c7665d3d37b9f8accbdc2f8a04454526be"
DRIVER_CURRENT_SHA256 = "f730621d77cb75f94e214794615b1125ba0f33cc454431a3432a8b43e8ddd0bb"
PROJECTOR_BEFORE_SHA256 = "f4cae560fb2dbd3d5105f1433c4a8c4461d134e3fa940c2a099abb789fa5f411"
PROJECTOR_CURRENT_SHA256 = "ff120f159df969b1f5425e371b9ea4d2166daa833bcb29000e66e2552d1b7548"
LEGACY_CANDIDATE_CHECKS = (
    b"        manifest_raw = P.regular_bytes(fd, 'candidate.json', MAX_REPORT, witnesses=witnesses)\n"
    b"        build_raw = P.regular_bytes(fd, 'frontend-build-record.json', MAX_REPORT, witnesses=witnesses)\n"
    b"        probe_raw = P.regular_bytes(fd, 'schema-probe.json', MAX_STATUS, witnesses=witnesses)\n"
    b'        manifest, build, probe = P.decode(manifest_raw), P.decode(build_raw), P.decode(probe_raw)\n'
    b"        require(type(manifest.get('schema')) is int and manifest['schema'] == 18 and manifest['git_head'] == expected\n"
    b"            and manifest.get('dirty') is False and manifest.get('tracked_clean', True) is True\n"
    b"            and set(manifest['binaries']) == {'batond', 'baton'}\n"
    b"            and all(manifest.get(k, expected) == expected for k in ('source_head', 'compiled_core_source_head')))\n"
    b"        require(build['source'] == expected and type(build['exit']) is int and build['exit'] == 0\n"
    b"            and build['source_head_and_tracked_unchanged'] is True\n"
    b"            and build.get('dirty', False) is False and build.get('tracked_clean', True) is True\n"
    b"            and build['command'] == ['npm', '--prefix', 'apps/desktop', 'run', 'build'])\n"
    b"        require(type(probe) is dict and set(probe) == {'source', 'api_version', 'database_schema_version', 'daemon_exit', 'joined'}\n"
    b"            and probe['source'] == expected and type(probe['api_version']) is int and probe['api_version'] == 1\n"
    b"            and type(probe['database_schema_version']) is int and probe['database_schema_version'] == 18\n"
    b"            and type(probe['daemon_exit']) is int and probe['daemon_exit'] == 0 and probe['joined'] is True)\n"
)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def checked_source(path, raw, expected_path, current_hash):
    if type(path) is not str or path != expected_path or type(raw) is not bytes:
        raise ValueError("Expected raw source bytes at the canonical path")
    if digest(raw) != current_hash:
        raise ValueError("Current schema 19 source differs from the complete pinned file")


def replace(raw, current, previous, count=1):
    if raw.count(current) != count:
        raise ValueError("Unexpected number of schema 19 source anchors")
    return raw.replace(current, previous, count)


def check_parent(raw, expected):
    if digest(raw) != expected:
        raise ValueError("Schema 19 inverse did not restore the complete pinned predecessor")
    return raw


def restore_driver_before_labels19(path, raw):
    checked_source(path, raw, DRIVER_PATH, DRIVER_CURRENT_SHA256)
    raw = replace(raw, b"Core19 Unix IPC", b"Core18 Unix IPC", count=2)
    raw = replace(raw,
        b"    platform = 'Linux' if sys.platform == 'linux' else 'macOS'\n"
        b"    print(json.dumps({'status': 'passed' if result else 'failed_or_interrupted',\n"
        b"        'dimension': f'{platform}/browser/generatedACP/private_test_host_not_OS'}))\n",
        b"    print(json.dumps({'status': 'passed' if result else 'failed_or_interrupted', "
        b"'dimension': 'Linux/browser/generatedACP/private_test_host_not_OS'}))\n")
    return check_parent(raw, DRIVER_BEFORE_SHA256)


def restore_projector_before_schema19(path, raw):
    checked_source(path, raw, PROJECTOR_PATH, PROJECTOR_CURRENT_SHA256)
    start = raw.index(b"def validated_candidate(")
    end = raw.index(b"def collect(", start)
    raw = raw[:start] + raw[end:]
    raw = replace(raw, b"Core19 Unix IPC", b"Core18 Unix IPC")
    call = b"        binding, probe = validated_candidate(fd, expected, witnesses)\n"
    raw = replace(raw, call + b"        summaries, sources, payloads, images, total =",
        LEGACY_CANDIDATE_CHECKS + b"        summaries, sources, payloads, images, total =")
    legacy_failure_checks = replace(LEGACY_CANDIDATE_CHECKS,
        b"        require(type(manifest.get('schema'))",
        b"        require(type(manifest) is dict and type(build) is dict and type(probe) is dict)\n"
        b"        require(type(manifest.get('schema'))")
    raw = replace(raw, call + b"        harness = expected_driver()\n",
        legacy_failure_checks + b"        harness = expected_driver()\n")
    raw = replace(raw, b"binding['manifest_sha256'], binding['build_record_sha256'], harness)",
        b"raw_sha(manifest_raw), raw_sha(build_raw), harness)", count=2)
    raw = replace(raw, b"require(source['tools_sha256'] == binding['tools_sha256'])",
        b"require(source['tools_sha256'] == manifest['binaries'])", count=2)
    raw = replace(raw, b"            'checks': 9, 'dimension': DIMENSION, 'actual_schema_probe': probe,\n",
        b"            'checks': 9, 'actual_schema_probe': "
        b"{'api_version': 1, 'database_schema_version': 18, 'joined': True},\n")
    raw = replace(raw,
        b"        return {'status': 'observed', **binding, 'driver_sha256': harness,\n"
        b"            'driver_exit': 1, 'dimension': DIMENSION, 'actual_schema_probe': probe,\n"
        b"            'cases': cases, 'observation_only': True, 'runtime_acceptance': False}\n",
        b"        return {'status': 'observed', 'source': expected, 'driver_sha256': harness,\n"
        b"            'driver_exit': 1, 'cases': cases, 'observation_only': True, 'runtime_acceptance': False}\n")
    return check_parent(raw, PROJECTOR_BEFORE_SHA256)
