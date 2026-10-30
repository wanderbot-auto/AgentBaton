//! Actual Unix IPC + generated ACP + managed Git integration. The private
//! approvals.finalize caller below is a trusted test host, not an OS dialog.
//! No normal-path authority, acceptance, pass, lease or stop is inserted by SQL.
use super::{git, result, Server};
use agentbaton_core::{goal_plan::*, goal_store::*};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const REPORT: &[u8] = b"fixed report";
const TARGET: &str = "refs/heads/integration";

struct RuntimeFixture {
    // Explicit Drop joins the daemon before deleting its authenticated roots.
    server: Server,
    _directory: RetainedDirectory,
    root: PathBuf,
    project_root: PathBuf,
    workspace: Value,
    task: Value,
    grant: Value,
    initial_target: String,
    validator_program: PathBuf,
    scope: Value,
    graph: Value,
}
// This guard exists before fixture setup, so construction failures retain the
// same generated roots too. It never touches user Git or another test's files.
struct RetainedDirectory(Option<tempfile::TempDir>);
impl Drop for RetainedDirectory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(directory) = self.0.take() {
                eprintln!(
                    "Retained integration failure fixture: {}",
                    directory.keep().display()
                );
            }
        }
    }
}
impl Drop for RuntimeFixture {
    fn drop(&mut self) {
        self.server.finish();
    }
}

