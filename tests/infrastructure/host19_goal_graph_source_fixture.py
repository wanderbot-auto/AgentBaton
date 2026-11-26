"""Test-only exact inverse of the Goal Graph schema 19 driver changes."""

import hashlib

DRIVER_PATH = 'apps/desktop/tests/goal-graph-real-ipc.py'
CURRENT_SHA256 = '48023cd5b30b5f622a54d262600c76e06cb358b61a32f698fdbec75055fc0f16'
PRE_UI_SIMPLIFICATION_SHA256 = '550256ce728cbc26c5873237e09888195cfe01726b697d46d5e2b881e46b4d18'
HISTORICAL_SHA256 = '5c07f3fadcad286dffba98021f5d83c16eb999261d5887c506e830f5b52ca6cb'


# The current browser selectors are an outer layer over the schema 19 driver.
UI_SIMPLIFICATION_EDITS = (
    (1605,
        (
            b"                    panel = page.get_by_test_id('goal-planning')\n"
            b"                    read_start = len(report['http_requests'])\n"
            b"                    expand(panel.locator(':scope > details'))\n"
            b"                    wait(lambda: report['http_requests'][read_start:], lambda entries: any(\n"
            b"                        row['method'] == 'goal_plans.list' and row['params'] == {'scope_id': scope['id']}\n"
            b"                        and not row['response'].get('error') for row in entries))\n"
            b"                    scope_choices = panel.get_by_role('combobox', name='Selected Goal scope', exact=True)\n"
            b'                    if scope_choices.count():\n'
            b"                        expect(scope_choices).to_have_value(scope['id'])\n"
            b"                    editor = panel.get_by_test_id('goal-plan-proposal')\n"
        ),
        (
            b"                    panel = page.get_by_test_id('goal-planning'); expand(panel.locator(':scope > details'))\n"
            b"                    expect(panel.get_by_role('combobox', name='Selected Goal scope', exact=True)).to_have_value(scope['id'])\n"
            b"                    editor = panel.locator('details').filter(has=page.get_by_text('Versioned plan proposal', exact=True)).last\n"
        ),
    ),
    (1616,
        (
            b"                    raw_summary = editor.get_by_text('Plan proposal JSON', exact=True).and_(editor.locator('summary'))\n"
            b"                    raw_details = raw_summary.locator('..')\n"
            b"                    assert not raw_details.evaluate('element=>element.open')\n"
            b'                    expand(raw_details)\n'
        ),
        b'',
    ),
    (1667,
        b'',
        b'                        target = graph.locator(\'[data-testid=goal-graph-inspect][data-run-id="\' + run_id + \'"]\')\n',
    ),
    (1668,
        (
            b"                            expand(graph.get_by_test_id('goal-graph-runs'))\n"
            b'                            run = graph.locator(\'[data-testid=goal-graph-run][data-run-id="\' + run_id + \'"]\')\n'
            b'                            expand(run)\n'
            b"                            run.get_by_test_id('goal-graph-inspect').click()\n"
            b"                            expect(graph).to_have_attribute('aria-busy', 'false')\n"
        ),
        b"                            target.click(); expect(graph).to_have_attribute('aria-busy', 'false')\n",
    ),
    (1761,
        (
            b"                        expect(history.get_by_role('status')).to_have_text('Blocked')\n"
            b"                        expect(history.get_by_text('Task accepted', exact=True)).to_have_count(0)\n"
            b"                        expect(history.get_by_test_id('goal-graph-blockers')).to_be_visible()\n"
            b"                        evidence = history.get_by_test_id('goal-graph-evidence')\n"
            b"                        assert not evidence.evaluate('element=>element.open')\n"
            b'                        expand(evidence)\n'
            b'                        failed_node = history.locator(\'[data-node-id="validate-report"] > details > summary\')\n'
            b"                        expect(failed_node).to_contain_text('Failed')\n"
            b'                        expect(failed_node).to_be_visible()\n'
        ),
        (
            b"                        expect(history.get_by_text('Task acceptance pending', exact=True)).to_be_visible()\n"
            b'                        expect(history.locator(\'[data-node-id="validate-report"] > details > summary\')).to_contain_text(\'Failed\')\n'
        ),
    ),
    (1794,
        (
            b"                        history = graph.get_by_test_id('goal-graph-history')\n"
            b"                        expect(history.get_by_test_id('goal-graph-current-artifacts')).to_contain_text(first_artifact['relative_path'])\n"
            b"                        expect(history.get_by_test_id('goal-graph-evidence')).not_to_have_attribute('open', '')\n"
            b"                        shot('01-work-candidate-before-owner', history, 'owner-report')\n"
        ),
        b"                        shot('01-work-candidate-before-owner', graph.get_by_test_id('goal-graph-history'), 'owner-report')\n",
    ),
    (1919,
        (
            b"                        artifacts = history.get_by_test_id('goal-graph-current-artifacts')\n"
            b"                        for artifact in accepted['task_acceptance']['artifact_refs']:\n"
            b"                            expect(artifacts).to_contain_text(artifact['relative_path'])\n"
        ),
        b'',
    ),
)


