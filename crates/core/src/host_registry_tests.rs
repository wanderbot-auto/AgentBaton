//! Real SQLite tests using an EXPLICIT PRIVATE19 fixture, not Store19 migration.
use super::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use tempfile::TempDir;

const DDL: &str = include_str!("../tests/fixtures/host_registration/ddl.sql");
const PAYLOAD: &str = include_str!("../tests/fixtures/host_registration/payload-v1.json");
const RECORD: &str = include_str!("../tests/fixtures/host_registration/record-v1.json");
const RECEIPT: &str = include_str!("../tests/fixtures/host_registration/receipt-v1.json");
const EVENT_PAYLOAD: &str =
    include_str!("../tests/fixtures/host_registration/event-payload-v1.json");
struct Fixture {
    _dir: TempDir,
    path: PathBuf,
    store: Store,
}
fn registration() -> HostRegistration {
    serde_json::from_str(PAYLOAD).unwrap()
}
fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-host.sqlite3");
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        18
    );
    store.conn.execute_batch(DDL).unwrap();
    store.conn.pragma_update(None, "user_version", 19).unwrap();
    Fixture {
        _dir: dir,
        path,
        store,
    }
}
// Deliberate test-only connection reconstruction. Product Store::open19 is NOT
// activated or bypassed. All reads/writes still use actual Core methods/SQLite.
fn reopen_explicit_fixture(path: &Path) -> Store {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    Store {
        conn,
        data_input: None,
        graph_input: None,
        factory_read_control: None,
        graph_capture_clocks: Default::default(),
        factory_recovery_queue_epochs: Default::default(),
    }
}
fn counts(store: &Store) -> (i64, i64, i64) {
    let hosts = store
        .conn
        .query_row("SELECT count(*) FROM host_registrations", [], |r| r.get(0))
        .unwrap();
    let events = store
        .conn
        .query_row(
            "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='host.registered'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let receipts = store
        .conn
        .query_row(
            "SELECT count(*) FROM idempotency WHERE scope=?1",
            [SCOPE],
            |r| r.get(0),
        )
        .unwrap();
    (hosts, events, receipts)
}
fn raw_record(store: &Store, id: &str) -> String {
    store
        .conn
        .query_row(
            "SELECT document FROM host_registrations WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}
fn change_record(store: &Store, id: &str, change: impl FnOnce(&mut HostRegistrationRecord)) {
    let mut record: HostRegistrationRecord = serde_json::from_str(&raw_record(store, id)).unwrap();
    change(&mut record);
    store
        .conn
        .execute(
            "UPDATE host_registrations SET document=?1 WHERE id=?2",
            params![encode(&record).unwrap(), id],
        )
        .unwrap();
}
fn change_receipt(store: &Store, key: &str, change: impl FnOnce(&mut HostCreationReceipt)) {
    let raw: String = store
        .conn
        .query_row(
            "SELECT result FROM idempotency WHERE scope=?1 AND key=?2",
            params![SCOPE, key],
            |r| r.get(0),
        )
        .unwrap();
    let mut receipt: HostCreationReceipt = serde_json::from_str(&raw).unwrap();
    change(&mut receipt);
    store
        .conn
        .execute(
            "UPDATE idempotency SET result=?1 WHERE scope=?2 AND key=?3",
            params![encode(&receipt).unwrap(), SCOPE, key],
        )
        .unwrap();
}
fn change_event(store: &Store, sequence: u64, change: impl FnOnce(&mut HostEvent)) {
    let raw: String = store
        .conn
        .query_row(
            "SELECT document FROM events WHERE sequence=?1",
            [sequence as i64],
            |r| r.get(0),
        )
        .unwrap();
    let mut event: HostEvent = serde_json::from_str(&raw).unwrap();
    change(&mut event);
    store
        .conn
        .execute(
            "UPDATE events SET document=?1 WHERE sequence=?2",
            params![encode(&event).unwrap(), sequence as i64],
        )
        .unwrap();
}
fn assert_refuses_corruption(store: &mut Store, id: &str) {
    let before = counts(store);
    assert!(store.find_host_registration("original").is_err());
    assert!(store.get_host_registration(id).is_err());
    assert!(store
        .list_host_registrations(&HostListRequest {
            limit: 25,
            after_created_sequence: None
        })
        .is_err());
    assert!(store.register_host(&registration(), "replacement").is_err());
    assert_eq!(counts(store), before, "must not repair or recreate");
}
#[test]
fn exact_golden_bytes_are_read_through_real_sqlite() {
    let mut f = fixture();
    assert_eq!(DDL, format!("{};\n{};\n", TABLE_SQL, INDEX_SQL));
    let record: HostRegistrationRecord = decode_canonical(RECORD, MAX_DOCUMENT).unwrap();
    let receipt: HostCreationReceipt = decode_canonical(RECEIPT, MAX_DOCUMENT).unwrap();
    record.validate().unwrap();
    receipt.validate().unwrap();
    assert_eq!(record.receipt(), receipt);
    assert_eq!(
        canonical_registration(&record.registration).unwrap(),
        PAYLOAD
    );
    assert_eq!(
        digest(PAYLOAD),
        "4a7cba703c7db8252649273ddfeca6764efe51526ee3fe4a9eeb625eb891c3d5"
    );
    let event_payload: HostEventPayload = decode_canonical(EVENT_PAYLOAD, MAX_EVENT).unwrap();
    let event = HostEvent {
        sequence: 0,
        event_id: "22222222-2222-4222-8222-222222222222".into(),
        aggregate_id: record.id.clone(),
        revision: 1,
        kind: "host.registered".into(),
        occurred_at: record.created_at.clone(),
        payload: event_payload,
        source: "user_action".into(),
    };
    f.store
        .conn
        .execute(
            "INSERT INTO events(sequence,document) VALUES(?1,?2)",
            params![41, encode(&event).unwrap()],
        )
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT INTO host_registrations VALUES(?1,?2,?2,1,41,?3)",
            params![record.id, record.registration.name, RECORD],
        )
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![SCOPE, record.creation_key, PAYLOAD, RECEIPT],
        )
        .unwrap();
    assert_eq!(
        f.store
            .find_host_registration("synthetic-create:v1")
            .unwrap(),
        Some(receipt)
    );
    assert_eq!(
        f.store.get_host_registration(&record.id).unwrap().record,
        record
    );
}
#[test]
fn actual_create_binds_event_receipt_and_metadata_without_authority() {
    let mut f = fixture();
    let task = f
        .store
        .create("synthetic-project", "preserve existing task", "task-key")
        .unwrap();
    let task_before = f.store.get(&task.id).unwrap();
    let input = registration();
    let receipt = f.store.register_host(&input, "original").unwrap();
    assert!(uuid_valid(&receipt.host_id));
    assert_ne!(receipt.host_id, input.endpoint.expected_host_identity);
    assert_eq!(receipt.registration, input);
    assert_eq!(f.store.register_host(&input, "original").unwrap(), receipt);
    assert_eq!(
        f.store.find_host_registration("original").unwrap(),
        Some(receipt.clone())
    );
    assert!(f.store.find_host_registration("absent").unwrap().is_none());
    assert_eq!(counts(&f.store), (1, 1, 1));
    assert_eq!(
        encode(&f.store.get(&task.id).unwrap()).unwrap(),
        encode(&task_before).unwrap()
    );
    let view = f.store.get_host_registration(&receipt.host_id).unwrap();
    assert_eq!(view.record.creation_facts, HostCreationFacts::unverified());
    let stored: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM events WHERE sequence=?1",
            [receipt.created_sequence as i64],
            |r| r.get(0),
        )
        .unwrap();
    let ordinary: crate::storage::Event = serde_json::from_str(&stored).unwrap();
    assert_eq!(ordinary.sequence, 0);
    assert_eq!(ordinary.aggregate_id, receipt.host_id);
    assert_eq!(ordinary.kind, "host.registered");
    let fk_violations: i64 = f
        .store
        .conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(fk_violations, 0);
}
#[test]
fn replay_precedes_capacity_and_repeated_names_are_separate_ids() {
    let mut f = fixture();
    let input = registration();
    let original = f.store.register_host(&input, "original").unwrap();
    for n in 1..MAX_HOSTS {
        let r = f.store.register_host(&input, &format!("fill:{n}")).unwrap();
        assert_ne!(r.host_id, original.host_id);
    }
    let before = counts(&f.store);
    assert_eq!(f.store.register_host(&input, "original").unwrap(), original);
    let mut changed = input.clone();
    changed.endpoint.expected_host_identity = "changed_host".into();
    assert_eq!(
        f.store
            .register_host(&changed, "original")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        f.store.register_host(&input, "new").unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.store.find_host_registration("original").unwrap(),
        Some(original)
    );
    assert_eq!(before, (128, 128, 128));
    assert_eq!(counts(&f.store), before);
}
#[test]
fn lost_return_value_and_actual_connection_reopen_reconcile_original_key() {
    let mut f = fixture();
    let expected = f.store.register_host(&registration(), "original").unwrap();
    // The committed receipt is deliberately not delivered to a caller. This is
    // Core-level response-loss simulation, not IPC/network acceptance evidence.
    let before = raw_record(&f.store, &expected.host_id);
    drop(f.store);
    assert_eq!(
        Store::open(&f.path).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    let mut reopened = reopen_explicit_fixture(&f.path);
    assert_eq!(
        reopened.find_host_registration("original").unwrap(),
        Some(expected.clone())
    );
    assert_eq!(
        reopened.register_host(&registration(), "original").unwrap(),
        expected
    );
    assert_eq!(raw_record(&reopened, &expected.host_id), before);
    assert_eq!(counts(&reopened), (1, 1, 1));
}
#[test]
fn simultaneous_same_key_has_one_committed_result() {
    let f = fixture();
    let barrier = Arc::new(Barrier::new(2));
    let children: Vec<_> = (0..2)
        .map(|_| {
            let p = f.path.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                let mut store = reopen_explicit_fixture(&p);
                b.wait();
                store.register_host(&registration(), "same-key").unwrap()
            })
        })
        .collect();
    let results: Vec<_> = children.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results[0], results[1]);
    assert_eq!(counts(&f.store), (1, 1, 1));
}
#[test]
fn simultaneous_last_capacity_slot_has_one_winner() {
    let mut f = fixture();
    for n in 0..127 {
        f.store
            .register_host(&registration(), &format!("fill:{n}"))
            .unwrap();
    }
    let barrier = Arc::new(Barrier::new(2));
    let children: Vec<_> = (0..2)
        .map(|n| {
            let p = f.path.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                let mut store = reopen_explicit_fixture(&p);
                b.wait();
                store.register_host(&registration(), &format!("race:{n}"))
            })
        })
        .collect();
    let results: Vec<_> = children.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .next()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(counts(&f.store), (128, 128, 128));
}
#[test]
fn identity_pin_configuration_and_name_changes_are_not_upserts() {
    let mut f = fixture();
    let input = registration();
    f.store.register_host(&input, "original").unwrap();
    let mut variants = Vec::new();
    let mut value = input.clone();
    value.name = "Another name".into();
    variants.push(value);
    let mut value = input.clone();
    value.endpoint.expected_host_identity = "other_host".into();
    variants.push(value);
    let mut value = input.clone();
    value.known_hosts.file_sha256 = "b".repeat(64);
    variants.push(value);
    let mut value = input.clone();
    value.transport_identity_ref = "identity:other".into();
    variants.push(value);
    let mut value = input.clone();
    value.configuration.reference = "config:other".into();
    variants.push(value);
    for changed in variants {
        assert_eq!(
            f.store
                .register_host(&changed, "original")
                .unwrap_err()
                .code,
            "CONFIG_CONFLICT"
        );
    }
    for key in ["", "contains space", "contains\ncontrol", "非ASCII"] {
        assert_eq!(
            f.store.register_host(&input, key).unwrap_err().code,
            "INVALID_REQUEST"
        );
    }
    let mut invalid = input;
    invalid.endpoint.expected_host_identity = "local".into();
    assert_eq!(
        f.store.register_host(&invalid, "new").unwrap_err().code,
        "INVALID_REQUEST"
    );
    assert_eq!(counts(&f.store), (1, 1, 1));
}
#[test]
fn absence_or_inexact_schema_never_migrates_or_creates() {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(&dir.path().join("schema18.sqlite3")).unwrap();
    assert_eq!(
        store
            .register_host(&registration(), "key")
            .unwrap_err()
            .code,
        "VERSION_UNSUPPORTED"
    );
    assert!(store.find_host_registration("key").is_err());
    assert_eq!(
        store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        18
    );
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='host_registrations'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    for mutation in [
        "DROP TABLE host_registrations",
        "DROP INDEX host_registrations_name_id",
        "CREATE INDEX unexpected ON host_registrations(name)",
        "PRAGMA user_version=20",
        "PRAGMA foreign_keys=OFF",
    ] {
        let mut f = fixture();
        f.store.conn.execute_batch(mutation).unwrap();
        assert!(
            f.store.register_host(&registration(), "key").is_err(),
            "{mutation}"
        );
    }
}
#[test]
fn persisted_record_versions_types_bounds_mirrors_and_facts_fail_closed() {
    let cases: [fn(&mut HostRegistrationRecord); 9] = [
        |r| r.record_version = 2,
        |r| r.revision = 2,
        |r| r.id = "local".into(),
        |r| r.created_sequence = MAX_SEQUENCE + 1,
        |r| r.created_at = "2026-02-30T00:00:00Z".into(),
        |r| r.creation_key = "bad key".into(),
        |r| r.creation_facts.execution_enabled = true,
        |r| r.registration.transport_identity_ref = "invalid\nref".into(),
        |r| r.registration.name = "Changed mirror".into(),
    ];
    for change in cases {
        let mut f = fixture();
        let r = f.store.register_host(&registration(), "original").unwrap();
        change_record(&f.store, &r.host_id, change);
        assert_refuses_corruption(&mut f.store, &r.host_id);
    }
    let mutations: [fn(String) -> String; 6] = [
        |_| "not JSON".into(),
        |_| " ".repeat(MAX_DOCUMENT + 1),
        |raw| raw.replacen("\"record_version\":1", "\"record_version\":1.0", 1),
        |raw| {
            raw.replacen(
                "\"record_version\":1",
                "\"record_version\":1,\"record_version\":1",
                1,
            )
        },
        |raw| {
            raw.replacen(
                "\"record_version\":1",
                "\"record_version\":1,\"connected\":true",
                1,
            )
        },
        |raw| raw.replacen("\"revision\":1", "\"revision\":null", 1),
    ];
    for mutate in mutations {
        let mut f = fixture();
        let r = f.store.register_host(&registration(), "original").unwrap();
        let good = raw_record(&f.store, &r.host_id);
        assert!(f
            .store
            .find_host_registration("original")
            .unwrap()
            .is_some());
        let bad = mutate(good.clone());
        assert_ne!(good, bad);
        f.store
            .conn
            .execute("UPDATE host_registrations SET document=?1", [bad])
            .unwrap();
        assert_refuses_corruption(&mut f.store, &r.host_id);
    }
    for mutation in [
        "UPDATE host_registrations SET name_key='rewritten'",
        "UPDATE host_registrations SET document=CAST(document AS BLOB)",
        "UPDATE host_registrations SET document=CAST(x'ff' AS TEXT)",
    ] {
        let mut f = fixture();
        let r = f.store.register_host(&registration(), "original").unwrap();
        f.store.conn.execute_batch(mutation).unwrap();
        assert_refuses_corruption(&mut f.store, &r.host_id);
    }
}
#[test]
fn missing_receipts_or_hosts_are_integrity_errors_not_absent() {
    for mutation in [
        "DELETE FROM idempotency WHERE scope='hosts.register:v1'",
        "DELETE FROM host_registrations",
        "UPDATE idempotency SET payload='{}' WHERE scope='hosts.register:v1'",
        "UPDATE idempotency SET key='bad key' WHERE scope='hosts.register:v1'",
        "UPDATE idempotency SET result='null' WHERE scope='hosts.register:v1'",
    ] {
        let mut f = fixture();
        let r = f.store.register_host(&registration(), "original").unwrap();
        f.store.conn.execute_batch(mutation).unwrap();
        assert_refuses_corruption(&mut f.store, &r.host_id);
    }
    let mut f = fixture();
    let r = f.store.register_host(&registration(), "original").unwrap();
    change_receipt(&f.store, "original", |r| r.receipt_version = 2);
    assert_refuses_corruption(&mut f.store, &r.host_id);
    let mut f = fixture();
    let r = f.store.register_host(&registration(), "original").unwrap();
    f.store
        .conn
        .execute(
            "UPDATE idempotency SET result=?1 WHERE scope=?2",
            params!["x".repeat(MAX_DOCUMENT + 1), SCOPE],
        )
        .unwrap();
    assert_refuses_corruption(&mut f.store, &r.host_id);
}
#[test]
fn event_linkage_is_checked_and_no_action_pins_sql_sequence() {
    let cases: [fn(&mut HostEvent); 7] = [
        |e| e.aggregate_id = "wrong".into(),
        |e| e.sequence = 1,
        |e| e.kind = "task.created".into(),
        |e| e.source = "agent_output".into(),
        |e| e.payload.registration_sha256 = "b".repeat(64),
        |e| e.payload.payload_version = 2,
        |e| e.occurred_at = "2000-01-01T00:00:00Z".into(),
    ];
    for change in cases {
        let mut f = fixture();
        let r = f.store.register_host(&registration(), "original").unwrap();
        change_event(&f.store, r.created_sequence, change);
        assert_refuses_corruption(&mut f.store, &r.host_id);
    }
    let mut f = fixture();
    let r = f.store.register_host(&registration(), "original").unwrap();
    assert!(f
        .store
        .conn
        .execute(
            "DELETE FROM events WHERE sequence=?1",
            [r.created_sequence as i64]
        )
        .is_err());
    assert!(f
        .store
        .conn
        .execute(
            "UPDATE events SET sequence=sequence+1 WHERE sequence=?1",
            [r.created_sequence as i64]
        )
        .is_err());
    assert_eq!(
        f.store.find_host_registration("original").unwrap(),
        Some(r.clone())
    );
    f.store.conn.execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM events WHERE json_extract(document,'$.kind')='host.registered'; PRAGMA foreign_keys=ON;").unwrap();
    assert_refuses_corruption(&mut f.store, &r.host_id);
}
#[test]
fn event_host_and_receipt_insert_failures_roll_back_all_three_domains() {
    for table in ["events", "host_registrations", "idempotency"] {
        let mut f = fixture();
        f.store.conn.execute_batch(&format!("CREATE TEMP TRIGGER synthetic_abort BEFORE INSERT ON main.{table} BEGIN SELECT RAISE(ABORT,'synthetic fault'); END;")).unwrap();
        assert!(f.store.register_host(&registration(), "original").is_err());
        assert_eq!(counts(&f.store), (0, 0, 0));
        f.store
            .conn
            .execute_batch("DROP TRIGGER temp.synthetic_abort")
            .unwrap();
        drop(f.store);
        let mut reopened = reopen_explicit_fixture(&f.path);
        assert!(reopened
            .find_host_registration("original")
            .unwrap()
            .is_none());
        reopened.register_host(&registration(), "original").unwrap();
        assert_eq!(counts(&reopened), (1, 1, 1));
    }
}
#[test]
fn actual_deferred_commit_failure_is_unknown_and_rolls_back_on_reopen() {
    let mut f = fixture();
    f.store.conn.execute_batch("CREATE TEMP TABLE synthetic_parent(id INTEGER PRIMARY KEY); CREATE TEMP TABLE synthetic_child(id INTEGER REFERENCES synthetic_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TEMP TRIGGER synthetic_commit_fault AFTER INSERT ON main.idempotency BEGIN INSERT INTO synthetic_child VALUES(42); END;").unwrap();
    let error = f
        .store
        .register_host(&registration(), "original")
        .unwrap_err();
    assert_eq!(error.code, "OUTCOME_UNKNOWN");
    assert_eq!(error.side_effect_state, "unknown");
    assert_eq!(counts(&f.store), (0, 0, 0));
    drop(f.store);
    let mut reopened = reopen_explicit_fixture(&f.path);
    assert!(reopened
        .find_host_registration("original")
        .unwrap()
        .is_none());
    reopened.register_host(&registration(), "original").unwrap();
}
#[test]
fn actual_journal_creation_failure_does_not_leave_a_partial_registration() {
    let mut f = fixture();
    // Explicit DELETE-journal fault fixture, not evidence of production WAL I/O.
    f.store
        .conn
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    let journal = PathBuf::from(format!("{}-journal", f.path.display()));
    std::fs::create_dir(&journal).unwrap();
    assert!(f.store.register_host(&registration(), "original").is_err());
    std::fs::remove_dir(&journal).unwrap();
    assert_eq!(counts(&f.store), (0, 0, 0));
    drop(f.store);
    let mut reopened = reopen_explicit_fixture(&f.path);
    assert!(reopened
        .find_host_registration("original")
        .unwrap()
        .is_none());
}
#[test]
fn unsafe_inserted_sequence_rolls_back_including_event_counter() {
    let mut f = fixture();
    f.store
        .conn
        .execute("DELETE FROM sqlite_sequence WHERE name='events'", [])
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT OR REPLACE INTO sqlite_sequence(name,seq) VALUES('events',?1)",
            [MAX_SEQUENCE as i64],
        )
        .unwrap();
    assert_eq!(
        f.store
            .register_host(&registration(), "original")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(counts(&f.store), (0, 0, 0));
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name='events'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        MAX_SEQUENCE as i64
    );
}
#[test]
fn bounded_list_makes_cursor_progress_and_rejects_invalid_cursors() {
    let mut f = fixture();
    for n in 0..6 {
        f.store
            .register_host(&registration(), &format!("key:{n}"))
            .unwrap();
    }
    let mut cursor = None;
    let mut ids = BTreeSet::new();
    loop {
        let page = f
            .store
            .list_host_registrations(&HostListRequest {
                limit: 2,
                after_created_sequence: cursor,
            })
            .unwrap();
        assert!(page.hosts.len() <= 2);
        assert!(encode(&page).unwrap().len() + FRAME_RESERVE <= MAX_RESPONSE);
        for h in page.hosts {
            assert!(ids.insert(h.record.id));
        }
        cursor = page.next_created_sequence;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 6);
    for request in [
        HostListRequest {
            limit: 0,
            after_created_sequence: None,
        },
        HostListRequest {
            limit: 26,
            after_created_sequence: None,
        },
        HostListRequest {
            limit: 1,
            after_created_sequence: Some(0),
        },
        HostListRequest {
            limit: 1,
            after_created_sequence: Some(MAX_SEQUENCE + 1),
        },
    ] {
        assert_eq!(
            f.store.list_host_registrations(&request).unwrap_err().code,
            "INVALID_REQUEST"
        );
    }
    let entries = audit(&f.store.conn).unwrap();
    let first = page(
        entries,
        &HostListRequest {
            limit: 1,
            after_created_sequence: None,
        },
        MAX_RESPONSE,
    )
    .unwrap();
    let limit = encode(&first).unwrap().len() + FRAME_RESERVE + 8;
    let limited = page(
        audit(&f.store.conn).unwrap(),
        &HostListRequest {
            limit: 25,
            after_created_sequence: None,
        },
        limit,
    )
    .unwrap();
    assert_eq!(limited.hosts.len(), 1);
    assert!(limited.next_created_sequence.is_some());
    assert!(encode(&limited).unwrap().len() + FRAME_RESERVE <= limit);
    assert!(page(
        audit(&f.store.conn).unwrap(),
        &HostListRequest {
            limit: 25,
            after_created_sequence: None
        },
        FRAME_RESERVE
    )
    .is_err());
}
#[test]
fn duplicate_creation_keys_and_over_cap_receipt_scans_are_not_absent() {
    let mut f = fixture();
    let a = f.store.register_host(&registration(), "original").unwrap();
    let b = f.store.register_host(&registration(), "second").unwrap();
    change_record(&f.store, &b.host_id, |r| r.creation_key = "original".into());
    assert_refuses_corruption(&mut f.store, &a.host_id);
    let mut f = fixture();
    for n in 0..128 {
        f.store
            .register_host(&registration(), &format!("key:{n}"))
            .unwrap();
    }
    let (payload, result): (String, String) = f
        .store
        .conn
        .query_row(
            "SELECT payload,result FROM idempotency WHERE scope=?1 LIMIT 1",
            [SCOPE],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT INTO idempotency VALUES(?1,'overflow',?2,?3)",
            params![SCOPE, payload, result],
        )
        .unwrap();
    assert!(f.store.find_host_registration("unknown").is_err());
    assert!(f.store.register_host(&registration(), "new").is_err());
    assert_eq!(counts(&f.store), (128, 128, 129));
}
#[test]
fn strict_decoders_preserve_versions_unicode_and_reject_unknown_fields() {
    let mut input = registration();
    input.name = "重复名称 Alpha".into();
    let mut f = fixture();
    let a = f.store.register_host(&input, "unicode:1").unwrap();
    let b = f.store.register_host(&input, "unicode:2").unwrap();
    assert_eq!(a.registration.name, input.name);
    assert_ne!(a.host_id, b.host_id);
    let actual = raw_record(&f.store, &a.host_id);
    for raw in [
        actual.replacen("\"record_version\":1", "\"record_version\":true", 1),
        actual.replacen("\"revision\":1", "\"revision\":\"1\"", 1),
    ] {
        assert!(decode_canonical::<HostRegistrationRecord>(&raw, MAX_DOCUMENT).is_err());
    }
    assert!(!timestamp_valid("2026-02-29T00:00:00Z"));
    assert!(timestamp_valid("2024-02-29T00:00:00.123Z"));
    assert!(!timestamp_valid("2026-11-18T00:00:00+00:00"));
    let record: HostRegistrationRecord = decode_canonical(RECORD, MAX_DOCUMENT).unwrap();
    let value: Value = serde_json::from_str(&encode(&record).unwrap()).unwrap();
    assert_eq!(value["creation_facts"]["execution_enabled"], json!(false));
}

