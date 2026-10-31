//! Authored tests. The implementation author does not execute Rust/SQL here.
use super::*;
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
};

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn selection(project: &Project) -> WorkspaceOpenSelection {
    WorkspaceOpenSelection::ProjectSource {
        project_id: project.id.clone(),
        expected_project_revision: project.revision,
    }
}
fn context(path: &Path) -> WorkspaceOpenContext {
    let pin = WorkspaceOpenDirectoryPin::capture(path).unwrap();
    WorkspaceOpenContext {
        selection: WorkspaceOpenSelection::ProjectSource {
            project_id: id(),
            expected_project_revision: 1,
        },
        canonical_path: path.to_str().unwrap().into(),
        directory_identity: pin.identities,
    }
}
fn identity(path: &Path, kind: WorkspaceKind) -> WorkspaceIdentity {
    let meta = fs::metadata(path).unwrap();
    WorkspaceIdentity {
        canonical_root: path.into(),
        device: meta.dev(),
        inode: meta.ino(),
        kind,
    }
}
fn store() -> (tempfile::TempDir, Store, Project) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("source");
    fs::create_dir(&path).unwrap();
    let mut store = Store::open(&root.path().join("db.sqlite3")).unwrap();
    let project = store
        .import_project(&path, "Directory fixture", "project")
        .unwrap();
    (root, store, project)
}
fn save_workspace(store: &Store, workspace: &ManagedWorkspace) {
    store.conn.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,payload,root) VALUES(?1,?2,?3,?4,?1,'{}',?5) ON CONFLICT(id) DO UPDATE SET document=excluded.document,state=excluded.state,root=excluded.root", rusqlite::params![workspace.id,workspace.project_id,serde_json::to_string(workspace).unwrap(),workspace.state,workspace.root.to_str().unwrap()]).unwrap();
}
fn managed(project: &Project, path: &Path, kind: WorkspaceKind) -> ManagedWorkspace {
    ManagedWorkspace {
        id: id(),
        project_id: project.id.clone(),
        identity: Some(identity(path, kind)),
        root: path.into(),
        base_revision: "fixture-no-execution".into(),
        created_at: project.created_at.clone(),
        state: "ready".into(),
        revision: 1,
    }
}
fn managed_selection(project: &Project, workspace: &ManagedWorkspace) -> WorkspaceOpenSelection {
    WorkspaceOpenSelection::ManagedWorkspace {
        project_id: project.id.clone(),
        expected_project_revision: project.revision,
        workspace_id: workspace.id.clone(),
        expected_workspace_revision: workspace.revision,
    }
}
fn error<T>(value: Result<T>, code: &str) {
    match value {
        Err(error) => {
            assert_eq!(error.code, code);
            assert_eq!(error.side_effect_state, "not_started");
        }
        Ok(_) => panic!("expected a directory refusal"),
    }
}

#[test]
fn selection_and_envelope_reject_extra_remote_paths_noncanonical_ids_and_noninteger_revisions() {
    let request = json!({"selection":{"kind":"project_source","project_id":id(),"expected_project_revision":1}});
    let parsed: WorkspaceOpenRequest = serde_json::from_value(request.clone()).unwrap();
    parsed.selection.validate().unwrap();
    for (field, value) in [
        ("path", json!("/untrusted")),
        ("host_id", json!("remote")),
        ("command", json!("sh")),
        ("workspace_id", json!(id())),
    ] {
        let mut raw = request.clone();
        raw["selection"][field] = value;
        assert!(serde_json::from_value::<WorkspaceOpenRequest>(raw).is_err());
    }
    for value in [
        json!(0),
        json!(-1),
        json!(1.0),
        json!("1"),
        json!(null),
        json!(true),
    ] {
        let mut raw = request.clone();
        raw["selection"]["expected_project_revision"] = value;
        assert!(serde_json::from_value::<WorkspaceOpenRequest>(raw).is_err());
    }
    for value in [
        "local",
        "",
        "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
        "urn:uuid:00000000-0000-0000-0000-000000000000",
        "00000000000000000000000000000000",
    ] {
        let mut raw = request.clone();
        raw["selection"]["project_id"] = json!(value);
        assert!(serde_json::from_value::<WorkspaceOpenRequest>(raw).is_err());
    }
    let mut raw = request.clone();
    raw["terminal"] = json!("macos_terminal");
    assert!(serde_json::from_value::<WorkspaceOpenRequest>(raw).is_err());
    let mut raw = request;
    raw["selection"]["expected_project_revision"] = json!(u64::MAX);
    assert!(serde_json::from_value::<WorkspaceOpenRequest>(raw).is_ok());
    assert!(serde_json::from_str::<WorkspaceOpenRequest>(&format!("{{\"selection\":{{\"kind\":\"project_source\",\"project_id\":\"{}\",\"expected_project_revision\":1,\"expected_project_revision\":2}}}}",id())).is_err());
}

