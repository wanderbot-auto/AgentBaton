"""Full-byte inverse of Task Workspace schema 19 driver and projector changes."""
import hashlib

DRIVER_PATH = "apps/desktop/tests/task-workspace-binding-real-ipc.py"
PROJECTOR_PATH = "scripts/record-task-workspace-ci-evidence.py"
DRIVER_PREVIOUS_SHA256 = "a4c0c4ef49a42c11000c25a135c664e121e47c5d94d3d1c90d5d22a47c5ce4b0"
DRIVER_CURRENT_SHA256 = "9c5a21b9a1964fb478ef1e0405ef36bf7652c28d658dbaf89c7b5ae20902595d"
DRIVER_SCHEMA19_MODAL_SHA256 = "e83fdd4a10b11877a79e11b3691ca96db37460ee897f693eced5d368d44e3bdd"
PROJECTOR_PREVIOUS_SHA256 = "dd602c15aac5429939a768ff5d3ff8cc7eab0d1c2c02fe8bed5ce4437fd9d1da"
PROJECTOR_CURRENT_SHA256 = "6f40d9aca7afb760f20c96ed9ba704160e8b9a3bbb618ec81a7651a9a8161a3f"
CURRENT_DRIVER_TABLES = (
    b"TABLES = tuple('''approval_proposals attempts budgets command_validation_policies\n"
    b'command_validation_runs command_workspace_fences connections conversation_budgets\n'
    b'conversation_dispatches conversation_turns conversations data_validation_policies\n'
    b'data_validation_runs data_workspace_creations data_workspace_previews data_workspace_receipts\n'
    b'dispatch_intents events execution_grants factory_attempts factory_budget_reservations\n'
    b'factory_child_goal_runs factory_child_plan_heads factory_child_plan_revisions factory_delegations\n'
    b'factory_handoffs factory_runs factory_scoped_handles factory_scopes factory_task_bindings\n'
    b'factory_task_lineage goal_external_action_journal goal_grant_revisions goal_graph_acceptances\n'
    b'goal_graph_candidates goal_graph_capabilities goal_graph_jobs goal_graph_owner_decisions\n'
    b'goal_graph_validations goal_ledgers goal_node_evidence goal_plan_heads goal_plan_proposals\n'
    b'goal_plan_revisions goal_run_nodes goal_run_reservations goal_runs goal_scopes host_registrations idempotency\n'
    b'integration_policies integrations integration_attempts integration_ref_fences\n'
    b'managed_workspaces projects session_runtime_snapshots session_turns sessions sqlite_sequence\n'
    b"tasks validation_policies validation_runs workspace_leases'''.split())\n"
)
CURRENT_DB_IMAGE = (
    b'def db_image(runtime):\n'
    b'    """All relational rows, including empty tables, sequence and FK state; never export DB bytes."""\n'
    b"    uri = (runtime / 'baton.sqlite3').absolute().as_uri() + '?mode=ro'\n"
    b'    with closing(sqlite3.connect(uri, uri=True, timeout=3)) as db:\n'
    b"        db.execute('PRAGMA query_only=ON')\n"
    b"        db.execute('BEGIN')\n"
    b"        version = db.execute('PRAGMA user_version').fetchone()\n"
    b'        require(version == (19,) and type(version[0]) is int)\n'
    b'        objects = db.execute("SELECT name, type, sql FROM main.sqlite_schema "\n'
    b'                             "WHERE type IN (\'table\',\'view\') ORDER BY name").fetchall()\n'
    b'        require({name for name, _, _ in objects} == set(TABLES)\n'
    b"                and all(kind == 'table' and type(sql) is str and sql.startswith('CREATE TABLE ')\n"
    b"                        for _, kind, sql in objects), 'Unexpected schema 19 table set')\n"
    b"        require(db.execute('SELECT 1 FROM main.host_registrations LIMIT 1').fetchone() is None,\n"
    b"                'Unexpected Host registration row')\n"
    b"        rows = {'host_registrations': []}\n"
    b'        for name in TABLES:\n'
    b"            if name == 'host_registrations':\n"
    b'                continue\n'
    b'            data = db.execute(\'SELECT * FROM main."\' + name + \'"\').fetchall()\n'
    b'            require(len(data) <= 10_000)\n'
    b'            rows[name] = sorted(repr(row) for row in data)\n'
    b"        require(not db.execute('PRAGMA foreign_key_check').fetchall())\n"
    b'        return rows\n'
    b'\n'
    b'\n'
)
PREVIOUS_DB_IMAGE = (
    b'def db_image(runtime):\n'
    b'    """All relational rows, including empty tables, sequence and FK state; never export DB bytes."""\n'
    b'    with sqlite3.connect(f\'file:{runtime / "baton.sqlite3"}?mode=ro\', uri=True, timeout=3) as db:\n'
    b"        db.execute('PRAGMA query_only=ON')\n"
    b"        db.execute('BEGIN')\n"
    b"        require(db.execute('PRAGMA user_version').fetchone()[0] == 18)\n"
    b'        names = [r[0] for r in db.execute("SELECT name FROM sqlite_schema WHERE type=\'table\' ORDER BY name")]\n'
    b'        rows = {}\n'
    b'        for name in names:\n'
    b"            require(bool(re.fullmatch(r'[A-Za-z0-9_]+', name)))\n"
    b'            data = db.execute(\'SELECT * FROM "\' + name + \'"\').fetchall()\n'
    b'            require(len(data) <= 10_000)\n'
    b'            rows[name] = sorted(repr(row) for row in data)\n'
    b"        require(not db.execute('PRAGMA foreign_key_check').fetchall())\n"
    b'        return rows\n'
    b'\n'
    b'\n'
)
CURRENT_PROBE = (
    b"        snapshot = result('system.snapshot', {})\n"
    b"        require(type(snapshot) is dict and type(snapshot.get('api_version')) is int and snapshot['api_version'] == 1\n"
    b"                and type(snapshot.get('database_schema_version')) is int and snapshot['database_schema_version'] == 19,\n"
    b"                'Actual private Core schema 19 required')\n"
    b'        no_execution(runtime)\n'
)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def replace(raw, current, previous):
    if raw.count(current) != 1:
        raise ValueError("Unexpected Task Workspace schema 19 source anchor")
    return raw.replace(current, previous, 1)