#[test]
fn safe_opaque_tables_indexes_and_blob_rowids_remain_unchanged() {
    let mut f = fixture();
    f.store.conn.execute_batch("CREATE TABLE host_notes(note TEXT,opaque BLOB); CREATE INDEX host_notes_note ON host_notes(note); INSERT INTO host_notes(rowid,note,opaque) VALUES(71,'uninterpreted',x'00ff01');").unwrap();
    let before: (i64, String, Vec<u8>) = f
        .store
        .conn
        .query_row("SELECT rowid,note,opaque FROM host_notes", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    let receipt = f.store.register_host(&registration(), "original").unwrap();
    assert!(f.store.get_host_registration(&receipt.host_id).is_ok());
    assert!(f.store.find_host_registration("absent").unwrap().is_none());
    let after: (i64, String, Vec<u8>) = f
        .store
        .conn
        .query_row("SELECT rowid,note,opaque FROM host_notes", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(before, after);
    let mut reopened = reopen_explicit_fixture(&f.path);
    assert_eq!(
        reopened.register_host(&registration(), "original").unwrap(),
        receipt
    );
    let after_reopen: (i64, String, Vec<u8>) = reopened
        .conn
        .query_row("SELECT rowid,note,opaque FROM host_notes", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(before, after_reopen);
    // Dependencies on the Host authority table are not ordinary opaque data.
    f.store
        .conn
        .execute_batch("CREATE TABLE consumer(host_id TEXT REFERENCES host_registrations(id))")
        .unwrap();
    assert_eq!(
        f.store.find_host_registration("original").unwrap_err().code,
        "VERSION_UNSUPPORTED"
    );
}
#[test]
fn sql_projects_oversized_or_wrong_type_bodies_as_null_without_hiding_rows() {
    for (target, column, projected) in [
        ("idempotency", "key", 0usize),
        ("idempotency", "payload", 1),
        ("idempotency", "result", 2),
        ("host_registrations", "id", 0),
        ("host_registrations", "name", 1),
        ("host_registrations", "name_key", 2),
        ("host_registrations", "document", 5),
        ("events", "document", 0),
    ] {
        for blob in [false, true] {
            let mut f = fixture();
            let receipt = f.store.register_host(&registration(), "original").unwrap();
            // Four MiB exceeds every allowed field. SQL CASE must retain the row
            // and project NULL before rusqlite is asked to copy a body.
            let text = "x".repeat(4 * 1024 * 1024);
            let sql = format!("UPDATE {target} SET {column}=?1");
            if blob {
                f.store.conn.execute(&sql, [text.as_bytes()]).unwrap();
            } else {
                f.store.conn.execute(&sql, [text]).unwrap();
            }
            let value: Option<String> = match target {
                "idempotency" => f
                    .store
                    .conn
                    .query_row(READ_RECEIPTS_SQL, [SCOPE], |r| r.get(projected))
                    .unwrap(),
                "host_registrations" => f
                    .store
                    .conn
                    .query_row(READ_HOSTS_SQL, [], |r| r.get(projected))
                    .unwrap(),
                _ => f
                    .store
                    .conn
                    .query_row(READ_EVENT_SQL, [receipt.created_sequence as i64], |r| {
                        r.get(projected)
                    })
                    .unwrap(),
            };
            assert!(value.is_none(), "{target}.{column}");
            assert!(
                f.store.find_host_registration("original").is_err(),
                "invalid rows cannot be filtered into absent"
            );
            assert!(f
                .store
                .register_host(&registration(), "replacement")
                .is_err());
        }
    }
}
