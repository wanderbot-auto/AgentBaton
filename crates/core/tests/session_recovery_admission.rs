//! Exercise the actual Store and Supervisor with a generated local ACP child.
//! The fixture supplies trusted identity and compatibility observations only.
use agentbaton_core::{
    execution::{DispatchIntent, IntentState},
    sessions::{
        ManagedSession, NativeSessionRecoveryEvidence, ProviderConnection, SelectedSessionHandoff,
        SessionRecoveryAdmission as Admission, SessionRecoveryReason as Reason,
        SessionRecoveryRequest, Supervisor,
    },
    storage::{Store, Task, TaskState},
    RuntimeBinding,
};
use agentbaton_providers::{
    connections::{
        CapabilityEvidence, EvidenceSource, ProtocolRoute, ProviderKind, SupportManifest,
        SupportState,
    },
    installation::BinaryDigest,
};
use rusqlite::{params, Connection};
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    supervisor: Supervisor,
    store: Store,
    private: tempfile::TempDir,
    db: PathBuf,
    workspace: PathBuf,
    traffic: PathBuf,
    task: Task,
    session: ManagedSession,
    connection: ProviderConnection,
}

impl Fixture {
    fn ready(known_identity: bool) -> Self {
        let private = tempfile::tempdir().unwrap();
        let root = private.path().canonicalize().unwrap();
        let source = root.join("source");
        let managed = root.join("managed");
        let runtime = root.join("runtime");
        for directory in [&source, &managed, &runtime] {
            fs::create_dir(directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(source.join("input.txt"), "fixed inputs\n").unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        git(&source, &["add", "input.txt"]);
        git(&source, &["commit", "-m", "Fixture input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let db = root.join("store.sqlite3");
        let mut store = Store::open(&db).unwrap();
        let project = store
            .import_project(&source, "Generated", "project")
            .unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let task = store
            .create_registered(&project.id, "Inspect fixture", "task")
            .unwrap();
        let executable = fixture_child(&root);
        let registered = store
            .register_connection(
                "Local synthetic verifier",
                "custom_acp",
                &executable,
                &[],
                "connection",
            )
            .unwrap();
        let mut supervisor = Supervisor::new(&runtime);
        supervisor.probe(&store, &registered.id).unwrap();
        wait_until(|| {
            supervisor.tick(&mut store).unwrap();
            store.get_connection(&registered.id).unwrap().probe_state != "pending"
        });
        let mut connection = store.get_connection(&registered.id).unwrap();
        assert_eq!(connection.probe_state, "ready");
        if known_identity {
            // The existing registration path only supplies an unknown identity.
            // Seed the trusted identity projection before grant creation.
            connection.identity_ref = "fixture-known-identity".into();
            connection.identity_status = "verified".into();
            Connection::open(&db)
                .unwrap()
                .execute(
                    "UPDATE connections SET document=?1 WHERE id=?2",
                    params![serde_json::to_string(&connection).unwrap(), connection.id],
                )
                .unwrap();
        }
        let grant = store
            .create_grant(
                &project.id,
                &workspace.id,
                &connection.id,
                3,
                1,
                60_000,
                true,
                "grant",
            )
            .unwrap();
        let session = store
            .create_session(
                &task.id,
                &workspace.id,
                &connection.id,
                &grant.id,
                task.revision,
                "session",
            )
            .unwrap();
        supervisor.launch(&mut store, &session.id).unwrap();
        wait_until(|| {
            supervisor.tick(&mut store).unwrap();
            let state = store.get_session(&session.id).unwrap().state;
            assert!(state == "ready" || state == "starting", "{state}");
            state == "ready"
        });
        let session = store.get_session(&session.id).unwrap();
        let task = store.get(&task.id).unwrap();
        Self {
            supervisor,
            store,
            private,
            db,
            workspace: workspace.root,
            traffic: root.join("traffic.log"),
            task,
            session,
            connection,
        }
    }

    fn stopped() -> Self {
        let mut fixture = Self::ready(true);
        fixture.stop();
        fixture
    }

    fn stop(&mut self) {
        self.supervisor.shutdown(&mut self.store).unwrap();
        self.refresh();
        assert_eq!(self.session.state, "stopped");
        assert_eq!(self.task.state, TaskState::Cancelled);
        assert_eq!(
            self.store.intent(&self.session.intent_id).unwrap().state,
            IntentState::Stopped
        );
    }

    fn refresh(&mut self) {
        self.session = self.store.get_session(&self.session.id).unwrap();
        self.task = self.store.get(&self.task.id).unwrap();
    }

    fn request(&self) -> SessionRecoveryRequest<'_> {
        SessionRecoveryRequest {
            session_id: &self.session.id,
            expected_session_revision: self.session.revision,
            expected_task_revision: self.task.revision,
            native: None,
            handoff: None,
        }
    }

    fn references(&self) -> RecoveryReferences {
        let probe = self.connection.version_evidence.as_ref().unwrap();
        let version = probe.version_line.clone().unwrap();
        RecoveryReferences {
            intent: self.store.intent(&self.session.intent_id).unwrap(),
            runtime: self
                .store
                .runtime_binding(&self.session.id)
                .unwrap()
                .unwrap(),
            support: SupportManifest {
                id: "synthetic-verifier".into(),
                installation_id: "generated-installation".into(),
                provider: ProviderKind::CustomAcp,
                provider_version: version.clone(),
                capabilities: vec![CapabilityEvidence {
                    capability: "session-load".into(),
                    state: SupportState::Supported,
                    source: EvidenceSource::ProtocolNegotiation,
                    evidence_ref: "generated-acp-initialize".into(),
                    provider_version: version,
                    observed_at_unix_ms: unix_ms(),
                }],
            },
            digest: probe.executable_sha256.clone(),
        }
    }

    fn handoff(&self) -> SelectedSessionHandoff {
        SelectedSessionHandoff {
            project_id: self.session.project_id.clone(),
            task_id: self.session.task_id.clone(),
            source_attempt_id: self.session.attempt_id.clone(),
            reference: "owner-selected-handoff".into(),
            sha256: BinaryDigest::parse(&agentbaton_workspaces::validation::sha256(
                b"selected synthetic handoff",
            ))
            .unwrap(),
        }
    }

    fn snapshot(&self) -> (String, i64, Vec<u8>) {
        let db = Connection::open(&self.db).unwrap();
        let records = db
            .query_row(
                "SELECT json_array(
                (SELECT document FROM sessions WHERE id=?1),
                (SELECT binding FROM attempts WHERE id=?2),
                (SELECT document FROM dispatch_intents WHERE id=?3),
                (SELECT document FROM tasks WHERE id=?4),
                (SELECT count(*) FROM attempts),
                (SELECT count(*) FROM session_turns),
                (SELECT count(*) FROM workspace_leases WHERE holder!='')
            )",
                params![
                    self.session.id,
                    self.session.attempt_id,
                    self.session.intent_id,
                    self.task.id
                ],
                |r| r.get(0),
            )
            .unwrap();
        let events = db
            .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
            .unwrap();
        (records, events, fs::read(&self.traffic).unwrap())
    }

    fn assess(&self, request: &SessionRecoveryRequest<'_>) -> agentbaton_core::Result<Admission> {
        self.supervisor
            .assess_session_recovery(&self.store, request)
    }

    fn new_attempt(&self, reason: Reason, handoff: Option<SelectedSessionHandoff>) -> Admission {
        Admission::NewAttemptRequired {
            prior_attempt_id: self.session.attempt_id.clone(),
            reason,
            handoff,
        }
    }

    fn reconcile(&self, reason: Reason) -> Admission {
        Admission::Reconcile {
            prior_attempt_id: self.session.attempt_id.clone(),
            reason,
        }
    }
}

struct RecoveryReferences {
    intent: DispatchIntent,
    runtime: RuntimeBinding,
    support: SupportManifest,
    digest: BinaryDigest,
}
impl RecoveryReferences {
    fn evidence<'a>(&'a self, f: &'a Fixture) -> NativeSessionRecoveryEvidence<'a> {
        NativeSessionRecoveryEvidence {
            session_id: &f.session.id,
            attempt_id: &f.session.attempt_id,
            native_session_id: f.session.native_session_id.as_deref().unwrap(),
            binding: &self.intent.binding,
            task_spec_revision: f.task.spec_revision,
            runtime: &self.runtime,
            connection_revision: f.connection.revision,
            route: ProtocolRoute::CustomAcp,
            executable_sha256: &self.digest,
            support: &self.support,
            valid_until_unix_ms: unix_ms() + 60_000,
        }
    }
}

fn fixture_child(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args([
            "-c",
            "import os,sys; print(os.path.realpath(sys.executable))",
        ])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let executable = root.join("fixture-acp");
    let body = r#"import json,os,sys
if sys.argv[1:] == ['--version']:
 print('Synthetic fixture 1.0')
 sys.exit(0)
for line in sys.stdin:
 request=json.loads(line)
 method=request['method']
 with open(os.path.join(os.path.dirname(__file__),'traffic.log'),'a') as log:
  log.write(method+'\n')
 if method=='initialize':
  result={'protocolVersion':1,'agentCapabilities':{'loadSession':True}}
 elif method=='session/new':
  result={'sessionId':'synthetic-native-id'}
 elif method=='session/prompt':
  assert request['params']['sessionId']=='synthetic-native-id'
  result={'stopReason':'end_turn'}
 else:
  sys.exit(3)
 print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}),flush=True)
