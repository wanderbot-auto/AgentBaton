use super::super::{
    ManagedCreationBinding, ManagedCreationKind, ManagedCreationOperation, TaskMessageIntent,
    JOURNAL, LIMIT, MESSAGE_JOURNAL,
};
use super::*;
use agentbaton_remote_host::registration::DeclaredDirectoryMapping;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::Command,
    sync::Barrier,
    thread,
};

const REQUEST_KEY: &str = "67efc991-d0c1-4641-8801-1df6e547d37f";
const OTHER_KEY: &str = "6ad7ad1f-ae87-404f-be9f-3df329a4a9f2";
const UNAVAILABLE: &str = "CLIENT_RECOVERY_UNAVAILABLE";
const REQUIRED: &str = "CLIENT_RECOVERY_REQUIRED";
const BUSY: &str = "CLIENT_RECOVERY_BUSY";

fn fixture() -> (tempfile::TempDir, ClientRecoveryJournal) {
    let parent = std::env::temp_dir().canonicalize().unwrap();
    let directory = tempfile::tempdir_in(parent).unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let journal = ClientRecoveryJournal::new(directory.path()).unwrap();
    (directory, journal)
}

fn registration() -> HostRegistration {
    serde_json::from_value(json!({
        "schema_version": 1,
        "name": "Synthetic Host",
        "declared_platform": "linux",
        "endpoint": {
            "expected_host_identity": "synthetic-host",
            "address": "example.invalid",
            "user": "baton",
            "port": 22,
            "host_key_alias": "synthetic-alias"
        },
        "known_hosts": {
            "reference": "known-hosts:synthetic",
            "file_sha256": "a".repeat(64)
        },
        "transport_identity_ref": "identity:synthetic",
        "configuration": {
            "reference": "config:synthetic",
            "declared_ownership": "external_read_only"
        },
        "directory_mappings": [{"id":"source","remote_root":"/synthetic/source"}]
    }))
    .unwrap()
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[track_caller]
fn assert_unavailable(journal: &ClientRecoveryJournal) {
    assert_eq!(
        journal.pending_host_registration().unwrap_err().code,
        UNAVAILABLE
    );
    let error = journal
        .reserve_host_registration(OTHER_KEY.into(), registration())
        .unwrap_err();
    assert_eq!(error.code, UNAVAILABLE);
    assert_eq!(error.side_effect_state, "not_started");
}

#[test]
fn reopens_exact_intent_and_preserves_it_on_every_second_reservation() {
    let (directory, journal) = fixture();
    let path = directory.path().join(HOST_JOURNAL);
    assert_eq!(journal.pending_host_registration().unwrap(), None);
    assert!(!path.exists());
    let registration = registration();
    let intent = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration.clone())
        .unwrap();
    assert_eq!(intent.intent_version, 1);
    assert_eq!(intent.request_key, REQUEST_KEY);
    assert_eq!(intent.registration, registration);
    assert_eq!(
        intent.registration_sha256,
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&registration).unwrap())
        )
    );
    assert_eq!(
        Uuid::parse_str(&intent.entry_id).unwrap().get_version_num(),
        4
    );
    assert!(valid_timestamp(&intent.created_at));
    let bytes = fs::read(&path).unwrap();
    for name in [HOST_JOURNAL, HOST_LOCK] {
        assert_eq!(
            fs::metadata(directory.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
    }

    drop(journal);
    let reopened = ClientRecoveryJournal::new(directory.path()).unwrap();
    for key in [REQUEST_KEY, OTHER_KEY] {
        let error = reopened
            .reserve_host_registration(key.into(), registration.clone())
            .unwrap_err();
        assert_eq!(error.code, REQUIRED);
        assert_eq!(error.side_effect_state, "not_started");
    }
    let mut different = registration;
    different.name = "Another synthetic Host".into();
    assert_eq!(
        reopened
            .reserve_host_registration(REQUEST_KEY.into(), different)
            .unwrap_err()
            .code,
        REQUIRED
    );
    assert_eq!(reopened.pending_host_registration().unwrap(), Some(intent));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn subprocess_reserve() {
    let Some(root) = std::env::var_os("BATON_HOST_JOURNAL_TEST_ROOT") else {
        return;
    };
    let key = std::env::var("BATON_HOST_JOURNAL_TEST_KEY").unwrap();
    let journal = ClientRecoveryJournal::new(Path::new(&root)).unwrap();
    let mut registration = registration();
    registration.name = key.clone();
    let code = match journal.reserve_host_registration(key, registration) {
        Ok(_) => 10,
        Err(error) if error.code == REQUIRED => 11,
        Err(error) if error.code == BUSY => 12,
        Err(error) => panic!("unexpected Host reserve error: {error:?}"),
    };
    std::process::exit(code);
}

#[test]
fn concurrent_processes_preserve_the_winning_key_and_body_after_exit() {
    let (directory, journal) = fixture();
    let barrier = Barrier::new(2);
    let run = |key: &str| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "client_recovery_journal::host_registration_recovery::tests::subprocess_reserve",
                "--nocapture",
            ])
            .env("BATON_HOST_JOURNAL_TEST_ROOT", directory.path())
            .env("BATON_HOST_JOURNAL_TEST_KEY", key);
        barrier.wait();
        command.output().unwrap()
    };
    let (a, b) = thread::scope(|scope| {
        let a = scope.spawn(|| run(REQUEST_KEY));
        let b = scope.spawn(|| run(OTHER_KEY));
        (a.join().unwrap(), b.join().unwrap())
    });
    let winner = match (a.status.code(), b.status.code()) {
        (Some(10), Some(11 | 12)) => REQUEST_KEY,
        (Some(11 | 12), Some(10)) => OTHER_KEY,
        _ => panic!("unexpected reserve results: {a:?}, {b:?}"),
    };
    drop(journal);
    let reopened = ClientRecoveryJournal::new(directory.path()).unwrap();
    let pending = reopened.pending_host_registration().unwrap().unwrap();
    let mut expected = registration();
    expected.name = winner.into();
    assert_eq!(pending.request_key, winner);
    assert_eq!(pending.registration, expected);
}

