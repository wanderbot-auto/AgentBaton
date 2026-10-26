//! Policy/storage contract tests use synthetic registered connection records, never Provider calls.
use agentbaton_core::execution::ExecutionBinding;
use agentbaton_core::policy::{require_enforcement, ExecutionGrant, GrantScope, NetworkScope};
use agentbaton_core::registry::{ManagedWorkspace, Project};
use agentbaton_core::storage::Store;
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    _db_dir: TempDir,
    _source: TempDir,
    _managed: TempDir,
    path: PathBuf,
    store: Store,
    project: Project,
    workspace: ManagedWorkspace,
}
fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@invalid")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn fixture() -> Fixture {
    let db_dir = tempfile::tempdir().unwrap();
    let path = db_dir.path().join("database.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let source = tempfile::tempdir().unwrap();
    git(source.path(), &["init", "--initial-branch=main"]);
    fs::write(source.path().join("source.txt"), "input\n").unwrap();
    git(source.path(), &["add", "source.txt"]);
    git(source.path(), &["commit", "-m", "initial"]);
    let head = git(source.path(), &["rev-parse", "HEAD"]);
    let project = store
        .import_project(source.path(), "Project", "import")
        .unwrap();
    let managed = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let workspace = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap();
    let connection = json!({"id":"connection-1","name":"Synthetic protocol fixture","provider":"custom_acp","probe_state":"ready","host_id":"local","revision":1,"environment_id":"environment-1","identity_ref":"identity-unknown-1","config_revision":"config-1","executable_sha256":"0".repeat(64),"capability_snapshot_id":"capability-1"});
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO connections(id,document) VALUES(?1,?2)",
            params!["connection-1", connection.to_string()],
        )
        .unwrap();
    Fixture {
        _db_dir: db_dir,
        _source: source,
        _managed: managed,
        path,
        store,
        project,
        workspace,
    }
}
fn create(f: &mut Fixture) -> ExecutionGrant {
    f.store
        .create_grant(
            &f.project.id,
            &f.workspace.id,
            "connection-1",
            4,
            2,
            60_000,
            true,
            "grant",
        )
        .unwrap()
}
fn binding(grant: &ExecutionGrant, workspace: &ManagedWorkspace) -> ExecutionBinding {
    let connection = &grant.connection_bindings["connection-1"];
    ExecutionBinding {
        connection_id: "connection-1".into(),
        environment_id: connection.environment_id.clone(),
        identity_ref: connection.identity_ref.clone(),
        host_id: connection.host_id.clone(),
        workspace_id: workspace.id.clone(),
        config_revision: connection.config_revision.clone(),
        input_revision: workspace.base_revision.clone(),
        capability_snapshot_id: connection.capability_snapshot_id.clone(),
        policy_revision: format!("{}:{}", grant.id, grant.revision),
    }
}
fn store_grant(f: &Fixture, grant: &ExecutionGrant) {
    Connection::open(&f.path)
        .unwrap()
        .execute(
            "UPDATE execution_grants SET parent_id=?1,document=?2 WHERE id=?3",
            params![
                grant.parent_id,
                serde_json::to_string(grant).unwrap(),
                grant.id
            ],
        )
        .unwrap();
}
fn delegated_fixture_root(f: &mut Fixture) -> ExecutionGrant {
    // Synthetic approved-policy fixture exercises the trusted scheduler delegation primitive.
    // Current user-facing grant action deliberately grants depth/fanout zero.
    let mut grant = create(f);
    grant.scope.max_depth = 2;
    grant.scope.max_children = 2;
    grant.scope.recipients = vec!["approved-recipient".into()];
    grant.scope.network = NetworkScope::Allowlisted {
        hosts: vec!["example.com".into()],
    };
    store_grant(f, &grant);
    grant
}

