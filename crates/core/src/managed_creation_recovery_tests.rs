//! These tests use SQLite and a generated --version executable, not daemon IPC.
use super::*;
use crate::client_recovery_journal::ManagedCreationOperation;
use std::{fs, os::unix::fs::PermissionsExt};
struct Fixture {
    _dir: tempfile::TempDir,
    db: PathBuf,
    store: Store,
    source: PathBuf,
    probe_count: PathBuf,
    marker: ClientRecoveryMarker,
    selection: Value,
}
fn fixture(kind: ManagedCreationKind) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let db = runtime.join("db");
    let mut store = Store::open(&db).unwrap();
    let python = std::process::Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let exe = root.join("generated-version");
    let probe_count = root.join("probe-count");
    fs::write(&exe,format!("#!{}\nimport sys\nassert sys.argv[1:]==['--version']\nopen({},'a').write('probe\\n')\nprint('generated version 1')\n",String::from_utf8(python.stdout).unwrap().trim(),serde_json::to_string(probe_count.to_str().unwrap()).unwrap())).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let proposal = store
        .propose_connection("Base", "codex", &exe, &[], "base")
        .unwrap();
    let base: ProviderConnection =
        serde_json::from_value(store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
    let mut supervisor = Supervisor::new(&runtime);
    supervisor.probe(&store, &base.id).unwrap();
    let started = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut store).unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let base = store.get_connection(&base.id).unwrap();
    assert_eq!(base.probe_state, "ready");
    let source = root.join("selected");
    let (request, digest) = match kind {
        ManagedCreationKind::ManagedSkill => {
            fs::create_dir(&source).unwrap();
            fs::write(
                source.join("SKILL.md"),
                "---\nname: selected\ndescription: local only\n---\nPRIVATE SKILL BODY CANARY\n",
            )
            .unwrap();
            let r = ManagedSkillRequest {
                base_connection_id: base.id.clone(),
                expected_base_revision: base.revision,
                new_name: "Derived skill".into(),
                operation: ManagedSkillOperation::Replace,
                skill_folder: Some(source.to_str().unwrap().into()),
            };
            let p = store.preview_managed_skill(&r).unwrap();
            (json!(r), p.preview_digest)
        }
        ManagedCreationKind::ManagedMcp => {
            fs::write(&source, "#!/bin/sh\nexit 99\n").unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
            let r = ManagedMcpRequest {
                base_connection_id: base.id.clone(),
                expected_base_revision: base.revision,
                new_name: "Derived MCP".into(),
                operation: ManagedMcpOperation::Replace,
                server: Some(ManagedMcpServerSelection {
                    name: "notes".into(),
                    executable: source.to_str().unwrap().into(),
                    enabled_tools: vec!["read_note".into()],
                }),
            };
            let p = store.preview_managed_mcp(&r).unwrap();
            (json!(r), p.preview_digest)
        }
    };
    let marker = ClientRecoveryMarker {
        entry_id: Uuid::new_v4().to_string(),
        kind,
        request_key: Uuid::new_v4().to_string(),
        preview_digest: digest.clone(),
        base_connection_id: base.id,
        base_connection_revision: base.revision,
        operation: ManagedCreationOperation::Replace,
        created_at: "2026-11-09T00:00:00Z".into(),
    };
    let selection = json!({"request":request,"expected_preview_digest":digest});
    Fixture {
        _dir: dir,
        db,
        store,
        source,
        probe_count,
        marker,
        selection,
    }
}
fn propose(f: &mut Fixture) -> Result<ApprovalProposal> {
    match f.marker.kind {
        ManagedCreationKind::ManagedSkill => f.store.propose_managed_skill(
            &serde_json::from_value(f.selection.clone()).unwrap(),
            &f.marker.request_key,
        ),
        ManagedCreationKind::ManagedMcp => f.store.propose_managed_mcp(
            &serde_json::from_value(f.selection.clone()).unwrap(),
            &f.marker.request_key,
        ),
    }
}
fn kinds() -> [ManagedCreationKind; 2] {
    [
        ManagedCreationKind::ManagedSkill,
        ManagedCreationKind::ManagedMcp,
    ]
}
#[test]
fn committed_creation_recovery_is_readonly_historical_and_never_reprobes() {
    for kind in kinds() {
        let mut f = fixture(kind);
        let p = propose(&mut f).unwrap();
        f.store.finalize_proposal(&p.id, 1).unwrap();
        let mut unknown = f
            .store
            .create("p", "unrelated unresolved outcome", "unknown-task")
            .unwrap();
        unknown.state = TaskState::OutcomeUnknown;
        f.store
            .conn
            .execute(
                "UPDATE tasks SET document=?1 WHERE id=?2",
                params![encode(&unknown).unwrap(), unknown.id],
            )
            .unwrap();
        let original = f.store.managed_creation_outcome(&f.marker).unwrap();
        assert_eq!(original.state, "confirmed");
        assert_eq!(original.connection.as_ref().unwrap().probe_state, "pending");
        let probes = fs::read(&f.probe_count).unwrap();
        if f.source.is_dir() {
            fs::remove_dir_all(&f.source).unwrap()
        } else {
            fs::remove_file(&f.source).unwrap()
        }
        let changes = f.store.conn.total_changes();
        let seq = f.store.sequence().unwrap();
        let result = f.store.abandon_managed_creation(&f.marker).unwrap();
        assert_eq!(result.state, "confirmed");
        let _proof = result.no_new_execution();
        assert_eq!(f.store.conn.total_changes(), changes);
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(fs::read(&f.probe_count).unwrap(), probes);
        assert!(propose(&mut f).is_ok());
        assert!(f.store.finalize_proposal(&p.id, 1).is_ok());
        drop(f.store);
        let store = Store::open(&f.db).unwrap();
        let seq = store.sequence().unwrap();
        let result = store.managed_creation_outcome(&f.marker).unwrap();
        assert_eq!(result.state, "confirmed");
        assert_eq!(
            result.connection.as_ref().unwrap().id,
            original.connection.as_ref().unwrap().id
        );
        assert_eq!(store.sequence().unwrap(), seq);
        assert_eq!(fs::read(&f.probe_count).unwrap(), probes);
        assert!(!encode(&result).unwrap().contains("PRIVATE SKILL BODY"));
        assert_eq!(
            store.get(&unknown.id).unwrap().state,
            TaskState::OutcomeUnknown
        );
        if kind == ManagedCreationKind::ManagedSkill {
            if let Some(path) = std::env::var_os("AGENTBATON_CREATION_RECOVERY_FIXTURE") {
                let path = PathBuf::from(path).with_extension("confirmed.json");
                fs::write(
                    path,
                    serde_json::to_vec_pretty(&json!({"marker":f.marker,"confirmed":result}))
                        .unwrap(),
                )
                .unwrap();
            }
        }
    }
}
#[test]
fn abandonment_seals_pending_and_missing_proposals_and_survives_reopen() {
    for kind in kinds() {
        for pending in [false, true] {
            let mut f = fixture(kind);
            let p = if pending {
                Some(propose(&mut f).unwrap())
            } else {
                None
            };
            let unresolved = f.store.managed_creation_outcome(&f.marker).unwrap();
            assert_eq!(unresolved.state, "unresolved");
            assert!(unresolved.connection.is_none());
            let before = f.store.list_connections().unwrap().len();
            let probes = fs::read(&f.probe_count).unwrap();
            let abandoned = f.store.abandon_managed_creation(&f.marker).unwrap();
            assert_eq!(abandoned.state, "abandoned");
            let seq = f.store.sequence().unwrap();
            assert_eq!(
                f.store.abandon_managed_creation(&f.marker).unwrap().state,
                "abandoned"
            );
            assert_eq!(f.store.sequence().unwrap(), seq);
            assert_eq!(propose(&mut f).unwrap_err().code, "PERMISSION_DENIED");
            if let Some(p) = p {
                assert_eq!(
                    f.store.finalize_proposal(&p.id, 1).unwrap_err().code,
                    "PERMISSION_DENIED"
                );
            }
            assert_eq!(f.store.list_connections().unwrap().len(), before);
            assert_eq!(fs::read(&f.probe_count).unwrap(), probes);
            let events: String = f
                .store
                .conn
                .query_row("SELECT group_concat(document) FROM events", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert!(!events.contains("PRIVATE SKILL BODY"));
            if !pending && kind == ManagedCreationKind::ManagedSkill {
                if let Some(path) = std::env::var_os("AGENTBATON_CREATION_RECOVERY_FIXTURE") {
                    fs::write(path,serde_json::to_vec_pretty(&json!({"marker":f.marker,"unresolved":unresolved,"abandoned":abandoned})).unwrap()).unwrap();
                }
            }
            drop(f.store);
            let mut store = Store::open(&f.db).unwrap();
            let seq = store.sequence().unwrap();
            assert_eq!(
                store.managed_creation_outcome(&f.marker).unwrap().state,
                "abandoned"
            );
            assert_eq!(
                store.abandon_managed_creation(&f.marker).unwrap().state,
                "abandoned"
            );
            assert_eq!(store.sequence().unwrap(), seq);
        }
    }
}
#[test]
fn marker_binding_mismatch_never_abandons_or_claims_another_creation() {
    for kind in kinds() {
        let mut f = fixture(kind);
        propose(&mut f).unwrap();
        let seq = f.store.sequence().unwrap();
        for field in 0..4 {
            let mut wrong = f.marker.clone();
            match field {
                0 => wrong.preview_digest = "0".repeat(64),
                1 => wrong.base_connection_id = Uuid::new_v4().to_string(),
                2 => wrong.base_connection_revision += 1,
                _ => wrong.operation = ManagedCreationOperation::Remove,
            }
            assert_eq!(
                f.store.managed_creation_outcome(&wrong).unwrap_err().code,
                "CONFIG_CONFLICT"
            );
            assert_eq!(
                f.store.abandon_managed_creation(&wrong).unwrap_err().code,
                "CONFIG_CONFLICT"
            );
        }
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(
            f.store.managed_creation_outcome(&f.marker).unwrap().state,
            "unresolved"
        );
        f.store.abandon_managed_creation(&f.marker).unwrap();
        let mut wrong = f.marker.clone();
        wrong.preview_digest = "0".repeat(64);
        assert!(f.store.abandon_managed_creation(&wrong).is_err());
        let mut tracking = f.marker.clone();
        tracking.entry_id = Uuid::new_v4().to_string();
        assert_eq!(
            f.store.managed_creation_outcome(&tracking).unwrap().state,
            "abandoned"
        );
    }
}
#[test]
fn immediate_transactions_linearize_finalize_against_abandon() {
    for kind in kinds() {
        let mut f = fixture(kind);
        let p = propose(&mut f).unwrap();
        let path = f.db.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let other = barrier.clone();
        let child = std::thread::spawn(move || {
            let mut store = Store::open(&path).unwrap();
            other.wait();
            store.finalize_proposal(&p.id, 1)
        });
        barrier.wait();
        let result = f.store.abandon_managed_creation(&f.marker).unwrap();
        let finalized = child.join().unwrap();
        match result.state.as_str() {
            "confirmed" => assert!(finalized.is_ok()),
            "abandoned" => assert_eq!(finalized.unwrap_err().code, "PERMISSION_DENIED"),
            _ => panic!("nonterminal abandon"),
        };
        assert_eq!(
            f.store.managed_creation_outcome(&f.marker).unwrap().state,
            result.state
        );
        assert_eq!(fs::read_to_string(&f.probe_count).unwrap(), "probe\n");
    }
}
#[test]
fn persistence_boundary_rechecks_tombstone_and_does_not_create_a_proposal() {
    for kind in kinds() {
        let mut f = fixture(kind);
        let snapshot = json!({"intent":f.selection,"request_key":f.marker.request_key});
        f.store.abandon_managed_creation(&f.marker).unwrap();
        let seq = f.store.sequence().unwrap();
        assert_eq!(
            f.store
                .persist_proposal(kind_name(kind), snapshot, &f.marker.request_key)
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(f.store.conn.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')=?1",[kind_name(kind)],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
}

#[test]
fn markerless_binding_status_reconciles_terminal_history_without_authorizing_retries() {
    let mut exported = Vec::new();
    for kind in kinds() {
        let mut f = fixture(kind);
        let b = binding(&f.marker);
        let seq = f.store.sequence().unwrap();
        let changes = f.store.conn.total_changes();
        let unresolved = f.store.managed_creation_binding_outcome(&b).unwrap();
        assert_eq!(unresolved.state, "unresolved");
        assert!(unresolved.connection.is_none());
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(f.store.conn.total_changes(), changes);
        let encoded = json!(unresolved);
        assert!(encoded.get("marker").is_none());
        assert!(encoded["binding"].get("entry_id").is_none());
        assert!(encoded["binding"].get("created_at").is_none());
        propose(&mut f).unwrap();
        let mut wrong = b.clone();
        wrong.preview_digest = "0".repeat(64);
        assert_eq!(
            f.store
                .managed_creation_binding_outcome(&wrong)
                .unwrap_err()
                .code,
            "CONFIG_CONFLICT"
        );
        f.store.abandon_managed_creation(&f.marker).unwrap();
        drop(f.store);
        let store = Store::open(&f.db).unwrap();
        let changes = store.conn.total_changes();
        let abandoned = store.managed_creation_binding_outcome(&b).unwrap();
        assert_eq!(abandoned.state, "abandoned");
        assert_eq!(store.conn.total_changes(), changes);
        assert!(store.managed_creation_binding_outcome(&wrong).is_err());
        let mut committed = fixture(kind);
        let p = propose(&mut committed).unwrap();
        committed.store.finalize_proposal(&p.id, 1).unwrap();
        let probes = fs::read(&committed.probe_count).unwrap();
        let changes = committed.store.conn.total_changes();
        let confirmed = committed
            .store
            .managed_creation_binding_outcome(&binding(&committed.marker))
            .unwrap();
        assert_eq!(confirmed.state, "confirmed");
        assert!(confirmed.connection.is_some());
        let _proof = confirmed.no_new_execution();
        assert_eq!(committed.store.conn.total_changes(), changes);
        assert_eq!(fs::read(&committed.probe_count).unwrap(), probes);
        if kind == ManagedCreationKind::ManagedSkill {
            exported = vec![json!(unresolved), json!(abandoned), json!(confirmed)];
        }
    }
    if let Some(path) = std::env::var_os("AGENTBATON_CREATION_BINDING_FIXTURE") {
        fs::write(path, serde_json::to_vec_pretty(&exported).unwrap()).unwrap();
    }
}