#[test]
fn held_host_lock_fails_closed_without_replacing_the_pending_request() {
    let (directory, journal) = fixture();
    let intent = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration())
        .unwrap();
    let path = directory.path().join(HOST_JOURNAL);
    let bytes = fs::read(&path).unwrap();
    let lock = journal.lock_file(HOST_LOCK).unwrap();
    let other = ClientRecoveryJournal::new(directory.path()).unwrap();
    let error = other.pending_host_registration().unwrap_err();
    assert_eq!(error.code, BUSY);
    assert_eq!(error.side_effect_state, "not_started");
    let error = other
        .reserve_host_registration(OTHER_KEY.into(), registration())
        .unwrap_err();
    assert_eq!(error.code, BUSY);
    assert_eq!(error.side_effect_state, "not_started");
    assert_eq!(fs::read(path).unwrap(), bytes);
    drop(lock);
    assert_eq!(other.pending_host_registration().unwrap(), Some(intent));
}

#[test]
fn host_and_both_existing_journals_have_independent_files_and_locks() {
    let (directory, journal) = fixture();
    let binding = ManagedCreationBinding {
        kind: ManagedCreationKind::ManagedMcp,
        request_key: REQUEST_KEY.into(),
        preview_digest: "a".repeat(64),
        base_connection_id: "synthetic".into(),
        base_connection_revision: 1,
        operation: ManagedCreationOperation::Replace,
    };
    let managed = journal.reserve(binding).unwrap();
    let message = TaskMessageIntent {
        session_id: OTHER_KEY.into(),
        request_key: REQUEST_KEY.into(),
        expected_revision: 1,
        text_sha256: "b".repeat(64),
    };
    let mut messages = journal.task_messages().unwrap();
    messages.reserve(message.clone()).unwrap();
    let managed_path = directory.path().join(JOURNAL);
    let message_path = directory.path().join(MESSAGE_JOURNAL);
    let managed_bytes = fs::read(&managed_path).unwrap();
    let message_bytes = fs::read(&message_path).unwrap();

    let managed_lock = journal.lock().unwrap();
    let host = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration())
        .unwrap();
    assert_eq!(journal.pending_host_registration().unwrap(), Some(host));
    drop(managed_lock);
    drop(messages);

    let _host_lock = journal.lock_file(HOST_LOCK).unwrap();
    assert_eq!(journal.list().unwrap(), vec![managed]);
    assert_eq!(journal.task_messages().unwrap().entries(), &[message]);
    assert_eq!(fs::read(managed_path).unwrap(), managed_bytes);
    assert_eq!(fs::read(message_path).unwrap(), message_bytes);
}

