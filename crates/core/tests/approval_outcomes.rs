//! Local storage/IPC fault injection only; the generated executable must never run.
use agentbaton_core::{
    ipc::{call, Daemon},
    sessions::ApprovalProposal,
    storage::Store,
};
use rusqlite::Connection;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn executable(root: &Path) -> std::path::PathBuf {
    let path = root.join("generated-never-run");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf unexpected > '{}'\nexit 97\n",
            root.join("unexpected-execution").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path.canonicalize().unwrap()
}
fn refuse_confirmation(db: &Connection) {
    db.execute_batch("CREATE TRIGGER refuse_approval_confirmation BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='approval.confirmed' BEGIN SELECT RAISE(ABORT,'generated confirmation fault'); END;").unwrap();
}
fn count(db: &Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |row| row.get(0)).unwrap()
}
fn assert_pending(db: &Connection, proposal: &ApprovalProposal) {
    let (document, result): (String, Option<String>) = db
        .query_row(
            "SELECT document,result FROM approval_proposals WHERE id=?1",
            [&proposal.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let actual: ApprovalProposal = serde_json::from_str(&document).unwrap();
    assert!(!actual.consumed);
    assert_eq!(actual.revision, proposal.revision);
    assert!(result.is_none());
    assert_eq!(count(db, "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='approval.confirmed'"), 0);
}
fn confirmation_count(db: &Connection) -> i64 {
    count(
        db,
        "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='approval.confirmed'",
    )
}

#[test]
fn connection_confirmation_fault_is_already_unknown_and_same_proposal_replays_original_effect() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("state.sqlite3");
    let mut store = Store::open(&database).unwrap();
    // Exercise a selected-directory alias on Linux as well as Darwin's common
    // /var -> /private/var temporary-directory alias. Only the fixture resolves
    // it; the production canonical-executable requirement remains unchanged.
    let alias = directory.path().join("selected-root-alias");
    symlink(directory.path().canonicalize().unwrap(), &alias).unwrap();
    assert_ne!(alias, alias.canonicalize().unwrap());
    let proposal = store
        .propose_connection(
            "Generated never-run",
            "custom_acp",
            &executable(&alias),
            &[],
            "owner-selection",
        )
        .unwrap();
    let db = Connection::open(&database).unwrap();
    refuse_confirmation(&db);
    let error = store
        .finalize_proposal(&proposal.id, proposal.revision)
        .unwrap_err();
    let original = store.list_connections().unwrap();
    assert_eq!(original.len(), 1);
    assert_pending(&db, &proposal);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM idempotency WHERE scope='local:connections.register'"
        ),
        1
    );
    let key: String = db
        .query_row(
            "SELECT key FROM idempotency WHERE scope='local:connections.register'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(key, format!("proposal:{}", proposal.id));
    eprintln!("existing metadata-fault control: code={}, side_effect_state={}, durable_connections=1, original_proposal_key_preserved=true", error.code, error.side_effect_state);
    // Baseline already has conservative SQLite error mapping; do not claim a
    // not_started bug or require a cosmetic error-code change here.
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert_eq!(error.side_effect_state, "unknown");
    db.execute_batch("DROP TRIGGER refuse_approval_confirmation;")
        .unwrap();
    let replay = store
        .finalize_proposal(&proposal.id, proposal.revision)
        .unwrap();
    assert_eq!(replay["id"], original[0].id);
    assert_eq!(store.list_connections().unwrap().len(), 1);
    assert_eq!(confirmation_count(&db), 1);
    assert_eq!(
        store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap(),
        replay
    );
    assert_eq!(confirmation_count(&db), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM approval_proposals"), 1);
    assert!(!directory.path().join("unexpected-execution").exists());
}

fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    let result = command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Generated fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "Generated fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}

#[test]
fn grant_confirmation_fault_keeps_the_original_authorization_and_no_new_owner_consent() {
    let directory = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let database = directory.path().join("state.sqlite3");
    let mut store = Store::open(&database).unwrap();
    let source = directory.path().join("source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("input"), "generated input").unwrap();
    git(&source, &["add", "input"]);
    git(
        &source,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            "generated",
        ],
    );
    let head = git(&source, &["rev-parse", "HEAD"]);
    let project = store
        .import_project(&source, "Generated project", "project")
        .unwrap();
    let managed = directory.path().join("managed");
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = store
        .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
        .unwrap();
    let connection = store
        .register_connection(
            "Storage fixture",
            "custom_acp",
            &executable(directory.path()),
            &[],
            "connection",
        )
        .unwrap();
    let db = Connection::open(&database).unwrap();
    // Explicit stored readiness fixture: no version probe or Provider process.
    db.execute(
        "UPDATE connections SET document=json_set(document,'$.probe_state','ready') WHERE id=?1",
        [&connection.id],
    )
    .unwrap();
    let proposal = store
        .propose_grant(
            &project.id,
            &workspace.id,
            &connection.id,
            2,
            1,
            60_000,
            "owner-grant",
        )
        .unwrap();
    refuse_confirmation(&db);
    let error = store
        .finalize_proposal(&proposal.id, proposal.revision)
        .unwrap_err();
    let original: String = db
        .query_row("SELECT id FROM execution_grants", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM execution_grants"), 1);
    assert_pending(&db, &proposal);
    assert_eq!(error.side_effect_state, "unknown");
    db.execute_batch("DROP TRIGGER refuse_approval_confirmation;")
        .unwrap();
    let replay = store
        .finalize_proposal(&proposal.id, proposal.revision)
        .unwrap();
    assert_eq!(replay["id"], original);
    assert_eq!(
        store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap(),
        replay
    );
    assert_eq!(count(&db, "SELECT count(*) FROM execution_grants"), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM approval_proposals"), 1);
    assert_eq!(confirmation_count(&db), 1);
    let keys: Vec<String> = db
        .prepare("SELECT key FROM idempotency WHERE scope LIKE '%grants%'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(keys, vec![format!("proposal:{}", proposal.id)]);
    assert!(!directory.path().join("unexpected-execution").exists());
}

#[test]
fn finalized_connection_probe_refusal_reports_confirmed_effect_and_reconciles_same_proposal() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = directory.path().canonicalize().unwrap().join("runtime");
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let run = stop.clone();
    struct Server(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);
    impl Drop for Server {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
            self.1.take().unwrap().join().unwrap();
        }
    }
    let server = Server(
        stop,
        Some(std::thread::spawn(move || daemon.run(run).unwrap())),
    );
    // A fixed private non-directory prevents environment preparation and thus
    // probe scheduling, without resource exhaustion or executing any program.
    fs::write(runtime.join("environments"), "preserve this file").unwrap();
    fs::set_permissions(
        runtime.join("environments"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let request = |value| call(&runtime, value).unwrap();
    let before = request(
        json!({"method":"approvals.finalize","params":{"proposal_id":"not-an-owner-proposal"},"expected_revision":1,"idempotency_key":"invalid"}),
    );
    assert_eq!(before["error"]["code"], "NOT_FOUND");
    assert_eq!(before["error"]["side_effect_state"], "not_started");
    let proposed = request(
        json!({"method":"connections.propose","params":{"name":"Generated never-run","provider":"custom_acp","executable":executable(directory.path()),"arguments":[]},"idempotency_key":"owner-selection"}),
    );
    assert!(proposed.get("error").is_none(), "{proposed}");
    let proposal = &proposed["result"];
    let finalize = json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}", proposal["id"].as_str().unwrap())});
    let failed = request(finalize.clone());
    let connections = request(json!({"method":"connections.list","params":{}}));
    let original = &connections["result"][0];
    assert_eq!(connections["result"].as_array().unwrap().len(), 1);
    assert_eq!(original["probe_state"], "pending");
    let db = Connection::open(runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(confirmation_count(&db), 1);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM approval_proposals WHERE result IS NOT NULL"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM idempotency WHERE scope='local:connections.register'"
        ),
        1
    );
    assert_eq!(
        fs::read_to_string(runtime.join("environments")).unwrap(),
        "preserve this file"
    );
    assert!(!directory.path().join("unexpected-execution").exists());
    eprintln!("actual IPC probe-refusal: error={}, durable_connection={}, confirmation_journal_count=1, probe_pending=true, executable_not_run=true", failed["error"], original["id"]);
    assert_eq!(failed["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(failed["error"]["side_effect_state"], "confirmed");
    assert_eq!(
        failed["error"]["details_ref"],
        format!(
            "connection-approved-probe-not-scheduled:{}",
            original["id"].as_str().unwrap()
        )
    );
    assert!(failed["error"]["message"]
        .as_str()
        .unwrap()
        .contains("same proposal"));
    let retry = request(finalize);
    assert_eq!(retry["error"], failed["error"]);
    assert_eq!(confirmation_count(&db), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM connections"), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM approval_proposals"), 1);
    drop(server);
    let mut store = Store::open(&runtime.join("baton.sqlite3")).unwrap();
    let reconciled = store
        .finalize_proposal(
            proposal["id"].as_str().unwrap(),
            proposal["revision"].as_u64().unwrap(),
        )
        .unwrap();
    assert_eq!(reconciled["id"], original["id"]);
    assert_eq!(confirmation_count(&db), 1);
    assert!(!directory.path().join("unexpected-execution").exists());
}