fn rpc(
    server: &Server,
    method: &str,
    params: Value,
    key: Option<&str>,
    cas: Option<Value>,
) -> Value {
    let mut frame = json!({"method":method,"params":params});
    if let Some(key) = key {
        frame["idempotency_key"] = json!(key);
    }
    if let Some(revision) = cas {
        frame["expected_revision"] = revision;
    }
    result(server.request(frame))
}
fn private_owner(server: &Server, proposal: &Value) -> Value {
    assert!(proposal["id"].as_str().is_some_and(|s| !s.is_empty()));
    rpc(
        server,
        "approvals.finalize",
        json!({"proposal_id":proposal["id"]}),
        Some(&format!("proposal:{}", proposal["id"].as_str().unwrap())),
        Some(proposal["revision"].clone()),
    )
}
fn approved(server: &Server, method: &str, params: Value, key: &str) -> Value {
    let p = rpc(server, method, params, Some(key), None);
    private_owner(server, &p)
}
fn observe(server: &Server, method: &str, params: Value, ready: impl Fn(&Value) -> bool) -> Value {
    // The existing local fixture's observation boundary; never a new runtime cap.
    let started = Instant::now();
    loop {
        let value = rpc(server, method, params.clone(), None, None);
        if ready(&value) {
            return value;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "observation expired: {method}, state={}, code={}",
            value["run"]["state"],
            value["run"]["last_error"]["code"]
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn provider(root: &Path) -> PathBuf {
    let output = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let python = String::from_utf8(output.stdout).unwrap();
    let script = root.join("generated-acp");
    // The private observation is written by the actual generated ACP, not by
    // the host pretending a TaskRequest was delivered. No network or model.
    let observation = serde_json::to_string(&root.join("observed-task-request.json")).unwrap();
    let body = r#"
import json, os, pathlib, sys
if '--version' in sys.argv:
 print('account-free integration ACP 1'); sys.exit(0)
def send(v):
 print(json.dumps(v), flush=True)
for line in sys.stdin:
 q=json.loads(line); m=q.get('method'); i=q.get('id')
 if m=='initialize':
  send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert q['params']['cwd']==os.getcwd() and q['params']['mcpServers']==[]
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'generated-integration-session'}})
 elif m=='session/prompt':
  text=''.join(p.get('text','') for p in q['params']['prompt'])
  c=json.loads(text)
  assert set(c)=={'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location','task_binding','validation_contracts'}
  assert c['schema_version']==1 and c['kind']=='agentbaton_goal_task_request'
  assert c['workspace']['canonical_root']==os.getcwd()
  assert c['node']['id']=='produce-report' and c['lineage']['attempt_id']
  assert c['inputs'][0]['candidate'] is None
  assert c['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
  assert c['policy']['enforced']['filesystem_sandbox'] is False
  assert c['policy']['enforced']['network_sandbox'] is False
  assert c['policy']['enforced']['output_quota'] is False
  pathlib.Path(OBSERVATION).write_text(json.dumps(c))
  pathlib.Path('reports').mkdir(exist_ok=True)
  pathlib.Path('reports/report.txt').write_bytes(b'fixed report')
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}})
"#;
    fs::write(
        &script,
        format!("#!{}\nOBSERVATION={}\n{}", python.trim(), observation, body),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    script
}
fn budget() -> BudgetLimits {
    BudgetLimits {
        attempts: 8,
        parallel: 1,
        runtime_ms: 20_000,
        output_bytes: 8192,
        children: 1,
        reworks: 0,
        tokens: None,
        cost_microunits: None,
    }
}
fn reservation(runtime_ms: u64, output_bytes: u64) -> AttemptReservation {
    AttemptReservation {
        runtime_ms,
        output_bytes,
        tokens: None,
        cost_microunits: None,
    }
}
impl RuntimeFixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("integration-runtime-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let root = directory.path().canonicalize().unwrap();
        let directory = RetainedDirectory(Some(directory));
        let project_root = root.join("project");
        fs::create_dir(&project_root).unwrap();
        git(&project_root, &["init", "--initial-branch=main"]);
        fs::create_dir(project_root.join("inputs")).unwrap();
        fs::write(project_root.join("inputs/file"), b"baseline").unwrap();
        fs::write(
            project_root.join("unselected.txt"),
            b"unselected baseline\n",
        )
        .unwrap();
        fs::write(project_root.join(".gitignore"), b"/reports/\n").unwrap();
        git(&project_root, &["add", "."]);
        git(
            &project_root,
            &["commit", "-m", "generated integration baseline"],
        );
        git(&project_root, &["branch", "integration"]);
        let initial_target = git(&project_root, &["rev-parse", TARGET]);
        let script = provider(&root);
        // A private, single-link image of a fixed local shell is approved by
        // the real Command policy. Do not relax system-program identity gates.
        let validator_program = root.join("fixed-sh");
        fs::copy(fs::canonicalize("/bin/sh").unwrap(), &validator_program).unwrap();
        fs::set_permissions(&validator_program, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(fs::metadata(&validator_program).unwrap().nlink(), 1);
        let server = Server::start(root.join("runtime"));
        assert_eq!(
            rpc(&server, "system.snapshot", json!({}), None, None)["database_schema_version"],
            17
        );
        let project = rpc(
            &server,
            "projects.import",
            json!({"path":project_root,"name":"Generated accepted integration"}),
            Some("project"),
            None,
        );
        let task = rpc(
            &server,
            "tasks.create",
            json!({"project_id":project["id"],"goal":"Accept and integrate only the exact generated report"}),
            Some("task"),
            None,
        );
        let resolved = rpc(
            &server,
            "workspaces.resolve",
            json!({"task_id":task["id"],"expected_task_revision":task["revision"],"expected_project_revision":project["revision"],"host_id":"local"}),
            None,
            None,
        );
        assert_eq!(resolved["execution_authorized"], false);
        let prepared = rpc(
            &server,
            "workspaces.prepare_task_git",
            json!({"task_id":task["id"],"expected_spec_revision":resolved["spec_revision"],"expected_project_revision":project["revision"],"host_id":"local",
            "expected_source_pin":{"canonical_root":resolved["source"]["canonical_root"],"device":resolved["source"]["device"],"inode":resolved["source"]["inode"],"common_dir":resolved["source"]["kind"]["common_dir"],"head":resolved["source_head"]},"preparation_key":resolved["preparation_key"]}),
            Some("prepare"),
            Some(task["revision"].clone()),
        );
        assert_eq!(prepared["identity_revalidated"], true);
        assert_eq!(prepared["execution_authorized"], false);
        assert_eq!(prepared["binding"]["state"], "ready");
        let workspace_id = prepared["binding"]["receipt"]["workspace_id"]
            .as_str()
            .unwrap();
        let workspaces = rpc(
            &server,
            "workspaces.list",
            json!({"project_id":project["id"]}),
            None,
            None,
        );
        let workspace = workspaces
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == workspace_id)
            .unwrap()
            .clone();
        let task = rpc(
            &server,
            "tasks.get",
            json!({"task_id":task["id"]}),
            None,
            None,
        );
        let connection = approved(
            &server,
            "connections.propose",
            json!({"name":"Generated account-free ACP","provider":"custom_acp","executable":script,"arguments":[]}),
            "connection",
        );
        observe(&server, "connections.list", json!({}), |rows| {
            rows.as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == connection["id"] && r["probe_state"] == "ready")
        });
        // Grant is approved after setup. Original Work 4s and shared runtime
        // 20s remain independent from this finite 30s authority lifetime.
        let grant = approved(
            &server,
            "grants.propose",
            json!({"project_id":project["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":8,"max_concurrency":1,"max_duration_ms":30_000}),
            "grant",
        );
        let selection = GoalScopeSelection {
            task_id:task["id"].as_str().unwrap().into(),expected_task_revision:task["revision"].as_u64().unwrap(),
            grant_id:grant["id"].as_str().unwrap().into(),expected_grant_revision:grant["revision"].as_u64().unwrap(),
            max_nodes:2,new_max_children:1,new_max_depth:1,root_budget:budget(),project_budget:budget(),
            inputs:vec![ArtifactRequirement {artifact_id:"fixed-input".into(),relative_path:"inputs/file".into(),max_bytes:1024,expected_sha256:Some(sha256(b"baseline"))}],
            outputs:vec![GoalOutputSelection {artifact:ArtifactRequirement {artifact_id:"report".into(),relative_path:"reports/report.txt".into(),max_bytes:1024,expected_sha256:None},validators:vec![]}],
            writable_prefixes:vec!["reports".into()],owner_acceptance_criteria:"Owner must accept the exact captured generated report, separately from process success".into(),
        };
        let scope = approved(
            &server,
            "goal_scopes.propose",
            serde_json::to_value(selection).unwrap(),
            "scope",
        );
        let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
        let contract = view.outputs["report"].clone();
        assert_eq!(
            view.contracts[&contract],
            vec!["goal-owner-review".to_owned()]
        );
        let producer = PlanNode {
            id: "produce-report".into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "integration-generated-report".into(),
            dependencies: vec![],
            workspace_id: view.workspace_id.clone(),
            executor: Executor::Connection {
                binding: Box::new(view.binding.clone()),
                program_sha256: view.program_sha256.clone(),
            },
            recipients: vec![],
            required_inputs: vec![RequiredInput {
                goal_task_id: view.task_id.clone(),
                artifact_id: "fixed-input".into(),
                contract_id: view.inputs["fixed-input"].contract_id.clone(),
                source: InputSource::ApprovedExternal {
                    sha256: view.inputs["fixed-input"].sha256.clone(),
                },
            }],
            outputs: vec![PlannedOutput {
                artifact_id: "report".into(),
                contract_id: contract.clone(),
            }],
            required_validators: view.contracts[&contract].clone(),
            side_effect: SideEffect::WorkspaceWrite,
            write_set: vec![WriteTarget::Workspace {
                path: "reports/report.txt".into(),
            }],
            budget: BudgetLimits {
                attempts: 1,
                parallel: 1,
                runtime_ms: 5000,
                output_bytes: 2048,
                children: 0,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            },
            reservation: reservation(4000, 2048),
            retry: RetryPolicy {
                max_attempts: 1,
                max_reworks: 0,
                retry_classes: vec![],
            },
            idempotency_key: "producer-once".into(),
            external_action_id: None,
        };
        let mut owner = producer.clone();
        owner.id = "owner-report".into();
        owner.kind = NodeKind::OwnerApproval;
        owner.goal = "Owner reviews exact report".into();
        owner.executor = Executor::Owner {
            rule_id: "goal-owner-review".into(),
        };
        owner.dependencies = vec![producer.id.clone()];
        owner.required_inputs = vec![RequiredInput {
            goal_task_id: view.task_id.clone(),
            artifact_id: "report".into(),
            contract_id: contract,
            source: InputSource::Node {
                node_id: producer.id.clone(),
                candidate_for_validation: true,
            },
        }];
        owner.outputs.clear();
        owner.write_set.clear();
        owner.side_effect = SideEffect::AdvisoryReadOnly;
        owner.idempotency_key = "owner-once".into();
        let plan = GoalPlanProposal {
            schema_version: 1,
            plan_id: "integration-runtime-plan".into(),
            goal: view.goal.clone(),
            scope_revision: view.authority_revision,
            revision: 1,
            expected_revision: 0,
            acceptance_rules: view.required_acceptance_rules.clone(),
            nodes: vec![producer, owner],
        };
        rpc(
            &server,
            "goal_plans.propose",
            json!({"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()}),
            Some("plan"),
            Some(json!(0)),
        );
        let ledger = rpc(&server, "system.snapshot", json!({}), None, None)["sequence"].clone();
        let graph = approved(
            &server,
            "goal_graph_runs.propose",
            json!({"scope_id":scope["id"],"expected_plan_revision":1,"expected_ledger_revision":ledger}),
            "graph",
        );
        Self {
            server,
            _directory: directory,
            root,
            project_root,
            workspace,
            task,
            grant,
            initial_target,
            validator_program,
            scope,
            graph,
        }
    }
    fn source(&self) -> PathBuf {
        PathBuf::from(self.workspace["root"].as_str().unwrap())
    }
    fn owner_ready(&self) -> Value {
        let d =
            observe(
                &self.server,
                "goal_graph_runs.get",
                json!({"run_id":self.graph["run"]["id"]}),
                |d| {
                    d["nodes"].as_array().unwrap().iter().any(|n| {
                        n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner"
                    })
                },
            );
        assert_eq!(d["task_accepted"], false);
        assert_eq!(d["candidates"].as_array().unwrap().len(), 1);
        let c = &d["candidates"][0];
        assert_eq!(c["source"], "baton_goal_evidence");
        assert_eq!(c["artifacts"][0]["sha256"], sha256(REPORT));
        assert_eq!(c["artifacts"][0]["bytes"], REPORT.len());
        assert!(
            c["stop_receipt_sequence"].as_u64().unwrap()
                < c["capture_receipt_sequence"].as_u64().unwrap()
        );
        let session = rpc(
            &self.server,
            "sessions.get",
            json!({"session_id":c["session_id"]}),
            None,
            None,
        );
        assert_eq!(session["state"], "stopped");
        let request: Value = serde_json::from_slice(
            &fs::read(self.root.join("observed-task-request.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(request["run_id"], self.graph["run"]["id"]);
        assert_eq!(request["lineage"]["attempt_id"], c["attempt_id"]);
        assert_eq!(request["plan_digest"], c["plan_digest"]);
        assert!(request["task_binding"].is_object());
        assert_eq!(
            fs::read(self.source().join("reports/report.txt")).unwrap(),
            REPORT
        );
        d
    }
    fn command_definition(&self, candidate: &Value) -> Value {
        approved(
            &self.server,
            "command_validation_policies.propose",
            json!({"session_id":candidate["session_id"],"executable":self.validator_program,"mode":"native_executable","script":null,
            "inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":sha256(b"baseline")}],
            "outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],
            "arguments":["-c","test -s reports/report.txt"],"environment":{},"expected_exit_codes":[0],"wall_time_ms":500,"cleanup_time_ms":500,"stdout_limit":128,"stderr_limit":128,
            "require":{"filesystem_isolation":false,"network_isolation":false,"process_tree_containment":false,"execution_time_input_immutability":false,"auxiliary_source_immutability":false,"sealed_executable_image":false}}),
            "correctness-definition",
        )
    }
    fn accept(&self) -> Value {
        let d = rpc(
            &self.server,
            "goal_graph_runs.get",
            json!({"run_id":self.graph["run"]["id"]}),
            None,
            None,
        );
        let p = rpc(
            &self.server,
            "goal_node_approvals.propose",
            json!({"run_id":self.graph["run"]["id"],"node_id":"owner-report","expected_plan_revision":d["run"]["plan_revision"],"expected_ledger_revision":d["ledger_revision"]}),
            Some("accept-exact-report"),
            Some(d["run"]["revision"].clone()),
        );
        assert_eq!(p["snapshot"]["candidate_receipts"], d["candidates"]);
        let decision = private_owner(&self.server, &p);
        assert_eq!(decision["decision"], "accepted");
        assert_eq!(decision["approval_id"], p["id"]);
        assert_eq!(decision["task_accepted"], true);
        let accepted = rpc(
            &self.server,
            "goal_graph_runs.get",
            json!({"run_id":self.graph["run"]["id"]}),
            None,
            None,
        );
        assert_eq!(accepted["run"]["state"], "accepted");
        assert_eq!(accepted["task_accepted"], true);
        assert_eq!(
            accepted["task_acceptance"]["source"],
            "baton_goal_acceptance"
        );
        let task = rpc(
            &self.server,
            "tasks.get",
            json!({"task_id":self.task["id"]}),
            None,
            None,
        );
        assert_eq!(task["state"], "accepted");
        assert_eq!(task["acceptance_ref"], accepted["task_acceptance"]["id"]);
        assert_eq!(
            task["acceptance_sequence"],
            accepted["task_acceptance"]["sequence"]
        );
        assert_eq!(
            task["revision"],
            accepted["task_acceptance"]["task_revision"]
        );
        accepted
    }
    fn approve_integration(&self, definition: &Value) -> Value {
        let context = rpc(
            &self.server,
            "integration_context.get",
            json!({"task_id":self.task["id"],"goal_scope_id":self.scope["id"]}),
            None,
            None,
        );
        assert_eq!(context["preview_only"], true);
        assert_eq!(
            context["bindings"]["goal_scope_revision"],
            self.scope["revision"]
        );
        let reference = json!({"kind":"command","policy_id":definition["id"],"revision":definition["revision"]});
        let matches = context["available_validators"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["reference"] == reference)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        // The original Scope requires only Owner review. This separately
        // approved integration names new fixed correctness authority explicitly.
        assert!(context["bindings"]["validators"]
            .as_array()
            .unwrap()
            .is_empty());
        let selection = json!({"task_id":self.task["id"],"expected_task_revision":context["task_revision"],"expected_spec_revision":context["bindings"]["spec_revision"],"goal_scope_id":self.scope["id"],"expected_scope_revision":context["bindings"]["goal_scope_revision"],"host_id":"local","target_ref":TARGET,"expected_target_head":self.initial_target,"max_rounds":1,"on_target_advance":"block","ttl_ms":30_000,
            "stage_reservations":{"preparation":reservation(1000,1024),"validators":[{"validator":reference,"reservation":reservation(1500,256)}],"cas":reservation(500,1024)}});
        let proposal = rpc(
            &self.server,
            "integration_policies.propose",
            selection.clone(),
            Some("integration-policy"),
            None,
        );
        assert_eq!(proposal["kind"], "integration");
        assert_eq!(proposal["snapshot"]["intent"], selection);
        assert_eq!(
            proposal["snapshot"]["bindings"]["validators"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            proposal["snapshot"]["bindings"]["validators"][0]["reference"],
            reference
        );
        let policy = private_owner(&self.server, &proposal);
        assert_eq!(policy["id"], proposal["id"]);
        assert_eq!(policy["approval_id"], proposal["id"]);
        assert_eq!(policy["selection"], selection);
        assert_eq!(policy["revision"], 1);
        assert_eq!(policy["bindings"]["root_grant_id"], self.grant["id"]);
        policy
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SourceImage {
    head: String,
    refs_except_target: Vec<String>,
    index: Vec<u8>,
    staged: String,
    unstaged: String,
    status: String,
    files: BTreeMap<PathBuf, Vec<u8>>,
}
fn source_image(root: &Path) -> SourceImage {
    fn files(root: &Path, current: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
            if relative == Path::new(".git") {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                files(root, &entry.path(), out);
            } else {
                assert!(metadata.is_file());
                out.insert(relative, fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut content = BTreeMap::new();
    files(root, root, &mut content);
    let path = PathBuf::from(git(root, &["rev-parse", "--git-path", "index"]));
    let index = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    SourceImage {
        head: git(root, &["rev-parse", "HEAD"]),
        refs_except_target: git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])
            .lines()
            .filter(|r| !r.starts_with(&format!("{TARGET} ")))
            .map(str::to_owned)
            .collect(),
        index: fs::read(index).unwrap(),
        staged: git(root, &["diff", "--cached", "--binary"]),
        unstaged: git(root, &["diff", "--binary"]),
        status: git(root, &["status", "--porcelain=v1", "--untracked-files=all"]),
        files: content,
    }
}
fn user_changes(f: &RuntimeFixture) {
    // Only the user's managed source, after authentic Graph acceptance and
    // before independent integration approval. Registered Project stays clean.
    let root = f.source();
    fs::write(root.join("unselected.txt"), b"staged owner change\n").unwrap();
    git(&root, &["add", "unselected.txt"]);
    fs::write(root.join("unselected.txt"), b"unstaged owner change\n").unwrap();
    fs::write(
        root.join("owner-untracked.txt"),
        b"keep untracked owner bytes\n",
    )
    .unwrap();
    assert!(git(&root, &["diff", "--cached", "--name-only"]).contains("unselected.txt"));
    assert!(git(&root, &["diff", "--name-only"]).contains("unselected.txt"));
    assert_eq!(git(&f.project_root, &["status", "--porcelain"]), "");
}
fn readonly(f: &RuntimeFixture) -> Connection {
    let db = Connection::open_with_flags(
        f.server.runtime.join("baton.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    db.execute_batch("PRAGMA query_only=ON; BEGIN;").unwrap();
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 17);
    db
}
fn events(db: &Connection) -> Vec<(u64, Value)> {
    let mut statement = db
        .prepare("SELECT sequence,document FROM events ORDER BY sequence")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .map(|r| {
            let (n, s) = r.unwrap();
            (u64::try_from(n).unwrap(), serde_json::from_str(&s).unwrap())
        })
        .collect();
    rows
}
fn unique_event<'a>(events: &'a [(u64, Value)], kind: &str, aggregate: &Value) -> &'a (u64, Value) {
    let matches = events
        .iter()
        .filter(|(_, e)| e["kind"] == kind && e["aggregate_id"] == *aggregate)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one exact {kind}");
    matches[0]
}
fn terminal(f: &RuntimeFixture) -> Value {
    let runs = observe(
        &f.server,
        "integrations.list",
        json!({"task_id":f.task["id"]}),
        |r| !r.as_array().unwrap().is_empty(),
    );
    assert_eq!(runs.as_array().unwrap().len(), 1);
    let id = &runs[0]["id"];
    let done = observe(
        &f.server,
        "integrations.get",
        json!({"integration_id":id}),
        |v| {
            let state = v["run"]["state"].as_str().unwrap();
            assert!(
                !matches!(
                    state,
                    "blocked" | "failed" | "cancelled" | "outcome_unknown" | "stale_target"
                ),
                "integration refused: state={state}, code={}",
                v["run"]["last_error"]["code"]
            );
            state == "confirmed_local_ref"
        },
    );
    assert_eq!(done["run"]["id"], *id);
    done
}
fn assert_closed_chain(
    f: &RuntimeFixture,
    policy: &Value,
    accepted: &Value,
    before_source: &SourceImage,
    before_project: &SourceImage,
) -> Value {
    let done = terminal(f);
    let run = &done["run"];
    assert_eq!(run["policy_id"], policy["id"]);
    assert_eq!(run["policy_revision"], policy["revision"]);
    assert_eq!(run["task_id"], f.task["id"]);
    assert_eq!(run["acceptance_ref"], accepted["task_acceptance"]["id"]);
    assert_eq!(
        run["acceptance_digest"],
        sha256(
            &serde_json::to_vec(
                &serde_json::from_value::<GraphTaskAcceptance>(accepted["task_acceptance"].clone())
                    .unwrap()
            )
            .unwrap()
        )
    );
    assert_eq!(run["expected_target_head"], f.initial_target);
    assert_eq!(run["target_ref"], TARGET);
    assert_eq!(run["round"], 1);
    assert_eq!(run["active_stage"], Value::Null);
    assert_eq!(run["last_error"], Value::Null);
    assert_eq!(run["io_completion"], "worker_joined");
    assert_eq!(done["fence"]["state"], "released");
    let commit = run["integration_commit"].as_str().unwrap();
    let captured = run["source_commit"].as_str().unwrap();
    assert_ne!(commit, f.initial_target);
    assert_ne!(captured, f.initial_target);
    assert_eq!(git(&f.project_root, &["rev-parse", TARGET]), commit);
    assert_eq!(
        git(&f.source(), &["rev-parse", &format!("{commit}^{{tree}}")]),
        run["integration_tree"].as_str().unwrap()
    );
    assert_eq!(
        git(&f.source(), &["rev-parse", &format!("{captured}^{{tree}}")]),
        run["source_tree"].as_str().unwrap()
    );
    // New commits/objects are expected; the sole selected content difference
    // is the exact accepted report, not staged or unstaged unselected edits.
    assert_eq!(
        git(
            &f.source(),
            &["diff", "--name-only", &f.initial_target, commit]
        ),
        "reports/report.txt"
    );
    assert_eq!(
        git(
            &f.source(),
            &["show", &format!("{commit}:reports/report.txt")]
        ),
        "fixed report"
    );
    assert_eq!(
        git(
            &f.source(),
            &["show", &format!("{captured}:reports/report.txt")]
        ),
        "fixed report"
    );
    assert_eq!(
        git(
            &f.source(),
            &["show", &format!("{captured}:unselected.txt")]
        ),
        "unselected baseline"
    );
    assert_eq!(source_image(&f.source()), *before_source);
    assert_eq!(source_image(&f.project_root), *before_project);
    let receipts = run["validation_receipts"].as_array().unwrap();
    assert_eq!(receipts.len(), 1);
    let validation = &receipts[0];
    assert_eq!(
        validation["validator"],
        policy["selection"]["stage_reservations"]["validators"][0]["validator"]
    );
    assert_eq!(
        validation["definition_digest"],
        policy["bindings"]["validators"][0]["definition_digest"]
    );
    assert_eq!(validation["integration_commit"], run["integration_commit"]);
    assert_eq!(validation["integration_tree"], run["integration_tree"]);
    assert_eq!(validation["passed"], true);
    assert_eq!(validation["stop_confirmed"], true);
    assert_eq!(validation["worker_joined"], true);
    assert!(validation["actual_runtime_ms"].is_u64());
    assert_eq!(validation["actual_output_bytes"], 0);
    let attempts = done["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 3);
    let mut ids = std::collections::BTreeSet::new();
    for (kind, attempt) in ["preparation", "validator", "cas"]
        .into_iter()
        .zip(attempts)
    {
        // Stored order is chronological, and each stage owns a distinct real attempt.
        assert_eq!(attempt["kind"], kind);
        assert_eq!(attempt["state"], "finished");
        assert_eq!(attempt["settled"], true);
        assert_eq!(attempt["io_completion"], "worker_joined");
        assert_eq!(attempt["last_error"], Value::Null);
        assert!(
            attempt["actual_runtime_ms"].as_u64().unwrap()
                <= attempt["reservation"]["runtime_ms"].as_u64().unwrap()
        );
        assert!(
            attempt["actual_output_bytes"].as_u64().unwrap()
                <= attempt["reservation"]["output_bytes"].as_u64().unwrap()
        );
        assert!(ids.insert(attempt["attempt_id"].as_str().unwrap()));
        assert!(attempt["receipt_sequence"].is_u64());
    }
    assert_eq!(attempts[0]["actual_output_bytes"], REPORT.len());
    assert_eq!(attempts[1]["attempt_id"], validation["attempt_id"]);
    let graph_after = rpc(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":f.graph["run"]["id"]}),
        None,
        None,
    );
    for field in [
        "run",
        "candidates",
        "validations",
        "owner_decisions",
        "task_acceptance",
    ] {
        assert_eq!(
            graph_after[field], accepted[field],
            "integration cannot rewrite original Graph history"
        );
    }
    let db = readonly(f);
    let all = events(&db);
    let candidate = &accepted["candidates"][0];
    let stopped = unique_event(&all, "session.stopped", &candidate["session_id"]);
    assert_eq!(stopped.1["source"], "baton_supervisor");
    assert_eq!(stopped.1["payload"]["process_reaped"], true);
    assert_eq!(
        stopped.0,
        candidate["stop_receipt_sequence"].as_u64().unwrap()
    );
    assert!(stopped.0 < candidate["capture_receipt_sequence"].as_u64().unwrap());
    let queued = unique_event(&all, "integration.queued", &run["id"]);
    assert_eq!(
        queued.1["payload"]["acceptance_ref"],
        accepted["task_acceptance"]["id"]
    );
    assert!(accepted["task_acceptance"]["sequence"].as_u64().unwrap() < queued.0);
    let observed = unique_event(&all, "integration.validator_observed", &run["id"]);
    assert_eq!(observed.0, validation["sequence"].as_u64().unwrap());
    assert_eq!(observed.1["payload"]["passed"], true);
    assert_eq!(observed.1["payload"]["stop_confirmed"], true);
    assert_eq!(observed.1["payload"]["worker_joined"], true);
    assert_eq!(
        sha256(&serde_json::to_vec(&observed.1["payload"]["view"]).unwrap()),
        validation["result_digest"]
    );
    let cas = unique_event(&all, "integration.cas_observed", &run["id"]);
    assert!(observed.0 < cas.0);
    assert_eq!(cas.1["payload"]["historical_cas_confirmed"], true);
    let record = &cas.1["payload"]["record"];
    assert_eq!(record["grant_ref"], policy["approval_id"]);
    assert_eq!(record["target_ref"], TARGET);
    assert_eq!(record["expected_target_head"], f.initial_target);
    assert_eq!(record["candidate_commit"], run["integration_commit"]);
    assert_eq!(record["state"], "confirmed_local_ref");
    for stage in attempts {
        let event = all
            .iter()
            .filter(|(_, e)| {
                e["kind"] == "integration.attempt_joined"
                    && e["aggregate_id"] == run["id"]
                    && e["payload"]["attempt_id"] == stage["attempt_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(event.len(), 1);
        assert_eq!(event[0].0, stage["receipt_sequence"].as_u64().unwrap());
        assert_eq!(event[0].1["payload"]["reservation_refunded"], false);
    }
    let count = |sql: &str| db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM integrations"), 1);
    assert_eq!(count("SELECT count(*) FROM integration_attempts"), 3);
    assert_eq!(count("SELECT count(*) FROM sessions"), 1);
    assert_eq!(count("SELECT count(*) FROM attempts"), 4);
    assert_eq!(
        count("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'"),
        0
    );
    assert_eq!(
        count("SELECT count(*) FROM workspace_leases WHERE holder!=''"),
        0
    );
    let source_reservation: String = db
        .query_row(
            "SELECT document FROM goal_run_reservations WHERE run_id=?1",
            [f.graph["run"]["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let source_reservation: Value = serde_json::from_str(&source_reservation).unwrap();
    assert_eq!(source_reservation["settled"], true);
    assert_eq!(source_reservation["actual_output_bytes"], REPORT.len());
    let total_runtime = source_reservation["actual_runtime_ms"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_runtime_ms"].as_u64().unwrap())
            .sum::<u64>();
    let total_output = source_reservation["actual_output_bytes"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_output_bytes"].as_u64().unwrap())
            .sum::<u64>();
    for domain in ["root", "project"] {
        let usage = &done["shared_usage"][domain];
        assert_eq!(usage["attempts"], 4);
        assert_eq!(usage["active"], 0);
        assert_eq!(usage["runtime_ms"], total_runtime);
        assert_eq!(usage["output_bytes"], total_output);
        assert_eq!(usage["tokens"], Value::Null);
        assert_eq!(usage["cost_microunits"], Value::Null);
    }
    // Selected artifacts plus retained stream bytes, not total FS accounting.
    assert_eq!(total_output, 24);
    let root = policy["bindings"]["root_task_id"].as_str().unwrap();
    let project = policy["bindings"]["project_id"].as_str().unwrap();
    for scope in [format!("root:{root}"), format!("project:{project}")] {
        let counters: (i64, i64) = db
            .query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (4, 0));
    }
    drop(db);
    done
}
fn assert_start_replay(f: &RuntimeFixture, policy: &Value, original: &Value, terminal: &Value) {
    let before = f.server.image();
    let replay = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("explicit-start"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(
        replay, *original,
        "same key returns the immutable original result, not a fresh projection"
    );
    assert_eq!(
        f.server.image(),
        before,
        "replaying committed start cannot append keys/events/attempts"
    );
    let view = rpc(
        &f.server,
        "integrations.get",
        json!({"integration_id":terminal["run"]["id"]}),
        None,
        None,
    );
    assert_eq!(view, *terminal);
    assert_eq!(f.server.image(), before);
    let converged = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("another-explicit-start"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(converged["run"]["id"], terminal["run"]["id"]);
    assert_eq!(converged["attempts"], terminal["attempts"]);
    let db = readonly(f);
    let count: i64 = db
        .query_row("SELECT count(*) FROM integrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let count: i64 = db
        .query_row("SELECT count(*) FROM integration_attempts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 3);
    drop(db);
}

#[test]
fn accepted_graph_explicit_start_fresh_correctness_and_exact_cas_preserve_owner_source() {
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let definition = f.command_definition(&ready["candidates"][0]);
    let accepted = f.accept();
    user_changes(&f);
    let before_source = source_image(&f.source());
    let before_project = source_image(&f.project_root);
    let policy = f.approve_integration(&definition);
    // The automatic watcher may have queued first after Native finalization.
    // Explicit start must converge; this does not assert an exclusive trigger.
    let start = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("explicit-start"),
        Some(policy["revision"].clone()),
    );
    let done = assert_closed_chain(&f, &policy, &accepted, &before_source, &before_project);
    assert_eq!(start["run"]["id"], done["run"]["id"]);
    assert_start_replay(&f, &policy, &start, &done);
}

#[test]
fn native_policy_before_task_acceptance_automatically_integrates_once_without_explicit_wake() {
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let definition = f.command_definition(&ready["candidates"][0]);
    let before_source = source_image(&f.source());
    let before_project = source_image(&f.project_root);
    let policy = f.approve_integration(&definition);
    assert_eq!(
        git(&f.project_root, &["rev-parse", TARGET]),
        f.initial_target
    );
    let none = rpc(
        &f.server,
        "integrations.list",
        json!({"task_id":f.task["id"]}),
        None,
        None,
    );
    assert!(none.as_array().unwrap().is_empty());
    let db = readonly(&f);
    let n: i64 = db
        .query_row("SELECT count(*) FROM integration_attempts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 0);
    drop(db);
    let accepted = f.accept();
    // No integrations.start call occurs before this actual terminal witness.
    let done = assert_closed_chain(&f, &policy, &accepted, &before_source, &before_project);
    let original = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("explicit-start"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(original, done);
    assert_start_replay(&f, &policy, &original, &done);
}