#[test]
fn identity_wire_is_canonical_text_and_preserves_values_above_javascript_precision() {
    let raw = json!({"device":"9007199254740993","inode":"18446744073709551615"});
    let parsed: WorkspaceDirectoryIdentity = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), raw);
    for value in [
        json!(1),
        json!("01"),
        json!("+1"),
        json!("-1"),
        json!("18446744073709551616"),
        json!("1.0"),
        json!(""),
    ] {
        assert!(serde_json::from_value::<WorkspaceDirectoryIdentity>(
            json!({"device":value,"inode":"1"})
        )
        .is_err());
    }
    assert!(serde_json::from_value::<WorkspaceDirectoryIdentity>(
        json!({"device":"1","inode":"1","path":"/extra"})
    )
    .is_err());
}

#[test]
fn managed_selection_requires_both_exact_revisions_and_no_extra_fields() {
    let original = json!({"kind":"managed_workspace","project_id":id(),"expected_project_revision":1,"workspace_id":id(),"expected_workspace_revision":2});
    let selection: WorkspaceOpenSelection = serde_json::from_value(original.clone()).unwrap();
    selection.validate().unwrap();
    for field in ["expected_project_revision", "expected_workspace_revision"] {
        for value in [json!(0), json!(1.0), json!("1"), json!(null)] {
            let mut raw = original.clone();
            raw[field] = value;
            assert!(serde_json::from_value::<WorkspaceOpenSelection>(raw).is_err());
        }
    }
    for field in ["project_id", "workspace_id"] {
        let mut raw = original.clone();
        raw[field] = json!("unregistered-path");
        assert!(serde_json::from_value::<WorkspaceOpenSelection>(raw).is_err());
    }
    let mut raw = original.clone();
    raw["identity"] = json!({"device":"1","inode":"2"});
    assert!(serde_json::from_value::<WorkspaceOpenSelection>(raw).is_err());
    let mut raw = original;
    raw.as_object_mut().unwrap().remove("workspace_id");
    assert!(serde_json::from_value::<WorkspaceOpenSelection>(raw).is_err());
}

#[test]
fn path_parser_rejects_alias_spellings_and_enforces_root_to_leaf_bound_without_io() {
    for path in ["relative", "/a/../b", "/a/./b", "/a//b", "/a/b/"] {
        assert!(components(Path::new(path)).is_err());
    }
    assert!(components(Path::new("/")).unwrap().is_empty());
    let maximum = format!("/{}", vec!["n"; 128].join("/"));
    assert_eq!(components(Path::new(&maximum)).unwrap().len() + 1, 129);
    error(
        components(Path::new(&(maximum + "/extra"))),
        "RESOURCE_LIMIT",
    );
}

