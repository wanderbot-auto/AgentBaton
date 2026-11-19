"""Fixed raw-source inverses for the five Integration Owner schema19 paths.

Every complete input and restored predecessor is pinned. No runtime Git or
source normalization participates in the legacy tests.
"""
import hashlib


# Each edit has leading context, current bytes, predecessor bytes and trailing context.
SOURCE_LAYERS = {
    'scripts/integration-owner-fixture.py': (
        '1cd7e797fdcdf43ad7fa7a1c92beb22a2c6365e9d6b52fe68768c518eb692c0c',
        '372bd720bc03427b73174f1237fd984ad7b9de0a6e282a0b6ca54a71d0ea9441',
        (
            (
                b'\n'
                b'ROOT = Path(__file__).resolve().parents[1]\n',
                b'SCHEMA_VERSION = 19\n',
                b'',
                b'\n'
                b'\n',
            ),
            (
                b'    if not value:\n'
                b'        raise ValueError(message)\n',
                b'\n'
                b'\n'
                b'def require_core_snapshot(value):\n'
                b'    require(type(value) is dict\n'
                b"            and type(value.get('api_version')) is int and value['api_version'] == 1\n"
                b"            and type(value.get('database_schema_version')) is int\n"
                b"            and value['database_schema_version'] == SCHEMA_VERSION\n"
                b"            and type(value.get('sequence')) is int and 0 <= value['sequence'] <= (1 << 64) - 1,\n"
                b"            'Actual API1 schema19 Core required')\n",
                b'',
                b'\n'
                b'\n',
            ),
            (
                b"        require('error' not in value and 'result' in value, 'Actual generated resource request failed: ' + method)\n"
                b"        result = value['result']\n",
                b"        if method == 'system.snapshot':\n"
                b'            require_core_snapshot(result)\n',
                b'',
                b'        if method in self.PROPOSALS:\n'
                b"            require(result['kind'] == self.PROPOSALS[method] and result['consumed'] is False)\n",
            ),
            (
                b'\n'
                b'\n',
                b'def paired_tools(daemon_path, companion_path):\n'
                b'    daemon, companion = Path(daemon_path).absolute(), Path(companion_path).absolute()\n'
                b"    require((daemon.name, companion.name) == ('batond', 'baton')\n"
                b'            and daemon.parent.resolve(strict=True) == companion.parent.resolve(strict=True),\n'
                b"            'Integration fixture requires the same-directory batond/baton build pair')\n"
                b'    return daemon, companion\n'
                b'\n'
                b'\n'
                b'def verify_bootstrap_identity(owner, installed, runtime, env, snapshot):\n'
                b"    completed = subprocess.run([str(installed / 'baton'), 'snapshot'], env=env,\n"
                b'                               stdin=subprocess.DEVNULL, capture_output=True, timeout=10, check=False)\n'
                b'    require(completed.returncode == 0 and 0 < len(completed.stdout) <= 4_194_304,\n'
                b"            'Matching private baton snapshot required')\n"
                b'    reply = json.loads(completed.stdout)\n'
                b"    require(type(reply) is dict and 'error' not in reply and owner.json_exact(reply.get('result'), snapshot),\n"
                b"            'Integration companion and private Unix IPC snapshots disagree')\n"
                b"    require(database(runtime)['sequence'] == snapshot['sequence'],\n"
                b"            'Integration private SQLite and Core snapshot sequences disagree')\n"
                b'\n'
                b'\n'
                b'@contextmanager\n'
                b'def bootstrap(owner, daemon_path, companion_path, output):\n'
                b'    """Actual Core only, when Root explicitly runs it. No OS success fabricated."""\n'
                b'    daemon_path, companion_path = paired_tools(daemon_path, companion_path)\n'
                b"    installed = output / 'private-tools'; installed.mkdir(mode=0o700)\n"
                b"    tools = {name: tool_copy(source, installed / name) for name, source in [('batond', daemon_path), ('baton', companion_path)]}\n"
                b"    require(tools['batond']['sha256'] != tools['baton']['sha256'], 'Integration tool roles share the same image')\n",
                b'@contextmanager\n'
                b'def bootstrap(owner, daemon_path, companion_path, output):\n'
                b'    """Actual Core only, when Root explicitly runs it. No OS success fabricated."""\n'
                b"    installed = output / 'private-tools'; installed.mkdir(mode=0o700)\n"
                b"    tools = {name: tool_copy(source, installed / name) for name, source in [('batond', daemon_path), ('baton', companion_path)]}\n",
                b"    private = output / 'bootstrap'; private.mkdir(mode=0o700)\n"
                b'    daemon = None; client = None; generated = {}; failure = None\n',
            ),
            (
                b"                wait(lambda: (runtime/'session.token').is_file() and (runtime/'batond.sock').exists(), bool, 10)\n"
                b"                snapshot = client.call('system.snapshot', {})\n",
                b'                verify_bootstrap_identity(owner, installed, runtime, env, snapshot)\n',
                b"                require(type(snapshot['database_schema_version']) is int and snapshot['database_schema_version'] == 18, 'Actual schema18 Core required')\n",
                b"                project = client.call('projects.import', {'path': str(source), 'name': 'Native independent integration generated'}, 'integration-owner:project')\n"
                b"                task = client.call('tasks.create', {'project_id': project['id'], 'goal': 'Generated independent integration approval'}, 'integration-owner:task')\n",
            ),
            (
                b'    helper = importlib.util.module_from_spec(spec); spec.loader.exec_module(helper)\n'
                b"    tables = tuple(sorted((*helper.SCHEMA_TABLES, 'integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences',\n",
                b"                           'conversations', 'conversation_dispatches', 'conversation_turns', 'conversation_budgets', 'host_registrations')))\n"
                b"    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro', uri=True) as db:\n"
                b"        db.execute('PRAGMA query_only=ON'); db.execute('PRAGMA trusted_schema=OFF'); db.execute('BEGIN')\n"
                b"        schema = db.execute('PRAGMA user_version').fetchone()[0]\n"
                b"        require(type(schema) is int and schema == SCHEMA_VERSION, 'Actual schema19 private SQLite required')\n",
                b"                           'conversations', 'conversation_dispatches', 'conversation_turns', 'conversation_budgets')))\n"
                b"    with sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro', uri=True) as db:\n"
                b"        db.execute('PRAGMA query_only=ON'); db.execute('PRAGMA trusted_schema=OFF'); db.execute('BEGIN')\n"
                b"        require(db.execute('PRAGMA user_version').fetchone()[0] == 18)\n",
                b'        require(tuple(r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type=\'table\' ORDER BY name")) == tables)\n'
                b'        columns, sizes, remaining, total_rows = {}, {}, 8_388_608, 0\n',
            ),
            (
                b"            require(remaining >= 0 and total_rows <= 100000, 'Private readonly raw history exceeds fixture bound')\n"
                b'            sizes[table] = count\n',
                b"        require(sizes['host_registrations'] == 0, 'Generated Integration fixture must have no Host authority')\n"
                b"        encode_cell = lambda v: {'sqlite_blob_hex': v.hex()} if type(v) is bytes else v\n"
                b'        rows = {table: [[encode_cell(v) for v in row] for row in db.execute(\'SELECT rowid,* FROM "\'+table+\'" ORDER BY rowid\')] for table in tables}\n'
                b'        db.rollback()\n'
                b"    return {'read_only': True, 'schema_version': schema, 'columns': columns, 'counts': sizes, 'rows': rows,\n",
                b"        encode_cell = lambda v: {'sqlite_blob_hex': v.hex()} if type(v) is bytes else v\n"
                b'        rows = {table: [[encode_cell(v) for v in row] for row in db.execute(\'SELECT rowid,* FROM "\'+table+\'" ORDER BY rowid\')] for table in tables}\n'
                b'        db.rollback()\n'
                b"    return {'read_only': True, 'schema_version': 18, 'columns': columns, 'counts': sizes, 'rows': rows,\n",
                b"            'sequence': max((r[1] for r in rows['events']), default=0)}\n"
                b'\n',
            ),
            (
                b'\n'
                b'\n',
                b'def current_database(value):\n'
                b"    return (type(value) is dict and value.get('read_only') is True\n"
                b"            and type(value.get('schema_version')) is int and value['schema_version'] == SCHEMA_VERSION)\n"
                b'\n'
                b'\n'
                b'def pending_error(owner, value, before):\n'
                b'    return (None if current_database(value) and owner.json_exact(value, before)\n'
                b"            else 'Pending integration review changed actual private database history')\n"
                b'\n'
                b'\n'
                b'def approved_error(owner, value, before, proposal, response):\n'
                b'    if (not current_database(before) or not current_database(value)\n'
                b"            or response.get('error') is not None or type(response.get('result')) is not dict\n",
                b'def pending_error(owner, value, before):\n'
                b"    return None if owner.json_exact(value, before) else 'Pending integration review changed actual private database history'\n"
                b'\n'
                b'\n'
                b'def approved_error(owner, value, before, proposal, response):\n'
                b"    if (value['schema_version'] != 18 or response.get('error') is not None or type(response.get('result')) is not dict\n",
                b"            or value['sequence'] != before['sequence'] + 2 or value['columns'] != before['columns']):\n"
                b"        return 'Missing exact integration response/schema/two-event transaction'\n",
            ),
            (
                b"        require(generated['daemon_cleanup']['ok'])\n"
                b"    owner.save(output/'result.json',{'status':'actual_integration_cancel_and_same_key_approval_visual_review_pending',\n",
                b"        'phases':results,'daemon_cleanup':generated['daemon_cleanup'], 'schema':SCHEMA_VERSION,\n",
                b"        'phases':results,'daemon_cleanup':generated['daemon_cleanup'], 'schema':18,\n",
                b"        'dimension':'macos_own_process_real_sheet','real_provider':'not_run','human_owner_approval':'not_claimed',\n"
                b"        'harness_binary_sha256':native['sha256'],'tools':generated['tools']})\n",
            ),
        ),
    ),
    'scripts/native-owner-dialog-smoke.py': (
        '293149dafc983d6c1fbc0b797486ce24ae7da380d81560ef8b1c2c078c074a53',
        '6a5406689896c52aef7d3d478bacf156cd27a8c1a5f0c8d3de552ba07d345d55',
        (
            (
                b"INTEGRATION_KIND = 'native_integration_os_dialog_fixture'\n"
                b"INTEGRATION_KEY = 'owner-os-integration'\n",
                b'INTEGRATION_SCHEMA_VERSION = 19\n',
                b'',
                b"INTEGRATION_TITLE = 'AgentBaton \xe2\x80\x94 Independent local integration approval'\n"
                b'INTEGRATION_GROUP_FIELDS = frozenset({\n',
            ),
            (
                b'        if (type(checkpoint) is not dict or set(checkpoint) != INTEGRATION_CHECKPOINT_FIELDS\n'
                b'                or any(type(n) is not int or not 0 <= n <= (1 << 64) - 1 for n in checkpoint.values())\n',
                b"                or checkpoint['schema_version'] != INTEGRATION_SCHEMA_VERSION):\n"
                b"            return 'Integration exact schema19/checkpoint typed count mismatch'\n",
                b"                or checkpoint['schema_version'] != 18):\n"
                b"            return 'Integration exact schema18/checkpoint typed count mismatch'\n",
                b"    if (before['sequence'] != baseline_sequence or any(before[k] != 0 for k in ['policies', 'runs', 'attempts', 'ref_fences'])\n"
                b"            or any(after[k] != 0 for k in ['runs', 'attempts', 'ref_fences'])):\n",
            ),
        ),
    ),
    'apps/desktop/src-tauri/src/native_owner_dialog_test/integration_database.rs': (
        'dbdf877c223be4090b9d60a6e3550daf351d2f7fd532949abeae22d4f1ef60fa',
        '8730e078ebb48136802c3a614c44636d656c9336f3974b37d5bf018a83762755',
        (
            (
                b"db.execute('PRAGMA query_only=ON');db.execute('PRAGMA trusted_schema=OFF');db.execute('BEGIN')\n"
                b"assert db.execute('PRAGMA query_only').fetchone()[0]==1\n",
                b"schema=db.execute('PRAGMA user_version').fetchone()[0];assert type(schema) is int and schema==19\n"
                b"tables=('integration_policies','integrations','integration_attempts','integration_ref_fences')\n"
                b"for table in (*tables,'host_registrations'):\n"
                b"    row=db.execute('SELECT type,sql FROM sqlite_master WHERE name=?',(table,)).fetchone()\n"
                b"    assert row is not None and row[0]=='table' and row[1].upper().startswith('CREATE TABLE ')\n"
                b"assert tuple(row[1] for row in db.execute('PRAGMA table_info(host_registrations)'))==('id','name','name_key','revision','created_sequence','document')\n"
                b"assert db.execute('SELECT count(*) FROM host_registrations').fetchone()==(0,)\n",
                b"schema=db.execute('PRAGMA user_version').fetchone()[0];assert schema==18\n"
                b"tables=('integration_policies','integrations','integration_attempts','integration_ref_fences')\n"
                b'for table in tables:\n'
                b"    row=db.execute('SELECT type,sql FROM sqlite_master WHERE name=?',(table,)).fetchone()\n"
                b"    assert row is not None and row[0]=='table' and row[1].upper().startswith('CREATE TABLE ')\n",
                b"counts=[db.execute('SELECT count(*) FROM '+t).fetchone()[0] for t in tables]\n"
                b"sequence=db.execute('SELECT COALESCE(max(sequence),0) FROM events').fetchone()[0]\n",
            ),
        ),
    ),
    'apps/desktop/src-tauri/src/native_owner_dialog_test/controller.rs': (
        '6f3da88a6043c2b215193a7066a9d53922d4abe410c35a426297a43c6d65dacf',
        'c82d39abb4074971a8bf43b25454b5b6698f6b3b6a7b35beb85dbfb41b8d959d',
        (
            (
                b'fn integration_counts(value: &Value, policies: u64) -> bool {\n'
                b'    value.as_object().is_some_and(|v| v.len() == 6)\n',
                b'        && value["schema_version"].as_u64() == Some(19)\n',
                b'        && value["schema_version"].as_u64() == Some(18)\n',
                b'        && value["sequence"].as_u64().is_some()\n'
                b'        && value["policies"].as_u64() == Some(policies)\n',
            ),
            (
                b'    use serde_json::json;\n'
                b'    fn witness(policies: u64, sequence: u64, owner: Value, policy: Value) -> Value {\n',
                b'        json!({"counts":{"schema_version":19,"sequence":sequence,"policies":policies,"runs":0,"attempts":0,"ref_fences":0},"proposal_matches":true,"proposal_result_matches":true,"policy_matches":true,"owner_event_matches":true,"policy_event_matches":true,"owner_event_sequence":owner,"policy_event_sequence":policy})\n',
                b'        json!({"counts":{"schema_version":18,"sequence":sequence,"policies":policies,"runs":0,"attempts":0,"ref_fences":0},"proposal_matches":true,"proposal_result_matches":true,"policy_matches":true,"owner_event_matches":true,"policy_event_matches":true,"owner_event_sequence":owner,"policy_event_sequence":policy})\n',
                b'    }\n'
                b'    #[test]\n',
            ),
            (
                b'            ("schema_version", json!(16)),\n'
                b'            ("schema_version", json!(17)),\n',
                b'            ("schema_version", json!(18)),\n'
                b'            ("schema_version", json!(20)),\n'
                b'            ("schema_version", json!(18.0)),\n'
                b'            ("schema_version", json!(19.0)),\n'
                b'            ("schema_version", json!("19")),\n'
                b'            ("schema_version", json!(true)),\n',
                b'            ("schema_version", json!(19)),\n'
                b'            ("schema_version", json!(18.0)),\n',
                b'            ("policies", json!(true)),\n'
                b'            ("runs", json!(1)),\n',
            ),
        ),
    ),
    'scripts/record-integration-owner-ci-evidence.py': (
        '1dcdd2a8d43fbc8f1a389eef645393b389cdb2dc99d0d1212595689adfa323ad',
        '458f7237529ba34468ce1b620940c19e904029b5ce8fd1688fa67e72b6a34700',
        (
            (
                b'        # All keys here are fixed and scalar. Unknown raw/private fields never\n'
                b'        # become a "safe" JSON value through passthrough or stringification.\n',
                b"        phases.append({'phase': phase, 'schema': group['after']['schema_version'],\n",
                b"        phases.append({'phase': phase, 'schema': 18,\n",
                b"            'baseline_sequence': baseline_sequence, 'after_sequence': group['after']['sequence'],\n"
                b"            'policy_count': group['after']['policies'], 'run_count': 0, 'attempt_count': 0, 'ref_fence_count': 0,\n",
            ),
            (
                b"            'exact_native_record_bindings': True})\n"
                b"    require(N.json_exact(cancel['integration_approval_database']['before'], approve['integration_approval_database']['before']))\n",
                b"    return {'contract': 'agentbaton.integration-owner-ci.v1', 'schema': phases[0]['schema'],\n",
                b"    return {'contract': 'agentbaton.integration-owner-ci.v1', 'schema': 18,\n",
                b"            'status': 'valid_native_report_projection', 'origin': 'native_report_asserted',\n"
                b"            'visual_review': 'not_established_by_collector', 'driver_cleanup': 'not_established_by_collector',\n",
            ),
        ),
    ),
}