#[test]
fn explicit_user_intent_generates_approval_and_no_native_enforcement_claim() {
    let mut f = fixture();
    assert_eq!(
        f.store
            .create_grant(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                4,
                2,
                60_000,
                false,
                "no-consent"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    let grant = create(&mut f);
    assert!(!grant.id.is_empty());
    assert!(!grant.approval_id.is_empty());
    assert_eq!(grant.revision, 1);
    assert_eq!(grant.depth, 0);
    assert_eq!(grant.scope.network, NetworkScope::Deny);
    assert!(grant.scope.recipients.is_empty());
    assert!(!grant.enforcement.os_network_sandbox);
    assert!(!grant.enforcement.native_identity_isolation);
    assert_eq!(
        require_enforcement(&grant, false, true, false)
            .unwrap_err()
            .code,
        "ENFORCEMENT_UNAVAILABLE"
    );
    assert_eq!(
        require_enforcement(&grant, false, false, true)
            .unwrap_err()
            .code,
        "ENFORCEMENT_UNAVAILABLE"
    );
    assert_eq!(grant, create(&mut f));
    assert_eq!(
        f.store
            .create_grant(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                5,
                2,
                60_000,
                true,
                "grant"
            )
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(f.store.list_grants(&f.project.id).unwrap().len(), 1);
}

#[test]
fn persisted_project_and_workspace_ownership_are_checked() {
    let mut f = fixture();
    let grant = create(&mut f);
    let other = tempfile::tempdir().unwrap();
    let other_project = f
        .store
        .import_project(other.path(), "Other", "other")
        .unwrap();
    assert_eq!(
        f.store
            .create_grant(
                &other_project.id,
                &f.workspace.id,
                "connection-1",
                1,
                1,
                1000,
                true,
                "cross-project"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        f.store
            .validate_grant(
                &grant.id,
                &other_project.id,
                &f.workspace.id,
                "connection-1"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        f.store
            .validate_grant(
                &grant.id,
                &f.project.id,
                &f.workspace.id,
                "unapproved-connection"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    f.store
        .archive_project(&f.project.id, 1, "archive")
        .unwrap();
    assert_eq!(
        f.store
            .validate_grant(&grant.id, &f.project.id, &f.workspace.id, "connection-1")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn connection_identity_host_environment_and_revision_cannot_silently_change() {
    for field in [
        "identity_ref",
        "environment_id",
        "config_revision",
        "executable_sha256",
        "capability_snapshot_id",
        "revision",
        "host_id",
    ] {
        let mut f = fixture();
        let grant = create(&mut f);
        let db = Connection::open(&f.path).unwrap();
        let raw: String = db
            .query_row(
                "SELECT document FROM connections WHERE id='connection-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        value[field] = if field == "revision" {
            json!(2)
        } else {
            json!("changed-account-host-or-snapshot")
        };
        db.execute(
            "UPDATE connections SET document=?1 WHERE id='connection-1'",
            [value.to_string()],
        )
        .unwrap();
        assert!(
            f.store
                .validate_grant(&grant.id, &f.project.id, &f.workspace.id, "connection-1")
                .is_err(),
            "{field}"
        );
    }
}

#[test]
fn attempts_bind_exact_server_grant_revision_and_immutable_snapshots() {
    let mut f = fixture();
    let grant = create(&mut f);
    let task = f
        .store
        .create_registered(&f.project.id, "Task", "task")
        .unwrap();
    let approved = binding(&grant, &f.workspace);
    f.store
        .validate_execution_grant(&grant.id, 1, &task.id, &approved)
        .unwrap();
    assert_eq!(
        f.store
            .validate_execution_grant(&grant.id, 2, &task.id, &approved)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    for field in [
        "identity_ref",
        "host_id",
        "environment_id",
        "config_revision",
        "capability_snapshot_id",
        "policy_revision",
        "input_revision",
    ] {
        let mut value = serde_json::to_value(&approved).unwrap();
        value[field] = json!("unapproved");
        let altered: ExecutionBinding = serde_json::from_value(value).unwrap();
        assert!(
            f.store
                .validate_execution_grant(&grant.id, 1, &task.id, &altered)
                .is_err(),
            "{field}"
        );
    }
    fs::write(f.workspace.root.join("source.txt"), "changed input").unwrap();
    assert_eq!(
        f.store
            .validate_execution_grant(&grant.id, 1, &task.id, &approved)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}

#[test]
fn revocation_and_expiry_block_new_dispatch_without_claiming_inflight_stop() {
    let mut f = fixture();
    let grant = create(&mut f);
    let revoked = f.store.revoke_grant(&grant.id, 1, "revoke").unwrap();
    assert!(revoked.revoked);
    assert_eq!(revoked.revision, 2);
    assert_eq!(
        revoked,
        f.store.revoke_grant(&grant.id, 1, "revoke").unwrap()
    );
    assert_eq!(
        f.store
            .revoke_grant(&grant.id, 1, "stale")
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(
        f.store
            .validate_grant(&grant.id, &f.project.id, &f.workspace.id, "connection-1")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    let mut expired = grant;
    expired.expires_at_ms = 0;
    store_grant(&f, &expired);
    assert_eq!(
        f.store
            .validate_grant(&expired.id, &f.project.id, &f.workspace.id, "connection-1")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    drop(f.store);
    let store = Store::open(&f.path).unwrap();
    assert_eq!(store.get_grant(&expired.id).unwrap().expires_at_ms, 0);
}

#[test]
fn pure_scope_subset_rejects_each_permission_or_budget_expansion() {
    let mut f = fixture();
    let parent = delegated_fixture_root(&mut f);
    let scope = &parent.scope;
    scope.is_narrower_than(scope).unwrap();
    for field in ["workspace_ids", "connection_ids", "host_ids", "recipients"] {
        let mut value = serde_json::to_value(scope).unwrap();
        value[field]
            .as_array_mut()
            .unwrap()
            .push(json!("new-recipient-host-or-identity"));
        let child: GrantScope = serde_json::from_value(value).unwrap();
        assert_eq!(
            child.is_narrower_than(scope).unwrap_err().code,
            "PERMISSION_DENIED"
        );
    }
    for field in [
        "max_attempts",
        "max_concurrency",
        "max_duration_ms",
        "max_depth",
        "max_children",
    ] {
        let mut value = serde_json::to_value(scope).unwrap();
        value[field] = json!(value[field].as_u64().unwrap() + 1);
        let child: GrantScope = serde_json::from_value(value).unwrap();
        assert!(child.is_narrower_than(scope).is_err(), "{field}");
    }
    let mut child = scope.clone();
    child.network = NetworkScope::Allowlisted {
        hosts: vec!["attacker.example".into()],
    };
    assert_eq!(
        child.is_narrower_than(scope).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    child.network = NetworkScope::Deny;
    child.recipients.clear();
    child.max_concurrency = 1;
    child.is_narrower_than(scope).unwrap();
    let deny = child.clone();
    child.network = NetworkScope::Allowlisted {
        hosts: vec!["example.com".into()],
    };
    assert_eq!(
        child.is_narrower_than(&deny).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    child.network = NetworkScope::Allowlisted {
        hosts: vec!["*.example.com".into()],
    };
    assert_eq!(child.validate().unwrap_err().code, "INVALID_REQUEST");
}

#[test]
fn persisted_delegation_preserves_root_budget_lineage_and_global_fanout() {
    let mut f = fixture();
    let parent = delegated_fixture_root(&mut f);
    let mut narrow = parent.scope.clone();
    narrow.max_attempts = 2;
    narrow.max_concurrency = 1;
    narrow.max_duration_ms = 1000;
    narrow.network = NetworkScope::Deny;
    narrow.recipients.clear();
    assert_eq!(
        f.store
            .delegate_grant(&parent.id, 2, narrow.clone(), "stale")
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let child = f
        .store
        .delegate_grant(&parent.id, 1, narrow.clone(), "child")
        .unwrap();
    assert_eq!(child.root_grant_id, parent.id);
    assert_eq!(child.approval_id, parent.approval_id);
    assert_eq!(child.depth, 1);
    assert!(child.expires_at_ms <= parent.expires_at_ms);
    let grandchild = f
        .store
        .delegate_grant(&child.id, 1, narrow.clone(), "grandchild")
        .unwrap();
    assert_eq!(grandchild.root_grant_id, parent.id);
    assert_eq!(grandchild.depth, 2);
    assert_eq!(
        f.store
            .delegate_grant(&parent.id, 1, narrow.clone(), "fanout-exhausted")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.store
            .delegate_grant(&grandchild.id, 1, narrow, "depth-exhausted")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    f.store.revoke_grant(&parent.id, 1, "revoke-root").unwrap();
    assert_eq!(
        f.store
            .validate_grant(
                &grandchild.id,
                &f.project.id,
                &f.workspace.id,
                "connection-1"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn corrupted_or_cyclic_lineage_fails_closed() {
    let mut f = fixture();
    let root = delegated_fixture_root(&mut f);
    let mut child = f
        .store
        .delegate_grant(&root.id, 1, root.scope.clone(), "child")
        .unwrap();
    child.parent_id = Some(child.id.clone());
    store_grant(&f, &child);
    assert_eq!(
        f.store
            .validate_grant(&child.id, &f.project.id, &f.workspace.id, "connection-1")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    child.parent_id = Some(root.id.clone());
    child.project_id = "wrong-project".into();
    store_grant(&f, &child);
    assert_eq!(
        f.store
            .validate_grant(&child.id, &f.project.id, &f.workspace.id, "connection-1")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn grant_record_and_approval_journal_are_transactional() {
    let mut f = fixture();
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_grant_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='execution_grant.approved' BEGIN SELECT RAISE(ABORT,'injected approval journal failure'); END;").unwrap();
    assert_eq!(
        f.store
            .create_grant(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                4,
                2,
                60_000,
                true,
                "grant"
            )
            .unwrap_err()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(f.store.list_grants(&f.project.id).unwrap().is_empty());
    db.execute_batch("DROP TRIGGER fail_grant_event").unwrap();
    create(&mut f);
}

#[test]
fn narrowed_ancestor_budget_counts_grandchildren_and_never_refunds_attempts() {
    let mut f = fixture();
    let mut root = delegated_fixture_root(&mut f);
    root.scope.max_concurrency = 3;
    root.scope.max_children = 4;
    store_grant(&f, &root);
    let mut scope = root.scope.clone();
    scope.max_attempts = 2;
    scope.max_concurrency = 1;
    scope.max_children = 3;
    let child = f
        .store
        .delegate_grant(&root.id, 1, scope.clone(), "child")
        .unwrap();
    let grandchild = f
        .store
        .delegate_grant(&child.id, 1, scope, "grandchild")
        .unwrap();
    let task = f
        .store
        .create_registered(&f.project.id, "Task", "task")
        .unwrap();
    let db = Connection::open(&f.path).unwrap();
    db.execute("INSERT INTO sessions(id,task_id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload) VALUES('session-1',?1,?2,?3,'connection-1',?4,?5,'running','{}','first','{}')",params![task.id,f.project.id,f.workspace.id,grandchild.id,root.id]).unwrap();
    assert_eq!(
        f.store.validate_grant_budget(&child.id).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    // Root still has free capacity; the narrower intermediate ancestor is what prevents widening.
    f.store.validate_grant_budget(&root.id).unwrap();
    db.execute(
        "UPDATE sessions SET state='stopped' WHERE id='session-1'",
        [],
    )
    .unwrap();
    f.store.validate_grant_budget(&child.id).unwrap();
    db.execute("INSERT INTO sessions(id,task_id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload) VALUES('session-2',?1,?2,?3,'connection-1',?4,?5,'failed','{}','second','{}')",params![task.id,f.project.id,f.workspace.id,grandchild.id,root.id]).unwrap();
    assert_eq!(
        f.store.validate_grant_budget(&child.id).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    f.store.validate_grant_budget(&root.id).unwrap();
}

#[test]
fn owner_confirmed_snapshot_must_match_current_registered_references() {
    let mut f = fixture();
    let refs = f
        .store
        .grant_proposal_snapshot(&f.project.id, &f.workspace.id, "connection-1")
        .unwrap();
    let mut tampered = refs.clone();
    tampered["project_revision"] = json!(2);
    assert_eq!(
        f.store
            .create_grant_checked(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                4,
                2,
                60_000,
                true,
                "tampered",
                Some(&tampered)
            )
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert!(f.store.list_grants(&f.project.id).unwrap().is_empty());
    f.store
        .create_grant_checked(
            &f.project.id,
            &f.workspace.id,
            "connection-1",
            4,
            2,
            60_000,
            true,
            "confirmed",
            Some(&refs),
        )
        .unwrap();
    let db = Connection::open(&f.path).unwrap();
    db.execute("UPDATE connections SET document=json_set(document,'$.revision',2,'$.identity_ref','changed-account') WHERE id='connection-1'",[]).unwrap();
    assert_eq!(
        f.store
            .create_grant_checked(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                4,
                2,
                60_000,
                true,
                "stale-proposal",
                Some(&refs)
            )
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert_eq!(f.store.list_grants(&f.project.id).unwrap().len(), 1);
}

#[test]
fn task_grant_cannot_widen_an_existing_shared_project_budget() {
    let mut f = fixture();
    f.store.configure_budget(true, &f.project.id, 2, 1).unwrap();
    assert_eq!(
        f.store
            .create_grant(
                &f.project.id,
                &f.workspace.id,
                "connection-1",
                4,
                2,
                60_000,
                true,
                "widen"
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    let budget = f.store.budget(true, &f.project.id).unwrap().unwrap();
    assert_eq!(budget.total_limit, 2);
    assert_eq!(budget.parallel_limit, 1);
    assert_eq!(budget.consumed, 0);
    assert!(f.store.list_grants(&f.project.id).unwrap().is_empty());
    f.store
        .create_grant(
            &f.project.id,
            &f.workspace.id,
            "connection-1",
            2,
            1,
            60_000,
            true,
            "narrow",
        )
        .unwrap();
}

#[test]
fn native_owner_grant_snapshot_identifies_the_exact_registered_recipient() {
    let mut f = fixture();
    let db = Connection::open(&f.path).unwrap();
    db.execute("UPDATE connections SET document=json_set(document,'$.name','First selected ACP','$.protocol_route','custom_acp','$.executable','/fixture/first-acp','$.arguments',json('[\"--first\"]')) WHERE id='connection-1'", []).unwrap();
    let mut second: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT document FROM connections WHERE id='connection-1'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    second["id"] = json!("connection-2");
    second["name"] = json!("Second selected ACP");
    second["executable"] = json!("/fixture/second-acp");
    second["arguments"] = json!(["--second"]);
    db.execute(
        "INSERT INTO connections(id,document) VALUES('connection-2',?1)",
        [second.to_string()],
    )
    .unwrap();
    let first = f
        .store
        .grant_proposal_snapshot(&f.project.id, &f.workspace.id, "connection-1")
        .unwrap();
    let second = f
        .store
        .grant_proposal_snapshot(&f.project.id, &f.workspace.id, "connection-2")
        .unwrap();
    assert_eq!(first["execution_recipient"]["name"], "First selected ACP");
    assert_eq!(
        first["execution_recipient"]["executable"],
        "/fixture/first-acp"
    );
    assert_eq!(
        first["execution_recipient"]["arguments"],
        json!(["--first"])
    );
    assert_eq!(second["execution_recipient"]["provider"], "custom_acp");
    assert_eq!(
        second["execution_recipient"]["protocol_route"],
        "custom_acp"
    );
    assert_eq!(
        second["execution_recipient"]["executable"],
        "/fixture/second-acp"
    );
    // Even a changed human-readable recipient field cannot reuse an already
    // displayed approval snapshot while retaining the older opaque digest refs.
    db.execute("UPDATE connections SET document=json_set(document,'$.executable','/fixture/replaced-acp') WHERE id='connection-1'", []).unwrap();
    let error = f
        .store
        .create_grant_checked(
            &f.project.id,
            &f.workspace.id,
            "connection-1",
            4,
            2,
            60_000,
            true,
            "stale-recipient",
            Some(&first),
        )
        .unwrap_err();
    assert_eq!(error.code, "INPUT_CHANGED");
    assert!(f.store.list_grants(&f.project.id).unwrap().is_empty());
}