#[test]
fn invalid_submissions_leave_the_host_journal_absent() {
    let (directory, journal) = fixture();
    for key in [
        REQUEST_KEY.to_uppercase(),
        REQUEST_KEY.replace('-', ""),
        "../invalid".into(),
    ] {
        let error = journal
            .reserve_host_registration(key, registration())
            .unwrap_err();
        assert_eq!(error.code, "INVALID_REQUEST");
        assert_eq!(error.side_effect_state, "not_started");
    }
    let mut invalid = registration();
    invalid.endpoint.expected_host_identity = "LOCAL".into();
    assert_eq!(
        journal
            .reserve_host_registration(REQUEST_KEY.into(), invalid)
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    assert!(!directory.path().join(HOST_LOCK).exists());
    assert!(!directory.path().join(HOST_JOURNAL).exists());
    assert_eq!(journal.pending_host_registration().unwrap(), None);
}

#[test]
fn corrupt_intents_and_closed_format_violations_are_never_treated_as_empty() {
    let (directory, journal) = fixture();
    let intent = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration())
        .unwrap();
    let path = directory.path().join(HOST_JOURNAL);
    let valid_bytes = fs::read(&path).unwrap();
    let valid: Value = serde_json::from_slice(&valid_bytes).unwrap();
    let mut corrupt = vec![
        b"invalid".to_vec(),
        br#"{"version":1,"version":1,"entries":[]}"#.to_vec(),
        br#"{"version":1,"entries":null}"#.to_vec(),
        serde_json::to_vec(&json!({"version":1,"entries":[intent.clone(), intent]})).unwrap(),
    ];
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/entries/0/intent_version", json!(2)),
        ("/entries/0/entry_id", json!("not-uuid")),
        ("/entries/0/request_key", json!(REQUEST_KEY.to_uppercase())),
        ("/entries/0/created_at", json!("2025-02-29T00:00:00Z")),
        ("/entries/0/created_at", json!("2024-02-29T00:00:00+00:00")),
        ("/entries/0/registration_sha256", json!("a".repeat(64))),
        (
            "/entries/0/registration/name",
            json!("Body changed without digest"),
        ),
        (
            "/entries/0/registration/directory_mappings/0/remote_root",
            json!("/invalid/../source"),
        ),
    ] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        corrupt.push(serde_json::to_vec(&changed).unwrap());
    }
    // A digest consistent with the bytes does not legalize invalid metadata.
    let mut invalid_metadata = registration();
    invalid_metadata.endpoint.expected_host_identity = "local".into();
    let mut changed = valid.clone();
    changed["entries"][0]["registration_sha256"] =
        json!(digest(&serde_json::to_vec(&invalid_metadata).unwrap()));
    changed["entries"][0]["registration"] = json!(invalid_metadata);
    corrupt.push(serde_json::to_vec(&changed).unwrap());
    for pointer in ["", "/entries/0", "/entries/0/registration/endpoint"] {
        let mut changed = valid.clone();
        changed.pointer_mut(pointer).unwrap()["unexpected"] = json!(true);
        corrupt.push(serde_json::to_vec(&changed).unwrap());
    }
    for bytes in corrupt {
        write_private(&path, &bytes);
        assert_unavailable(&journal);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn private_file_checks_apply_to_both_host_paths() {
    for name in [HOST_JOURNAL, HOST_LOCK] {
        let (directory, journal) = fixture();
        let path = directory.path().join(name);
        let target = directory.path().join("target");
        let bytes = &br#"{"version":1,"entries":[]}"#[..];
        write_private(&target, bytes);
        symlink(&target, &path).unwrap();
        assert_unavailable(&journal);
        assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
        assert_eq!(fs::read(&target).unwrap(), bytes);
        fs::remove_file(&path).unwrap();

        fs::hard_link(&target, &path).unwrap();
        assert_unavailable(&journal);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&target).unwrap(), bytes);
        fs::remove_file(&path).unwrap();

        write_private(&path, bytes);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_unavailable(&journal);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(&path).unwrap();

        fs::create_dir(&path).unwrap();
        assert_unavailable(&journal);
        assert!(path.is_dir());
    }
}