#[test]
fn full_chain_pin_accepts_directory_names_as_data_and_rejects_wrong_context_facts() {
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .canonicalize()
        .unwrap()
        .join("- Unicode 空间\nfolder");
    fs::create_dir(&path).unwrap();
    let original = context(&path);
    let held = pin_open_context(&original).unwrap();
    held.recheck().unwrap();
    assert_eq!(
        original.directory_identity.len(),
        components(&path).unwrap().len() + 1
    );
    for index in [0, original.directory_identity.len() - 1] {
        let mut changed = original.clone();
        changed.directory_identity[index].inode = "0".into();
        error(pin_open_context(&changed), "INPUT_CHANGED");
    }
    let mut changed = original.clone();
    changed.directory_identity.pop();
    error(pin_open_context(&changed), "INPUT_CHANGED");
    let mut changed = original.clone();
    changed.directory_identity[0].device = "01".into();
    error(pin_open_context(&changed), "INVALID_REQUEST");
    let mut raw = serde_json::to_value(original).unwrap();
    raw["authority"] = json!(true);
    assert!(serde_json::from_value::<WorkspaceOpenContext>(raw).is_err());
}

#[test]
fn ancestor_replacement_is_detected_even_when_the_original_leaf_inode_is_preserved() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let ancestor = root.join("parent");
    let leaf = ancestor.join("leaf");
    fs::create_dir_all(&leaf).unwrap();
    let original = context(&leaf);
    let pin = pin_open_context(&original).unwrap();
    fs::rename(&ancestor, root.join("old-parent")).unwrap();
    fs::create_dir(&ancestor).unwrap();
    fs::rename(root.join("old-parent/leaf"), &leaf).unwrap();
    let leaf_now = fs::metadata(&leaf).unwrap();
    assert_eq!(
        leaf_now.ino().to_string(),
        original.directory_identity.last().unwrap().inode
    );
    error(pin.recheck(), "INPUT_CHANGED");
    error(pin_open_context(&original), "INPUT_CHANGED");
}

#[test]
fn leaf_and_ancestor_symlinks_and_leaf_replacement_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let path = root.join("directory");
    fs::create_dir(&path).unwrap();
    let original = context(&path);
    let pin = pin_open_context(&original).unwrap();
    fs::rename(&path, root.join("old-directory")).unwrap();
    symlink(root.join("old-directory"), &path).unwrap();
    error(pin.recheck(), "INPUT_CHANGED");
    error(pin_open_context(&original), "INPUT_CHANGED");
    let alias = root.join("alias");
    symlink(&root, &alias).unwrap();
    error(
        WorkspaceOpenDirectoryPin::capture(&alias.join("old-directory")),
        "INPUT_CHANGED",
    );
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    error(pin.recheck(), "INPUT_CHANGED");
}

#[test]
fn store_reads_registered_source_without_writes_and_detects_stale_or_replaced_registration() {
    let (_root, store, project) = store();
    let selected = selection(&project);
    let changes = store.conn.total_changes();
    let sequence = store.sequence().unwrap();
    let observed = store.workspace_open_context(&selected).unwrap();
    pin_open_context(&observed).unwrap().recheck().unwrap();
    assert_eq!(observed.selection, selected);
    assert_eq!(
        observed.canonical_path,
        project.identity.canonical_root.to_str().unwrap()
    );
    assert_eq!(store.conn.total_changes(), changes);
    assert_eq!(store.sequence().unwrap(), sequence);
    error(
        store.workspace_open_context(&WorkspaceOpenSelection::ProjectSource {
            project_id: project.id.clone(),
            expected_project_revision: 2,
        }),
        "STALE_REVISION",
    );
    store
        .conn
        .execute("UPDATE projects SET inode='0' WHERE id=?1", [&project.id])
        .unwrap();
    error(store.workspace_open_context(&selected), "INPUT_CHANGED");
    store
        .conn
        .execute(
            "UPDATE projects SET inode=?1 WHERE id=?2",
            rusqlite::params![project.identity.inode.to_string(), project.id],
        )
        .unwrap();
    fs::rename(
        &project.identity.canonical_root,
        project.identity.canonical_root.with_file_name("old-source"),
    )
    .unwrap();
    fs::create_dir(&project.identity.canonical_root).unwrap();
    error(store.workspace_open_context(&selected), "INPUT_CHANGED");
}