# Line offset in pre-UI schema 19 bytes, exact source span, exact historical span.
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
    (1306,
        (
            b'def verify_graph_run_inputs(report, manifest_path: Path, binary: Path):\n'
            b"    require(graph_source() == (report['source'], report['git_tree'])\n"
            b"        and sha(manifest_path.read_bytes()) == report['manifest_sha256']\n"
            b"        and sha(binary.read_bytes()) == report['daemon_sha256']\n"
            b"        and sha(Path(__file__).read_bytes()) == report['harness_sha256'],\n"
            b"        'Graph candidate inputs changed during the run')\n"
            b'\n'
            b'\n'
            b'def record_graph_failure(report, failure, clean):\n'
            b"    report['status'] = 'failed_or_interrupted'\n"
            b"    report['failure'] = {'type': type(failure).__name__, 'message': clean(failure), 'traceback': clean(traceback.format_exc())}\n"
            b'\n'
            b'\n'
        ),
        b'',
    ),
    (1320,
        (
            b'    """Runnable real driver. Every receipt comes from the supplied exact schema19 Core.\n'
        ),
        (
            b'    """Runnable real driver. Every receipt comes from the supplied exact schema18 Core.\n'
        ),
    ),
    (1332,
        (
            b'        manifest_path = args.candidate_manifest.resolve(strict=True)\n'
            b'        manifest_raw = manifest_path.read_bytes()\n'
            b'        source_head, source_tree = graph_source()\n'
            b'        manifest, daemon_digest = verify_graph_candidate(binary, manifest_raw, source_head, source_tree)\n'
            b"        assert daemon_digest != 'fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e', 'Old Owner-only Core5541 is not the new Graph validation-contract API'\n"
        ),
        (
            b'        manifest_raw = args.candidate_manifest.resolve(strict=True).read_bytes()\n'
            b'        manifest = verify_candidate(binary, manifest_raw)  # before any process/token/listener\n'
            b"        assert sha(binary.read_bytes()) != 'fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e', 'Old Owner-only Core5541 is not the new Graph validation-contract API'\n"
        ),
    ),
    (1338,
        (
            b"        require(sys.platform == 'linux' and Path('/proc/self/stat').is_file(), 'Current Graph browser evidence requires Linux procfs')\n"
        ),
        b'',
    ),
    (1340,
        (
            b'    except (AssertionError, OSError, ValueError, KeyError, ImportError, subprocess.CalledProcessError) as failure:\n'
        ),
        (
            b'    except (AssertionError, OSError, ValueError, KeyError, ImportError) as failure:\n'
        ),
    ),
    (1342,
        (
            b"            'harness_sha256': sha(Path(__file__).read_bytes()), 'schema_required': GRAPH_SCHEMA,\n"
        ),
        (
            b"            'harness_sha256': sha(Path(__file__).read_bytes()), 'schema_required': 18,\n"
        ),
    ),
    (1352,
        (
            b"        'candidate': manifest, 'source': source_head, 'git_tree': source_tree, 'schema_required': GRAPH_SCHEMA,\n"
            b"        'manifest_sha256': sha(manifest_raw), 'daemon_sha256': daemon_digest,\n"
        ),
        (
            b"        'candidate': manifest, 'manifest_sha256': sha(manifest_raw), 'daemon_sha256': sha(binary.read_bytes()),\n"
        ),
    ),
    (1401,
        (
            b"            report['schema_evidence'] = {'manifest_schema': manifest['schema'], **graph_schema_evidence(host, runtime)}\n"
            b"            passed('exact schema 19 candidate, private IPC snapshot and read-only SQLite agree before fixture setup')\n"
        ),
        b'',
    ),
    (1474,
        (
            b'                with graph_database(runtime) as db:\n'
        ),
        (
            b"                with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"                    db.execute('PRAGMA query_only=ON')\n"
        ),
    ),
    (1722,
        (
            b'                        with graph_database(runtime) as db:\n'
        ),
        (
            b"                        with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as db:\n"
            b"                            db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
        ),
    ),
    (1806,
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
    (1909,
        (
            b'            record_graph_failure(report, failure, clean)\n'
        ),
        (
            b"            report['failure'] = {'type': type(failure).__name__, 'message': clean(failure), 'traceback': clean(traceback.format_exc())}\n"
        ),
    ),
    (2005,
        (
            b'                try:\n'
            b'                    verify_graph_run_inputs(report, manifest_path, binary)\n'
            b"                    passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and all exact generated worker identities gone')\n"
            b'                except (AssertionError, OSError, ValueError, subprocess.CalledProcessError) as failure:\n'
            b'                    record_graph_failure(report, failure, clean)\n'
        ),
        (
            b"                passed('HTTP handlers and serve thread joined without unexpected errors; daemon exit0 and all exact generated worker identities gone')\n"
        ),
    ),
    (2038,
        (
            b"    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': GRAPH_SCHEMA}\n"
        ),
        (
            b"    attribution = {'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'schema_required': 18}\n"
        ),
    ),
    (2073,
        (
            b'    # The owner-only and validator scenarios require explicit invocation.\n'
        ),
        (
            b'    # Full five-node/negative matrix remains pending. Only explicit --owner-only\n'
            b'    # invokes the separate real four-node driver, with a schema18 preflight.\n'
        ),
    ),
    (2077,
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


def _require_driver_bytes(path, raw):
    if type(path) is not str or path != DRIVER_PATH or type(raw) is not bytes:
        raise ValueError('Expected raw bytes at the canonical Goal Graph driver path')


def _reverse_pinned_edits(raw, edits, expected_sha256):
    lines = raw.splitlines(keepends=True)
    for start, current, original in reversed(edits):
        end = start + len(current.splitlines(keepends=True))
        if not 0 <= start <= end <= len(lines) or b''.join(lines[start:end]) != current:
            raise ValueError('Goal Graph inverse source span has changed')
        lines[start:end] = original.splitlines(keepends=True)
    original = b''.join(lines)
    if hashlib.sha256(original).hexdigest() != expected_sha256:
        raise ValueError('Goal Graph inverse did not restore the pinned original')
    return original


def restore_goal_graph_before_ui_simplification(path, raw):
    """Remove the reviewed UI selector changes and restore the exact schema 19 source."""
    _require_driver_bytes(path, raw)
    if hashlib.sha256(raw).hexdigest() != CURRENT_SHA256:
        raise ValueError('Current Goal Graph source has changed')
    return _reverse_pinned_edits(raw, UI_SIMPLIFICATION_EDITS, PRE_UI_SIMPLIFICATION_SHA256)


def restore_goal_graph_before_schema19(path, raw):
    """Restore the original schema 18 source from either exact reviewed schema 19 variant."""
    _require_driver_bytes(path, raw)
    if hashlib.sha256(raw).hexdigest() != PRE_UI_SIMPLIFICATION_SHA256:
        raw = restore_goal_graph_before_ui_simplification(path, raw)
    return _reverse_pinned_edits(raw, EDITS, HISTORICAL_SHA256)