#[test]
fn an_open_journal_refuses_permission_drift_and_replaced_runtime_directory() {
    let (directory, _) = fixture();
    let root = directory.path().join("runtime");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let journal = ClientRecoveryJournal::new(&root).unwrap();
    journal
        .reserve_host_registration(REQUEST_KEY.into(), registration())
        .unwrap();
    let original = fs::read(root.join(HOST_JOURNAL)).unwrap();

    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert_unavailable(&journal);
    assert!(ClientRecoveryJournal::new(&root).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let moved = directory.path().join("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert_unavailable(&journal);
    assert!(!root.join(HOST_JOURNAL).exists());
    assert_eq!(fs::read(moved.join(HOST_JOURNAL)).unwrap(), original);
}

#[test]
fn maximal_registration_fits_with_the_full_wrapper_and_read_limit_applies_to_all_bytes() {
    let (directory, journal) = fixture();
    let mut registration = registration();
    // Quotes and backslashes maximize name expansion after JSON serialization.
    registration.name = "\\\"".repeat(128);
    registration.endpoint.expected_host_identity = "h".repeat(64);
    registration.endpoint.address = "a".repeat(253);
    registration.endpoint.user = "u".repeat(64);
    registration.endpoint.port = u16::MAX;
    registration.endpoint.host_key_alias = "k".repeat(64);
    registration.known_hosts.reference = "k".repeat(256);
    registration.transport_identity_ref = "i".repeat(256);
    registration.configuration.reference = "c".repeat(256);
    registration.directory_mappings = (0..8)
        .map(|i| DeclaredDirectoryMapping {
            id: format!("{}{i}", "m".repeat(63)),
            remote_root: format!("/{}", "r".repeat(511)),
        })
        .collect();
    registration.validate().unwrap();
    let intent = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration)
        .unwrap();

    let path = directory.path().join(HOST_JOURNAL);
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({
            "version": 1, "entries": [intent]
        })
    );
    assert!(
        bytes.len() < LIMIT as usize,
        "wrapped size: {}",
        bytes.len()
    );
    assert_eq!(
        journal.pending_host_registration().unwrap(),
        Some(intent.clone())
    );

    bytes.resize(LIMIT as usize, b' ');
    write_private(&path, &bytes);
    assert_eq!(journal.pending_host_registration().unwrap(), Some(intent));
    bytes.push(b' ');
    write_private(&path, &bytes);
    assert_unavailable(&journal);
    assert_eq!(fs::read(path).unwrap(), bytes);
}

fn read_registration(
    store: &mut crate::storage::Store,
    key: &str,
) -> Result<HostRegistrationLookup> {
    Ok(HostRegistrationLookup {
        lookup_version: 1,
        key: key.into(),
        receipt: store.find_host_registration(key)?,
    })
}

struct CommittedRegistration {
    directory: tempfile::TempDir,
    journal: ClientRecoveryJournal,
    store: crate::storage::Store,
    intent: HostRegistrationIntent,
    receipt: HostCreationReceipt,
}

