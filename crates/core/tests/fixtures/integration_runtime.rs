//! Actual Unix IPC + generated ACP + managed Git integration. The private
//! approvals.finalize caller below is a trusted test host, not an OS dialog.
//! No normal-path authority, acceptance, pass, lease or stop is inserted by SQL.
use super::{git, result, Server};
use agentbaton_core::{goal_plan::*, goal_store::*};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
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
                let _ = writeln!(
                    std::io::stderr().lock(),
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
        Self::with_attempt_limit(8)
    }
    fn with_attempt_limit(max_attempts: u32) -> Self {
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
            json!({"project_id":project["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":max_attempts,"max_concurrency":1,"max_duration_ms":30_000}),
            "grant",
        );
        let selection = GoalScopeSelection {
            task_id:task["id"].as_str().unwrap().into(),expected_task_revision:task["revision"].as_u64().unwrap(),
            grant_id:grant["id"].as_str().unwrap().into(),expected_grant_revision:grant["revision"].as_u64().unwrap(),
            max_nodes:2,new_max_children:1,new_max_depth:1,root_budget:reprepare_budget(max_attempts),project_budget:reprepare_budget(max_attempts),
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
            "stage_reservations":{"preparation":reservation(5000,1024),"validators":[{"validator":reference,"reservation":reservation(5000,256)}],"cas":reservation(5000,1024)}});
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
struct OriginalCommand {
    definition: Value,
    policy_rows: Vec<Vec<SqlValue>>,
    old_result_rows: Vec<Vec<SqlValue>>,
}
fn command_rows(db: &Connection, sql: &str, policy_id: &str) -> Vec<Vec<SqlValue>> {
    let mut statement = db.prepare(sql).unwrap();
    let columns = statement.column_count();
    let rows = statement
        .query_map([policy_id], |r| {
            (0..columns)
                .map(|i| r.get::<_, SqlValue>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    rows
}
impl OriginalCommand {
    fn capture(f: &RuntimeFixture, definition: &Value) -> Self {
        let db = readonly(f);
        let id = definition["id"].as_str().unwrap();
        let raw: String = db
            .query_row(
                "SELECT document FROM command_validation_policies WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            serde_json::from_str::<Value>(&raw).unwrap() == *definition,
            "the actual finalized definition must match its original stored document"
        );
        let policy_rows = command_rows(
            &db,
            "SELECT rowid,* FROM command_validation_policies WHERE id=?1 ORDER BY rowid",
            id,
        );
        assert_eq!(policy_rows.len(), 1);
        let old_result_rows = command_rows(
            &db,
            "SELECT rowid,* FROM command_validation_runs WHERE policy_id=?1 ORDER BY rowid",
            id,
        );
        // This Owner-only Graph never ran the old one-shot Command policy.
        // New merged validation must not manufacture an old source pass.
        assert!(old_result_rows.is_empty());
        Self {
            definition: definition.clone(),
            policy_rows,
            old_result_rows,
        }
    }
    fn assert_unchanged(&self, f: &RuntimeFixture, db: &Connection) {
        let id = self.definition["id"].as_str().unwrap();
        assert!(
            command_rows(
                db,
                "SELECT rowid,* FROM command_validation_policies WHERE id=?1 ORDER BY rowid",
                id
            ) == self.policy_rows,
            "integration must not rewrite the original approved Command record"
        );
        assert!(
            command_rows(
                db,
                "SELECT rowid,* FROM command_validation_runs WHERE policy_id=?1 ORDER BY rowid",
                id
            ) == self.old_result_rows,
            "integration must not rewrite or invent old one-shot Command results"
        );
        let policies = rpc(
            &f.server,
            "command_validation_policies.list",
            json!({"task_id":f.task["id"]}),
            None,
            None,
        );
        let matches = policies
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] == self.definition["id"])
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert!(
            *matches[0] == self.definition,
            "fresh public read must retain the exact original approved definition"
        );
    }
}
fn exact_artifact<'a>(items: &'a Value, id: &str) -> &'a Value {
    let matches = items
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["requirement"]["artifact_id"] == id)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}
fn observed_artifact<'a>(items: &'a Value, id: &str) -> &'a Value {
    let matches = items
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["artifact_id"] == id)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}
fn assert_merged_command(
    f: &RuntimeFixture,
    db: &Connection,
    run: &Value,
    receipt: &Value,
    policy: &Value,
    original: &OriginalCommand,
    view: &Value,
) {
    original.assert_unchanged(f, db);
    assert_eq!(view["process_started"], true);
    assert_eq!(view["exit_code"], 0);
    assert_eq!(view["exit_signal"], Value::Null);
    assert_eq!(view["exit_observed_within_budget"], true);
    assert_eq!(view["cleanup_completed_within_budget"], true);
    assert_eq!(view["cleanup"]["owned_leader_reaped"], true);
    assert_eq!(view["cleanup"]["all_capture_pipes_observed_eof"], true);
    assert_eq!(view["passed"], true);
    assert_eq!(view["goal_io"]["state"], "observed");
    assert_eq!(view["task_acceptance"], "not_established");
    assert_eq!(view["integration_authorized"], false);
    assert_eq!(view["approval_id"], policy["approval_id"]);
    assert_eq!(view["policy_revision"], receipt["definition_digest"]);
    let contract = &view["contract"];
    let original_contract = &original.definition["contract"];
    let subject = json!({"task_id":f.task["id"],"attempt_id":receipt["attempt_id"],"spec_revision":policy["selection"]["expected_spec_revision"],"input_revision":policy["bindings"]["source"]["head"],
        "version":{"kind":"git","target_ref":TARGET,"expected_target_head":f.initial_target,"source_candidate":run["source_commit"],"integration_candidate":run["integration_commit"],"tree":run["integration_tree"]}});
    assert_eq!(contract["file_contract"]["subject"], subject);
    assert_ne!(
        subject["attempt_id"],
        original_contract["file_contract"]["subject"]["attempt_id"]
    );
    let cwd = &contract["cwd"];
    assert_ne!(cwd["canonical_root"], f.workspace["root"]);
    let raw: String = db
        .query_row(
            "SELECT document FROM managed_workspaces WHERE root=?1",
            [cwd["canonical_root"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let managed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(managed["identity"], *cwd);
    assert_eq!(managed["base_revision"], run["integration_commit"]);
    assert_ne!(managed["id"], f.workspace["id"]);
    assert_eq!(cwd["kind"]["kind"], "git");
    assert_eq!(cwd["kind"]["head"], run["integration_commit"]);
    for field in [
        "arguments",
        "mode",
        "script",
        "argv0",
        "image_policy",
        "environment_sha256",
        "expected_exit_codes",
        "wall_time_ms",
        "cleanup_time_ms",
        "stdout_limit",
        "stderr_limit",
        "require",
    ] {
        assert_eq!(
            contract[field], original_contract[field],
            "new subject cannot alter approved {field}"
        );
    }
    for field in ["canonical_root", "root_device", "root_inode"] {
        assert_eq!(
            contract["program"][field],
            original_contract["program"][field]
        );
    }
    for field in ["artifact_id", "relative_path", "expected_sha256"] {
        assert_eq!(
            contract["program"]["requirement"][field],
            original_contract["program"]["requirement"][field]
        );
    }
    assert!(
        contract["program"]["requirement"]["max_bytes"]
            .as_u64()
            .unwrap()
            <= original_contract["program"]["requirement"]["max_bytes"]
                .as_u64()
                .unwrap()
    );
    let integrity = &view["file_integrity"];
    assert_eq!(integrity["subject"], subject);
    assert_eq!(integrity["policy_contract"], contract["file_contract"]);
    assert_eq!(integrity["passed"], true);
    for (id, collection, original_collection, path, bytes) in [
        (
            "fixed-input",
            "inputs",
            "inputs",
            "inputs/file",
            b"baseline".as_slice(),
        ),
        ("report", "outputs", "outputs", "reports/report.txt", REPORT),
    ] {
        let bound = exact_artifact(&contract["file_contract"][collection], id);
        let old = exact_artifact(&original_contract["file_contract"][original_collection], id);
        let checked = observed_artifact(&integrity[collection], id);
        assert_eq!(bound["canonical_root"], cwd["canonical_root"]);
        assert_eq!(bound["root_device"], cwd["device"]);
        assert_eq!(bound["root_inode"], cwd["inode"]);
        assert_eq!(bound["requirement"]["relative_path"], path);
        assert_eq!(
            bound["requirement"]["relative_path"],
            old["requirement"]["relative_path"]
        );
        assert_eq!(bound["requirement"]["expected_sha256"], sha256(bytes));
        assert_eq!(bound["requirement"]["max_bytes"], bytes.len());
        assert!(
            bound["requirement"]["max_bytes"].as_u64().unwrap()
                <= old["requirement"]["max_bytes"].as_u64().unwrap()
        );
        assert_eq!(checked["relative_path"], path);
        assert_eq!(checked["root_device"], cwd["device"]);
        assert_eq!(checked["root_inode"], cwd["inode"]);
        assert_eq!(checked["expected_sha256"], sha256(bytes));
        assert_eq!(checked["observed_sha256"], sha256(bytes));
        assert_eq!(checked["size"], bytes.len());
        assert_eq!(checked["state"], "passed");
    }
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
    original_command: &OriginalCommand,
    before_source: &SourceImage,
    before_project: &SourceImage,
) -> Value {
    let done = terminal(f);
    let run = &done["run"];
    assert_eq!(run["policy_id"], policy["id"]);
    assert_eq!(run["policy_revision"], policy["revision"]);
    assert_eq!(run["task_id"], f.task["id"]);
    assert_eq!(run["acceptance_ref"], accepted["task_acceptance"]["id"]);
    let original_acceptance = &accepted["task_acceptance"];
    let typed: GraphTaskAcceptance = serde_json::from_value(original_acceptance.clone()).unwrap();
    // Preserve the complete original Value and reject unknown/missing fields.
    // Core's frozen integration digest contract serializes to Value first;
    // a struct's declaration-order JSON bytes are not that compact Value.
    assert_eq!(serde_json::to_value(typed).unwrap(), *original_acceptance);
    assert_eq!(
        run["acceptance_digest"],
        sha256(&serde_json::to_vec(original_acceptance).unwrap())
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
    assert_merged_command(
        f,
        &db,
        run,
        validation,
        policy,
        original_command,
        &observed.1["payload"]["view"],
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
    let cas_io = &cas.1["payload"]["io"];
    assert_eq!(cas_io["stdout_eof"], true);
    assert_eq!(cas_io["stderr_eof"], true);
    assert_eq!(cas_io["owned_leader_reaped"], true);
    let cas_retained = cas_io["stdout_retained_bytes"].as_u64().unwrap()
        + cas_io["stderr_retained_bytes"].as_u64().unwrap();
    assert_eq!(attempts[2]["actual_output_bytes"], cas_retained);
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
    assert_eq!(total_output, 24 + cas_retained);
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
    let original_command = OriginalCommand::capture(&f, &definition);
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
    let done = assert_closed_chain(
        &f,
        &policy,
        &accepted,
        &original_command,
        &before_source,
        &before_project,
    );
    assert_eq!(start["run"]["id"], done["run"]["id"]);
    assert_start_replay(&f, &policy, &start, &done);
}

#[test]
fn native_policy_before_task_acceptance_automatically_integrates_once_without_explicit_wake() {
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let definition = f.command_definition(&ready["candidates"][0]);
    let original_command = OriginalCommand::capture(&f, &definition);
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
    let done = assert_closed_chain(
        &f,
        &policy,
        &accepted,
        &original_command,
        &before_source,
        &before_project,
    );
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

impl RuntimeFixture {
    fn failing_command_definition(&self, candidate: &Value) -> Value {
        // A separately reviewed fixed command, not a forged validation result.
        // Only the actual process exit makes this correctness check fail.
        approved(
            &self.server,
            "command_validation_policies.propose",
            json!({"session_id":candidate["session_id"],"executable":self.validator_program,"mode":"native_executable","script":null,
            "inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":sha256(b"baseline")}],
            "outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],
            "arguments":["-c","printf '%s\\n' 'fixed correctness failure' >&2; exit 7"],"environment":{},"expected_exit_codes":[0],"wall_time_ms":500,"cleanup_time_ms":500,"stdout_limit":128,"stderr_limit":128,
            "require":{"filesystem_isolation":false,"network_isolation":false,"process_tree_containment":false,"execution_time_input_immutability":false,"auxiliary_source_immutability":false,"sealed_executable_image":false}}),
            "failing-correctness-definition",
        )
    }
}

#[test]
fn fresh_merged_correctness_failure_never_admits_cas_and_retains_charged_receipts() {
    const STDERR: &[u8] = b"fixed correctness failure\n";
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let definition = f.failing_command_definition(&ready["candidates"][0]);
    let original_command = OriginalCommand::capture(&f, &definition);
    let accepted = f.accept();
    user_changes(&f);
    let before_source = source_image(&f.source());
    let before_project = source_image(&f.project_root);
    let policy = f.approve_integration(&definition);
    let first = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("failed-explicit-start"),
        Some(policy["revision"].clone()),
    );
    let failed = observe(
        &f.server,
        "integrations.get",
        json!({"integration_id":first["run"]["id"]}),
        |v| {
            let state = v["run"]["state"].as_str().unwrap();
            assert!(
                !matches!(
                    state,
                    "confirmed_local_ref"
                        | "blocked"
                        | "stale_target"
                        | "cancelled"
                        | "outcome_unknown"
                ),
                "expected actual correctness failure, observed {state}/{}",
                v["run"]["last_error"]["code"]
            );
            state == "failed"
        },
    );
    let run = &failed["run"];
    assert_eq!(run["last_error"]["code"], "VALIDATION_FAILED");
    assert_eq!(run["round"], 1);
    assert_eq!(run["active_stage"], Value::Null);
    assert_eq!(run["io_completion"], "worker_joined");
    assert_eq!(run["acceptance_ref"], accepted["task_acceptance"]["id"]);
    assert_eq!(run["policy_id"], policy["id"]);
    assert_eq!(run["policy_revision"], policy["revision"]);
    assert_eq!(failed["fence"]["state"], "released");
    assert_eq!(
        git(&f.project_root, &["rev-parse", TARGET]),
        f.initial_target
    );
    let candidate = run["integration_commit"].as_str().unwrap();
    assert_ne!(candidate, f.initial_target);
    assert_eq!(
        git(
            &f.source(),
            &["rev-parse", &format!("{candidate}^{{tree}}")]
        ),
        run["integration_tree"].as_str().unwrap()
    );
    assert_eq!(
        git(
            &f.source(),
            &["show", &format!("{candidate}:reports/report.txt")]
        ),
        "fixed report"
    );
    assert_eq!(source_image(&f.source()), before_source);
    assert_eq!(source_image(&f.project_root), before_project);
    let attempts = failed["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_ne!(attempts[0]["attempt_id"], attempts[1]["attempt_id"]);
    for (kind, attempt) in ["preparation", "validator"].into_iter().zip(attempts) {
        assert_eq!(attempt["kind"], kind);
        assert_eq!(attempt["state"], "finished");
        assert_eq!(attempt["settled"], true);
        assert_eq!(attempt["io_completion"], "worker_joined");
        assert!(
            attempt["actual_runtime_ms"].as_u64().unwrap()
                <= attempt["reservation"]["runtime_ms"].as_u64().unwrap()
        );
        assert!(
            attempt["actual_output_bytes"].as_u64().unwrap()
                <= attempt["reservation"]["output_bytes"].as_u64().unwrap()
        );
    }
    assert_eq!(attempts[0]["last_error"], Value::Null);
    assert_eq!(attempts[0]["actual_output_bytes"], REPORT.len());
    assert_eq!(attempts[1]["last_error"]["code"], "VALIDATION_FAILED");
    assert_eq!(attempts[1]["actual_output_bytes"], STDERR.len());
    let validations = run["validation_receipts"].as_array().unwrap();
    assert_eq!(validations.len(), 1);
    let validation = &validations[0];
    assert_eq!(validation["attempt_id"], attempts[1]["attempt_id"]);
    assert_eq!(
        validation["validator"],
        policy["selection"]["stage_reservations"]["validators"][0]["validator"]
    );
    assert_eq!(
        validation["definition_digest"],
        policy["bindings"]["validators"][0]["definition_digest"]
    );
    assert_eq!(validation["passed"], false);
    assert_eq!(validation["stop_confirmed"], true);
    assert_eq!(validation["worker_joined"], true);
    assert_eq!(validation["integration_commit"], run["integration_commit"]);
    assert_eq!(validation["integration_tree"], run["integration_tree"]);
    assert_eq!(
        validation["actual_runtime_ms"],
        attempts[1]["actual_runtime_ms"]
    );
    assert_eq!(validation["actual_output_bytes"], STDERR.len());
    let db = readonly(&f);
    original_command.assert_unchanged(&f, &db);
    let all = events(&db);
    let observed = unique_event(&all, "integration.validator_observed", &run["id"]);
    assert_eq!(observed.0, validation["sequence"].as_u64().unwrap());
    assert_eq!(observed.1["payload"]["passed"], false);
    assert_eq!(observed.1["payload"]["stop_confirmed"], true);
    assert_eq!(observed.1["payload"]["worker_joined"], true);
    let view = &observed.1["payload"]["view"];
    assert_eq!(
        sha256(&serde_json::to_vec(view).unwrap()),
        validation["result_digest"]
    );
    assert_eq!(view["process_started"], true);
    assert_eq!(view["exit_code"], 7);
    assert_eq!(view["exit_signal"], Value::Null);
    assert_eq!(view["reason"], "exited");
    assert_eq!(view["passed"], false);
    assert_eq!(view["exit_observed_within_budget"], true);
    assert_eq!(view["cleanup_completed_within_budget"], true);
    assert_eq!(view["cleanup"]["owned_leader_reaped"], true);
    assert_eq!(view["cleanup"]["all_capture_pipes_observed_eof"], true);
    assert_eq!(view["stderr"]["captured_bytes"], STDERR.len());
    assert_eq!(view["stderr"]["observed_bytes_at_least"], STDERR.len());
    assert_eq!(view["stderr"]["sha256_of_captured"], sha256(STDERR));
    assert_eq!(view["stderr"]["truncated"], false);
    assert_eq!(view["stderr"]["eof_observed"], true);
    assert_eq!(view["stdout"]["captured_bytes"], 0);
    assert_eq!(view["goal_io"]["state"], "observed");
    assert_eq!(view["task_acceptance"], "not_established");
    assert_eq!(view["integration_authorized"], false);
    let subject = json!({"task_id":f.task["id"],"attempt_id":validation["attempt_id"],"spec_revision":policy["selection"]["expected_spec_revision"],"input_revision":policy["bindings"]["source"]["head"],
        "version":{"kind":"git","target_ref":TARGET,"expected_target_head":f.initial_target,"source_candidate":run["source_commit"],"integration_candidate":run["integration_commit"],"tree":run["integration_tree"]}});
    assert_eq!(view["contract"]["file_contract"]["subject"], subject);
    assert_eq!(view["file_integrity"]["subject"], subject);
    assert_eq!(
        view["file_integrity"]["policy_contract"],
        view["contract"]["file_contract"]
    );
    assert_eq!(
        view["file_integrity"]["passed"], true,
        "artifact integrity is not command correctness"
    );
    for field in [
        "arguments",
        "mode",
        "script",
        "argv0",
        "environment_sha256",
        "expected_exit_codes",
        "wall_time_ms",
        "cleanup_time_ms",
        "stdout_limit",
        "stderr_limit",
        "require",
    ] {
        assert_eq!(view["contract"][field], definition["contract"][field]);
    }
    let cwd = &view["contract"]["cwd"];
    assert_ne!(cwd["canonical_root"], f.workspace["root"]);
    let raw: String = db
        .query_row(
            "SELECT document FROM managed_workspaces WHERE root=?1",
            [cwd["canonical_root"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let managed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(managed["identity"], *cwd);
    assert_eq!(managed["base_revision"], run["integration_commit"]);
    for (id, collection, bytes) in [
        ("fixed-input", "inputs", b"baseline".as_slice()),
        ("report", "outputs", REPORT),
    ] {
        let artifact = observed_artifact(&view["file_integrity"][collection], id);
        assert_eq!(artifact["expected_sha256"], sha256(bytes));
        assert_eq!(artifact["observed_sha256"], sha256(bytes));
        assert_eq!(artifact["size"], bytes.len());
        assert_eq!(artifact["state"], "passed");
        assert_eq!(artifact["root_device"], cwd["device"]);
        assert_eq!(artifact["root_inode"], cwd["inode"]);
    }
    assert!(!all
        .iter()
        .any(|(_, e)| e["aggregate_id"] == run["id"] && e["kind"] == "integration.cas_observed"));
    for stage in attempts {
        let joined = all
            .iter()
            .filter(|(_, e)| {
                e["aggregate_id"] == run["id"]
                    && e["kind"] == "integration.attempt_joined"
                    && e["payload"]["attempt_id"] == stage["attempt_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, stage["receipt_sequence"].as_u64().unwrap());
        assert_eq!(joined[0].1["payload"]["reservation_refunded"], false);
    }
    let count = |sql: &str| db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM integrations"), 1);
    assert_eq!(count("SELECT count(*) FROM integration_attempts"), 2);
    assert_eq!(
        count("SELECT count(*) FROM integration_attempts WHERE kind='cas'"),
        0
    );
    assert_eq!(count("SELECT count(*) FROM attempts"), 3);
    assert_eq!(
        count("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'"),
        0
    );
    assert_eq!(
        count("SELECT count(*) FROM workspace_leases WHERE holder!=''"),
        0
    );
    let raw: String = db
        .query_row(
            "SELECT document FROM goal_run_reservations WHERE run_id=?1",
            [f.graph["run"]["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let graph_cost: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(graph_cost["settled"], true);
    let runtime = graph_cost["actual_runtime_ms"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_runtime_ms"].as_u64().unwrap())
            .sum::<u64>();
    let output = graph_cost["actual_output_bytes"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_output_bytes"].as_u64().unwrap())
            .sum::<u64>();
    assert_eq!(output, 24 + STDERR.len() as u64);
    for domain in ["root", "project"] {
        assert_eq!(failed["shared_usage"][domain]["attempts"], 3);
        assert_eq!(failed["shared_usage"][domain]["active"], 0);
        assert_eq!(failed["shared_usage"][domain]["runtime_ms"], runtime);
        assert_eq!(failed["shared_usage"][domain]["output_bytes"], output);
    }
    for scope in [
        format!(
            "root:{}",
            policy["bindings"]["root_task_id"].as_str().unwrap()
        ),
        format!(
            "project:{}",
            policy["bindings"]["project_id"].as_str().unwrap()
        ),
    ] {
        let counters: (i64, i64) = db
            .query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (3, 0));
    }
    drop(db);
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
        assert_eq!(graph_after[field], accepted[field]);
    }
    assert_eq!(
        graph_after["task_accepted"], true,
        "integration correctness failure cannot erase historical Task acceptance"
    );
    let image = f.server.image();
    let replay = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("failed-explicit-start"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(replay, first);
    assert_eq!(f.server.image(), image);
    // Frozen queue_integration_start resolves an existing run before fresh
    // admission, even for a new key. It cannot issue a new stage or skip failure.
    let existing = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("new-key-cannot-retry-correctness"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(existing, failed);
    let after_key = f.server.image();
    let again = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("new-key-cannot-retry-correctness"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(again, failed);
    assert_eq!(f.server.image(), after_key);
    let after = rpc(
        &f.server,
        "integrations.get",
        json!({"integration_id":run["id"]}),
        None,
        None,
    );
    assert_eq!(after, failed);
    assert_eq!(f.server.image(), after_key);
    assert_eq!(
        git(&f.project_root, &["rev-parse", TARGET]),
        f.initial_target
    );
    assert_eq!(source_image(&f.source()), before_source);
    assert_eq!(source_image(&f.project_root), before_project);
}

fn integration_meter_image(server: &Server) -> Vec<(String, Vec<Vec<SqlValue>>)> {
    server
        .image()
        .into_iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "budgets"
                    | "goal_ledgers"
                    | "attempts"
                    | "dispatch_intents"
                    | "goal_run_reservations"
                    | "workspace_leases"
                    | "integration_attempts"
                    | "integration_ref_fences"
            )
        })
        .collect()
}

#[test]
fn policy_revoked_before_acceptance_cannot_be_rearmed_by_owner_or_any_start_key() {
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let definition = f.command_definition(&ready["candidates"][0]);
    let original_command = OriginalCommand::capture(&f, &definition);
    let before_source = source_image(&f.source());
    let before_project = source_image(&f.project_root);
    let policy = f.approve_integration(&definition);
    assert_eq!(policy["revoked"], false);
    let policy_id = &policy["id"];
    let old_key = "cannot-start-before-task-acceptance";
    // A failed preaccept request is not a successful replay receipt or a permit.
    let image = f.server.image();
    let denied = f.server.request(json!({"method":"integrations.start","params":{"policy_id":policy_id},"expected_revision":policy["revision"],"idempotency_key":old_key}));
    assert_eq!(denied["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(denied["error"]["side_effect_state"], "not_started");
    assert_eq!(f.server.image(), image);
    let revoked = rpc(
        &f.server,
        "integration_policies.revoke",
        json!({"policy_id":policy_id}),
        Some("revoke-before-acceptance"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(revoked["id"], *policy_id);
    assert_eq!(revoked["revision"], 2);
    assert_eq!(revoked["revoked"], true);
    let image = f.server.image();
    let replay = rpc(
        &f.server,
        "integration_policies.revoke",
        json!({"policy_id":policy_id}),
        Some("revoke-before-acceptance"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(replay, revoked);
    assert_eq!(f.server.image(), image);
    let meter_before_acceptance = integration_meter_image(&f.server);
    let accepted = f.accept();
    assert_eq!(accepted["task_accepted"], true);
    assert_eq!(
        integration_meter_image(&f.server),
        meter_before_acceptance,
        "Owner acceptance must not reserve/charge work under a revoked integration policy"
    );
    // Give the actual Actor repeated independent read requests while its armed
    // watcher runs. Do not use a fake timer or synthesize an integration row.
    for _ in 0..4 {
        rpc(&f.server, "system.snapshot", json!({}), None, None);
        let runs = rpc(
            &f.server,
            "integrations.list",
            json!({"task_id":f.task["id"]}),
            None,
            None,
        );
        assert!(runs.as_array().unwrap().is_empty());
    }
    let image = f.server.image();
    for (revision, key, code) in [
        (policy["revision"].clone(), old_key, "STALE_REVISION"),
        (revoked["revision"].clone(), old_key, "AUTH_REQUIRED"),
        (
            revoked["revision"].clone(),
            "new-key-cannot-revive-policy",
            "AUTH_REQUIRED",
        ),
        (
            policy["revision"].clone(),
            "new-key-with-old-policy-cas",
            "STALE_REVISION",
        ),
    ] {
        let reply=f.server.request(json!({"method":"integrations.start","params":{"policy_id":policy_id},"expected_revision":revision,"idempotency_key":key}));
        assert_eq!(reply["error"]["code"], code);
        assert_eq!(reply["error"]["side_effect_state"], "not_started");
        assert_eq!(
            f.server.image(),
            image,
            "refusal must not record a key, stage, fence, event or fee"
        );
    }
    let db = readonly(&f);
    original_command.assert_unchanged(&f, &db);
    let proposal_raw: String = db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [policy_id.as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let original_proposal: Value = serde_json::from_str(&proposal_raw).unwrap();
    assert_eq!(original_proposal["id"], *policy_id);
    assert_eq!(original_proposal["kind"], "integration");
    assert_eq!(original_proposal["consumed"], true);
    let all = events(&db);
    let revoke_event = unique_event(&all, "integration.policy_revoked", policy_id);
    assert_eq!(revoke_event.1["revision"], revoked["revision"]);
    assert_eq!(revoke_event.1["payload"]["new_dispatch"], false);
    assert!(revoke_event.0 < accepted["task_acceptance"]["sequence"].as_u64().unwrap());
    assert!(!all.iter().any(|(_, e)| matches!(
        e["kind"].as_str(),
        Some(
            "integration.queued"
                | "integration.attempt_prepared"
                | "integration.stage_dispatched"
                | "integration.cas_observed"
        )
    )));
    let count = |sql: &str| db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM integration_policies"), 1);
    assert_eq!(count("SELECT count(*) FROM integrations"), 0);
    assert_eq!(count("SELECT count(*) FROM integration_attempts"), 0);
    assert_eq!(count("SELECT count(*) FROM integration_ref_fences"), 0);
    assert_eq!(count("SELECT count(*) FROM attempts"), 1);
    assert_eq!(
        count("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'"),
        0
    );
    assert_eq!(
        count("SELECT count(*) FROM workspace_leases WHERE holder!=''"),
        0
    );
    for scope in [
        format!(
            "root:{}",
            policy["bindings"]["root_task_id"].as_str().unwrap()
        ),
        format!(
            "project:{}",
            policy["bindings"]["project_id"].as_str().unwrap()
        ),
    ] {
        let counters: (i64, i64) = db
            .query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (1, 0));
    }
    drop(db);
    // Recover the actual original consumed proposal's revision. The private
    // finalizer may return its immutable original approval receipt; this does
    // not mutate current revoked authority or grant a new integration start.
    let image = f.server.image();
    let historical = private_owner(&f.server, &original_proposal);
    assert_eq!(historical, policy);
    assert_eq!(f.server.image(), image);
    let current = rpc(
        &f.server,
        "integration_policies.get",
        json!({"policy_id":policy_id}),
        None,
        None,
    );
    assert_eq!(current, revoked);
    let after_history=f.server.request(json!({"method":"integrations.start","params":{"policy_id":policy_id},"expected_revision":current["revision"],"idempotency_key":"history-is-not-a-live-permission"}));
    assert_eq!(after_history["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(after_history["error"]["side_effect_state"], "not_started");
    assert_eq!(f.server.image(), image);
    assert_eq!(integration_meter_image(&f.server), meter_before_acceptance);
    let graph_after = rpc(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":f.graph["run"]["id"]}),
        None,
        None,
    );
    assert_eq!(graph_after["task_accepted"], true);
    for field in [
        "run",
        "candidates",
        "validations",
        "owner_decisions",
        "task_acceptance",
    ] {
        assert_eq!(graph_after[field], accepted[field]);
    }
    let grants = rpc(
        &f.server,
        "grants.list",
        json!({"project_id":policy["bindings"]["project_id"]}),
        None,
        None,
    );
    let matching = grants
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["id"] == f.grant["id"])
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0]["revoked"], false);
    assert_eq!(
        git(&f.project_root, &["rev-parse", TARGET]),
        f.initial_target
    );
    assert_eq!(source_image(&f.source()), before_source);
    assert_eq!(source_image(&f.project_root), before_project);
}

// The observer owns only its short-lived ps subprocess. It never signals a
// product PID. The observed token is correlation data, never permission.
struct OwnedPs {
    child: std::process::Child,
    reaped: bool,
}
impl Drop for OwnedPs {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn bounded_ps(args: &[&str]) -> String {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    fn nonblocking(pipe: &impl AsRawFd) {
        let fd = pipe.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
    }
    fn pump(pipe: &mut impl Read, out: &mut Vec<u8>, cap: usize, deadline: Instant) -> bool {
        let mut buffer = [0; 8192];
        loop {
            assert!(
                Instant::now() < deadline,
                "read-only ps observation timed out"
            );
            match pipe.read(&mut buffer) {
                Ok(0) => return true,
                Ok(n) => {
                    assert!(out.len() + n <= cap, "read-only ps output bound exceeded");
                    out.extend_from_slice(&buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return false,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
                Err(_) => panic!("read-only ps pipe failed"),
            }
        }
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    let child = Command::new("ps")
        .args(args)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut owned = OwnedPs {
        child,
        reaped: false,
    };
    let mut stdout = owned.child.stdout.take().unwrap();
    let mut stderr = owned.child.stderr.take().unwrap();
    nonblocking(&stdout);
    nonblocking(&stderr);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let (mut out_eof, mut err_eof) = (false, false);
    loop {
        assert!(
            Instant::now() < deadline,
            "read-only ps observation timed out"
        );
        if !out_eof {
            out_eof = pump(&mut stdout, &mut out, 1024 * 1024, deadline);
        }
        if !err_eof {
            err_eof = pump(&mut stderr, &mut err, 4096, deadline);
        }
        if let Some(status) = owned.child.try_wait().unwrap() {
            owned.reaped = true;
            assert!(status.success(), "read-only ps command returned nonzero");
            if out_eof && err_eof {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(err.is_empty(), "read-only ps emitted diagnostic output");
    String::from_utf8(out).unwrap()
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ObservedCommandIdentity {
    pid: u32,
    parent: u32,
    group: u32,
    uid: u32,
}
fn numeric_ps_row(line: &str) -> (ObservedCommandIdentity, &str) {
    let mut words = line.split_whitespace();
    let mut number = || words.next().unwrap().parse::<u32>().unwrap();
    let identity = ObservedCommandIdentity {
        pid: number(),
        parent: number(),
        group: number(),
        uid: number(),
    };
    let state = words.next().unwrap();
    assert!(words.next().is_none(), "unexpected numeric ps columns");
    (identity, state)
}
fn stopped_command_identity(program: &Path, token: &str) -> ObservedCommandIdentity {
    let started = Instant::now();
    let parent = std::process::id();
    let uid = unsafe { libc::geteuid() };
    let expected = format!("{} -c kill -STOP \"$$\"; exit 0 {token}", program.display());
    loop {
        let numeric = bounded_ps(&["-axo", "pid=,ppid=,pgid=,uid=,stat="]);
        let mut matches = Vec::new();
        for line in numeric.lines().filter(|s| !s.trim().is_empty()) {
            let (identity, state) = numeric_ps_row(line);
            if identity.parent != parent
                || identity.uid != uid
                || identity.pid != identity.group
                || !state.starts_with('T')
            {
                continue;
            }
            // Read argv only after numeric ownership matches this test's own
            // stopped child/process-group leader. Never collect all host argv.
            let pid = identity.pid.to_string();
            let full = bounded_ps(&["-p", &pid, "-ww", "-o", "pid=,ppid=,pgid=,uid=,stat=,args="]);
            let lines = full
                .lines()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), 1);
            let mut offset = 0;
            for _ in 0..5 {
                while offset < lines[0].len() && lines[0].as_bytes()[offset].is_ascii_whitespace() {
                    offset += 1;
                }
                while offset < lines[0].len() && !lines[0].as_bytes()[offset].is_ascii_whitespace()
                {
                    offset += 1;
                }
            }
            let (again, again_state) = numeric_ps_row(&lines[0][..offset]);
            assert_eq!(again, identity);
            assert!(again_state.starts_with('T'));
            let actual = lines[0][offset..].trim_start();
            if actual == expected {
                matches.push(identity);
            }
        }
        assert!(
            matches.len() <= 1,
            "multiple own leaders matched one approved token"
        );
        if let Some(identity) = matches.pop() {
            // A second independent ownership/state/argv observation immediately
            // precedes the fresh runtime CAS read and real cancel request.
            let pid = identity.pid.to_string();
            let full = bounded_ps(&["-p", &pid, "-ww", "-o", "pid=,ppid=,pgid=,uid=,stat=,args="]);
            let lines = full
                .lines()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), 1);
            let mut offset = 0;
            for _ in 0..5 {
                while offset < lines[0].len() && lines[0].as_bytes()[offset].is_ascii_whitespace() {
                    offset += 1;
                }
                while offset < lines[0].len() && !lines[0].as_bytes()[offset].is_ascii_whitespace()
                {
                    offset += 1;
                }
            }
            let (again, state) = numeric_ps_row(&lines[0][..offset]);
            assert_eq!(again, identity);
            assert!(state.starts_with('T'));
            assert_eq!(lines[0][offset..].trim_start(), expected.as_str());
            return identity;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "approved command leader was not observed stopped"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn original_command_identity_is_gone(original: &ObservedCommandIdentity) -> bool {
    // No signal and no argv scan. A different reused PID is not the original
    // child; the actual joined Core receipt remains the stop proof.
    !bounded_ps(&["-axo", "pid=,ppid=,pgid=,uid=,stat="])
        .lines()
        .filter(|s| !s.trim().is_empty())
        .any(|line| numeric_ps_row(line).0 == *original)
}
impl RuntimeFixture {
    fn stopping_command_definition(&self, candidate: &Value, token: &str) -> Value {
        assert!(token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        approved(
            &self.server,
            "command_validation_policies.propose",
            json!({"session_id":candidate["session_id"],"executable":self.validator_program,"mode":"native_executable","script":null,
            "inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":sha256(b"baseline")}],
            "outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],
            "arguments":["-c","kill -STOP \"$$\"; exit 0",token],"environment":{},"expected_exit_codes":[0],"wall_time_ms":3000,"cleanup_time_ms":500,"stdout_limit":128,"stderr_limit":128,
            "require":{"filesystem_isolation":false,"network_isolation":false,"process_tree_containment":false,"execution_time_input_immutability":false,"auxiliary_source_immutability":false,"sealed_executable_image":false}}),
            "owned-stopped-command-definition",
        )
    }
}

#[test]
fn cancelling_actual_owned_merged_validator_reaps_without_cas_or_refunding_attempts() {
    let f = RuntimeFixture::new();
    let ready = f.owner_ready();
    let token = format!("integration-cancel-{}", uuid::Uuid::new_v4());
    let definition = f.stopping_command_definition(&ready["candidates"][0], &token);
    let original_command = OriginalCommand::capture(&f, &definition);
    let accepted = f.accept();
    let before_source = source_image(&f.source());
    let before_project = source_image(&f.project_root);
    let policy = f.approve_integration(&definition);
    let start = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("cancel-case-start"),
        Some(policy["revision"].clone()),
    );
    let identity = stopped_command_identity(&f.validator_program, &token);
    let live = rpc(
        &f.server,
        "integrations.get",
        json!({"integration_id":start["run"]["id"]}),
        None,
        None,
    );
    assert_eq!(live["run"]["active_stage"], "validator");
    assert_eq!(live["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(live["attempts"][1]["kind"], "validator");
    assert_eq!(live["attempts"][1]["settled"], false);
    let cancel = rpc(
        &f.server,
        "integrations.cancel",
        json!({"integration_id":start["run"]["id"]}),
        Some("cancel-actual-owned-validator"),
        Some(live["run"]["revision"].clone()),
    );
    assert_eq!(cancel["run"]["state"], "cancel_requested");
    assert_eq!(cancel["run"]["cancel_requested"], true);
    assert_eq!(
        cancel["attempts"][1]["settled"], false,
        "cancel intent is not a joined stop receipt"
    );
    let stopped = observe(
        &f.server,
        "integrations.get",
        json!({"integration_id":start["run"]["id"]}),
        |v| {
            let state = v["run"]["state"].as_str().unwrap();
            assert!(
                !matches!(
                    state,
                    "outcome_unknown"
                        | "blocked"
                        | "failed"
                        | "confirmed_local_ref"
                        | "stale_target"
                ),
                "actual running cancellation did not close: {state}/{}",
                v["run"]["last_error"]["code"]
            );
            state == "cancelled"
        },
    );
    let run = &stopped["run"];
    assert_eq!(run["cancel_requested"], true);
    assert_eq!(run["last_error"]["code"], "CANCEL_REQUESTED");
    assert_eq!(run["active_stage"], Value::Null);
    assert_eq!(run["io_completion"], "worker_joined");
    assert_eq!(stopped["fence"]["state"], "released");
    assert_eq!(run["acceptance_ref"], accepted["task_acceptance"]["id"]);
    let attempts = stopped["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    for attempt in attempts {
        assert_eq!(attempt["state"], "finished");
        assert_eq!(attempt["settled"], true);
        assert_eq!(attempt["io_completion"], "worker_joined");
        assert!(
            attempt["actual_runtime_ms"].as_u64().unwrap()
                <= attempt["reservation"]["runtime_ms"].as_u64().unwrap()
        );
    }
    assert_eq!(attempts[1]["kind"], "validator");
    assert_eq!(attempts[1]["last_error"]["code"], "CANCEL_REQUESTED");
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
    assert_eq!(validation["passed"], false);
    assert_eq!(validation["stop_confirmed"], true);
    assert_eq!(validation["worker_joined"], true);
    assert_eq!(validation["attempt_id"], attempts[1]["attempt_id"]);
    assert_eq!(validation["integration_commit"], run["integration_commit"]);
    assert_eq!(validation["integration_tree"], run["integration_tree"]);
    let db = readonly(&f);
    original_command.assert_unchanged(&f, &db);
    let all = events(&db);
    let observed = unique_event(&all, "integration.validator_observed", &run["id"]);
    let view = &observed.1["payload"]["view"];
    let cancel_event = unique_event(&all, "integration.cancel_requested", &run["id"]);
    let dispatched = all
        .iter()
        .filter(|(_, e)| {
            e["aggregate_id"] == run["id"]
                && e["kind"] == "integration.stage_dispatched"
                && e["payload"]["kind"] == "validator"
        })
        .collect::<Vec<_>>();
    assert_eq!(dispatched.len(), 1);
    assert!(dispatched[0].0 < cancel_event.0 && cancel_event.0 < observed.0);
    let subject = json!({"task_id":f.task["id"],"attempt_id":validation["attempt_id"],"spec_revision":policy["selection"]["expected_spec_revision"],"input_revision":policy["bindings"]["source"]["head"],
        "version":{"kind":"git","target_ref":TARGET,"expected_target_head":f.initial_target,"source_candidate":run["source_commit"],"integration_candidate":run["integration_commit"],"tree":run["integration_tree"]}});
    assert_eq!(view["contract"]["file_contract"]["subject"], subject);
    assert_eq!(view["approval_id"], policy["approval_id"]);
    assert_eq!(view["policy_revision"], validation["definition_digest"]);
    assert_eq!(observed.0, validation["sequence"].as_u64().unwrap());
    assert_eq!(
        sha256(&serde_json::to_vec(view).unwrap()),
        validation["result_digest"]
    );
    assert_eq!(
        view["contract"]["arguments"],
        definition["contract"]["arguments"]
    );
    assert_eq!(view["contract"]["wall_time_ms"], 3000);
    assert_eq!(view["contract"]["cleanup_time_ms"], 500);
    assert_eq!(
        view["contract"]["file_contract"]["subject"]["attempt_id"],
        validation["attempt_id"]
    );
    assert_eq!(
        view["contract"]["file_contract"]["subject"]["version"]["integration_candidate"],
        run["integration_commit"]
    );
    assert_eq!(
        view["contract"]["file_contract"]["subject"]["version"]["tree"],
        run["integration_tree"]
    );
    assert_eq!(view["process_started"], true);
    assert_eq!(view["cancellation_observed"], true);
    assert_eq!(view["reason"], "cancel_requested");
    assert_eq!(view["passed"], false);
    assert_eq!(
        view["exit_observed_within_budget"], false,
        "do not invent a natural within-budget success after cancellation"
    );
    assert_eq!(view["exit_code"], Value::Null);
    assert_eq!(view["exit_signal"], libc::SIGKILL);
    assert_eq!(view["cleanup_completed_within_budget"], true);
    assert_eq!(view["cleanup"]["owned_leader_reaped"], true);
    assert_eq!(view["cleanup"]["all_capture_pipes_observed_eof"], true);
    assert!(
        view["process_and_cleanup_elapsed_ms"].as_u64().unwrap() < 3000,
        "this case must cancel before the fixed command wall deadline"
    );
    assert!(original_command_identity_is_gone(&identity));
    assert!(!all
        .iter()
        .any(|(_, e)| e["aggregate_id"] == run["id"] && e["kind"] == "integration.cas_observed"));
    for stage in attempts {
        let joined = all
            .iter()
            .filter(|(_, e)| {
                e["aggregate_id"] == run["id"]
                    && e["kind"] == "integration.attempt_joined"
                    && e["payload"]["attempt_id"] == stage["attempt_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].1["payload"]["reservation_refunded"], false);
    }
    let count = |sql: &str| db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM integration_attempts"), 2);
    assert_eq!(
        count("SELECT count(*) FROM integration_attempts WHERE kind='cas'"),
        0
    );
    assert_eq!(count("SELECT count(*) FROM attempts"), 3);
    assert_eq!(
        count("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'"),
        0
    );
    assert_eq!(
        count("SELECT count(*) FROM workspace_leases WHERE holder!=''"),
        0
    );
    let raw: String = db
        .query_row(
            "SELECT document FROM goal_run_reservations WHERE run_id=?1",
            [f.graph["run"]["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let graph_cost: Value = serde_json::from_str(&raw).unwrap();
    let runtime = graph_cost["actual_runtime_ms"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_runtime_ms"].as_u64().unwrap())
            .sum::<u64>();
    let output = graph_cost["actual_output_bytes"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_output_bytes"].as_u64().unwrap())
            .sum::<u64>();
    assert_eq!(output, 24);
    for domain in ["root", "project"] {
        assert_eq!(stopped["shared_usage"][domain]["attempts"], 3);
        assert_eq!(stopped["shared_usage"][domain]["active"], 0);
        assert_eq!(stopped["shared_usage"][domain]["runtime_ms"], runtime);
        assert_eq!(stopped["shared_usage"][domain]["output_bytes"], output);
    }
    drop(db);
    let image = f.server.image();
    let replay = rpc(
        &f.server,
        "integrations.cancel",
        json!({"integration_id":run["id"]}),
        Some("cancel-actual-owned-validator"),
        Some(live["run"]["revision"].clone()),
    );
    assert_eq!(replay, cancel);
    assert_eq!(f.server.image(), image);
    let accepted_after = rpc(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":f.graph["run"]["id"]}),
        None,
        None,
    );
    assert_eq!(accepted_after["task_accepted"], true);
    assert_eq!(
        accepted_after["task_acceptance"],
        accepted["task_acceptance"]
    );
    assert_eq!(
        git(&f.project_root, &["rev-parse", TARGET]),
        f.initial_target
    );
    assert_eq!(source_image(&f.source()), before_source);
    assert_eq!(source_image(&f.project_root), before_project);
    // Also consume the actual daemon thread before the final ownership probe.
    // The same Server/Daemon Drop path handles an earlier assertion panic and
    // retains its generated root; this is not a separate dynamic panic test.
    let mut f = f;
    f.server.finish();
    assert!(original_command_identity_is_gone(&identity));
}

// Only the separate bounded-three-round case selects twelve attempts. All
// existing callers still enter new() -> with_attempt_limit(8), with unchanged
// runtime/output/parallel/Grant TTL and original five-second stage selections.
fn reprepare_budget(attempts: u32) -> BudgetLimits {
    assert!(matches!(attempts, 8 | 12));
    let mut limits = budget();
    limits.attempts = attempts;
    limits
}

struct ReprepareGate {
    ready: fs::File,
    release: fs::File,
    ready_path: PathBuf,
    release_path: PathBuf,
}
impl ReprepareGate {
    fn new(root: &Path) -> Self {
        use std::os::unix::ffi::OsStrExt;
        use std::{ffi::CString, fs::OpenOptions, os::unix::fs::OpenOptionsExt};
        let open = |name: &str| {
            let path = root.join(name);
            let raw = CString::new(path.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&path)
                .unwrap();
            (path, file)
        };
        let (ready_path, ready) = open("reprepare-ready.fifo");
        let (release_path, release) = open("reprepare-release.fifo");
        Self {
            ready,
            release,
            ready_path,
            release_path,
        }
    }
    fn paths(&self) -> (&Path, &Path) {
        (&self.ready_path, &self.release_path)
    }
    fn check_pins(&self) {
        use std::os::unix::fs::FileTypeExt;
        for (held, path) in [
            (&self.ready, &self.ready_path),
            (&self.release, &self.release_path),
        ] {
            let fd = held.metadata().unwrap();
            let entry = fs::symlink_metadata(path).unwrap();
            assert!(entry.file_type().is_fifo());
            assert_eq!((fd.dev(), fd.ino()), (entry.dev(), entry.ino()));
            assert_eq!(fd.uid(), unsafe { libc::geteuid() });
            assert_eq!(fd.nlink(), 1);
            assert_eq!(fd.mode() & 0o7777, 0o600);
        }
    }
    fn next_owned_validator(&mut self) -> ObservedCommandIdentity {
        use std::io::Read;
        // Same five-second observation boundary as the existing fixture, not
        // a renewal of the Command's independently enforced 1000ms wall clock.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut bytes = Vec::new();
        loop {
            assert!(
                Instant::now() < deadline,
                "owned FIFO ready observation expired"
            );
            self.check_pins();
            let mut part = [0; 64];
            match self.ready.read(&mut part) {
                Ok(0) => panic!("held FIFO unexpectedly reached EOF"),
                Ok(n) => {
                    assert!(bytes.len() + n <= 32, "bounded numeric ready marker");
                    bytes.extend_from_slice(&part[..n]);
                    if bytes.ends_with(b"\n") {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => panic!("owned FIFO read failed"),
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let text = std::str::from_utf8(&bytes)
            .unwrap()
            .strip_suffix('\n')
            .unwrap();
        assert!(!text.is_empty() && text.bytes().all(|c| c.is_ascii_digit()));
        let pid: u32 = text.parse().unwrap();
        assert!(pid > 1);
        let numeric = bounded_ps(&["-p", &pid.to_string(), "-o", "pid=,ppid=,pgid=,uid=,stat="]);
        let rows = numeric
            .lines()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            1,
            "actual validator is still live at the barrier"
        );
        let (identity, state) = numeric_ps_row(rows[0]);
        assert_eq!(identity.pid, pid);
        assert_eq!(identity.parent, std::process::id());
        assert_eq!(identity.group, pid);
        assert_eq!(identity.uid, unsafe { libc::geteuid() });
        assert!(!state.starts_with('Z'));
        identity
    }
    fn release(&mut self) {
        self.check_pins();
        // One PIPE_BUF-small nonblocking write; never waits for a failed child.
        assert_eq!(self.release.write(b"continue\n").unwrap(), 9);
    }
}

impl RuntimeFixture {
    fn reprepare_command_definition(&self, candidate: &Value, gate: &ReprepareGate) -> Value {
        let (ready, release) = gate.paths();
        let script = "test -s reports/report.txt || exit 21; printf '%s\\n' \"$$\" > \"$1\" || exit 22; IFS= read -r reply < \"$2\" || exit 23; [ \"$reply\" = continue ] || exit 24";
        approved(
            &self.server,
            "command_validation_policies.propose",
            json!({
                "session_id":candidate["session_id"],"executable":self.validator_program,"mode":"native_executable","script":null,
                "inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":sha256(b"baseline")}],
                "outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],
                "arguments":["-c",script,"bounded-reprepare",ready,release],"environment":{},"expected_exit_codes":[0],
                "wall_time_ms":1000,"cleanup_time_ms":500,"stdout_limit":128,"stderr_limit":128,
                "require":{"filesystem_isolation":false,"network_isolation":false,"process_tree_containment":false,"execution_time_input_immutability":false,"auxiliary_source_immutability":false,"sealed_executable_image":false}
            }),
            "reprepare-command-definition",
        )
    }
    fn approve_three_round_integration(&self, definition: &Value) -> Value {
        let context = rpc(
            &self.server,
            "integration_context.get",
            json!({"task_id":self.task["id"],"goal_scope_id":self.scope["id"]}),
            None,
            None,
        );
        assert_eq!(context["preview_only"], true);
        assert_eq!(self.grant["scope"]["max_attempts"], 12);
        assert_eq!(self.grant["scope"]["max_duration_ms"], 30_000);
        let reference = json!({"kind":"command","policy_id":definition["id"],"revision":definition["revision"]});
        assert_eq!(
            context["available_validators"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["reference"] == reference)
                .count(),
            1
        );
        assert!(context["bindings"]["validators"]
            .as_array()
            .unwrap()
            .is_empty());
        for domain in ["root", "project"] {
            let limits = &context["bindings"][format!("{domain}_budget")];
            let usage = &context["shared_usage"][domain];
            assert_eq!(limits["attempts"], 12);
            assert_eq!(limits["runtime_ms"], 20_000);
            assert_eq!(limits["output_bytes"], 8192);
            assert_eq!(usage["active"], 0);
            assert_eq!(usage["attempts"], 1);
            assert!(usage["attempts"].as_u64().unwrap().checked_add(9).unwrap() <= 12);
            assert!(
                usage["runtime_ms"]
                    .as_u64()
                    .unwrap()
                    .checked_add(18_000)
                    .unwrap()
                    <= 20_000,
                "Graph left insufficient known runtime for the approved three-round bound"
            );
            assert!(
                usage["output_bytes"]
                    .as_u64()
                    .unwrap()
                    .checked_add(6912)
                    .unwrap()
                    <= 8192
            );
        }
        let selection = json!({"task_id":self.task["id"],"expected_task_revision":context["task_revision"],"expected_spec_revision":context["bindings"]["spec_revision"],"goal_scope_id":self.scope["id"],"expected_scope_revision":context["bindings"]["goal_scope_revision"],"host_id":"local","target_ref":TARGET,"expected_target_head":self.initial_target,"max_rounds":3,"on_target_advance":"bounded_descendant_reprepare","ttl_ms":30_000,
        "stage_reservations":{"preparation":reservation(2000,1024),"validators":[{"validator":reference,"reservation":reservation(2000,256)}],"cas":reservation(2000,1024)}});
        let proposal = rpc(
            &self.server,
            "integration_policies.propose",
            selection.clone(),
            Some("three-round-policy"),
            None,
        );
        assert_eq!(proposal["kind"], "integration");
        assert_eq!(proposal["snapshot"]["intent"], selection);
        let policy = private_owner(&self.server, &proposal);
        assert_eq!(policy["id"], proposal["id"]);
        assert_eq!(policy["approval_id"], proposal["id"]);
        assert_eq!(policy["selection"], selection);
        policy
    }
}

// Fixed generated-repository commands use private regular capture files, not
// a pipe reader that could detach or wait forever. The original owned Child is
// killed/waited by the existing OwnedPs guard on every deadline/error path.
fn reprepare_host_git(f: &RuntimeFixture, args: &[&str], deadline: Instant) -> String {
    use std::io::Read;
    assert!(
        Instant::now() < deadline,
        "original Host advance deadline expired"
    );
    let stdout = tempfile::NamedTempFile::new_in(&f.root).unwrap();
    let stderr = tempfile::NamedTempFile::new_in(&f.root).unwrap();
    let child = Command::new("git")
        .arg("-C")
        .arg(&f.project_root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Generated fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(stdout.as_file().try_clone().unwrap())
        .stderr(stderr.as_file().try_clone().unwrap())
        .spawn()
        .unwrap();
    let mut owned = OwnedPs {
        child,
        reaped: false,
    };
    let status = loop {
        assert!(Instant::now() < deadline, "bounded Host Git expired");
        if let Some(status) = owned.child.try_wait().unwrap() {
            owned.reaped = true;
            break status;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(status.success(), "bounded Host Git returned nonzero");
    assert!(stdout.as_file().metadata().unwrap().len() <= 128);
    assert!(stderr.as_file().metadata().unwrap().len() <= 4096);
    let mut out = String::new();
    let mut err = Vec::new();
    // Child and parent file descriptors share an offset; reopen readonly only
    // after real leader reap, retaining the private NamedTempFile owners.
    fs::File::open(stdout.path())
        .unwrap()
        .take(129)
        .read_to_string(&mut out)
        .unwrap();
    fs::File::open(stderr.path())
        .unwrap()
        .take(4097)
        .read_to_end(&mut err)
        .unwrap();
    assert!(err.is_empty(), "bounded Host Git emitted diagnostics");
    assert!(
        Instant::now() < deadline,
        "original Host deadline expired after join"
    );
    out.trim().to_owned()
}

fn advance_owned_test_target(
    f: &RuntimeFixture,
    expected: &str,
    round: u64,
    deadline: Instant,
) -> String {
    let tree = reprepare_host_git(f, &["rev-parse", &format!("{expected}^{{tree}}")], deadline);
    // Commit objects + the one exact approved slot only; never git add, commit
    // with the user's index, checkout, reset, stash or mutation of other refs.
    let next = reprepare_host_git(
        f,
        &[
            "commit-tree",
            &tree,
            "-p",
            expected,
            "-m",
            &format!("owned descendant after round {round}"),
        ],
        deadline,
    );
    reprepare_host_git(
        f,
        &["merge-base", "--is-ancestor", expected, &next],
        deadline,
    );
    reprepare_host_git(
        f,
        &["update-ref", "--no-deref", TARGET, &next, expected],
        deadline,
    );
    assert_eq!(
        reprepare_host_git(f, &["rev-parse", TARGET], deadline),
        next
    );
    next
}

#[test]
fn three_real_rounds_revalidate_each_merged_candidate_after_consecutive_descendant_advances() {
    let mut f = RuntimeFixture::with_attempt_limit(12);
    let ready = f.owner_ready();
    let mut gate = ReprepareGate::new(&f.root);
    let definition = f.reprepare_command_definition(&ready["candidates"][0], &gate);
    let original = OriginalCommand::capture(&f, &definition);
    let accepted = f.accept();
    user_changes(&f);
    let source_before = source_image(&f.source());
    let project_before = source_image(&f.project_root);
    let policy = f.approve_three_round_integration(&definition);
    let start = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("three-round-start"),
        Some(policy["revision"].clone()),
    );
    let id = start["run"]["id"].clone();
    let mut heads = vec![f.initial_target.clone()];
    let mut candidates = Vec::new();
    let mut captured = Vec::new();
    let mut leaders = Vec::new();
    for round in 1u64..=3 {
        // Receiving from this actually executed approved command is the
        // synchronization point. Poll sleeps only yield; they never pick races.
        leaders.push(gate.next_owned_validator());
        let live = rpc(
            &f.server,
            "integrations.get",
            json!({"integration_id":id}),
            None,
            None,
        );
        assert_eq!(live["run"]["round"], round);
        assert_eq!(live["run"]["active_stage"], "validator");
        assert_eq!(
            live["run"]["expected_target_head"],
            heads[(round - 1) as usize]
        );
        assert_eq!(
            live["run"]["acceptance_ref"],
            accepted["task_acceptance"]["id"]
        );
        let active = live["attempts"].as_array().unwrap().last().unwrap();
        assert_eq!(active["kind"], "validator");
        assert_eq!(active["round"], round);
        assert_eq!(active["settled"], false);
        candidates.push(
            live["run"]["integration_commit"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        captured.push(live["run"]["source_commit"].as_str().unwrap().to_owned());
        // This is a further finite Host sub-bound, not a new Command clock.
        // Core independently keeps its original 1000+500ms Command deadline.
        let host_deadline = Instant::now() + Duration::from_millis(500);
        if round < 3 {
            heads.push(advance_owned_test_target(
                &f,
                &heads[(round - 1) as usize],
                round,
                host_deadline,
            ));
        }
        assert!(Instant::now() < host_deadline, "Host release bound expired");
        gate.release();
    }
    let done = terminal(&f);
    let run = &done["run"];
    assert_eq!(run["id"], id);
    assert_eq!(run["round"], 3);
    assert_eq!(run["expected_target_head"], heads[2]);
    assert_eq!(run["policy_id"], policy["id"]);
    assert_eq!(run["policy_revision"], 1);
    assert_eq!(run["acceptance_ref"], accepted["task_acceptance"]["id"]);
    assert_eq!(run["active_stage"], Value::Null);
    assert_eq!(run["last_error"], Value::Null);
    assert_eq!(run["io_completion"], "worker_joined");
    assert_eq!(done["fence"]["state"], "released");
    assert_eq!(
        candidates
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(run["integration_commit"], candidates[2]);
    assert_eq!(git(&f.project_root, &["rev-parse", TARGET]), candidates[2]);
    assert_eq!(
        git(
            &f.project_root,
            &["diff", "--name-only", &f.initial_target, &candidates[2]]
        ),
        "reports/report.txt"
    );
    assert_eq!(
        git(
            &f.project_root,
            &["show", &format!("{}:reports/report.txt", candidates[2])]
        ),
        "fixed report"
    );
    assert_eq!(source_image(&f.source()), source_before);
    assert_eq!(source_image(&f.project_root), project_before);
    let db = readonly(&f);
    original.assert_unchanged(&f, &db);
    let all = events(&db);
    let attempts = done["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 9);
    let expected_kinds = [
        "preparation",
        "validator",
        "cas",
        "preparation",
        "validator",
        "cas",
        "preparation",
        "validator",
        "cas",
    ];
    let mut attempt_ids = std::collections::BTreeSet::new();
    for (i, attempt) in attempts.iter().enumerate() {
        assert_eq!(attempt["kind"], expected_kinds[i]);
        assert_eq!(attempt["round"], u64::try_from(i / 3 + 1).unwrap());
        assert_eq!(attempt["state"], "finished");
        assert_eq!(attempt["settled"], true);
        assert_eq!(attempt["io_completion"], "worker_joined");
        assert_eq!(attempt["last_error"], Value::Null);
        assert_eq!(attempt["reservation"]["runtime_ms"], 2000);
        assert!(attempt["actual_runtime_ms"].as_u64().unwrap() <= 2000);
        assert!(
            attempt["actual_output_bytes"].as_u64().unwrap()
                <= attempt["reservation"]["output_bytes"].as_u64().unwrap()
        );
        assert!(attempt_ids.insert(attempt["attempt_id"].as_str().unwrap()));
        let joined = all
            .iter()
            .filter(|(_, e)| {
                e["aggregate_id"] == id
                    && e["kind"] == "integration.attempt_joined"
                    && e["payload"]["attempt_id"] == attempt["attempt_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].1["payload"]["reservation_refunded"], false);
        assert_eq!(joined[0].0, attempt["receipt_sequence"].as_u64().unwrap());
        if i == 2 || i == 5 {
            assert_eq!(attempt["actual_output_bytes"], 0);
        }
    }
    let receipts = run["validation_receipts"].as_array().unwrap();
    assert_eq!(receipts.len(), 3);
    for (i, receipt) in receipts.iter().enumerate() {
        let round = u64::try_from(i + 1).unwrap();
        assert_eq!(receipt["round"], round);
        assert_eq!(receipt["attempt_id"], attempts[i * 3 + 1]["attempt_id"]);
        assert_eq!(
            receipt["validator"],
            policy["selection"]["stage_reservations"]["validators"][0]["validator"]
        );
        assert_eq!(
            receipt["definition_digest"],
            policy["bindings"]["validators"][0]["definition_digest"]
        );
        assert_eq!(receipt["passed"], true);
        assert_eq!(receipt["stop_confirmed"], true);
        assert_eq!(receipt["worker_joined"], true);
        assert_eq!(receipt["integration_commit"], candidates[i]);
        let observed = all
            .iter()
            .filter(|(_, e)| {
                e["aggregate_id"] == id
                    && e["kind"] == "integration.validator_observed"
                    && e["payload"]["attempt_id"] == receipt["attempt_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].0, receipt["sequence"].as_u64().unwrap());
        let view = &observed[0].1["payload"]["view"];
        assert_eq!(
            sha256(&serde_json::to_vec(view).unwrap()),
            receipt["result_digest"]
        );
        assert_eq!(view["process_started"], true);
        assert_eq!(view["exit_code"], 0);
        assert_eq!(view["exit_signal"], Value::Null);
        assert_eq!(view["task_acceptance"], "not_established");
        assert_eq!(view["integration_authorized"], false);
        assert_eq!(view["exit_observed_within_budget"], true);
        assert_eq!(view["cleanup_completed_within_budget"], true);
        assert_eq!(view["cleanup"]["owned_leader_reaped"], true);
        assert_eq!(view["cleanup"]["all_capture_pipes_observed_eof"], true);
        assert_eq!(view["passed"], true);
        assert_eq!(view["goal_io"]["state"], "observed");
        assert_eq!(view["approval_id"], policy["approval_id"]);
        assert_eq!(view["policy_revision"], receipt["definition_digest"]);
        let contract = &view["contract"];
        for field in [
            "arguments",
            "mode",
            "script",
            "argv0",
            "image_policy",
            "environment_sha256",
            "expected_exit_codes",
            "wall_time_ms",
            "cleanup_time_ms",
            "stdout_limit",
            "stderr_limit",
            "require",
        ] {
            assert_eq!(contract[field], original.definition["contract"][field]);
        }
        let subject = &contract["file_contract"]["subject"];
        assert_eq!(subject["task_id"], f.task["id"]);
        assert_eq!(subject["attempt_id"], receipt["attempt_id"]);
        assert_eq!(
            subject["spec_revision"],
            policy["selection"]["expected_spec_revision"]
        );
        assert_eq!(
            subject["input_revision"],
            policy["bindings"]["source"]["head"]
        );
        assert_eq!(subject["version"]["expected_target_head"], heads[i]);
        assert_eq!(subject["version"]["kind"], "git");
        assert_eq!(subject["version"]["target_ref"], TARGET);
        assert_eq!(subject["version"]["source_candidate"], captured[i]);
        assert_eq!(subject["version"]["integration_candidate"], candidates[i]);
        assert_eq!(subject["version"]["tree"], receipt["integration_tree"]);
        assert_eq!(view["file_integrity"]["subject"], *subject);
        assert_eq!(view["file_integrity"]["passed"], true);
        assert_eq!(contract["cwd"]["kind"]["head"], candidates[i]);
        for (artifact, collection, bytes) in [
            ("fixed-input", "inputs", b"baseline".as_slice()),
            ("report", "outputs", REPORT),
        ] {
            let pin = exact_artifact(&contract["file_contract"][collection], artifact);
            let observed = observed_artifact(&view["file_integrity"][collection], artifact);
            assert_eq!(pin["canonical_root"], contract["cwd"]["canonical_root"]);
            assert_eq!(pin["requirement"]["expected_sha256"], sha256(bytes));
            assert_eq!(observed["observed_sha256"], sha256(bytes));
            assert_eq!(observed["state"], "passed");
        }
    }
    let advances = all
        .iter()
        .filter(|(_, e)| {
            e["aggregate_id"] == id && e["kind"] == "integration.descendant_target_observed"
        })
        .collect::<Vec<_>>();
    assert_eq!(advances.len(), 2);
    for (i, advance) in advances.iter().enumerate() {
        assert_eq!(advance.1["payload"]["from_round"], i + 1);
        assert_eq!(advance.1["payload"]["next_round"], i + 2);
        assert_eq!(advance.1["payload"]["from_head"], heads[i]);
        assert_eq!(advance.1["payload"]["to_head"], heads[i + 1]);
        assert_eq!(advance.1["payload"]["read_worker_joined"], true);
        assert_eq!(advance.1["payload"]["new_permission"], false);
        assert_eq!(
            advance.1["payload"]["attempt_id"],
            attempts[i * 3 + 2]["attempt_id"]
        );
        assert!(receipts[i]["sequence"].as_u64().unwrap() < advance.0);
    }
    let cas = unique_event(&all, "integration.cas_observed", &id);
    assert_eq!(cas.1["payload"]["historical_cas_confirmed"], true);
    assert_eq!(cas.1["payload"]["record"]["expected_target_head"], heads[2]);
    assert_eq!(
        cas.1["payload"]["record"]["candidate_commit"],
        candidates[2]
    );
    assert!(receipts[2]["sequence"].as_u64().unwrap() < cas.0);
    assert_eq!(cas.1["payload"]["io"]["owned_leader_reaped"], true);
    assert_eq!(cas.1["payload"]["io"]["stdout_eof"], true);
    assert_eq!(cas.1["payload"]["io"]["stderr_eof"], true);
    let raw: String = db
        .query_row(
            "SELECT document FROM goal_run_reservations WHERE run_id=?1",
            [f.graph["run"]["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let graph_cost: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(graph_cost["settled"], true);
    let runtime = graph_cost["actual_runtime_ms"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_runtime_ms"].as_u64().unwrap())
            .sum::<u64>();
    let output = graph_cost["actual_output_bytes"].as_u64().unwrap()
        + attempts
            .iter()
            .map(|a| a["actual_output_bytes"].as_u64().unwrap())
            .sum::<u64>();
    for domain in ["root", "project"] {
        assert_eq!(done["shared_usage"][domain]["attempts"], 10);
        assert_eq!(done["shared_usage"][domain]["active"], 0);
        assert_eq!(done["shared_usage"][domain]["runtime_ms"], runtime);
        assert_eq!(done["shared_usage"][domain]["output_bytes"], output);
    }
    assert!(runtime <= 20_000);
    assert!(output <= 8192);
    assert_eq!(
        db.query_row("SELECT count(*) FROM integrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        10
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    for domain in [
        format!(
            "root:{}",
            policy["bindings"]["root_task_id"].as_str().unwrap()
        ),
        format!(
            "project:{}",
            policy["bindings"]["project_id"].as_str().unwrap()
        ),
    ] {
        let counters: (i64, i64) = db
            .query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [domain],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (10, 0));
    }
    drop(db);
    let stable = f.server.image();
    let replay = rpc(
        &f.server,
        "integrations.start",
        json!({"policy_id":policy["id"]}),
        Some("three-round-start"),
        Some(policy["revision"].clone()),
    );
    assert_eq!(replay, start);
    assert_eq!(f.server.image(), stable);
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
        assert_eq!(graph_after[field], accepted[field]);
    }
    for leader in &leaders {
        assert!(original_command_identity_is_gone(leader));
    }
    f.server.finish();
    for leader in &leaders {
        assert!(original_command_identity_is_gone(leader));
    }
}