# Undo the current Grant/Factory Store19 change before the pinned Integration layer.
OWNER_RUNNER = 'scripts/native-owner-dialog-smoke.py'
GRANT_FACTORY_STORE19_LAYER = (
    'c29a8e2e117a88894a1068301f494e4c286c37635e884598ed34af74e3067e84',
    SOURCE_LAYERS[OWNER_RUNNER][0],
    (
        (
            b'"""\n'
            b'import argparse\n',
            b'from contextlib import closing, contextmanager, ExitStack\n',
            b'from contextlib import contextmanager, ExitStack\n',
            b'import hashlib\n'
            b'import importlib.util\n',
        ),
        (
            b'\n'
            b'\n',
            b'# Grant and Factory evidence must come from the current Store schema.\n'
            b'GRANT_FACTORY_SCHEMA_VERSION = 19\n',
            b'# Exact schema18 tables: never substitute old test DDL or silently skip absent\n'
            b'# tables. Every Goal store/runtime/graph table is empty in this Grant-only probe.\n',
            b"GOAL_TABLES = ('goal_grant_revisions', 'goal_scopes', 'goal_plan_heads',\n"
            b"               'goal_plan_revisions', 'goal_plan_proposals', 'goal_ledgers',\n",
        ),
        (
            b"INTEGRATION_TABLES = ('integration_policies', 'integrations', 'integration_attempts', 'integration_ref_fences')\n"
            b"CONVERSATION_TABLES = ('conversations', 'conversation_dispatches', 'conversation_turns', 'conversation_budgets')\n",
            b"HOST_TABLES = ('host_registrations',)\n"
            b"ZERO_TABLES = ('attempts', 'sessions', *GOAL_TABLES, *FACTORY_TABLES, *INTEGRATION_TABLES, *CONVERSATION_TABLES, *HOST_TABLES)\n",
            b"ZERO_TABLES = ('attempts', 'sessions', *GOAL_TABLES, *FACTORY_TABLES, *INTEGRATION_TABLES, *CONVERSATION_TABLES)\n",
            b'\n'
            b'\n',
        ),
        (
            b'\n'
            b'def database(runtime, proposal_id, *, factory_planning=False):\n',
            b"    uri = (runtime / 'baton.sqlite3').as_uri() + '?mode=ro'\n"
            b'    with closing(sqlite3.connect(uri, uri=True)) as conn, conn:\n'
            b"        conn.execute('PRAGMA query_only=ON')\n"
            b"        conn.execute('BEGIN')\n"
            b"        schema = conn.execute('PRAGMA user_version').fetchone()[0]\n"
            b'        if type(schema) is not int or schema != GRANT_FACTORY_SCHEMA_VERSION:\n'
            b"            raise ValueError('Actual Store19 Grant/Factory SQLite required')\n"
            b'        tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type=\'table\'")}\n'
            b'        missing = set(ZERO_TABLES) - tables\n'
            b'        if missing:\n'
            b"            raise sqlite3.OperationalError('Missing Grant/Factory evidence tables: ' + ', '.join(sorted(missing)))\n",
            b"    with sqlite3.connect((runtime / 'baton.sqlite3').as_uri() + '?mode=ro', uri=True) as conn:\n"
            b"        conn.execute('PRAGMA query_only=ON')\n"
            b"        conn.execute('BEGIN')\n"
            b"        schema = conn.execute('PRAGMA user_version').fetchone()[0]\n",
            b"        sequence = conn.execute('SELECT coalesce(max(sequence),0) FROM events').fetchone()[0]\n"
            b"        rows = conn.execute('SELECT document,result FROM approval_proposals WHERE id=?', (proposal_id,)).fetchall()\n",
        ),
        (
            b"    if (type(sequence) is not int or type(proposal.get('revision')) is not int\n"
            b"            or proposal['revision'] <= 0\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), GRANT_FACTORY_SCHEMA_VERSION)\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 18)\n",
            b"            or type(value.get('sequence')) is not int or not json_exact(value['sequence'], sequence)\n"
            b"            or not json_exact(value.get('proposal'), proposal) or proposal.get('kind') != 'grant'\n",
        ),
        (
            b"    if (type(baseline) is not int or type(proposal.get('revision')) is not int\n"
            b"            or proposal['revision'] <= 0\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), GRANT_FACTORY_SCHEMA_VERSION)\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 18)\n",
            b'            or not isinstance(grants, list) or len(grants) != 1\n'
            b"            or current.get('id') != proposal['id'] or not json_exact(current.get('snapshot'), proposal['snapshot'])\n",
        ),
        (
            b"    proposal = fixture['proposal']\n"
            b"    if (type(before.get('sequence')) is not int or type(proposal.get('revision')) is not int\n",
            b"            or proposal['revision'] <= 0 or value.get('read_only') is not True or not json_exact(value.get('schema_version'), GRANT_FACTORY_SCHEMA_VERSION)\n",
            b"            or proposal['revision'] <= 0 or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 18)\n",
            b"            or not json_exact(value.get('sequence'), before['sequence'])\n"
            b"            or proposal.get('kind') != 'factory' or proposal.get('consumed') is not False\n",
        ),
        (
            b"    if (type(before.get('sequence')) is not int or type(proposal.get('revision')) is not int\n"
            b"            or proposal['revision'] <= 0 or not isinstance(scope, dict) or response.get('error') is not None\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), GRANT_FACTORY_SCHEMA_VERSION)\n",
            b"            or value.get('read_only') is not True or not json_exact(value.get('schema_version'), 18)\n",
            b"            or not json_exact(value.get('sequence'), before['sequence'] + 1)\n"
            b"            or not json_exact(value.get('proposal'), current)\n",
        ),
    ),
)


