"""Test-only exact inverse of the Goal Graph schema 19 driver changes."""

import hashlib

DRIVER_PATH = 'apps/desktop/tests/goal-graph-real-ipc.py'
CURRENT_SHA256 = '81713f3007b3c6d742e401725bc10926f541a4dd90c028c61ffc6a7f324e16b7'
HISTORICAL_SHA256 = '5c07f3fadcad286dffba98021f5d83c16eb999261d5887c506e830f5b52ca6cb'

# Line offset in the current bytes, exact current span, exact historical span.
EDITS = (
    (14,
        (
            b'from contextlib import closing, contextmanager\n'
        ),
        b'',
    ),
    (42,
        (
            b'GRAPH_SCHEMA = 19\n'
        ),
        b'',
    ),
    (49,
        (
            b"    'Root-frozen schema19 binary/manifest and new composed Graph UI assets',\n"
        ),
        (
            b"    'Root-frozen updated schema18 binary/manifest and new composed Graph UI assets',\n"
        ),
    ),
    (698,
        (
            b'def _integration_counts(db):\n'
        ),
        (
            b'def current_integration_counts(db):\n'
            b"    schema = db.execute('PRAGMA user_version').fetchone()[0]\n"
            b'    assert type(schema) is int and schema == 18\n'
        ),
    ),
    (709,
        (
            b'def _factory_counts(db):\n'
        ),
        (
            b'def current_factory_counts(db):\n'
            b'    """Global private-fixture counts, not root-filtered authority or Core receipts.\n'
            b'\n'
            b'    Exact current schema, all12 Factory and four zero integration tables are mandatory. Missing/unknown DB\n'
            b'    state raises; no absent table, failed query or NULL count becomes zero.\n'
            b'    """\n'
            b"    schema = db.execute('PRAGMA user_version').fetchone()[0]\n"
            b'    assert type(schema) is int and schema == 18\n'
            b'    current_integration_counts(db)\n'
        ),
    ),
    (718,
        (
            b'def current_integration_counts(db):\n'
            b'    """Keep the schema 18 contract for historical Factory evidence consumers."""\n'
            b"    schema = db.execute('PRAGMA user_version').fetchone()[0]\n"
            b'    assert type(schema) is int and schema == 18\n'
            b'    return _integration_counts(db)\n'
            b'\n'
            b'\n'
            b'def current_factory_counts(db):\n'
            b'    """Keep the schema 18 contract for historical Factory evidence consumers."""\n'
            b"    schema = db.execute('PRAGMA user_version').fetchone()[0]\n"
            b'    assert type(schema) is int and schema == 18\n'
            b'    current_integration_counts(db)\n'
            b'    return _factory_counts(db)\n'
            b'\n'
            b'\n'
        ),
        b'',
    ),
    (734,
        (
            b'    """Keep the schema 18 contract for historical Factory evidence consumers."""\n'
        ),
        b'',
    ),
    (742,
        (
            b'\n'
            b'\n'
            b'def graph_source():\n'
            b'    root = Path(__file__).resolve().parents[3]\n'
            b"    require(not subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=no'], cwd=root),\n"
            b"        'Graph source has tracked changes')\n"
            b"    head, tree = subprocess.check_output(['git', 'rev-parse', 'HEAD', 'HEAD^{tree}'], cwd=root, text=True).splitlines()\n"
            b'    return head, tree\n'
            b'\n'
            b'\n'
            b'def verify_graph_candidate(binary: Path, manifest_raw: bytes, head: str, tree: str) -> tuple[dict, str]:\n'
            b'    manifest = json.loads(manifest_raw, object_pairs_hook=unique_object)\n'
            b"    require(type(manifest) is dict and type(manifest.get('schema')) is int\n"
            b"        and manifest['schema'] == GRAPH_SCHEMA, 'Only an explicit frozen schema19 binary may execute current Graph')\n"
            b"    heads = [manifest[name] for name in ('git_head', 'source_head', 'compiled_core_source_head') if name in manifest]\n"
            b"    require(('git_head' in manifest or 'source_head' in manifest)\n"
            b'        and all(type(value) is str and value == head for value in heads)\n'
            b"        and manifest.get('git_tree') == tree and manifest.get('dirty') is False\n"
            b"        and manifest.get('tracked_clean', True) is True, 'Candidate must match the clean Graph source and tree')\n"
            b"    binaries = manifest.get('binaries')\n"
            b"    require(type(binaries) is dict, 'Candidate is missing its binary pins')\n"
            b"    entry = binaries.get('batond')\n"
            b"    expected = entry.get('sha256') if type(entry) is dict else entry\n"
            b"    require(type(expected) is str and re.fullmatch('[0-9a-f]{64}', expected) is not None\n"
            b"        and sha(binary.read_bytes()) == expected, 'Candidate daemon changed')\n"
            b'    return manifest, expected\n'
            b'\n'
            b'\n'
            b'def graph_database_schema(db):\n'
            b"    version = db.execute('PRAGMA user_version').fetchone()[0]\n"
            b"    require(type(version) is int and version == GRAPH_SCHEMA, 'Read-only Graph database must report integer schema 19')\n"
            b'    return version\n'
            b'\n'
            b'\n'
            b'def graph_factory_counts(db):\n'
            b'    graph_database_schema(db)\n'
            b'    _integration_counts(db)\n'
            b'    return _factory_counts(db)\n'
            b'\n'
            b'\n'
            b'@contextmanager\n'
            b'def graph_database(runtime: Path):\n'
            b"    with closing(sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True)) as db:\n"
            b"        db.execute('PRAGMA query_only=ON')\n"
            b"        db.execute('BEGIN')\n"
            b'        graph_database_schema(db)\n'
            b'        yield db\n'
            b'\n'
            b'\n'
            b'def graph_schema_evidence(host, runtime: Path):\n'
            b"    response = host.call(FixedCall('system.snapshot', {}))\n"
            b"    snapshot = response.get('result')\n"
            b"    require('error' not in response and type(snapshot) is dict\n"
            b"        and type(snapshot.get('api_version')) is int and snapshot['api_version'] == 1\n"
            b"        and type(snapshot.get('database_schema_version')) is int\n"
            b"        and snapshot['database_schema_version'] == GRAPH_SCHEMA, 'Private Graph snapshot must report API 1 and integer schema 19')\n"
            b'    with graph_database(runtime) as db:\n'
            b'        database_version = graph_database_schema(db)\n'
            b"    return {'ipc_api_version': snapshot['api_version'], 'ipc_database_schema_version': snapshot['database_schema_version'],\n"
            b"        'sqlite_user_version': database_version}\n"
        ),
        b'',
    ),
    (866,
        (
            b'    with graph_database(runtime) as db:\n'
            b"        assert graph_factory_counts(db)['child_goal_runs'] == 0\n"
        ),
        (
            b"    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
            b"        assert db.execute('PRAGMA user_version').fetchone()[0] == 18\n"
            b"        assert current_factory_counts(db)['child_goal_runs'] == 0\n"
        ),
    ),
    (1272,
        (
            b"    for schema in [11, 12, 13, 14, 15, 16, 17, 18, 20, 13.0, 14.0, 15.0, 16.0, 18.0, 19.0, True, '13', '14', '15', '16', '17', '19', None]:\n"
        ),
        (
            b"    for schema in [11, 12, 13, 14, 15, 16, 17, 19, 13.0, 14.0, 15.0, 16.0, 18.0, True, '13', '14', '15', '16', '17', None]:\n"
        ),
    ),
    (1274,
        (
            b"            verify_graph_candidate(Path('/must-not-read-a-binary'), json.dumps({'schema': schema}).encode(), 'a' * 40, 'b' * 40)\n"
            b'        except BoundaryRefusal:\n'
            b"            check('wrong/missing schema refused before binary access ' + str(schema), True)\n"
        ),
        (
            b"            verify_candidate(Path('/must-not-read-a-binary'), json.dumps({'schema': schema}).encode())\n"
            b'        except AssertionError:\n'
            b"            check('old/missing schema refused before binary access ' + str(schema), True)\n"
        ),
    ),
    (1278,
        (
            b"            raise AssertionError('Wrong schema was admitted')\n"
        ),
        (
            b"            raise AssertionError('Old schema was admitted')\n"
        ),
    ),
    (1307,
        (
            b'    """Runnable real driver. Every receipt comes from the supplied exact schema19 Core.\n'
        ),
        (
            b'    """Runnable real driver. Every receipt comes from the supplied exact schema18 Core.\n'
        ),
    ),
    (1319,
        (
            b'        manifest_path = args.candidate_manifest.resolve(strict=True)\n'
            b'        manifest_raw = manifest_path.read_bytes()\n'
            b'        source, tree = graph_source()\n'
            b'        manifest, daemon_digest = verify_graph_candidate(binary, manifest_raw, source, tree)\n'
            b"        assert daemon_digest != 'fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e', 'Old Owner-only Core5541 is not the new Graph validation-contract API'\n"
        ),
        (
            b'        manifest_raw = args.candidate_manifest.resolve(strict=True).read_bytes()\n'
            b'        manifest = verify_candidate(binary, manifest_raw)  # before any process/token/listener\n'
            b"        assert sha(binary.read_bytes()) != 'fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e', 'Old Owner-only Core5541 is not the new Graph validation-contract API'\n"
        ),
    ),
    (1325,
        (
            b"        require(sys.platform == 'linux' and Path('/proc/self/stat').is_file(), 'Current Graph browser evidence requires Linux procfs')\n"
        ),
        b'',
    ),
    (1327,
        (
            b'    except (AssertionError, OSError, ValueError, KeyError, ImportError, subprocess.CalledProcessError) as failure:\n'
        ),
        (
            b'    except (AssertionError, OSError, ValueError, KeyError, ImportError) as failure:\n'
        ),
    ),
    (1329,
        (
            b"            'harness_sha256': sha(Path(__file__).read_bytes()), 'schema_required': GRAPH_SCHEMA,\n"
        ),
        (
            b"            'harness_sha256': sha(Path(__file__).read_bytes()), 'schema_required': 18,\n"
        ),
    ),
    (1339,
        (
            b"        'candidate': manifest, 'source': source, 'git_tree': tree, 'schema_required': GRAPH_SCHEMA,\n"
            b"        'manifest_sha256': sha(manifest_raw), 'daemon_sha256': daemon_digest,\n"
        ),
        (
            b"        'candidate': manifest, 'manifest_sha256': sha(manifest_raw), 'daemon_sha256': sha(binary.read_bytes()),\n"
        ),
    ),
    (1388,
        (
            b"            report['schema_evidence'] = {'manifest_schema': manifest['schema'], **graph_schema_evidence(host, runtime)}\n"
            b"            passed('exact schema 19 candidate, private IPC snapshot and read-only SQLite agree before fixture setup')\n"
        ),
        b'',
    ),
    (1461,
        (
            b'                with graph_database(runtime) as db:\n'
        ),
        (
            b"                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"                    db.execute('PRAGMA query_only=ON')\n"
        ),
    ),
    (1709,
        (
            b'                        with graph_database(runtime) as db:\n'
        ),
        (
            b"                        with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"                            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
        ),
    ),
    (1793,
        (
            b'                        with graph_database(runtime) as db:\n'
            b"                            assert graph_factory_counts(db)['child_goal_runs'] == 0\n"
        ),
        (
            b"                        with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"                            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
            b"                            assert db.execute('PRAGMA user_version').fetchone()[0] == 18\n"
            b"                            assert current_factory_counts(db)['child_goal_runs'] == 0\n"
        ),
    ),
    (1992,
        (
            b'                try:\n'
            b"                    require(graph_source() == (source, tree) and sha(manifest_path.read_bytes()) == report['manifest_sha256']\n"
            b"                        and sha(binary.read_bytes()) == report['daemon_sha256']\n"
            b"                        and sha(Path(__file__).read_bytes()) == report['harness_sha256'],\n"
            b"                        'Graph candidate inputs changed during the run')\n"
            b"                    passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and all exact generated worker identities gone')\n"
            b'                except (AssertionError, OSError, ValueError, subprocess.CalledProcessError) as failure:\n'
            b"                    report['status'] = 'failed_or_interrupted'\n"
            b"                    report['failure'] = {'type': type(failure).__name__, 'message': clean(failure)}\n"
        ),
        (
            b"                passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and all exact generated worker identities gone')\n"
        ),
    ),
    (2029,
        (
            b"    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': GRAPH_SCHEMA}\n"
        ),
        (
            b"    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': 18}\n"
        ),
    ),
    (2064,
        (
            b'    # The owner-only and validator scenarios require explicit invocation.\n'
        ),
        (
            b'    # Full five-node/negative matrix remains pending. Only explicit --owner-only\n'
            b'    # invokes the separate real four-node driver, with a schema18 preflight.\n'
        ),
    ),
    (2068,
        (
            b'        raw = args.candidate_manifest.resolve(strict=True).read_bytes()\n'
            b"        report['manifest_sha256'] = sha(raw)\n"
            b'        try:\n'
            b"            require(args.batond is not None, 'A candidate daemon is required for Graph attribution')\n"
            b'            manifest, _ = verify_graph_candidate(args.batond, raw, *graph_source())\n'
            b"            report['manifest_schema'] = manifest['schema']\n"
            b'        except (AssertionError, OSError, ValueError, subprocess.CalledProcessError) as failure:\n'
            b"            report['refusal'] = str(failure)\n"
        ),
        (
            b'        raw = args.candidate_manifest.resolve(strict=True).read_bytes(); manifest = json.loads(raw)\n'
            b"        report['manifest_sha256'] = hashlib.sha256(raw).hexdigest()\n"
            b"        if type(manifest.get('schema')) is not int or manifest.get('schema') != 18:\n"
            b"            report['refusal'] = 'Older schema binary is not exact Graph17 execution evidence'\n"
            b'        else:\n'
            b"            report['manifest_schema'] = 18\n"
        ),
    ),
)


def restore_goal_graph_before_schema19(path, raw):
    """Return the original bytes from one complete and reviewed source variant."""
    if type(path) is not str or path != DRIVER_PATH or type(raw) is not bytes:
        raise ValueError('Expected raw bytes at the canonical Goal Graph driver path')
    if hashlib.sha256(raw).hexdigest() != CURRENT_SHA256:
        raise ValueError('Current Goal Graph source has changed')
    lines = raw.splitlines(keepends=True)
    for start, current, original in reversed(EDITS):
        end = start + len(current.splitlines(keepends=True))
        if not 0 <= start <= end <= len(lines) or b''.join(lines[start:end]) != current:
            raise ValueError('Goal Graph inverse source span has changed')
        lines[start:end] = original.splitlines(keepends=True)
    original = b''.join(lines)
    if hashlib.sha256(original).hexdigest() != HISTORICAL_SHA256:
        raise ValueError('Goal Graph inverse did not restore the pinned original')
    return original
