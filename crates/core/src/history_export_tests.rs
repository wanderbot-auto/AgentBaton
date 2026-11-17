use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    store: Store,
    project: String,
    task: String,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&root.join("db.sqlite")).unwrap();
        std::fs::create_dir(root.join("source")).unwrap();
        let project = store
            .import_project(&root.join("source"), "Export 中文", "project-key")
            .unwrap()
            .id;
        let task = store
            .create_registered(&project, "goal sk-test-known-credential", "task-key")
            .unwrap()
            .id;
        Self {
            _temp: temp,
            root,
            store,
            project,
            task,
        }
    }
    fn directory(&self) -> PathBuf {
        self.root.join("history-exports")
    }
    fn selection(&self) -> Selection {
        Selection {
            version: 1,
            project_id: self.project.clone(),
            from_sequence: 0,
            to_sequence: None,
            tasks: true,
            messages: true,
            events: true,
        }
    }
    fn event(&self, aggregate: &str, payload: Value) {
        self.store.conn.execute("INSERT INTO events(document) VALUES(?1)",[json!({"event_id":Uuid::new_v4().to_string(),"aggregate_id":aggregate,"kind":"task.update","revision":1,"occurred_at":"2026-11-17T00:00:00Z","source":"fixture","payload":payload}).to_string()]).unwrap();
    }
    fn preview(&self) -> Preview {
        self.store
            .history_export_preview(&self.directory(), &self.selection())
            .unwrap()
    }
}
#[test]
fn package_hashes_redaction_and_project_ownership() {
    let f = Fixture::new();
    f.event(&f.task,json!({"text":"Authorization: Bearer KNOWN-SECRET","api_key":"KNOWN-API","raw":{"password":"KNOWN-PASSWORD"}}));
    f.event(
        &f.task,
        json!({"update":{"kind":"agent_text","text":"已有正文 hello","raw":{"auth":"KNOWN-AUTH"}}}),
    );
    f.event(&f.task,json!({"update":{"kind":"tool_call","tool_call_id":"tool-1","arguments":{"authorization":"KNOWN-TOOL"}}}));
    f.event(
        "other-project-task",
        json!({"text":"CROSS-PROJECT-CONTENT"}),
    );
    f.event(
        &f.task,
        json!({"project_id":"other-project","text":"CROSS-PROJECT-MIRROR"}),
    );
    let before = f.store.sequence().unwrap();
    let p = f.preview();
    assert!(p.redacted_fields >= 2);
    assert_eq!(p.excluded_events, 3); // includes global project import event
    let receipt = publish(&f.directory(), &p.reference).unwrap();
    assert_eq!(receipt.state, "ready");
    let bytes = std::fs::read(published_path(&f.directory(), &p.reference).unwrap()).unwrap();
    assert_eq!(sha(&bytes), p.reference.package_sha256);
    let text = String::from_utf8(bytes.clone()).unwrap();
    for secret in [
        "KNOWN-SECRET",
        "KNOWN-API",
        "KNOWN-PASSWORD",
        "KNOWN-AUTH",
        "KNOWN-TOOL",
        "sk-test-known-credential",
        "CROSS-PROJECT-CONTENT",
        "CROSS-PROJECT-MIRROR",
    ] {
        assert!(!text.contains(secret), "leaked {secret}");
    }
    assert!(text.contains("已有正文 hello"));
    assert!(text.contains("tool-1"));
    let package: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(package["manifest"]["restorable"], false);
    for file in package["files"].as_object().unwrap().values() {
        let content = file["content"].as_str().unwrap();
        assert_eq!(file["sha256"], sha(content.as_bytes()));
    }
    assert_eq!(f.store.sequence().unwrap(), before);
    assert!(f
        .store
        .conn
        .query_row("SELECT document FROM tasks WHERE id=?1", [&f.task], |r| {
            r.get::<_, String>(0)
        })
        .is_ok());
}
#[test]
fn immutable_snapshot_restart_reconcile_and_idempotent_publication() {
    let mut f = Fixture::new();
    let p = f.preview();
    f.event(&f.task, json!({"text":"AFTER-SNAPSHOT"}));
    f.store
        .create_registered(&f.project, "LATER-TASK", "later")
        .unwrap();
    let r = publish(&f.directory(), &p.reference).unwrap();
    let path = published_path(&f.directory(), &p.reference).unwrap();
    let inode = std::fs::metadata(&path).unwrap().ino();
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("AFTER-SNAPSHOT"));
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("LATER-TASK"));
    drop(f.store);
    f.store = Store::open(&f.root.join("db.sqlite")).unwrap();
    assert_eq!(get(&f.directory(), &p.reference).unwrap().state, "ready");
    assert_eq!(
        publish(&f.directory(), &p.reference).unwrap().filename,
        r.filename
    );
    assert_eq!(std::fs::metadata(path).unwrap().ino(), inode);
}
#[test]
fn selection_tamper_range_categories_and_empty_records() {
    let f = Fixture::new();
    let mut s = f.selection();
    s.from_sequence = f.store.sequence().unwrap();
    s.tasks = false;
    let p = f.store.history_export_preview(&f.directory(), &s).unwrap();
    assert!(p.counts.values().all(|n| *n == 0));
    let mut wrong = p.reference.clone();
    wrong.selection_digest = "a".repeat(64);
    assert_eq!(
        publish(&f.directory(), &wrong).unwrap_err().code,
        "INPUT_CHANGED"
    );
    s.to_sequence = Some(s.from_sequence - 1);
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &s)
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    s.to_sequence = Some(u64::MAX);
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &s)
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    s.to_sequence = None;
    s.events = false;
    s.messages = false;
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &s)
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    let mut refpath = p.reference;
    refpath.preview_id = "../escape".into();
    assert_eq!(
        get(&f.directory(), &refpath).unwrap_err().code,
        "INVALID_REQUEST"
    );
}
#[test]
fn symlinks_permissions_conflict_corruption_and_interruption_fail_closed() {
    let f = Fixture::new();
    let p = f.preview();
    let root = f.directory();
    let result = root.join(format!("{}.history.json", p.reference.preview_id));
    let outside = f.root.join("outside");
    std::fs::write(&outside, "DO NOT OVERWRITE").unwrap();
    symlink(&outside, &result).unwrap();
    assert!(publish(&root, &p.reference).is_err());
    assert_eq!(
        std::fs::read_to_string(&outside).unwrap(),
        "DO NOT OVERWRITE"
    );
    std::fs::remove_file(&result).unwrap();
    std::fs::write(&result, "conflict").unwrap();
    std::fs::set_permissions(&result, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        publish(&root, &p.reference).unwrap_err().code,
        "INPUT_CHANGED"
    );
    assert_eq!(std::fs::read_to_string(&result).unwrap(), "conflict");
    std::fs::remove_file(&result).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        publish(&root, &p.reference).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let snapshot = root.join(format!("{}.snapshot", p.reference.preview_id));
    std::fs::set_permissions(&snapshot, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        get(&root, &p.reference).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    std::fs::set_permissions(&snapshot, std::fs::Permissions::from_mode(0o600)).unwrap();
    let partial = Reference {
        preview_id: Uuid::new_v4().to_string(),
        ..p.reference.clone()
    };
    std::fs::write(
        root.join(format!("{}.snapshot.writing", partial.preview_id)),
        "partial",
    )
    .unwrap();
    assert_eq!(get(&root, &partial).unwrap_err().code, "NOT_FOUND");
    let link = f.root.join("link");
    symlink(&root, &link).unwrap();
    assert!(get(&link, &p.reference).is_err());
    let missing = f.root.join("nonexistent-parent").join("export");
    assert!(f
        .store
        .history_export_preview(&missing, &f.selection())
        .is_err());
}
#[test]
fn row_byte_and_retained_preview_limits_never_truncate() {
    let f = Fixture::new();
    f.event(&f.task, json!({"text":"x".repeat(262145)}));
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &f.selection())
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert!(!f.directory().exists());
    let f = Fixture::new();
    let document = json!({"project_id":"unowned"}).to_string();
    let tx = f.store.conn.unchecked_transaction().unwrap();
    for n in 0..10001 {
        tx.execute(
            "INSERT INTO tasks(id,document) VALUES(?1,?2)",
            rusqlite::params![format!("other-{n}"), document],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &f.selection())
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    let f = Fixture::new();
    let root = f.directory();
    ExportDirectory::open(&root).unwrap();
    for n in 0..64 {
        std::fs::write(root.join(format!("retained-{n}")), "").unwrap();
    }
    assert_eq!(
        f.store
            .history_export_preview(&root, &f.selection())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
}
#[test]
fn authentication_and_arbitrary_fields_are_not_accepted() {
    assert!(serde_json::from_value::<Selection>(json!({"version":1,"project_id":"p","from_sequence":0,"to_sequence":null,"tasks":true,"messages":false,"events":false,"destination":"/tmp/leak"})).is_err());
    assert!(serde_json::from_value::<Reference>(
        json!({"preview_id":"id","selection_digest":"x","package_sha256":"x","approved":true})
    )
    .is_err());
}

#[test]
fn publication_interrupted_after_link_reconciles_without_overwrite() {
    let f = Fixture::new();
    let p = f.preview();
    FAIL_AFTER_LINK.with(|flag| flag.set(true));
    let error = publish(&f.directory(), &p.reference).unwrap_err();
    assert_eq!(error.side_effect_state, "unknown");
    let r = get(&f.directory(), &p.reference).unwrap();
    assert_eq!(r.state, "ready");
    let inode = std::fs::metadata(published_path(&f.directory(), &p.reference).unwrap())
        .unwrap()
        .ino();
    publish(&f.directory(), &p.reference).unwrap();
    assert_eq!(
        std::fs::metadata(published_path(&f.directory(), &p.reference).unwrap())
            .unwrap()
            .ino(),
        inode
    );
}
#[test]
fn metadata_only_avoids_unselected_journal_body() {
    let f = Fixture::new();
    f.event(&f.task, json!({"text":"x".repeat(262145)}));
    let mut s = f.selection();
    s.messages = false;
    s.events = false;
    let p = f.store.history_export_preview(&f.directory(), &s).unwrap();
    assert_eq!(p.counts["tasks.json"], 1);
}
#[test]
fn cross_entity_ambiguous_aggregate_is_excluded_and_session_task_mirror_checked() {
    let f = Fixture::new();
    f.store
        .conn
        .execute_batch("PRAGMA foreign_keys=OFF")
        .unwrap();
    let document = json!({"id":f.task,"project_id":"another-project"});
    f.store.conn.execute("INSERT INTO conversations(id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload) VALUES(?1,'another-project','w','c','g','g','stopped',?2,'k','{}')",rusqlite::params![f.task,document.to_string()]).unwrap();
    f.event(&f.task, json!({"text":"AMBIGUOUS-EVENT"}));
    let p = f.preview();
    let r = publish(&f.directory(), &p.reference).unwrap();
    assert!(r.preview.excluded_events >= 2);
    assert!(
        !std::fs::read_to_string(published_path(&f.directory(), &p.reference).unwrap())
            .unwrap()
            .contains("AMBIGUOUS-EVENT")
    );
    let doc = json!({"id":"s","project_id":f.project,"task_id":f.task});
    f.store.conn.execute("INSERT INTO sessions(id,task_id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload) VALUES('s','wrong-task',?1,'w','c','g','g','stopped',?2,'k','{}')",rusqlite::params![f.project,doc.to_string()]).unwrap();
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &f.selection())
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}

#[test]
fn stable_preview_intent_recovers_before_live_capture_and_full_capacity() {
    let f = Fixture::new();
    let selection = f.selection();
    let intent = Uuid::new_v4().to_string();
    let first = f
        .store
        .capture_history_export(&selection, &intent, Instant::now() + Duration::from_secs(2))
        .unwrap()
        .persist(&f.directory())
        .unwrap();
    let original = std::fs::read(f.directory().join(format!("{intent}.snapshot"))).unwrap();
    f.event(&f.task, json!({"text":"LATER".repeat(60000)})); // current capture now fails single-record bound
    assert_eq!(
        f.store
            .capture_history_export(
                &selection,
                &Uuid::new_v4().to_string(),
                Instant::now() + Duration::from_secs(2)
            )
            .err()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
    let directory = ExportDirectory::open(&f.directory()).unwrap();
    for _ in 0..31 {
        directory
            .write_new(&format!("{}.snapshot", Uuid::new_v4()), b"{}")
            .unwrap();
    }
    let again = existing_preview(&f.directory(), &selection, &intent)
        .unwrap()
        .unwrap();
    assert_eq!(again.reference, first.reference);
    assert_eq!(again.snapshot_sequence, first.snapshot_sequence);
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 32);
    assert_eq!(
        existing_preview(&f.directory(), &selection, &Uuid::new_v4().to_string())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
    let mut changed = selection.clone();
    changed.messages = false;
    assert_eq!(
        existing_preview(&f.directory(), &changed, &intent)
            .unwrap_err()
            .code,
        "IDEMPOTENCY_CONFLICT"
    );
    assert_eq!(
        std::fs::read(f.directory().join(format!("{intent}.snapshot"))).unwrap(),
        original
    );
    // Exercise the actual two-stage worker interface: an existing result must not
    // reach Actor capture against the now-oversize live record.
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut workers = Workers::default();
    workers.start(
        f.directory(),
        Operation::Lookup(selection, intent),
        "retry".into(),
        0,
        crate::ipc::ReadyRequest {
            raw: String::new(),
            reply: crate::ipc::ReadyReply::Wire(sender),
            deadline: Instant::now() + Duration::from_secs(5),
            factory_dispatch: None,
        },
    );
    let until = Instant::now() + Duration::from_secs(5);
    let response = loop {
        workers.poll(&f.store);
        if let Ok(response) = receiver.try_recv() {
            break response;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    workers.join();
    assert_eq!(
        response["result"]["package_sha256"],
        first.reference.package_sha256
    );
}
#[test]
fn thirty_two_intents_reserve_all_publication_slots_and_keep_existing_recoverable() {
    let f = Fixture::new();
    let selection = f.selection();
    let mut previews = Vec::new();
    for _ in 0..32 {
        let key = Uuid::new_v4().to_string();
        previews.push(
            f.store
                .capture_history_export(&selection, &key, Instant::now() + Duration::from_secs(2))
                .unwrap()
                .persist(&f.directory())
                .unwrap(),
        );
    }
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 32);
    for p in &previews {
        publish(&f.directory(), &p.reference).unwrap();
    }
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 64);
    assert_eq!(
        existing_preview(&f.directory(), &selection, &Uuid::new_v4().to_string())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
    assert_eq!(
        existing_preview(
            &f.directory(),
            &selection,
            &previews[0].reference.preview_id
        )
        .unwrap()
        .unwrap()
        .reference,
        previews[0].reference
    );
    // A retained interrupted-unlink name is the same inode, not another payload.
    for p in &previews {
        std::fs::hard_link(
            f.directory()
                .join(format!("{}.snapshot", p.reference.preview_id)),
            f.directory()
                .join(format!("{}.snapshot.writing", p.reference.preview_id)),
        )
        .unwrap();
    }
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 96);
    assert_eq!(
        existing_preview(&f.directory(), &selection, &Uuid::new_v4().to_string())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
    assert!(existing_preview(
        &f.directory(),
        &selection,
        &previews[0].reference.preview_id
    )
    .unwrap()
    .is_some());
}
#[test]
fn interrupted_and_corrupt_intents_are_never_recaptured_or_silently_replaced() {
    let f = Fixture::new();
    let selection = f.selection();
    let key = Uuid::new_v4().to_string();
    let directory = ExportDirectory::open(&f.directory()).unwrap();
    directory
        .write_new(&format!("{key}.snapshot.writing"), b"partial")
        .unwrap();
    assert_eq!(
        existing_preview(&f.directory(), &selection, &key)
            .unwrap_err()
            .code,
        "PREVIEW_INCOMPLETE"
    );
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 1);
    let preview = f.preview();
    let path = f
        .directory()
        .join(format!("{}.snapshot", preview.reference.preview_id));
    std::fs::write(&path, b"{").unwrap();
    assert_eq!(
        existing_preview(&f.directory(), &selection, &preview.reference.preview_id)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert_eq!(std::fs::read(path).unwrap(), b"{");
}

#[test]
fn unsafe_javascript_sequence_and_expired_capture_fail_without_output() {
    let f = Fixture::new();
    let selection = f.selection();
    let key = Uuid::new_v4().to_string();
    assert_eq!(
        f.store
            .capture_history_export(&selection, &key, Instant::now() - Duration::from_millis(1))
            .err()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
    f.store
        .conn
        .execute(
            "INSERT INTO events(sequence,document) VALUES(9007199254740992,'{}')",
            [],
        )
        .unwrap();
    assert_eq!(
        f.store
            .history_export_preview(&f.directory(), &selection)
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert!(!f.directory().exists());
}

#[test]
fn oversize_stored_intent_is_corruption_not_a_retriable_capture_limit() {
    let f = Fixture::new();
    let preview = f.preview();
    let path = f
        .directory()
        .join(format!("{}.snapshot", preview.reference.preview_id));
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len((MAX_PACKAGE + 1) as u64)
        .unwrap();
    assert_eq!(
        existing_preview(
            &f.directory(),
            &f.selection(),
            &preview.reference.preview_id
        )
        .unwrap_err()
        .code,
        "INPUT_CHANGED"
    );
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 1);
}

// Linux filesystems permit names that macOS APFS does not necessarily accept.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_unknown_names_consume_distinct_reserved_capacity_without_lossy_aliases() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    ExportDirectory::open(&f.directory()).unwrap();
    for byte in 0x80..0xa0 {
        std::fs::write(
            f.directory()
                .join(std::ffi::OsString::from_vec(vec![b'x', byte])),
            b"unfamiliar",
        )
        .unwrap();
    }
    assert_eq!(
        existing_preview(&f.directory(), &f.selection(), &Uuid::new_v4().to_string())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 32);
}

#[test]
fn unfamiliar_uuid_filename_does_not_alias_a_generated_intent_slot() {
    let f = Fixture::new();
    let preview = f.preview();
    std::fs::write(
        f.directory().join(&preview.reference.preview_id),
        b"unfamiliar",
    )
    .unwrap();
    for n in 0..30 {
        std::fs::write(f.directory().join(format!("unfamiliar-{n}")), b"keep").unwrap();
    }
    assert_eq!(
        existing_preview(&f.directory(), &f.selection(), &Uuid::new_v4().to_string())
            .unwrap_err()
            .code,
        "EXPORT_CAPACITY"
    );
    assert_eq!(std::fs::read_dir(f.directory()).unwrap().count(), 32);
    assert!(existing_preview(
        &f.directory(),
        &f.selection(),
        &preview.reference.preview_id
    )
    .unwrap()
    .is_some());
}