def _restore_pinned_source(raw, layer):
    current_sha, parent_sha, edits = layer
    if hashlib.sha256(raw).hexdigest() != current_sha:
        raise ValueError("Current Integration Owner source has changed")
    for leading, current, parent, trailing in edits:
        anchor = leading + current + trailing
        if raw.count(anchor) != 1:
            raise ValueError("Expected exactly one full Integration Owner anchor")
        raw = raw.replace(anchor, leading + parent + trailing, 1)
    if hashlib.sha256(raw).hexdigest() != parent_sha:
        raise ValueError("Integration Owner inverse did not restore the pinned predecessor")
    return raw


def restore_grant_factory_owner_before_store19(raw):
    """Recover the pinned Integration Owner input from the current runner."""
    if type(raw) is not bytes:
        raise ValueError("Invalid raw Grant and Factory Owner source type")
    return _restore_pinned_source(raw, GRANT_FACTORY_STORE19_LAYER)


def restore_integration_owner_before_schema19(path, raw):
    """Recover exactly the prior file for a pinned raw input and path."""
    if type(path) is not str or path not in SOURCE_LAYERS or type(raw) is not bytes:
        raise ValueError("Unknown path or invalid raw Integration Owner source type")
    if path == OWNER_RUNNER:
        raw = restore_grant_factory_owner_before_store19(raw)
    return _restore_pinned_source(raw, SOURCE_LAYERS[path])