impl CommittedRegistration {
    fn new() -> Self {
        let (directory, journal) = fixture();
        let mut store =
            crate::storage::Store::open(&directory.path().join("registry.sqlite3")).unwrap();
        let intent = journal
            .reserve_host_registration(REQUEST_KEY.into(), registration())
            .unwrap();
        let receipt = store
            .register_host(&intent.registration, &intent.request_key)
            .unwrap();
        Self {
            directory,
            journal,
            store,
            intent,
            receipt,
        }
    }

    fn journal_path(&self) -> std::path::PathBuf {
        self.directory.path().join(HOST_JOURNAL)
    }
}

#[test]
fn lookup_reply_requires_explicit_null_or_a_closed_receipt() {
    let absent = json!({"lookup_version":1,"key":REQUEST_KEY,"receipt":null});
    let decoded: HostRegistrationLookup = serde_json::from_value(absent.clone()).unwrap();
    assert_eq!(decoded.receipt, None);
    assert_eq!(serde_json::to_value(decoded).unwrap(), absent);
    for invalid in [
        json!({"lookup_version":1,"key":REQUEST_KEY}),
        json!({"lookup_version":1,"key":REQUEST_KEY,"receipt":null,"extra":true}),
        json!({"lookup_version":1,"key":REQUEST_KEY,"receipt":{}}),
    ] {
        assert!(serde_json::from_value::<HostRegistrationLookup>(invalid).is_err());
    }
    let duplicate =
        format!(r#"{{"lookup_version":1,"key":"{REQUEST_KEY}","receipt":null,"receipt":null}}"#);
    assert!(serde_json::from_str::<HostRegistrationLookup>(&duplicate).is_err());
}

#[test]
fn a_fresh_matching_store_receipt_acknowledges_only_the_host_journal() {
    let mut f = CommittedRegistration::new();
    let journal_path = f.journal_path();
    let stored_bytes = fs::read(&journal_path).unwrap();
    let lookup = read_registration(&mut f.store, REQUEST_KEY).unwrap();
    assert_eq!(lookup.receipt.as_ref(), Some(&f.receipt));
    f.intent.validate_lookup(&lookup).unwrap();
    assert_eq!(fs::read(&journal_path).unwrap(), stored_bytes);

    f.journal
        .reserve(ManagedCreationBinding {
            kind: ManagedCreationKind::ManagedMcp,
            request_key: OTHER_KEY.into(),
            preview_digest: "a".repeat(64),
            base_connection_id: "synthetic".into(),
            base_connection_revision: 1,
            operation: ManagedCreationOperation::Replace,
        })
        .unwrap();
    let _managed_lock = f.journal.lock().unwrap();
    let mut messages = f.journal.task_messages().unwrap();
    messages
        .reserve(TaskMessageIntent {
            session_id: OTHER_KEY.into(),
            request_key: REQUEST_KEY.into(),
            expected_revision: 1,
            text_sha256: "b".repeat(64),
        })
        .unwrap();
    let legacy_files = [JOURNAL, MESSAGE_JOURNAL].map(|name| {
        let path = f.directory.path().join(name);
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    });

    let reopened = ClientRecoveryJournal::new(f.directory.path()).unwrap();
    assert!(reopened
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |key| {
                assert_eq!(key, REQUEST_KEY);
                read_registration(&mut f.store, key)
            },
        )
        .unwrap());

    assert_eq!(
        ClientRecoveryJournal::new(f.directory.path())
            .unwrap()
            .pending_host_registration()
            .unwrap(),
        None
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&journal_path).unwrap()).unwrap(),
        json!({"version":1,"entries":[]})
    );
    for (path, bytes) in legacy_files {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(
        f.store.find_host_registration(REQUEST_KEY).unwrap(),
        Some(f.receipt.clone())
    );
    let host = f.store.get_host_registration(&f.receipt.host_id).unwrap();
    assert_eq!(host.record.created_sequence, f.receipt.created_sequence);
    assert_eq!(host.record.creation_key, REQUEST_KEY);
    let cleared_bytes = fs::read(&journal_path).unwrap();
    assert!(!reopened
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |_| { panic!("a repeated acknowledgement must not look up a cleared intent") }
        )
        .unwrap());
    assert_eq!(fs::read(journal_path).unwrap(), cleared_bytes);
}

