//! Generated local protocol peer only: never a vendor executable or paid request.
use crate::conversations::*;
use crate::policy::ExecutionGrant;
use crate::registry::{ManagedWorkspace, Project};
use crate::sessions::ProviderConnection;
use crate::storage::{decode, encode, Store};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub(crate) struct Fixture {
    pub _dir: tempfile::TempDir,
    pub path: PathBuf,
    pub store: Store,
    pub project: Project,
    pub workspace: ManagedWorkspace,
    pub connection: ProviderConnection,
    pub grant: ExecutionGrant,
    pub runtime: PathBuf,
}
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
pub(crate) fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("input"), "fixture").unwrap();
    git(&source, &["add", "input"]);
    git(&source, &["commit", "-m", "initial"]);
    let head = git(&source, &["rev-parse", "HEAD"]);
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let path = runtime.join("store.sqlite");
    let mut store = Store::open(&path).unwrap();
    let project = store.import_project(&source, "Fixture", "project").unwrap();
    let managed = root.join("managed");
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = store
        .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
        .unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let exe = root.join("generated-acp");
    let body = r#"
import sys,json,os
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize': result={'protocolVersion':1,'agentCapabilities':{}}
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  assert r['params']['_meta']['claudeCode']['options']['settingSources']==[]
  assert os.path.isfile(os.path.join(os.environ['CLAUDE_CONFIG_DIR'],'settings.json'))
  result={'sessionId':'generated-independent-conversation'}
 elif m=='session/prompt':
  text=r['params']['prompt'][0]['text']
  with open('generated-prompts.jsonl','a') as f:f.write(json.dumps(text)+'\n')
  if text=='hold':continue
  print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'generated-independent-conversation','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'Generated reply: '+text}}}}),flush=True)
  result={'stopReason':'end_turn'}
 elif m=='session/cancel':continue
 else:raise AssertionError('Unexpected native action')
 print(json.dumps({'jsonrpc':'2.0','id':i,'result':result}),flush=True)
