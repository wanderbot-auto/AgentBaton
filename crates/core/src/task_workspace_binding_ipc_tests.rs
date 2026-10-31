use super::*;
use agentbaton_workspaces::{data_workspace::DataLimits, validation::ArtifactRequirement};

struct OwnedDaemon(Daemon);
impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        self.0.data_workers.reap_owned_workers();
    }
}
type TableImage = Vec<(String, Vec<Vec<rusqlite::types::Value>>)>;
fn table_image(db: &rusqlite::Connection) -> TableImage {
    let mut stmt=db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_stat%' ORDER BY name").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut stmt = db.prepare(&format!("SELECT * FROM {name}")).unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get::<_, rusqlite::types::Value>(i))
                        .collect::<std::result::Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}

#[test]
fn six_body_fields_map_only_envelope_task_cas_and_reject_numeric_or_extra_identity() {
    let raw = json!({"task_id":"task","expected_spec_revision":2,"expected_project_revision":3,"host_id":"local","expected_source_pin":{"canonical_root":"/source","device":"9007199254740993","inode":"18446744073709551615","common_dir":"/source/.git","head":"a".repeat(40)},"preparation_key":"code-prep:".to_owned()+&"b".repeat(64)});
    let p: task_binding_wire::PrepareTaskGit = serde_json::from_value(raw.clone()).unwrap();
    let request = p.with_task_revision(7);
    assert_eq!(request.expected_task_revision, 7);
    assert_eq!(request.expected_source_pin.device, 9_007_199_254_740_993);
    assert_eq!(request.expected_source_pin.inode, u64::MAX);
    for field in [
        "expected_task_revision",
        "path",
        "managed_root",
        "destination",
        "grant_id",
    ] {
        let mut bad = raw.clone();
        bad[field] = json!(1);
        assert!(serde_json::from_value::<task_binding_wire::PrepareTaskGit>(bad).is_err());
    }
    for value in [
        json!(1),
        json!(1.0),
        json!(true),
        json!("01"),
        json!("18446744073709551616"),
    ] {
        let mut bad = raw.clone();
        bad["expected_source_pin"]["device"] = value;
        assert!(serde_json::from_value::<task_binding_wire::PrepareTaskGit>(bad).is_err());
    }
}

#[test]
fn real_owned_data_io_pending_reap_blocks_new_preparation_but_not_readonly_task_status() {
    use std::os::unix::fs::PermissionsExt;
    // Darwin's default temporary parent can contain /var -> /private/var.
    // Create a short owned fixture beneath the canonical /tmp parent instead;
    // production runtime symlink refusal remains unchanged.
    let parent = std::path::Path::new("/tmp").canonicalize().unwrap();
    let root = tempfile::Builder::new()
        .prefix("ab-twb-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(&parent)
        .unwrap();
    assert_eq!(root.path().parent(), Some(parent.as_path()));
    assert_eq!(root.path().canonicalize().unwrap().as_path(), root.path());
    assert_eq!(
        std::fs::symlink_metadata(root.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    let source = root.path().join("data");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("input"), "bounded actual data\n").unwrap();
    let mut owned = OwnedDaemon(Daemon::bind(&root.path().join("runtime")).unwrap());
    let d = &mut owned.0;
    let project = d
        .store
        .import_project(&source, "Generated data", "project")
        .unwrap();
    let task = d
        .store
        .create_registered(&project.id, "Read-only status control", "task")
        .unwrap();
    let selection = crate::data_workspaces::DataWorkspaceSelection {
        project_id: project.id.clone(),
        files: vec![ArtifactRequirement {
            artifact_id: "input".into(),
            relative_path: "input".into(),
            max_bytes: 64,
            expected_sha256: None,
        }],
        limits: DataLimits {
            max_file_bytes: 64,
            max_total_bytes: 64,
            max_files: 1,
        },
    };
    let preview = d
        .data_workers
        .preview(&mut d.store, &selection, project.revision, "preview")
        .unwrap();
    assert_eq!(preview.state, "checking");
    assert!(d.data_workers.has_workers());
    // Do not tick the owner yet: even a completed I/O thread remains a real owned,
    // unreaped worker. No fabricated Supervisor/session or stop receipt is inserted.
    let before = table_image(&d.store.conn);
    let frame = json!({"api_version":1,"request_id":"busy-preparation","token":d.token,"method":"workspaces.prepare_task_git","params":{"task_id":task.id,"expected_spec_revision":task.spec_revision,"expected_project_revision":project.revision,"host_id":"local","expected_source_pin":{"canonical_root":source,"device":project.identity.device.to_string(),"inode":project.identity.inode.to_string(),"common_dir":source.join(".git"),"head":"a".repeat(40)},"preparation_key":"code-prep:".to_owned()+&"b".repeat(64)},"expected_revision":task.revision,"idempotency_key":"prepare"});
    let refused = d.handle(&frame.to_string());
    assert_eq!(refused["error"]["code"], "WORKSPACE_BUSY");
    assert_eq!(refused["error"]["side_effect_state"], "not_started");
    let status=d.handle(&json!({"api_version":1,"request_id":"read-while-busy","token":d.token,"method":"tasks.get","params":{"task_id":task.id}}).to_string());
    assert_eq!(status["result"]["id"], task.id);
    assert!(before == table_image(&d.store.conn));
    d.data_workers.shutdown(&mut d.store).unwrap();
    assert!(!d.data_workers.has_workers());
}