#[test]
fn absent_or_failed_lookup_never_writes_the_host_journal() {
    let (directory, journal) = fixture();
    let mut store =
        crate::storage::Store::open(&directory.path().join("registry.sqlite3")).unwrap();
    let intent = journal
        .reserve_host_registration(REQUEST_KEY.into(), registration())
        .unwrap();
    let path = directory.path().join(HOST_JOURNAL);
    let bytes = fs::read(&path).unwrap();

    assert!(!journal
        .acknowledge_host_registration(&intent.entry_id, OTHER_KEY, 1, |key| {
            assert_eq!(key, REQUEST_KEY);
            read_registration(&mut store, key)
        })
        .unwrap());
    assert_eq!(fs::read(&path).unwrap(), bytes);

    let disconnected = CoreError::transport();
    assert_eq!(
        journal
            .acknowledge_host_registration(&intent.entry_id, OTHER_KEY, 1, |key| {
                assert_eq!(key, REQUEST_KEY);
                Err(disconnected.clone())
            })
            .unwrap_err(),
        disconnected
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(journal.pending_host_registration().unwrap(), Some(intent));
    assert_eq!(store.find_host_registration(REQUEST_KEY).unwrap(), None);
}

#[test]
fn acknowledgement_rejects_wrong_key_body_host_or_sequence() {
    let mut f = CommittedRegistration::new();
    let path = f.journal_path();
    let bytes = fs::read(&path).unwrap();
    let other = f
        .store
        .register_host(&f.intent.registration, OTHER_KEY)
        .unwrap();
    let wrong_key = f
        .journal
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &other.host_id,
            other.created_sequence,
            |key| {
                assert_eq!(key, REQUEST_KEY);
                read_registration(&mut f.store, OTHER_KEY)
            },
        )
        .unwrap_err();
    assert_eq!(wrong_key.code, "CONFIG_CONFLICT");
    assert_eq!(fs::read(&path).unwrap(), bytes);

    let mut changed = read_registration(&mut f.store, REQUEST_KEY).unwrap();
    changed.receipt.as_mut().unwrap().registration.name = "Different valid Host metadata".into();
    let wrong_body = f
        .journal
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |_| Ok(changed),
        )
        .unwrap_err();
    assert_eq!(wrong_body.code, "CONFIG_CONFLICT");
    assert_eq!(fs::read(&path).unwrap(), bytes);

    for (host_id, seq) in [
        (other.host_id.as_str(), f.receipt.created_sequence),
        (f.receipt.host_id.as_str(), other.created_sequence),
    ] {
        assert_eq!(
            f.journal
                .acknowledge_host_registration(&f.intent.entry_id, host_id, seq, |key| {
                    read_registration(&mut f.store, key)
                })
                .unwrap_err()
                .code,
            "CONFIG_CONFLICT"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    assert_eq!(
        f.journal.pending_host_registration().unwrap(),
        Some(f.intent)
    );
}

#[test]
fn acknowledgement_reuses_receipt_semantic_checks() {
    let mut f = CommittedRegistration::new();
    let path = f.journal_path();
    let bytes = fs::read(&path).unwrap();
    let lookup = read_registration(&mut f.store, REQUEST_KEY).unwrap();
    for (pointer, value, code) in [
        ("/lookup_version", json!(2), "VERSION_UNSUPPORTED"),
        ("/receipt/receipt_version", json!(2), "VERSION_UNSUPPORTED"),
        ("/receipt/revision", json!(2), "OUTCOME_UNKNOWN"),
        (
            "/receipt/created_at",
            json!("2025-02-29T00:00:00Z"),
            "OUTCOME_UNKNOWN",
        ),
        ("/receipt/created_sequence", json!(0), "OUTCOME_UNKNOWN"),
        ("/receipt/host_id", json!("invalid"), "OUTCOME_UNKNOWN"),
    ] {
        let mut invalid = serde_json::to_value(&lookup).unwrap();
        *invalid.pointer_mut(pointer).unwrap() = value;
        let invalid: HostRegistrationLookup = serde_json::from_value(invalid).unwrap();
        assert_eq!(
            f.journal
                .acknowledge_host_registration(
                    &f.intent.entry_id,
                    &f.receipt.host_id,
                    f.receipt.created_sequence,
                    |_| Ok(invalid),
                )
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn acknowledgement_compares_all_captured_fields_after_the_lookup() {
    let mut f = CommittedRegistration::new();
    let path = f.journal_path();
    let mut edited = f.intent.clone();
    edited.created_at = "2000-02-29T01:02:03Z".into();
    assert_ne!(edited, f.intent);
    let bytes = serde_json::to_vec(&json!({"version":1,"entries":[edited]})).unwrap();
    assert!(!f
        .journal
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |key| {
                let lookup = read_registration(&mut f.store, key)?;
                write_private(&path, &bytes);
                Ok(lookup)
            },
        )
        .unwrap());
    assert_eq!(f.journal.pending_host_registration().unwrap(), Some(edited));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn a_concurrent_acknowledgement_and_reservation_cannot_lose_the_new_intent() {
    let mut f = CommittedRegistration::new();
    let other = ClientRecoveryJournal::new(f.directory.path()).unwrap();
    let store_path = f.directory.path().join("registry.sqlite3");
    let mut replacement = None;
    assert!(!f
        .journal
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |key| {
                let lookup = read_registration(&mut f.store, key)?;
                replacement = Some(thread::scope(|scope| {
                    scope
                        .spawn(|| {
                            let mut store = crate::storage::Store::open(&store_path).unwrap();
                            assert!(other
                                .acknowledge_host_registration(
                                    &f.intent.entry_id,
                                    &f.receipt.host_id,
                                    f.receipt.created_sequence,
                                    |key| { read_registration(&mut store, key) },
                                )
                                .unwrap());
                            other
                                .reserve_host_registration(
                                    REQUEST_KEY.into(),
                                    f.intent.registration.clone(),
                                )
                                .unwrap()
                        })
                        .join()
                        .unwrap()
                }));
                Ok(lookup)
            },
        )
        .unwrap());
    let replacement = replacement.unwrap();
    assert_ne!(replacement.entry_id, f.intent.entry_id);
    assert_eq!(replacement.request_key, f.intent.request_key);
    assert_eq!(replacement.registration, f.intent.registration);
    let bytes = fs::read(f.journal_path()).unwrap();
    assert!(!other
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |_| panic!("a stale entry ID must not perform an external lookup"),
        )
        .unwrap());
    assert_eq!(
        f.journal.pending_host_registration().unwrap(),
        Some(replacement)
    );
    assert_eq!(fs::read(f.journal_path()).unwrap(), bytes);
}

#[test]
fn acknowledgement_reports_lock_contention_after_lookup_and_keeps_intent() {
    let mut f = CommittedRegistration::new();
    let path = f.journal_path();
    let bytes = fs::read(&path).unwrap();
    let mut held = None;
    let error = f
        .journal
        .acknowledge_host_registration(
            &f.intent.entry_id,
            &f.receipt.host_id,
            f.receipt.created_sequence,
            |key| {
                let lookup = read_registration(&mut f.store, key)?;
                held = Some(f.journal.lock_file(HOST_LOCK)?);
                Ok(lookup)
            },
        )
        .unwrap_err();
    assert!(held.is_some());
    assert_eq!(error.code, BUSY);
    assert_eq!(error.side_effect_state, "not_started");
    drop(held);
    assert_eq!(
        f.journal.pending_host_registration().unwrap(),
        Some(f.intent)
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}