"#;
    fs::write(
        &exe,
        format!(
            "#!{}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            body
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let mut connection = store
        .register_connection("Generated", "claude_code", &exe, &[], "connection")
        .unwrap();
    connection.probe_state = "ready".into();
    store
        .conn
        .execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&connection).unwrap(), connection.id],
        )
        .unwrap();
    let grant = store
        .create_grant(
            &project.id,
            &workspace.id,
            &connection.id,
            8,
            4,
            10000,
            true,
            "grant",
        )
        .unwrap();
    Fixture {
        _dir: dir,
        path,
        store,
        project,
        workspace,
        connection,
        grant,
        runtime,
    }
}
impl Fixture {
    pub(crate) fn create(&mut self, key: &str) -> Conversation {
        self.store
            .create_conversation(
                &self.project.id,
                "Independent conversation",
                &self.workspace.id,
                &self.connection.id,
                &self.grant.id,
                key,
            )
            .unwrap()
    }
}
fn wait_state(
    f: &mut Fixture,
    s: &mut ConversationSupervisor,
    id: &str,
    state: &str,
) -> Conversation {
    let start = Instant::now();
    loop {
        s.tick(&mut f.store).unwrap();
        let c = f.store.get_conversation(id).unwrap();
        if c.state == state {
            return c;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "state {}, error {:?}, expected {}",
            c.state,
            c.last_error,
            state
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn taskless_actor_generated_child_two_turns_replay_and_owned_stop() {
    let mut f = fixture();
    let c = f.create("create");
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM session_turns", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor.launch(&mut f.store, &c.id).unwrap();
    let mut current = wait_state(&mut f, &mut supervisor, &c.id, "ready");
    assert_eq!(
        current.native_session_id.as_deref(),
        Some("generated-independent-conversation")
    );
    assert!(current.runtime_snapshot.is_some());
    if let Ok(path) = std::env::var("AGENTBATON_CONVERSATION_FIXTURE_OUTPUT") {
        fs::write(path, serde_json::to_vec_pretty(&current).unwrap()).unwrap();
    }
    assert!(!f.workspace.root.join("generated-prompts.jsonl").exists());
    for (index, text) in ["first context", "follow up"].iter().enumerate() {
        let key = format!("turn-{index}");
        let revision = current.revision;
        let (queued, send) = f
            .store
            .send_conversation(&c.id, revision, text, &key)
            .unwrap();
        assert!(send);
        assert_eq!(queued.state, "running");
        assert!(
            !f.store
                .send_conversation(&c.id, revision, text, &key)
                .unwrap()
                .1
        );
        supervisor.send(&c.id, text).unwrap();
        current = wait_state(&mut f, &mut supervisor, &c.id, "ready");
    }
    let observed = fs::read_to_string(f.workspace.root.join("generated-prompts.jsonl")).unwrap();
    assert_eq!(observed.lines().count(), 2);
    assert_eq!(f.create("create").id, c.id);
    let cancelled = f
        .store
        .cancel_conversation(&c.id, current.revision, "cancel")
        .unwrap();
    assert_eq!(cancelled.state, "cancel_requested");
    supervisor.cancel(&c.id);
    let stopped = wait_state(&mut f, &mut supervisor, &c.id, "stopped");
    assert!(!supervisor.has_workers());
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT holder FROM workspace_leases WHERE resource=?1",
                [&f.workspace.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        ""
    );
    assert_eq!(f.create("create").state, "stopped");
    supervisor.launch(&mut f.store, &c.id).unwrap();
    assert!(!supervisor.has_workers());
    assert_eq!(stopped.turn_state.as_deref(), Some("completed"));
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    validate_schema(&f.store.conn).unwrap();
    drop(f.store);
    let reopened = Store::open(&f.path).unwrap();
    assert_eq!(reopened.get_conversation(&c.id).unwrap().state, "stopped");
}
#[test]
fn restart_preserves_unknown_fence_budget_and_never_replays() {
    let mut f = fixture();
    let c = f.create("create");
    f.store
        .conn
        .execute(
            "UPDATE conversation_dispatches SET state='sent' WHERE conversation_id=?1",
            [&c.id],
        )
        .unwrap();
    drop(f.store);
    let mut store = Store::open(&f.path).unwrap();
    let unknown = store.get_conversation(&c.id).unwrap();
    assert_eq!(unknown.state, "outcome_unknown");
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT holder FROM workspace_leases WHERE resource=?1",
                [&f.workspace.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        c.id
    );
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    assert!(supervisor
        .record_local_launch_failure(&mut store, &c.id)
        .is_err());
    supervisor.launch(&mut store, &c.id).unwrap();
    assert!(!supervisor.has_workers());
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT active FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project.id)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .create_conversation(
                &f.project.id,
                "Independent conversation",
                &f.workspace.id,
                &f.connection.id,
                &f.grant.id,
                "create"
            )
            .unwrap()
            .state,
        "outcome_unknown"
    );
    assert!(store
        .create_conversation(
            &f.project.id,
            "new",
            &f.workspace.id,
            &f.connection.id,
            &f.grant.id,
            "new"
        )
        .is_err());
    assert!(store
        .cancel_conversation(&c.id, unknown.revision, "cancel")
        .is_err());
}
#[test]
fn admission_is_symmetric_with_tasks_and_no_start_failure_is_owned() {
    let mut f = fixture();
    let c = f.create("conversation");
    let task = f
        .store
        .create(&f.project.id, "formal task", "task")
        .unwrap();
    assert_eq!(
        f.store
            .create_session(
                &task.id,
                &f.workspace.id,
                &f.connection.id,
                &f.grant.id,
                task.revision,
                "blocked"
            )
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    assert!(supervisor
        .record_local_launch_failure(&mut f.store, &c.id)
        .is_err());
    fs::write(&f.connection.executable, "changed").unwrap();
    assert!(supervisor.launch(&mut f.store, &c.id).is_err());
    supervisor
        .record_local_launch_failure(&mut f.store, &c.id)
        .unwrap();
    assert_eq!(f.store.get_conversation(&c.id).unwrap().state, "failed");
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT active FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project.id)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
#[test]
fn cancel_pending_turn_preserves_uncertainty_but_reaps_owned_process() {
    let mut f = fixture();
    let c = f.create("c");
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor.launch(&mut f.store, &c.id).unwrap();
    let ready = wait_state(&mut f, &mut supervisor, &c.id, "ready");
    let (queued, _) = f
        .store
        .send_conversation(&c.id, ready.revision, "hold", "turn")
        .unwrap();
    supervisor.send(&c.id, "hold").unwrap();
    f.store
        .cancel_conversation(&c.id, queued.revision, "cancel")
        .unwrap();
    supervisor.cancel(&c.id);
    let stopped = wait_state(&mut f, &mut supervisor, &c.id, "stopped");
    assert_eq!(stopped.turn_state.as_deref(), Some("outcome_unknown"));
    assert!(
        !f.store
            .send_conversation(&c.id, ready.revision, "hold", "turn")
            .unwrap()
            .1
    );
    assert!(!supervisor.has_workers());
}
#[test]
fn denied_grant_and_changed_runtime_do_not_spawn() {
    let mut f = fixture();
    assert!(f
        .store
        .create_conversation(
            &f.project.id,
            "missing approval",
            &f.workspace.id,
            &f.connection.id,
            "missing",
            "bad"
        )
        .is_err());
    assert_eq!(f.store.list_conversations().unwrap().len(), 0);
    let c = f.create("c");
    let mut grant = f.grant.clone();
    grant.revoked = true;
    f.store
        .conn
        .execute(
            "UPDATE execution_grants SET document=?1 WHERE id=?2",
            params![encode(&grant).unwrap(), grant.id],
        )
        .unwrap();
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    assert!(supervisor.launch(&mut f.store, &c.id).is_err());
    supervisor
        .record_local_launch_failure(&mut f.store, &c.id)
        .unwrap();
    assert!(!supervisor.has_workers());
    assert_eq!(f.store.get_conversation(&c.id).unwrap().state, "failed");
}
#[test]
fn narrower_shared_root_parallel_limit_cannot_be_bypassed_by_conversations() {
    let mut f = fixture();
    let managed = f._dir.path().join("managed-second");
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    let project = f.store.get_project(&f.project.id).unwrap();
    let workspace = f
        .store
        .create_workspace(
            &project.id,
            project.revision,
            &f.workspace.base_revision,
            &managed,
            "second-workspace",
        )
        .unwrap();
    let second = f
        .store
        .create_grant(
            &project.id,
            &workspace.id,
            &f.connection.id,
            8,
            4,
            10000,
            true,
            "second-grant",
        )
        .unwrap();
    // Trusted test fixture authorizes two exact registered workspaces under one
    // root. No renderer path can alter persisted execution grant snapshots.
    let mut grant = f.grant.clone();
    grant.scope.workspace_ids.push(workspace.id.clone());
    grant.workspace_bindings.extend(second.workspace_bindings);
    f.store
        .conn
        .execute(
            "UPDATE execution_grants SET document=?1 WHERE id=?2",
            params![encode(&grant).unwrap(), grant.id],
        )
        .unwrap();
    f.grant = grant;
    let project_limits = json!({"attempts":8,"parallel":4,"runtime_ms":80000,"output_bytes":1048576,"children":4,"reworks":0,"tokens":null,"cost_microunits":null});
    let mut root_limits = project_limits.clone();
    root_limits["parallel"] = json!(1);
    for (scope, limits) in [
        (format!("project:{}", f.project.id), project_limits),
        (format!("root-grant:{}", f.grant.root_grant_id), root_limits),
    ] {
        let meter = json!({"limits":limits,"observed_attempts":0,"runtime_ms":0,"output_bytes":0,"tokens":0,"cost_microunits":0});
        f.store
            .conn
            .execute(
                "INSERT INTO goal_ledgers VALUES(?1,?2)",
                params![scope, encode(&meter).unwrap()],
            )
            .unwrap();
    }
    let first = f.create("first");
    assert_eq!(first.state, "starting");
    assert_eq!(
        f.store
            .create_conversation(
                &f.project.id,
                "second",
                &workspace.id,
                &f.connection.id,
                &f.grant.id,
                "second"
            )
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(f.store.list_conversations().unwrap().len(), 1);
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT active FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project.id)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    // A lease can be freed only by an authenticated local never-started receipt.
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    fs::write(&f.connection.executable, "changed").unwrap();
    assert!(supervisor.launch(&mut f.store, &first.id).is_err());
    supervisor
        .record_local_launch_failure(&mut f.store, &first.id)
        .unwrap();
    let raw: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM goal_ledgers WHERE scope=?1",
            [format!("root-grant:{}", f.grant.root_grant_id)],
            |r| r.get(0),
        )
        .unwrap();
    let meter: Value = decode(&raw).unwrap();
    assert!(meter["runtime_ms"].is_number());
    assert_eq!(meter["observed_attempts"], 1);
}
#[test]
fn task_writer_blocks_conversation_and_unknown_ancestors_count() {
    let mut f = fixture();
    let task = f
        .store
        .create(&f.project.id, "formal task", "task")
        .unwrap();
    let s = f
        .store
        .create_session(
            &task.id,
            &f.workspace.id,
            &f.connection.id,
            &f.grant.id,
            task.revision,
            "session",
        )
        .unwrap();
    assert_eq!(
        f.store
            .create_conversation(
                &f.project.id,
                "blocked",
                &f.workspace.id,
                &f.connection.id,
                &f.grant.id,
                "conversation"
            )
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(f.store.list_conversations().unwrap().len(), 0);
    assert_eq!(f.store.get_session(&s.id).unwrap().state, "starting");
}
#[test]
fn post_migration_schema_limit_rolls_back_instead_of_creating_unreopenable_database() {
    let mut db = Connection::open_in_memory().unwrap();
    for i in 0..250 {
        db.execute_batch(&format!("CREATE TABLE opaque_{i}(id TEXT)"))
            .unwrap();
    }
    db.execute_batch("PRAGMA user_version=17").unwrap();
    assert_eq!(
        crate::conversations::migrate(&mut db, 17).unwrap_err().code,
        "INPUT_CHANGED"
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='conversations'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn cleared_lease_does_not_erase_active_writer_history() {
    let mut f = fixture();
    let c = f.create("first");
    f.store
        .conn
        .execute(
            "UPDATE workspace_leases SET holder='' WHERE resource=?1",
            [&f.workspace.id],
        )
        .unwrap();
    assert_eq!(
        f.store
            .create_conversation(
                &f.project.id,
                "second",
                &f.workspace.id,
                &f.connection.id,
                &f.grant.id,
                "second"
            )
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    let task = f.store.create(&f.project.id, "task", "task").unwrap();
    assert_eq!(
        f.store
            .create_session(
                &task.id,
                &f.workspace.id,
                &f.connection.id,
                &f.grant.id,
                task.revision,
                "session"
            )
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(f.store.get_conversation(&c.id).unwrap().state, "starting");
}

#[test]
fn live_revocation_reaps_owned_conversation_and_keeps_turn_history() {
    let mut f = fixture();
    let c = f.create("create");
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor.launch(&mut f.store, &c.id).unwrap();
    let ready = wait_state(&mut f, &mut supervisor, &c.id, "ready");
    let (queued, _) = f
        .store
        .send_conversation(&c.id, ready.revision, "hold", "turn")
        .unwrap();
    supervisor.send(&c.id, "hold").unwrap();
    let mut grant = f.grant.clone();
    grant.revoked = true;
    f.store
        .conn
        .execute(
            "UPDATE execution_grants SET document=?1 WHERE id=?2",
            params![encode(&grant).unwrap(), grant.id],
        )
        .unwrap();
    let stopped = wait_state(&mut f, &mut supervisor, &c.id, "stopped");
    assert_eq!(stopped.turn_state.as_deref(), Some("outcome_unknown"));
    assert!(stopped.revision > queued.revision);
    assert!(!supervisor.has_workers());
}

#[test]
fn stopped_conversation_invalidates_preexisting_graph_writer_epoch() {
    let mut f = fixture();
    let prior_epoch = 0;
    assert!(!has_writer_after(&f.store.conn, &f.workspace.id, prior_epoch).unwrap());
    let c = f.create("conversation-after-candidate");
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor.launch(&mut f.store, &c.id).unwrap();
    let ready = wait_state(&mut f, &mut supervisor, &c.id, "ready");
    f.store
        .cancel_conversation(&c.id, ready.revision, "stop")
        .unwrap();
    supervisor.cancel(&c.id);
    wait_state(&mut f, &mut supervisor, &c.id, "stopped");
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT holder FROM workspace_leases WHERE resource=?1",
                [&f.workspace.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        ""
    );
    assert!(has_writer_after(&f.store.conn, &f.workspace.id, prior_epoch).unwrap());
    assert!(!has_writer_after(
        &f.store.conn,
        &f.workspace.id,
        i64::try_from(c.fencing_token).unwrap()
    )
    .unwrap());
}