"#;
    fs::write(&executable, format!("#!{}\n{body}", python.trim())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

fn git(path: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Recovery fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "fixture process did not reach the expected state"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}

#[test]
fn accepts_only_as_candidate_with_pins_and_preserves_the_old_attempt_and_protocol_traffic() {
    let mut f = Fixture::ready(true);
    let prompt = "Synthetic original turn";
    let (_, queued) = f
        .store
        .send_session(&f.session.id, prompt, f.session.revision, "original-turn")
        .unwrap();
    assert!(queued);
    f.supervisor.send(&f.session.id, prompt).unwrap();
    wait_until(|| {
        f.supervisor.tick(&mut f.store).unwrap();
        f.store
            .get_session(&f.session.id)
            .unwrap()
            .turn_state
            .as_deref()
            == Some("completed")
    });
    f.stop();
    let refs = f.references();
    let evidence = refs.evidence(&f);
    let before = f.snapshot();
    assert_eq!(before.2, b"initialize\nsession/new\nsession/prompt\n");
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        Admission::NativeEligible {
            attempt_id: f.session.attempt_id.clone(),
            native_session_id: "synthetic-native-id".into(),
        }
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn missing_compatibility_preserves_only_the_explicitly_selected_same_attempt_handoff() {
    let f = Fixture::stopped();
    let selected = f.handoff();
    let before = f.snapshot();
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            handoff: Some(&selected),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(
            Reason::CompatibilityEvidenceUnavailable,
            Some(selected.clone())
        )
    );
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.new_attempt(Reason::CompatibilityEvidenceUnavailable, None)
    );
    for wrong in [
        SelectedSessionHandoff {
            project_id: "another-project".into(),
            ..selected.clone()
        },
        SelectedSessionHandoff {
            task_id: "another-task".into(),
            ..selected.clone()
        },
        SelectedSessionHandoff {
            source_attempt_id: "another-attempt".into(),
            ..selected.clone()
        },
    ] {
        assert_eq!(
            f.assess(&SessionRecoveryRequest {
                handoff: Some(&wrong),
                ..f.request()
            })
            .unwrap_err()
            .code,
            "INVALID_REQUEST"
        );
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn changed_workspace_input_or_approved_task_spec_requires_new_attempt() {
    let f = Fixture::stopped();
    let refs = f.references();
    let evidence = NativeSessionRecoveryEvidence {
        task_spec_revision: f.task.spec_revision + 1,
        ..refs.evidence(&f)
    };
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(Reason::WorkspaceOrInputChanged, None)
    );
    let evidence = refs.evidence(&f);
    fs::write(f.workspace.join("input.txt"), "changed\n").unwrap();
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(Reason::WorkspaceOrInputChanged, None)
    );
    fs::write(f.workspace.join("input.txt"), "fixed inputs\n").unwrap();
    git(
        &f.workspace,
        &["commit", "--allow-empty", "-m", "Different input revision"],
    );
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(Reason::WorkspaceOrInputChanged, None)
    );
}

