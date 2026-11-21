//! Private handle/dispatch tests with real SQLite and actual A migration.
//! Binding a fixture listener is not running an actor or exercising a UDS wire.
use super::*;
use crate::ipc::{Daemon, API_VERSION};
use rusqlite::types::Value as SqlValue;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

const PAYLOAD: &str = include_str!("../tests/fixtures/host_registration/payload-v1.json");
const ABSENT_ID: &str = "11111111-1111-4111-8111-111111111111";
struct Fixture {
    daemon: Daemon,
    _directory: TempDir,
    next_id: u32,
}
impl Fixture {
    fn product19() -> Self {
        let root = std::env::var_os("BATON_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = tempfile::Builder::new()
            .prefix("host-dispatch-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(root)
            .unwrap();
        let runtime = directory.path().canonicalize().unwrap();
        let daemon = Daemon::bind(&runtime).unwrap();
        assert_eq!(
            daemon
                .store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            19
        );
        Self {
            daemon,
            _directory: directory,
            next_id: 0,
        }
    }
    // Explicit structural18 connection in a private handle fixture. The listener
    // belongs to an ordinary19 bind; no actor/wire or old-program proof here.
    fn structural18() -> Self {
        let mut fixture = Self::product19();
        fixture.daemon.store.conn = crate::host_registry_migration::schema18_reference().unwrap();
        fixture
    }
    fn migrated19() -> Self {
        let mut f = Self::structural18();
        // Lawful opaque shadow exists BEFORE the actual migration. No copied
        // Host DDL/version stamp or ordinary Store19 startup claim.
        f.daemon.store.conn.execute_batch("CREATE TABLE pragma_foreign_key_list(raw BLOB); INSERT INTO pragma_foreign_key_list(rowid,raw) VALUES(71,x'00ff80');").unwrap();
        crate::host_registry_migration::migrate(&mut f.daemon.store.conn).unwrap();
        Self::assert_version(&f, 19);
        f
    }
    fn assert_version(&self, version: i64) {
        assert_eq!(
            self.daemon
                .store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            version
        );
    }
    fn packet(&self, method: &str, params: Value, key: Option<&str>) -> Value {
        let mut packet = json!({"api_version":API_VERSION,"request_id":"host-dispatch","token":self.daemon.token,"method":method,"params":params});
        if let Some(key) = key {
            packet["idempotency_key"] = json!(key);
        }
        packet
    }
    fn call(&mut self, method: &str, params: Value, key: Option<&str>) -> Value {
        self.next_id += 1;
        let mut packet = self.packet(method, params, key);
        packet["request_id"] = json!(format!("host-dispatch-{}", self.next_id));
        self.handle(packet)
    }
    fn handle(&mut self, packet: Value) -> Value {
        self.daemon.handle(&serde_json::to_string(&packet).unwrap())
    }
}
fn metadata() -> Value {
    serde_json::from_str(PAYLOAD).unwrap()
}
fn result(reply: Value) -> Value {
    assert!(
        reply.get("error").is_none(),
        "unexpected dispatch error: {reply}"
    );
    reply.get("result").expect("result required").clone()
}
fn code(reply: &Value) -> &str {
    reply["error"]["code"]
        .as_str()
        .expect("error code required")
}
fn host_counts(f: &Fixture) -> (i64, i64, i64) {
    let db = &f.daemon.store.conn;
    (
        db.query_row("SELECT count(*) FROM host_registrations", [], |r| r.get(0))
            .unwrap(),
        db.query_row(
            "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='host.registered'",
            [],
            |r| r.get(0),
        )
        .unwrap(),
        db.query_row(
            "SELECT count(*) FROM idempotency WHERE scope='hosts.register:v1'",
            [],
            |r| r.get(0),
        )
        .unwrap(),
    )
}
type AuthorityImage = Vec<(String, Vec<Vec<SqlValue>>)>;
fn authority_image(f: &Fixture) -> AuthorityImage {
    let db = &f.daemon.store.conn;
    let names:Vec<String>=db.prepare("SELECT name FROM main.sqlite_schema WHERE type='table' AND name!='host_registrations' ORDER BY name").unwrap()
        .query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    names
        .into_iter()
        .map(|name| {
            let quoted = name.replace('"', "\"\"");
            let filter = match name.as_str() {
                "events" => " WHERE json_extract(document,'$.kind')!='host.registered'",
                "idempotency" => " WHERE scope!='hosts.register:v1'",
                "sqlite_sequence" => " WHERE name!='events'",
                _ => "",
            };
            let mut statement = db
                .prepare(&format!("SELECT * FROM \"{quoted}\"{filter}"))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<SqlValue>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}
#[test]
fn owner_api_request_id_guards_precede_host_params_and_schema() {
    let mut f = Fixture::structural18();
    let before = f.daemon.store.conn.total_changes();
    let mut bad = f.packet("hosts.register", json!({"not_metadata":true}), Some("key"));
    bad["token"] = json!("wrong-owner");
    bad["api_version"] = json!(999);
    bad["request_id"] = json!("");
    assert_eq!(code(&f.handle(bad.clone())), "AUTH_REQUIRED");
    bad["token"] = json!(f.daemon.token);
    assert_eq!(code(&f.handle(bad.clone())), "VERSION_UNSUPPORTED");
    bad["api_version"] = json!(API_VERSION);
    assert_eq!(code(&f.handle(bad.clone())), "INVALID_REQUEST");
    bad["request_id"] = json!("x".repeat(129));
    assert_eq!(code(&f.handle(bad.clone())), "INVALID_REQUEST");
    bad["request_id"] = json!("valid");
    assert_eq!(code(&f.handle(bad)), "INVALID_REQUEST");
    assert_eq!(
        code(&f.call("hosts.register", metadata(), Some("key"))),
        "VERSION_UNSUPPORTED"
    );
    assert_eq!(f.daemon.store.conn.total_changes(), before);
    f.assert_version(18);
}
#[test]
fn actual_migrated_dispatch_register_get_list_lookup_are_metadata_only() {
    let mut f = Fixture::migrated19();
    f.daemon
        .store
        .create("local", "existing task remains untouched", "existing-task")
        .unwrap();
    let before = authority_image(&f);
    let receipt = result(f.call("hosts.register", metadata(), Some("create:1")));
    assert_eq!(receipt["receipt_version"], 1);
    assert_eq!(receipt["revision"], 1);
    assert_eq!(receipt["registration"], metadata());
    assert_ne!(
        receipt["host_id"],
        receipt["registration"]["endpoint"]["expected_host_identity"]
    );
    let view = result(f.call("hosts.get", json!({"host_id":receipt["host_id"]}), None));
    assert_eq!(
        view["record"]["created_sequence"],
        receipt["created_sequence"]
    );
    assert_eq!(view["registration"], "registered");
    assert_eq!(view["record"]["creation_facts"]["execution_enabled"], false);
    assert_eq!(
        view["record"]["creation_facts"]["remote_connection"],
        "not_contacted"
    );
    let page = result(f.call("hosts.list", json!({"limit":25}), None));
    assert_eq!(page["hosts"].as_array().unwrap().len(), 1);
    let lookup = result(f.call("hosts.find_registration", json!({"key":"create:1"}), None));
    assert_eq!(
        lookup,
        json!({"lookup_version":1,"key":"create:1","receipt":receipt})
    );
    let absent = result(f.call(
        "hosts.find_registration",
        json!({"key":"not-committed"}),
        None,
    ));
    assert_eq!(
        absent,
        json!({"lookup_version":1,"key":"not-committed","receipt":null})
    );
    assert_eq!(host_counts(&f), (1, 1, 1));
    assert_eq!(authority_image(&f), before);
    let opaque: (i64, Vec<u8>) = f
        .daemon
        .store
        .conn
        .query_row("SELECT rowid,raw FROM pragma_foreign_key_list", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(opaque, (71, vec![0, 255, 128]));
}
#[test]
fn original_key_replay_and_lookup_do_not_recreate_or_use_global_revision_as_host_revision() {
    let mut f = Fixture::migrated19();
    f.daemon
        .store
        .create("local", "first event", "task-before")
        .unwrap();
    let first = f.call("hosts.register", metadata(), Some("original"));
    let receipt = result(first.clone());
    assert_eq!(receipt["revision"], 1);
    assert_eq!(receipt["created_sequence"], 2);
    assert_eq!(first["revision"], 2);
    f.daemon
        .store
        .create("local", "later event", "task-after")
        .unwrap();
    let replay = f.call("hosts.register", metadata(), Some("original"));
    assert_ne!(first["request_id"], replay["request_id"]);
    assert_eq!(result(replay.clone()), receipt);
    assert_eq!(replay["revision"], 3);
    assert_eq!(
        result(f.call("hosts.find_registration", json!({"key":"original"}), None))["receipt"],
        receipt
    );
    let mut changed = metadata();
    changed["transport_identity_ref"] = json!("identity:changed");
    assert_eq!(
        code(&f.call("hosts.register", changed, Some("original"))),
        "CONFIG_CONFLICT"
    );
    assert_eq!(host_counts(&f), (1, 1, 1));
}
#[test]
fn all_read_envelopes_require_none_mutation_fields_and_accept_existing_null_semantics() {
    let mut f = Fixture::migrated19();
    let receipt = result(f.call("hosts.register", metadata(), Some("one")));
    let queries = [
        ("hosts.find_registration", json!({"key":"one"})),
        ("hosts.get", json!({"host_id":receipt["host_id"]})),
        ("hosts.list", json!({"limit":1})),
    ];
    let before = authority_image(&f);
    let counts = host_counts(&f);
    for (method, params) in queries {
        let baseline = result(f.call(method, params.clone(), None));
        let mut nulls = f.packet(method, params.clone(), None);
        nulls["expected_revision"] = Value::Null;
        nulls["idempotency_key"] = Value::Null;
        assert_eq!(result(f.handle(nulls)), baseline);
        for (field, value) in [
            ("expected_revision", json!(0)),
            ("expected_revision", json!(1)),
            ("idempotency_key", json!("key")),
            ("idempotency_key", json!("")),
        ] {
            let mut bad = f.packet(method, params.clone(), None);
            bad[field] = value;
            assert_eq!(code(&f.handle(bad)), "INVALID_REQUEST");
        }
    }
    assert_eq!(authority_image(&f), before);
    assert_eq!(host_counts(&f), counts);
}
#[test]
fn create_requires_original_key_no_revision_and_strict_metadata_shape_types_version() {
    let mut f = Fixture::migrated19();
    let before = authority_image(&f);
    for key in [None, Some(""), Some("bad key"), Some("非ASCII")] {
        assert_eq!(
            code(&f.call("hosts.register", metadata(), key)),
            "INVALID_REQUEST"
        );
    }
    for revision in [json!(0), json!(1)] {
        let mut p = f.packet("hosts.register", metadata(), Some("key"));
        p["expected_revision"] = revision;
        assert_eq!(code(&f.handle(p)), "INVALID_REQUEST");
    }
    let mut bads = vec![Value::Null, json!([]), json!("metadata"), json!({})];
    for (key, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!(1.0)),
        ("schema_version", json!(true)),
        ("schema_version", json!("1")),
        ("grant", json!({"enabled":true})),
        ("connected", json!(true)),
        ("directory_mappings", json!(null)),
    ] {
        let mut bad = metadata();
        bad[key] = value;
        bads.push(bad);
    }
    let mut nested = metadata();
    nested["endpoint"]["command"] = json!("sh");
    bads.push(nested);
    for bad in bads {
        assert_eq!(
            code(&f.call("hosts.register", bad, Some("key"))),
            "INVALID_REQUEST"
        );
    }
    assert_eq!(host_counts(&f), (0, 0, 0));
    assert_eq!(authority_image(&f), before);
}
#[test]
fn read_queries_are_closed_typed_and_use_registry_uuid_not_protocol_identity() {
    let mut f = Fixture::migrated19();
    let before = authority_image(&f);
    let invalids = [
        (
            "hosts.find_registration",
            json!({"key":"missing","retry":true}),
        ),
        ("hosts.find_registration", json!({"key":null})),
        ("hosts.get", json!({"host_id":"test_host"})),
        ("hosts.get", json!({"host_id":ABSENT_ID,"grant":true})),
        ("hosts.list", json!({"limit":0})),
        ("hosts.list", json!({"limit":26})),
        ("hosts.list", json!({"limit":1.0})),
        ("hosts.list", json!({"limit":true})),
        ("hosts.list", json!({"limit":1,"after_created_sequence":0})),
        (
            "hosts.list",
            json!({"limit":1,"after_created_sequence":9_007_199_254_740_992u64}),
        ),
        ("hosts.list", json!({"limit":1,"unexpected":false})),
    ];
    for (method, params) in invalids {
        assert_eq!(code(&f.call(method, params, None)), "INVALID_REQUEST");
    }
    assert_eq!(
        code(&f.call("hosts.get", json!({"host_id":ABSENT_ID}), None)),
        "NOT_FOUND"
    );
    assert_eq!(host_counts(&f), (0, 0, 0));
    assert_eq!(authority_image(&f), before);
}
#[test]
fn bounded_params_enforce_serialized_bytes_before_typed_decode() {
    let f = Fixture::structural18();
    let mut r: Request = serde_json::from_value(f.packet(
        "hosts.get",
        Value::String("a".repeat(MAX_HOST_PARAMS - 2)),
        None,
    ))
    .unwrap();
    assert_eq!(
        bounded_json(&r.params, MAX_HOST_PARAMS).unwrap().len(),
        MAX_HOST_PARAMS
    );
    r.params = Value::String("a".repeat(MAX_HOST_PARAMS - 1));
    assert!(bounded_json(&r.params, MAX_HOST_PARAMS).is_err());
    let mut f = Fixture::migrated19();
    let before = host_counts(&f);
    let large = json!({"key":"a".repeat(MAX_HOST_PARAMS)});
    assert_eq!(
        code(&f.call("hosts.find_registration", large, None)),
        "INVALID_REQUEST"
    );
    let mut large = metadata();
    large["name"] = json!("a".repeat(MAX_HOST_PARAMS));
    assert_eq!(
        code(&f.call("hosts.register", large, Some("key"))),
        "INVALID_REQUEST"
    );
    assert_eq!(host_counts(&f), before);
}
#[test]
fn complete_response_includes_lf_request_id_escaping_and_maximum_revision_digits() {
    let f = Fixture::structural18();
    let mut packet = f.packet("hosts.list", json!({"limit":1}), None);
    packet["request_id"] = json!("\n".repeat(128));
    let request: Request = serde_json::from_value(packet).unwrap();
    let overhead = serde_json::to_vec(
        &json!({"request_id":request.request_id,"result":"","revision":u64::MAX,"warnings":[]}),
    )
    .unwrap()
    .len();
    let exact = "a".repeat(MAX_HOST_RESPONSE - overhead - RESPONSE_LF_BYTES);
    let value = response(&request, &exact, false).unwrap();
    let mut framed = serde_json::to_vec(
        &json!({"request_id":request.request_id,"result":value,"revision":u64::MAX,"warnings":[]}),
    )
    .unwrap();
    framed.push(b'\n');
    assert_eq!(framed.len(), MAX_HOST_RESPONSE);
    let over = exact + "a";
    assert_eq!(
        response(&request, &over, false).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    let write_error = response(&request, &over, true).unwrap_err();
    assert_eq!(write_error.code, "OUTCOME_UNKNOWN");
    assert_eq!(write_error.side_effect_state, "unknown");
}
#[test]
fn actual_paginated_dispatch_responses_are_bounded_and_progress_without_duplicate_ids() {
    let mut f = Fixture::migrated19();
    for n in 0..29 {
        result(f.call("hosts.register", metadata(), Some(&format!("page:{n}"))));
    }
    let mut cursor = Value::Null;
    let mut ids = std::collections::BTreeSet::new();
    loop {
        let mut packet = f.packet(
            "hosts.list",
            json!({"limit":25,"after_created_sequence":cursor}),
            None,
        );
        packet["request_id"] = json!("\"\n".repeat(64));
        let reply = f.handle(packet);
        assert!(serde_json::to_vec(&reply).unwrap().len() + RESPONSE_LF_BYTES <= MAX_HOST_RESPONSE);
        let page = result(reply);
        let rows = page["hosts"].as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(rows.len() <= 25);
        for row in rows {
            assert!(ids.insert(row["record"]["id"].as_str().unwrap().to_owned()));
        }
        cursor = page["next_created_sequence"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(ids.len(), 29);
    assert_eq!(host_counts(&f), (29, 29, 29));
}
#[test]
fn unsupported_structural18_and_future20_return_errors_without_creating_schema_or_records() {
    let mut f = Fixture::structural18();
    for version in [18, 20] {
        if version == 20 {
            f.daemon
                .store
                .conn
                .pragma_update(None, "user_version", 20)
                .unwrap();
        }
        let before = f.daemon.store.conn.total_changes();
        for (method, params, key) in [
            ("hosts.register", metadata(), Some("key")),
            ("hosts.find_registration", json!({"key":"key"}), None),
            ("hosts.get", json!({"host_id":ABSENT_ID}), None),
            ("hosts.list", json!({"limit":1}), None),
        ] {
            let reply = f.call(method, params, key);
            assert_eq!(code(&reply), "VERSION_UNSUPPORTED");
            assert!(reply.get("result").is_none());
            assert!(
                serde_json::to_vec(&reply).unwrap().len() + RESPONSE_LF_BYTES < MAX_HOST_RESPONSE
            );
        }
        assert_eq!(f.daemon.store.conn.total_changes(), before);
        f.assert_version(version);
        assert_eq!(
            f.daemon
                .store
                .conn
                .query_row(
                    "SELECT count(*) FROM main.sqlite_schema WHERE name='host_registrations'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
#[test]
fn store_integrity_errors_and_side_effect_uncertainty_are_never_successful_null() {
    let mut f = Fixture::migrated19();
    result(f.call("hosts.register", metadata(), Some("original")));
    f.daemon
        .store
        .conn
        .execute(
            "DELETE FROM idempotency WHERE scope='hosts.register:v1'",
            [],
        )
        .unwrap();
    let native = f
        .daemon
        .store
        .find_host_registration("original")
        .unwrap_err();
    let reply = f.call("hosts.find_registration", json!({"key":"original"}), None);
    assert_eq!(reply["error"], serde_json::to_value(&native).unwrap());
    assert_eq!(code(&reply), "OUTCOME_UNKNOWN");
    assert_eq!(reply["error"]["side_effect_state"], "unknown");
    assert!(reply.get("result").is_none());
    assert_eq!(
        code(&f.call("hosts.register", metadata(), Some("new-key"))),
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(
        f.daemon
            .store
            .conn
            .query_row("SELECT count(*) FROM host_registrations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn scoped_or_unknown_methods_cannot_gain_a_generic_host_entry() {
    let mut f = Fixture::migrated19();
    let before = host_counts(&f);
    let scoped = json!({"api_version":1,"request_id":"scoped","handle_id":ABSENT_ID,"scoped_token":"not-owner","method":"hosts.register","params":metadata(),"idempotency_key":"key"});
    let reply = f.handle(scoped);
    assert!(reply.get("error").is_some());
    assert!(reply.get("result").is_none());
    let mut packet = f.packet("hosts.register", metadata(), Some("key"));
    packet["token"] = json!("not-owner");
    assert_eq!(code(&f.handle(packet)), "AUTH_REQUIRED");
    for method in [
        "hosts.probe",
        "hosts.retry",
        "hosts.delete",
        "hosts.run",
        "hosts.register_extra",
    ] {
        let reply = f.call(method, json!({}), None);
        assert!(reply.get("error").is_some());
        assert!(reply.get("result").is_none());
    }
    assert_eq!(host_counts(&f), before);
}

#[test]
fn normal_store19_bound_dispatch_registers_without_private_schema_override() {
    let mut fixture = Fixture::product19();
    let reply = fixture.call("hosts.register", metadata(), Some("normal19"));
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(reply.get("result").is_some());
    let path = fixture._directory.path().join("baton.sqlite3");
    // Actual lifecycle/reopen is exercised by the public Store tests; this is
    // the normal19 bound-handle path, not a running actor/UDS test.
    assert!(path.is_file());
}