#[test]
fn ready_git_and_data_records_open_without_inspecting_head_or_assigning_tasks() {
    let (root, store, project) = store();
    for (name, kind) in [
        (
            "git",
            WorkspaceKind::Git {
                common_dir: root.path().join("not-executed-git"),
                head: "a".repeat(40),
                dirty: true,
            },
        ),
        ("data", WorkspaceKind::Directory),
    ] {
        let path = root.path().canonicalize().unwrap().join(name);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("dirty or data content"), b"local edit").unwrap();
        // Typed registry fixture, not a claim that data/Git creation ran here.
        let workspace = managed(&project, &path, kind);
        save_workspace(&store, &workspace);
        let before = store.conn.total_changes();
        let sequence = store.sequence().unwrap();
        let selected = managed_selection(&project, &workspace);
        let observed = store.workspace_open_context(&selected).unwrap();
        assert_eq!(observed.selection, selected);
        pin_open_context(&observed).unwrap().recheck().unwrap();
        assert_eq!(store.conn.total_changes(), before);
        assert_eq!(store.sequence().unwrap(), sequence);
    }
}

#[test]
fn workspace_membership_state_identity_and_both_revisions_are_required() {
    let (root, store, project) = store();
    let path = root.path().canonicalize().unwrap().join("managed");
    fs::create_dir(&path).unwrap();
    let workspace = managed(&project, &path, WorkspaceKind::Directory);
    save_workspace(&store, &workspace);
    let selected = managed_selection(&project, &workspace);
    for state in ["creating", "outcome_unknown"] {
        let mut row = workspace.clone();
        row.state = state.into();
        save_workspace(&store, &row);
        error(store.workspace_open_context(&selected), "INPUT_CHANGED");
    }
    let mut row = workspace.clone();
    row.identity = None;
    save_workspace(&store, &row);
    error(store.workspace_open_context(&selected), "INPUT_CHANGED");
    let mut row = workspace.clone();
    row.identity.as_mut().unwrap().inode = 0;
    save_workspace(&store, &row);
    error(store.workspace_open_context(&selected), "INPUT_CHANGED");
    save_workspace(&store, &workspace);
    let stale = WorkspaceOpenSelection::ManagedWorkspace {
        project_id: project.id.clone(),
        expected_project_revision: project.revision,
        workspace_id: workspace.id.clone(),
        expected_workspace_revision: 2,
    };
    error(store.workspace_open_context(&stale), "STALE_REVISION");
    let stale = WorkspaceOpenSelection::ManagedWorkspace {
        project_id: project.id.clone(),
        expected_project_revision: 2,
        workspace_id: workspace.id.clone(),
        expected_workspace_revision: workspace.revision,
    };
    error(store.workspace_open_context(&stale), "STALE_REVISION");
    let other = project.identity.canonical_root.with_file_name("other");
    fs::create_dir(&other).unwrap();
    let mut store = store;
    let other = store.import_project(&other, "Other", "other").unwrap();
    let foreign = WorkspaceOpenSelection::ManagedWorkspace {
        project_id: other.id,
        expected_project_revision: other.revision,
        workspace_id: workspace.id,
        expected_workspace_revision: workspace.revision,
    };
    error(store.workspace_open_context(&foreign), "PERMISSION_DENIED");
}

#[test]
fn archived_oversized_and_malformed_stored_records_fail_closed() {
    let (_root, store, mut project) = store();
    let selected = selection(&project);
    project.archived = true;
    store
        .conn
        .execute(
            "UPDATE projects SET document=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&project).unwrap(), project.id],
        )
        .unwrap();
    error(store.workspace_open_context(&selected), "PERMISSION_DENIED");
    store
        .conn
        .execute(
            "UPDATE projects SET document=?1 WHERE id=?2",
            rusqlite::params!["x".repeat(MAX_DOCUMENT_BYTES as usize + 1), project.id],
        )
        .unwrap();
    error(store.workspace_open_context(&selected), "RESOURCE_LIMIT");
    store
        .conn
        .execute(
            "UPDATE projects SET document='{}' WHERE id=?1",
            [&project.id],
        )
        .unwrap();
    error(store.workspace_open_context(&selected), "INPUT_CHANGED");
}