#[test]
fn version_identity_configuration_and_runtime_evidence_are_exact() {
    let f = Fixture::stopped();
    let refs = f.references();
    let mut old_support = refs.support.clone();
    old_support.provider_version = "Synthetic outdated version".into();
    old_support.capabilities[0]
        .provider_version
        .clone_from(&old_support.provider_version);
    let mut no_resume = refs.support.clone();
    no_resume.capabilities[0].state = SupportState::Unsupported;
    let mut fixture_only = refs.support.clone();
    fixture_only.capabilities[0].source = EvidenceSource::FixtureTest;
    let mut other_binding = refs.intent.binding.clone();
    other_binding.identity_ref = "another-identity".into();
    let mut other_config = refs.intent.binding.clone();
    other_config.config_revision = "different-config".into();
    let mut other_snapshot = refs.runtime.clone();
    other_snapshot.plan_digest = "another-plan".into();
    for evidence in [
        NativeSessionRecoveryEvidence {
            support: &old_support,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            support: &no_resume,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            support: &fixture_only,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            binding: &other_binding,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            binding: &other_config,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            runtime: &other_snapshot,
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            native_session_id: "another-native",
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            attempt_id: "another-attempt",
            ..refs.evidence(&f)
        },
        NativeSessionRecoveryEvidence {
            valid_until_unix_ms: 1,
            ..refs.evidence(&f)
        },
    ] {
        assert_eq!(
            f.assess(&SessionRecoveryRequest {
                native: Some(&evidence),
                ..f.request()
            })
            .unwrap(),
            f.new_attempt(Reason::CompatibilityEvidenceMismatch, None)
        );
    }
    fs::write(&f.connection.executable, b"changed synthetic executable").unwrap();
    let evidence = refs.evidence(&f);
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(Reason::CompatibilityEvidenceMismatch, None)
    );
}

