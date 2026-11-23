"""Exact test-only inverse for the schema19 native smoke source change.

The restored parent is the pinned input of native_acceptance_followup.json.
The existing consumer still restores and checks the older historical blob.
"""
import hashlib

NATIVE_SMOKE_PATH = 'apps/desktop/src-tauri/src/bin/native_smoke.rs'
CURRENT_SHA256 = '58721865993d21d3aa7a34be91ae1080bcb850f9c157e7daa9611bf7d5f8ee9c'
PARENT_SHA256 = '5145fdce52955077dcfd591b712977452dec3cfd09bb4bb06807968e0a1bcdfa'

# Each edit pins the leading context, current bytes, parent bytes, and trailing context.
SOURCE_EDITS = (
    (
        b'    if !valid_goal_execution_counts(&snapshot) {\n'
        b'        return Err(\n',
        b'            "Goal execution fixture must have schema19 and no run/node/reservation authority"\n',
        b'            "Goal execution fixture must have schema18 and no run/node/reservation authority"\n',
        b'                .into(),\n'
        b'        );\n',
    ),
    (
        b'fn valid_goal_execution_counts(value: &Value) -> bool {\n'
        b'    value.as_object().is_some_and(|record| record.len() == 5)\n',
        b'        && value["schema_version"].as_u64() == Some(19)\n',
        b'        && value["schema_version"].as_u64() == Some(18)\n',
        b'        && ["run_count", "node_count", "reservation_count"]\n'
        b'            .iter()\n',
    ),
    (
        b'fn valid_goal_graph_counts(value: &Value) -> bool {\n'
        b'    value.as_object().is_some_and(|record| record.len() == 8)\n',
        b'        && value["schema_version"].as_u64() == Some(19)\n',
        b'        && value["schema_version"].as_u64() == Some(18)\n',
        b'        && GOAL_GRAPH_COUNT_FIELDS\n'
        b'            .iter()\n',
    ),
    (
        b'        serde_json::from_slice(&bytes).map_err(|_| "invalid readonly Goal graph counts")?;\n'
        b'    if !valid_goal_graph_counts(&snapshot) {\n',
        b'        return Err("Goal graph fixture must have schema19 and six empty graph tables".into());\n',
        b'        return Err("Goal graph fixture must have schema18 and six empty graph tables".into());\n',
        b'    }\n'
        b'    Ok(snapshot)\n',
    ),
    (
        b'fn valid_factory_counts(value: &Value) -> bool {\n'
        b'    value.as_object().is_some_and(|record| record.len() == 14)\n',
        b'        && value["schema_version"].as_u64() == Some(19)\n',
        b'        && value["schema_version"].as_u64() == Some(18)\n',
        b'        && value["sequence"].as_u64().is_some()\n'
        b'        && FACTORY_COUNT_FIELDS\n',
    ),
    (
        b'    if !valid_factory_counts(&snapshot) {\n'
        b'        return Err(\n',
        b'            "Factory fixture requires exact schema19 and twelve independently empty tables".into(),\n',
        b'            "Factory fixture requires exact schema18 and twelve independently empty tables".into(),\n',
        b'        );\n'
        b'    }\n',
    ),
    (
        b'fn valid_integration_counts(value: &Value) -> bool {\n'
        b'    value.as_object().is_some_and(|record| record.len() == 6)\n',
        b'        && value["schema_version"].as_u64() == Some(19)\n',
        b'        && value["schema_version"].as_u64() == Some(18)\n',
        b'        && value["sequence"].as_u64().is_some()\n'
        b'        && INTEGRATION_COUNT_FIELDS\n',
    ),
    (
        b'    if !valid_integration_counts(&snapshot) {\n'
        b'        return Err(\n',
        b'            "Integration readonly fixture requires exact schema19 and four empty tables".into(),\n',
        b'            "Integration readonly fixture requires exact schema18 and four empty tables".into(),\n',
        b'        );\n'
        b'    }\n',
    ),
    (
        b'    }\n'
        b'    fn execution_counts_fixture(sequence: u64) -> Value {\n',
        b'        json!({"schema_version":19,"run_count":0,"node_count":0,"reservation_count":0,"sequence":sequence})\n',
        b'        json!({"schema_version":18,"run_count":0,"node_count":0,"reservation_count":0,"sequence":sequence})\n',
        b'    }\n'
        b'    fn execution_result_fixture(sequence: u64) -> Value {\n',
    ),
    (
        b'    fn execution_readonly_actual_schema_counts_and_journal_observations_never_write() {\n'
        b'        let (_directory, runtime, mut store) = fresh_execution_fixture();\n',
        b'        let database = runtime.join("baton.sqlite3");\n'
        b'        let original = fs::read(&database).unwrap();\n',
        b'',
        b'        let before = goal_execution_database_snapshot(&runtime).unwrap();\n'
        b'        assert_eq!(before, execution_counts_fixture(0));\n',
    ),
    (
        b'        validate_goal_execution_evidence(&execution_result_fixture(0), &report).unwrap();\n'
        b'        assert_eq!(store.sequence().unwrap(), 0);\n',
        b'        assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'',
        b'        store\n'
        b'            .create(\n',
    ),
    (
        b'            ("schema_version","PRAGMA user_version=16",16),\n'
        b'            ("schema_version","PRAGMA user_version=17",17),\n',
        b'            ("schema_version","PRAGMA user_version=18",18),\n'
        b'            ("schema_version","PRAGMA user_version=20",20),\n',
        b'            ("schema_version","PRAGMA user_version=19",19),\n',
        b'            ("run_count","INSERT INTO goal_runs VALUES(\'count-probe\',\'scope\',\'project\',\'task\',1,\'queued\',\'{}\')",1),\n'
        b'            ("node_count","INSERT INTO goal_run_nodes VALUES(\'count-probe\',1,\'node\',0,\'not_started\',NULL,NULL,\'{}\')",1),\n',
    ),
    (
        b'            let status=Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();\n'
        b'            assert!(status.success());\n',
        b'            let database = runtime.join("baton.sqlite3");\n'
        b'            let original = fs::read(&database).unwrap();\n'
        b'            assert!(goal_execution_database_snapshot(&runtime).is_err());\n'
        b'            assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'            assert!(goal_execution_database_snapshot(&runtime).is_err());\n',
        b'            let mut command = Command::new("python3");\n'
        b'            command\n',
    ),
    (
        b'\n'
        b'    fn graph_counts_fixture(sequence: u64) -> Value {\n',
        b'        json!({"schema_version":19,"capability_count":0,"job_count":0,"candidate_count":0,"validation_count":0,"owner_decision_count":0,"acceptance_count":0,"sequence":sequence})\n',
        b'        json!({"schema_version":18,"capability_count":0,"job_count":0,"candidate_count":0,"validation_count":0,"owner_decision_count":0,"acceptance_count":0,"sequence":sequence})\n',
        b'    }\n'
        b'    fn graph_report_fixture(sequence: u64) -> Value {\n',
    ),
    (
        b'    fn graph_readonly_actual_schema_six_counts_and_journal_never_write() {\n'
        b'        let (_directory, runtime, mut store) = fresh_execution_fixture();\n',
        b'        let database = runtime.join("baton.sqlite3");\n'
        b'        let original = fs::read(&database).unwrap();\n',
        b'',
        b'        let before = goal_graph_database_snapshot(&runtime).unwrap();\n'
        b'        assert_eq!(before, graph_counts_fixture(0));\n',
    ),
    (
        b'        assert_eq!(\n'
        b'            goal_execution_database_snapshot(&runtime).unwrap()["schema_version"],\n',
        b'            19\n',
        b'            18\n',
        b'        );\n'
        b'        let mut report = graph_report_fixture(0);\n',
    ),
    (
        b'        validate_goal_graph_evidence(&graph_result_fixture(0), &report).unwrap();\n'
        b'        assert_eq!(store.sequence().unwrap(), 0);\n',
        b'        assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'',
        b'        store\n'
        b'            .create(\n',
    ),
    (
        b'            ("schema_version", "PRAGMA user_version=16", 16),\n'
        b'            ("schema_version", "PRAGMA user_version=17", 17),\n',
        b'            ("schema_version", "PRAGMA user_version=18", 18),\n'
        b'            ("schema_version", "PRAGMA user_version=20", 20),\n',
        b'            ("schema_version", "PRAGMA user_version=19", 19),\n',
        b'            ("capability_count", "INSERT INTO goal_graph_capabilities(run_id,approval_id,version,document) VALUES(\'probe\',\'approval\',1,\'{}\')", 1),\n'
        b'            ("job_count", "INSERT INTO goal_graph_jobs(id,run_id,node_id,plan_revision,state,attempt_id,workspace_id,document) VALUES(\'probe\',\'run\',\'node\',1,\'queued\',NULL,\'workspace\',\'{}\')", 1),\n',
    ),
    (
        b'            let status = Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();\n'
        b'            assert!(status.success());\n',
        b'            let database = runtime.join("baton.sqlite3");\n'
        b'            let original = fs::read(&database).unwrap();\n'
        b'            assert!(goal_graph_database_snapshot(&runtime).is_err());\n'
        b'            assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'            assert!(goal_graph_database_snapshot(&runtime).is_err());\n',
        b'            let mut command = Command::new("python3");\n'
        b'            command\n',
    ),
    (
        b'    }\n'
        b'    #[test]\n',
        b'    fn graph_final_evidence_requires_independent_schema19_and_exact_core_denial() {\n',
        b'    fn graph_final_evidence_requires_independent_schema18_and_exact_core_denial() {\n',
        b'        let mut report = graph_report_fixture(2);\n'
        b'        let counts = graph_counts_fixture(2);\n',
    ),
    (
        b'        let old_before = goal_database_snapshot(&runtime).unwrap();\n'
        b'        let execution_before = goal_execution_database_snapshot(&runtime).unwrap();\n',
        b'        assert_eq!(before["schema_version"], 19);\n',
        b'        assert_eq!(before["schema_version"], 18);\n',
        b'        let listed = ipc::call(\n'
        b'            &runtime,\n',
    ),
    (
        b'    }\n'
        b'    fn factory_counts_fixture(sequence: u64) -> Value {\n',
        b'        let mut counts = json!({"schema_version":19,"sequence":sequence});\n',
        b'        let mut counts = json!({"schema_version":18,"sequence":sequence});\n',
        b'        for field in FACTORY_COUNT_FIELDS {\n'
        b'            counts[field] = json!(0);\n',
    ),
    (
        b'    }\n'
        b'    #[test]\n',
        b'    fn factory_readonly_requires_exact_schema19_all_twelve_typed_counts_and_ordered_context() {\n',
        b'    fn factory_readonly_requires_exact_schema18_all_twelve_typed_counts_and_ordered_context() {\n',
        b'        let counts = factory_counts_fixture(3);\n'
        b'        assert!(valid_factory_counts(&counts));\n',
    ),
    (
        b'            assert!(!valid_factory_counts(&changed));\n'
        b'        }\n',
        b'        for schema in [11, 12, 13, 14, 15, 16, 17, 18, 20] {\n',
        b'        for schema in [11, 12, 13, 14, 15, 16, 17, 19] {\n',
        b'            let mut changed = counts.clone();\n'
        b'            changed["schema_version"] = json!(schema);\n',
    ),
    (
        b'    }\n'
        b'    #[test]\n',
        b'    fn factory_actual_schema19_query_and_core_count_projection_leave_history_unchanged() {\n',
        b'    fn factory_actual_schema18_query_and_core_count_projection_leave_history_unchanged() {\n',
        b'        let (_directory, runtime, mut store) = fresh_execution_fixture();\n'
        b'        let task = store\n',
    ),
    (
        b'            )\n'
        b'            .unwrap();\n',
        b'        let database = runtime.join("baton.sqlite3");\n'
        b'        let original = fs::read(&database).unwrap();\n',
        b'',
        b'        let before = factory_database_snapshot(&runtime).unwrap();\n'
        b'        assert_eq!(before, factory_counts_fixture(store.sequence().unwrap()));\n',
    ),
    (
        b'        assert_eq!(before["sequence"], old_execution["sequence"]);\n'
        b'        assert_eq!(before["sequence"], old_graph["sequence"]);\n',
        b'        assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'',
        b'    }\n'
        b'    #[test]\n',
    ),
    (
        b'            let status=Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();\n'
        b'            assert!(status.success());\n',
        b'            let database = runtime.join("baton.sqlite3");\n'
        b'            let original = fs::read(&database).unwrap();\n'
        b'            assert!(factory_database_snapshot(&runtime).is_err());\n'
        b'            assert_eq!(fs::read(&database).unwrap(), original);\n',
        b'            assert!(factory_database_snapshot(&runtime).is_err());\n',
        b'            let mut command = Command::new("python3");\n'
        b'            command\n',
    ),
    (
        b'            "PRAGMA user_version=16",\n'
        b'            "PRAGMA user_version=17",\n',
        b'            "PRAGMA user_version=18",\n'
        b'            "PRAGMA user_version=20",\n',
        b'            "PRAGMA user_version=19",\n',
        b'            "DROP TABLE factory_scopes",\n'
        b'            "DROP TABLE factory_budget_reservations",\n',
    ),
    (
        b'            let status=Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();\n'
        b'            assert!(status.success());\n',
        b'            let database = runtime.join("baton.sqlite3");\n'
        b'            let original = fs::read(&database).unwrap();\n'
        b'            assert!(factory_database_snapshot(&runtime).is_err());\n'
        b'            assert_eq!(fs::read(&database).unwrap(), original);\n'
        b'            drop(store);\n'
        b'        }\n'
        b'    }\n'
        b'    fn integration_counts_fixture(sequence: u64) -> Value {\n'
        b'        json!({"schema_version":19,"sequence":sequence,"policies":0,"runs":0,"attempts":0,"ref_fences":0})\n',
        b'            assert!(factory_database_snapshot(&runtime).is_err());\n'
        b'            drop(store);\n'
        b'        }\n'
        b'    }\n'
        b'    fn integration_counts_fixture(sequence: u64) -> Value {\n'
        b'        json!({"schema_version":18,"sequence":sequence,"policies":0,"runs":0,"attempts":0,"ref_fences":0})\n',
        b'    }\n'
        b'    fn integration_report_fixture(sequence: u64) -> Value {\n',
    ),
    (
        b'    }\n'
        b'    #[test]\n',
        b'    fn current_readonly_counts_require_schema19_as_a_json_integer() {\n'
        b'        let guards: [(Value, fn(&Value) -> bool); 4] = [\n'
        b'            (execution_counts_fixture(0), valid_goal_execution_counts),\n'
        b'            (graph_counts_fixture(0), valid_goal_graph_counts),\n'
        b'            (factory_counts_fixture(0), valid_factory_counts),\n'
        b'            (integration_counts_fixture(0), valid_integration_counts),\n'
        b'        ];\n'
        b'        for (counts, valid) in guards {\n'
        b'            assert_eq!(counts["schema_version"], 19);\n'
        b'            assert!(valid(&counts));\n'
        b'            for version in [json!(18), json!(20), json!("19"), json!(19.0)] {\n'
        b'                let mut invalid = counts.clone();\n'
        b'                invalid["schema_version"] = version;\n'
        b'                assert!(!valid(&invalid), "{invalid}");\n'
        b'            }\n'
        b'            let mut missing = counts;\n'
        b'            missing.as_object_mut().unwrap().remove("schema_version");\n'
        b'            assert!(!valid(&missing), "{missing}");\n'
        b'        }\n'
        b'    }\n'
        b'    #[test]\n'
        b'    fn integration_readonly_has_independent_exact_schema19_and_four_typed_zero_counts() {\n',
        b'    fn integration_readonly_has_independent_exact_schema18_and_four_typed_zero_counts() {\n',
        b'        let counts = integration_counts_fixture(7);\n'
        b'        assert!(valid_integration_counts(&counts));\n',
    ),
    (
        b'            assert!(!valid_integration_counts(&missing), "{field}");\n'
        b'        }\n',
        b'        for schema in [0, 12, 13, 14, 15, 16, 17, 18, 20] {\n',
        b'        for schema in [0, 12, 13, 14, 15, 16, 17, 19] {\n',
        b'            let mut changed = counts.clone();\n'
        b'            changed["schema_version"] = json!(schema);\n',
    ),
    (
        b'                serde_json::from_slice(&bounded_goal_query(query).unwrap()).unwrap();\n'
        b'            assert_eq!(observed[field], 1);\n',
        b'            assert_eq!(observed["schema_version"], 19);\n',
        b'            assert_eq!(observed["schema_version"], 18);\n',
        b'            drop(store);\n'
        b'        }\n',
    ),
    (
        b'            "PRAGMA user_version=16",\n'
        b'            "PRAGMA user_version=17",\n',
        b'            "PRAGMA user_version=18",\n'
        b'            "PRAGMA user_version=20",\n',
        b'            "PRAGMA user_version=19",\n',
        b'            "DROP TABLE integration_policies",\n'
        b'            "DROP TABLE integrations",\n',
    ),
)


def restore_native_smoke_before_schema19(path, raw):
    """Peel this layer from the exact raw Rust source."""
    if type(path) is not str or path != NATIVE_SMOKE_PATH or type(raw) is not bytes:
        raise ValueError("Expected raw bytes for the canonical native smoke path")
    from tests.infrastructure.mantine_picker_source_fixture import restore_mantine_picker_source
    raw = restore_mantine_picker_source(path, raw, error=ValueError)
    if hashlib.sha256(raw).hexdigest() != CURRENT_SHA256:
        raise ValueError("Current native smoke source has changed")

    for leading, current, parent, trailing in SOURCE_EDITS:
        anchor = leading + current + trailing
        if raw.count(anchor) != 1:
            raise ValueError("Expected exactly one complete native smoke source span")
        raw = raw.replace(anchor, leading + parent + trailing, 1)

    if hashlib.sha256(raw).hexdigest() != PARENT_SHA256:
        raise ValueError("Native smoke inverse did not restore the pinned parent")
    return raw