def restore_schema18_source(path, raw):
    if type(path) is not str or type(raw) is not bytes or path not in (DRIVER_PATH, PROJECTOR_PATH):
        raise ValueError("Expected raw Task Workspace bytes at a canonical source path")
    if path == DRIVER_PATH:
        current, previous = DRIVER_CURRENT_SHA256, DRIVER_PREVIOUS_SHA256
        edits = (
            (b"real schema19 Owner Unix IPC", b"real schema18 Owner Unix IPC"),
            (b"from contextlib import closing\n", b""),
            (CURRENT_DRIVER_TABLES, b""),
            (CURRENT_DB_IMAGE, PREVIOUS_DB_IMAGE),
            (b"return {'schema': 19, 'no_execution_or_grants': True,",
             b"return {'schema': 18, 'no_execution_or_grants': True,"),
            (b"    manifest_raw = manifest_path.read_bytes()\n"
             b"    require(len(manifest_raw) <= 2_097_152)\n"
             b"    manifest = decode(manifest_raw)\n"
             b"    require(type(manifest) is dict and type(manifest.get('schema')) is int and manifest['schema'] == 19)\n",
             b"    manifest = decode(manifest_path.read_bytes())\n"
             b"    require(type(manifest.get('schema')) is int and manifest['schema'] == 18)\n"),
            (b"'manifest_sha256': sha(manifest_raw)",
             b"'manifest_sha256': sha(manifest_path.read_bytes())"),
            (CURRENT_PROBE, b""),
        )
    else:
        current, previous = PROJECTOR_CURRENT_SHA256, PROJECTOR_PREVIOUS_SHA256
        edits = (
            (b"goal_run_nodes goal_run_reservations goal_runs goal_scopes host_registrations idempotency",
             b"goal_run_nodes goal_run_reservations goal_runs goal_scopes idempotency"),
            (b"value['schema'] == 19", b"value['schema'] == 18"),
            (b"    require(counts['host_registrations'] == 0)\n", b""),
            (b"return {'schema': 19, 'driver_asserted_no_execution':",
             b"return {'schema': 18, 'driver_asserted_no_execution':"),
        )
    if digest(raw) != current:
        raise ValueError("Current Task Workspace source differs from the pinned file")
    if path == DRIVER_PATH:
        # Restore the exact schema 19 driver before reversing the schema migration.
        raw = replace(raw, b"page.get_by_role('region', name='Execution & approval', exact=True)",
                      b"page.get_by_role('dialog', name='Execution & approval', exact=True)")
        back = b"dialog.get_by_role('button', name='Back', exact=True).click()"
        if raw.count(back) != 3:
            raise ValueError("Unexpected inline Task Workspace return anchors")
        raw = raw.replace(back, b"dialog.get_by_role('button', name='Close', exact=True).click()")
        if digest(raw) != DRIVER_SCHEMA19_MODAL_SHA256:
            raise ValueError("Task Workspace inverse did not restore the complete schema 19 driver")
    for new, old in edits:
        raw = replace(raw, new, old)
    if digest(raw) != previous:
        raise ValueError("Task Workspace inverse did not restore the complete schema 18 file")
    return raw