#[test]
fn legacy_snapshot_or_unverified_identity_cannot_resume() {
    let mut f = Fixture::ready(false);
    f.stop();
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.new_attempt(Reason::IdentityUnverified, None)
    );
    let db = Connection::open(&f.db).unwrap();
    db.execute(
        "DELETE FROM session_runtime_snapshots WHERE session_id=?1",
        [&f.session.id],
    )
    .unwrap();
    db.execute(
        "UPDATE sessions SET document=json_remove(document,'$.runtime_snapshot') WHERE id=?1",
        [&f.session.id],
    )
    .unwrap();
    f.refresh();
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.new_attempt(Reason::RuntimeSnapshotUnavailable, None)
    );
}

#[test]
fn live_or_disconnected_process_and_lost_dispatch_history_require_reconciliation() {
    let mut f = Fixture::ready(true);
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.reconcile(Reason::UnresolvedProcess)
    );
    let db = Connection::open(&f.db).unwrap();
    f.stop();
    db.execute("UPDATE dispatch_intents SET state='outcome_unknown',document=json_set(document,'$.state','outcome_unknown') WHERE id=?1",
        [&f.session.intent_id]).unwrap();
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.reconcile(Reason::UnresolvedProcess)
    );
    db.execute(
        "DELETE FROM dispatch_intents WHERE id=?1",
        [&f.session.intent_id],
    )
    .unwrap();
    assert_eq!(
        f.assess(&f.request()).unwrap(),
        f.reconcile(Reason::IncompleteHistory)
    );
}

#[test]
fn unknown_turn_and_expired_writer_lease_do_not_yield_a_handoff_or_new_attempt() {
    let f = Fixture::stopped();
    let selected = f.handoff();
    let request = SessionRecoveryRequest {
        handoff: Some(&selected),
        ..f.request()
    };
    let db = Connection::open(&f.db).unwrap();
    db.execute(
        "INSERT INTO session_turns VALUES('fixture-turn',?1,'outcome_unknown','{}','fixture-key','{}')",
        [&f.session.id],
    ).unwrap();
    assert_eq!(
        f.assess(&request).unwrap(),
        f.reconcile(Reason::UnresolvedTurn)
    );
    db.execute("DELETE FROM session_turns WHERE id='fixture-turn'", [])
        .unwrap();
    db.execute(
        "INSERT INTO events(document) VALUES(json_object(
             'aggregate_id',?1,'kind','session.turn_ended',
             'payload',json_object('turn_state','outcome_unknown')))",
        [&f.session.id],
    )
    .unwrap();
    let unknown_turn_event = db.last_insert_rowid();
    assert_eq!(
        f.assess(&request).unwrap(),
        f.reconcile(Reason::UnresolvedTurn)
    );
    db.execute("DELETE FROM events WHERE sequence=?1", [unknown_turn_event])
        .unwrap();
    db.execute(
        "UPDATE workspace_leases SET holder='unjoined-writer',expires_at=1 WHERE resource=?1",
        [&f.session.workspace_id],
    )
    .unwrap();
    assert_eq!(
        f.assess(&request).unwrap(),
        f.reconcile(Reason::UnresolvedWriter)
    );
}

#[test]
fn matching_evidence_cannot_hide_on_disk_configuration_drift() {
    let f = Fixture::stopped();
    let refs = f.references();
    let evidence = refs.evidence(&f);
    let config = f
        .private
        .path()
        .join("runtime/runtime-config")
        .join(&refs.runtime.snapshot.id)
        .join("acp/configuration.json");
    fs::write(&config, b"not the approved bytes").unwrap();
    assert_eq!(
        f.assess(&SessionRecoveryRequest {
            native: Some(&evidence),
            ..f.request()
        })
        .unwrap(),
        f.new_attempt(Reason::RuntimeSnapshotUnavailable, None)
    );
}

#[test]
fn stale_caller_cannot_reuse_a_recovery_decision() {
    let f = Fixture::stopped();
    for request in [
        SessionRecoveryRequest {
            expected_session_revision: f.session.revision - 1,
            ..f.request()
        },
        SessionRecoveryRequest {
            expected_task_revision: f.task.revision - 1,
            ..f.request()
        },
    ] {
        assert_eq!(f.assess(&request).unwrap_err().code, "STALE_REVISION");
    }
}
