//! Actual public admission boundaries with a generated local malformed protocol
//! child. No vendor/model/account. Timing is diagnostic, never a larger deadline
//! or a substitute for the unchanged 1000ms command lifecycle acceptance test.
use agentbaton_core::{
    sessions::Supervisor,
    storage::{Store, TaskState},
};
use rusqlite::Connection;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn phase<T>(label: &str, action: impl FnOnce() -> T) -> T {
    // Optional external Git-count/profile harness is confined to this test
    // executable. The production library reads no profiling environment flag.
    if let Ok(path) = std::env::var("AGENTBATON_TEST_ADMISSION_PROFILE") {
        fs::write(path, label).unwrap();
    }
    let start = Instant::now();
    let result = action();
    eprintln!(
        "ADMISSION_PROFILE phase={label} elapsed_us={}",
        start.elapsed().as_micros()
    );
    result
}
struct Fixture {
    _dir: tempfile::TempDir,
    db: PathBuf,
    root: PathBuf,
    store: Store,
    supervisor: Supervisor,
    task: String,
    workspace: String,
    connection: String,
    grant: String,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = dir.path().canonicalize().unwrap().join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let db = runtime.join("baton.sqlite3");
        drop(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&db)
                .unwrap(),
        );
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), "fixed input\n").unwrap();
        fs::write(source.join(".gitignore"), "fixture-started\n").unwrap();
        git(&source, &["add", "input.txt", ".gitignore"]);
        git(&source, &["commit", "-m", "generated input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let executable = dir.path().join("fixture-provider");
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        fs::write(&executable,format!("#!{}\nimport sys,json\nif '--version' in sys.argv:\n print('generated fixture 1.0'); sys.exit(0)\nfor line in sys.stdin:\n r=json.loads(line)\n if r.get('method')=='initialize':\n  open('fixture-started','w').write('owned generated child')\n  print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':{{'protocolVersion':1,'agentCapabilities':{{}}}}}}),flush=True)\n elif r.get('method')=='session/new':\n  print('malformed generated protocol',flush=True); sys.exit(0)\n",String::from_utf8(python.stdout).unwrap().trim())).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let executable = executable.canonicalize().unwrap();
        let managed = runtime.join("workspaces");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&db).unwrap();
        assert_eq!(
            Connection::open(&db)
                .unwrap()
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            16
        );
        let p = store
            .import_project(&source, "Admission fixture", "project")
            .unwrap();
        let w = store
            .create_workspace(&p.id, p.revision, &head, &managed, "workspace")
            .unwrap();
        let task = store
            .create_registered(&p.id, "Exercise real admission guard", "task")
            .unwrap();
        let c = store
            .register_connection(
                "Generated credential-free malformed child",
                "custom_acp",
                &executable,
                &[],
                "connection",
            )
            .unwrap();
        let mut supervisor = Supervisor::new(&runtime);
        supervisor.probe(&store, &c.id).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        while store.get_connection(&c.id).unwrap().probe_state != "ready" {
            assert!(Instant::now() < end);
            supervisor.tick(&mut store).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        let grant = store
            .create_grant(&p.id, &w.id, &c.id, 4, 1, 60_000, true, "grant")
            .unwrap();
        Self {
            _dir: dir,
            db,
            root: w.root,
            store,
            supervisor,
            task: task.id,
            workspace: w.id,
            connection: c.id,
            grant: grant.id,
        }
    }
}
#[test]
fn real_admission_profile_and_post_reservation_input_drift_remain_guarded() {
    let mut f = Fixture::new();
    phase("readonly_validation", || {
        f.store.validate_grant(
            &f.grant,
            &f.store.get(&f.task).unwrap().project_id,
            &f.workspace,
            &f.connection,
        )
    })
    .unwrap();
    let s = phase("reservation", || {
        f.store
            .create_session(&f.task, &f.workspace, &f.connection, &f.grant, 1, "reserve")
    })
    .unwrap();
    fs::write(f.root.join("input.txt"), "changed after reservation\n").unwrap();
    assert_eq!(
        phase("drifted_launch", || f
            .supervisor
            .launch(&mut f.store, &s.id))
        .unwrap_err()
        .code,
        "INPUT_CHANGED"
    );
    assert!(!f.root.join("fixture-started").exists());
    assert_eq!(
        f.store.intent(&s.intent_id).unwrap().state,
        agentbaton_core::execution::IntentState::Prepared
    );
    assert_eq!(f.store.get(&f.task).unwrap().state, TaskState::Draft);
    fs::write(f.root.join("input.txt"), "fixed input\n").unwrap();
    phase("launch", || f.supervisor.launch(&mut f.store, &s.id)).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while f.supervisor.has_workers() {
        assert!(Instant::now() < end);
        f.supervisor.tick(&mut f.store).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(f.root.join("fixture-started").exists());
    assert_eq!(
        f.store.get(&f.task).unwrap().state,
        TaskState::OutcomeUnknown
    );
    assert_eq!(
        f.store.intent(&s.intent_id).unwrap().state,
        agentbaton_core::execution::IntentState::Stopped
    );
    let c = Connection::open(&f.db).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("root:{}", f.task)],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
