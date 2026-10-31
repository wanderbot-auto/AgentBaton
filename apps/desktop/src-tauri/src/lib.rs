#[cfg(all(feature = "native-smoke", not(debug_assertions)))]
compile_error!("native-smoke is debug-only and cannot be enabled in release application builds");

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
mod approval;
mod integration_approval;
mod native_parent;
mod workspace_open;

// Read-only limits shared with the feature-gated Native test binary. The
// default application exports no test view; all values have one definition.
#[cfg(feature = "native-smoke")]
pub mod native_review_test_limits {
    pub const REVIEW_MESSAGE_BYTES: usize = super::native_parent::REVIEW_MESSAGE_BYTES;
    pub const REVIEW_ROW_ENCODED_BYTES: usize = super::native_parent::REVIEW_ROW_ENCODED_BYTES;
    pub const REVIEW_ROWS_PER_PAGE: usize = super::native_parent::REVIEW_ROWS_PER_PAGE;
}

#[cfg(feature = "native-smoke")]
pub mod native_integration_test_view {
    pub const TITLE: &str = super::integration_approval::TITLE;
    pub fn review_messages(
        proposal: &serde_json::Value,
        selection: &agentbaton_core::integration::IntegrationPolicySelection,
    ) -> Result<Vec<String>, String> {
        super::integration_approval::review_messages(
            &serde_json::json!({"result":proposal}),
            selection,
        )
    }
    pub fn receipt_matches(
        response: &serde_json::Value,
        proposal: &serde_json::Value,
        selection: &agentbaton_core::integration::IntegrationPolicySelection,
    ) -> bool {
        super::integration_approval::receipt_matches(response, proposal, selection)
    }
    pub fn current_schema_matches(response: &serde_json::Value) -> bool {
        super::require_current_database_schema(response).is_ok()
    }
}

struct Runtime {
    directory: PathBuf,
}

async fn call(runtime: &Runtime, command: Value, connect: bool) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if connect {
            return ensure_daemon(&directory);
        }
        // Preserve side-effect uncertainty across the Tauri bridge. A failed
        // write transport is not proof that the daemon did not commit it.
        Ok(ipc::call(&directory, command).unwrap_or_else(
            |error| json!({"request_id":null,"error":error,"revision":0,"warnings":[]}),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn confirm_proposed(
    runtime: &Runtime,
    window: tauri::WebviewWindow,
    kind: approval::ApprovalKind,
    params: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(&directory, json!({"method":kind.proposal_method(),"params":params,"idempotency_key":idempotency_key})).map_err(|error| error.to_string())?;
            approval::confirm_and_finalize(
                proposal,
                kind,
                |message| parent.confirm("AgentBaton — Native owner approval", message),
                |command| parent.finalize(|| ipc::call(&directory, command).map_err(|error| error.to_string())),
            )
        })
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn system_snapshot(runtime: tauri::State<'_, Runtime>) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"system.snapshot","params":{}}),
        true,
    )
    .await
}

#[tauri::command]
async fn tasks_create(
    runtime: tauri::State<'_, Runtime>,
    goal: String,
    idempotency_key: String,
    project_id: Option<String>,
) -> Result<Value, String> {
    let mut params = json!({"goal":goal});
    if let Some(project_id) = project_id {
        params["project_id"] = json!(project_id);
    }
    call(
        &runtime,
        json!({"method":"tasks.create","params":params,"idempotency_key":idempotency_key}),
        false,
    )
    .await
}

#[tauri::command]
async fn projects_list(runtime: tauri::State<'_, Runtime>) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"projects.list","params":{}}),
        false,
    )
    .await
}

#[tauri::command]
async fn projects_import(
    runtime: tauri::State<'_, Runtime>,
    path: String,
    name: String,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"projects.import","params":{"path":path,"name":name},"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn projects_archive(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"projects.archive","params":{"project_id":project_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn projects_reassociate(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
    path: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"projects.reassociate","params":{"project_id":project_id,"path":path},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn workspaces_create(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
    expected_head: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    // Destination is deliberately absent. Only the daemon chooses managed roots.
    call(&runtime, json!({"method":"workspaces.create","params":{"project_id":project_id,"expected_head":expected_head},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

fn task_workspace_preparation_frame(
    selection: &agentbaton_core::TaskGitWorkspacePreparation,
    idempotency_key: &str,
) -> Value {
    json!({"method":"workspaces.prepare_task_git","params":{
        "task_id":selection.task_id,
        "expected_spec_revision":selection.expected_spec_revision,
        "expected_project_revision":selection.expected_project_revision,
        "host_id":selection.host_id,
        "expected_source_pin":selection.expected_source_pin,
        "preparation_key":selection.preparation_key
    },"expected_revision":selection.expected_task_revision,"idempotency_key":idempotency_key})
}

#[tauri::command]
async fn workspaces_prepare_task_git(
    runtime: tauri::State<'_, Runtime>,
    selection: agentbaton_core::TaskGitWorkspacePreparation,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        task_workspace_preparation_frame(&selection, &idempotency_key),
        false,
    )
    .await
}

fn task_get_frame(task_id: &str) -> Value {
    json!({"method":"tasks.get","params":{"task_id":task_id}})
}

#[tauri::command]
async fn tasks_get(runtime: tauri::State<'_, Runtime>, task_id: String) -> Result<Value, String> {
    // Stored Task/binding metadata is not a fresh filesystem or execution proof.
    call(&runtime, task_get_frame(&task_id), false).await
}

fn workspace_resolution_frame(
    selection: &agentbaton_core::CodeWorkspacePreparationRequest,
) -> Value {
    json!({"method":"workspaces.resolve","params":selection})
}

#[tauri::command]
async fn open_workspace_terminal(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    request: Value,
) -> Value {
    workspace_open::handoff(runtime.directory.clone(), window, request).await
}

#[tauri::command]
async fn workspaces_resolve(
    runtime: tauri::State<'_, Runtime>,
    selection: agentbaton_core::CodeWorkspacePreparationRequest,
) -> Result<Value, String> {
    // A fixed owner read. Never start a daemon or call an approval/write wrapper.
    call(&runtime, workspace_resolution_frame(&selection), false).await
}

#[tauri::command]
async fn workspaces_list(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"workspaces.list","params":{"project_id":project_id}}),
        false,
    )
    .await
}

#[tauri::command]
async fn data_workspace_previews_start(
    runtime: tauri::State<'_, Runtime>,
    selection: Value,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"data_workspace_previews.start","params":selection,"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
#[tauri::command]
async fn data_workspace_previews_list(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_workspace_previews.list","params":{"project_id":project_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_workspace_previews_get(
    runtime: tauri::State<'_, Runtime>,
    preview_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_workspace_previews.get","params":{"preview_id":preview_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_workspace_previews_cancel(
    runtime: tauri::State<'_, Runtime>,
    preview_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"data_workspace_previews.cancel","params":{"preview_id":preview_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
fn data_workspace_proposal_frame(
    preview_id: &str,
    expected_revision: u64,
    idempotency_key: &str,
) -> Value {
    json!({"method":"data_workspaces.propose","params":{"preview_id":preview_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
#[tauri::command]
async fn data_workspace_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    preview_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                data_workspace_proposal_frame(&preview_id, expected_revision, &idempotency_key),
            )
            .map_err(|error| error.to_string())?;
            approval::confirm_data_workspace_and_finalize(
                proposal,
                &preview_id,
                expected_revision,
                |message| parent.confirm("AgentBaton — Native data copy approval", message),
                |command| {
                    parent.finalize(|| {
                        ipc::call(&directory, command).map_err(|error| error.to_string())
                    })
                },
            )
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn data_workspaces_list(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_workspaces.list","params":{"project_id":project_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_workspaces_get(
    runtime: tauri::State<'_, Runtime>,
    creation_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_workspaces.get","params":{"creation_id":creation_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_workspaces_cancel(
    runtime: tauri::State<'_, Runtime>,
    creation_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"data_workspaces.cancel","params":{"creation_id":creation_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}

#[tauri::command]
async fn connections_list(runtime: tauri::State<'_, Runtime>) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"connections.list","params":{}}),
        false,
    )
    .await
}

#[tauri::command]
async fn connections_register(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    name: String,
    executable: String,
    arguments: Vec<String>,
    provider: String,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        window,
        approval::ApprovalKind::Connection,
        json!({"name":name,"executable":executable,"arguments":arguments,"provider":provider}),
        idempotency_key,
    )
    .await
}

#[tauri::command]
async fn grants_list(
    runtime: tauri::State<'_, Runtime>,
    project_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"grants.list","params":{"project_id":project_id}}),
        false,
    )
    .await
}

#[tauri::command]
// Preserve the existing typed wire contract, including host-owned confirmation arguments.
#[allow(clippy::too_many_arguments)]
async fn grants_create(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    project_id: String,
    workspace_id: String,
    connection_id: String,
    max_attempts: u32,
    max_concurrency: u32,
    max_duration_ms: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(&runtime, window, approval::ApprovalKind::Grant, json!({"project_id":project_id,"workspace_id":workspace_id,"connection_id":connection_id,"max_attempts":max_attempts,"max_concurrency":max_concurrency,"max_duration_ms":max_duration_ms}), idempotency_key).await
}

#[tauri::command]
async fn grants_revoke(
    runtime: tauri::State<'_, Runtime>,
    grant_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"grants.revoke","params":{"grant_id":grant_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn sessions_list(runtime: tauri::State<'_, Runtime>) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"sessions.list","params":{}}),
        false,
    )
    .await
}

#[tauri::command]
async fn sessions_get(
    runtime: tauri::State<'_, Runtime>,
    session_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"sessions.get","params":{"session_id":session_id}}),
        false,
    )
    .await
}

#[tauri::command]
async fn sessions_create(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    expected_revision: u64,
    workspace_id: String,
    connection_id: String,
    grant_id: String,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"sessions.create","params":{"task_id":task_id,"workspace_id":workspace_id,"connection_id":connection_id,"grant_id":grant_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn sessions_send(
    runtime: tauri::State<'_, Runtime>,
    session_id: String,
    expected_revision: u64,
    text: String,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"sessions.send","params":{"session_id":session_id,"text":text},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn sessions_cancel(
    runtime: tauri::State<'_, Runtime>,
    session_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"sessions.cancel","params":{"session_id":session_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn tasks_cancel(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime, json!({"method":"tasks.cancel","params":{"task_id":task_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}), false).await
}

#[tauri::command]
async fn validation_policies_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"validation_policies.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn validation_policy_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        window,
        approval::ApprovalKind::Validation,
        selection,
        idempotency_key,
    )
    .await
}
#[tauri::command]
async fn validations_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"validations.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn validations_run(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"validations.run","params":{"policy_id":policy_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
#[tauri::command]
async fn validation_policies_revoke(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"validation_policies.revoke","params":{"policy_id":policy_id},"expected_revision":expected_revision}),false).await
}

#[tauri::command]
async fn data_validation_policies_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_validation_policies.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_validation_policy_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        window,
        approval::ApprovalKind::DataValidation,
        selection,
        idempotency_key,
    )
    .await
}
#[tauri::command]
async fn data_validations_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"data_validations.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn data_validations_run(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"data_validations.run","params":{"policy_id":policy_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
#[tauri::command]
async fn data_validation_policies_revoke(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"data_validation_policies.revoke","params":{"policy_id":policy_id},"expected_revision":expected_revision}),false).await
}

#[tauri::command]
async fn command_validation_policies_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"command_validation_policies.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn command_validation_policy_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        window,
        approval::ApprovalKind::CommandValidation,
        selection,
        idempotency_key,
    )
    .await
}
#[tauri::command]
async fn command_validation_policies_revoke(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"command_validation_policies.revoke","params":{"policy_id":policy_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
#[tauri::command]
async fn command_validations_run(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"command_validations.run","params":{"policy_id":policy_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
#[tauri::command]
async fn command_validations_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"command_validations.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn command_validations_get(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"command_validations.get","params":{"run_id":run_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn command_validations_cancel(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"command_validations.cancel","params":{"run_id":run_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}

fn goal_graph_proposal_frame(selection: &Value, idempotency_key: &str) -> Value {
    json!({"method":"goal_graph_runs.propose","params":selection,"idempotency_key":idempotency_key})
}
fn goal_node_proposal_frame(
    selection: &Value,
    expected_revision: u64,
    idempotency_key: &str,
) -> Value {
    json!({"method":"goal_node_approvals.propose","params":selection,"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
fn goal_graph_cancel_frame(run_id: &str, expected_revision: u64, idempotency_key: &str) -> Value {
    json!({"method":"goal_graph_runs.cancel","params":{"run_id":run_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
fn goal_graph_advance_frame(
    run_id: &str,
    expected_plan_revision: u64,
    expected_ledger_revision: u64,
    expected_revision: u64,
    idempotency_key: &str,
) -> Value {
    json!({"method":"goal_graph_runs.advance_plan","params":{"run_id":run_id,"expected_plan_revision":expected_plan_revision,"expected_ledger_revision":expected_ledger_revision},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}

fn integration_policy_proposal_frame(
    selection: &agentbaton_core::integration::IntegrationPolicySelection,
    key: &str,
) -> Value {
    json!({"method":approval::ApprovalKind::Integration.proposal_method(),"params":selection,"idempotency_key":key})
}
fn integration_reference_frame(
    method: &str,
    field: &str,
    id: &str,
    revision: Option<u64>,
    key: Option<&str>,
) -> Value {
    let mut value = json!({"method":method,"params":{field:id}});
    if let Some(revision) = revision {
        value["expected_revision"] = json!(revision);
    }
    if let Some(key) = key {
        value["idempotency_key"] = json!(key);
    }
    value
}
fn integration_list_frame(
    method: &str,
    task_id: &str,
    after_id: Option<String>,
    limit: Option<u32>,
) -> Value {
    json!({"method":method,"params":agentbaton_core::integration::IntegrationListQuery{task_id:task_id.into(),after_id,limit}})
}
async fn integration_call(runtime: &Runtime, frame: Value) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = ipc::call(&directory, json!({"method":"system.snapshot","params":{}}))
            .map_err(|e| e.to_string())?;
        require_current_database_schema(&snapshot)?;
        Ok(ipc::call(&directory, frame).unwrap_or_else(
            |error| json!({"request_id":null,"error":error,"revision":0,"warnings":[]}),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn integration_policy_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: agentbaton_core::integration::IntegrationPolicySelection,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let snapshot = ipc::call(&directory, json!({"method":"system.snapshot","params":{}}))
                .map_err(|e| e.to_string())?;
            require_current_database_schema(&snapshot)?;
            let response = ipc::call(
                &directory,
                integration_policy_proposal_frame(&selection, &idempotency_key),
            )
            .map_err(|e| e.to_string())?;
            integration_approval::confirm_and_finalize(
                response,
                &selection,
                |message| parent.confirm(integration_approval::TITLE, message),
                |frame| parent.finalize(|| ipc::call(&directory, frame).map_err(|e| e.to_string())),
            )
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
fn integration_targets_frame(
    task_id: &str,
    after_ref: Option<String>,
    limit: Option<u32>,
) -> Value {
    json!({"method":"integration_targets.list","params":agentbaton_core::integration::IntegrationTargetQuery{task_id:task_id.into(),after_ref,limit}})
}
fn integration_context_frame(task_id: &str, goal_scope_id: &str) -> Value {
    json!({"method":"integration_context.get","params":agentbaton_core::integration::IntegrationContextQuery{task_id:task_id.into(),goal_scope_id:goal_scope_id.into()}})
}
#[tauri::command]
async fn integration_context_get(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    goal_scope_id: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_context_frame(&task_id, &goal_scope_id),
    )
    .await
}
#[tauri::command]
async fn integration_targets_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    after_ref: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_targets_frame(&task_id, after_ref, limit),
    )
    .await
}
#[tauri::command]
async fn integration_policies_get(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integration_policies.get",
            "policy_id",
            &policy_id,
            None,
            None,
        ),
    )
    .await
}
#[tauri::command]
async fn integration_policies_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    after_id: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_list_frame("integration_policies.list", &task_id, after_id, limit),
    )
    .await
}
#[tauri::command]
async fn integration_policies_revoke(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integration_policies.revoke",
            "policy_id",
            &policy_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
    )
    .await
}
#[tauri::command]
async fn integrations_start(
    runtime: tauri::State<'_, Runtime>,
    policy_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integrations.start",
            "policy_id",
            &policy_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
    )
    .await
}
#[tauri::command]
async fn integrations_get(
    runtime: tauri::State<'_, Runtime>,
    integration_id: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integrations.get",
            "integration_id",
            &integration_id,
            None,
            None,
        ),
    )
    .await
}
#[tauri::command]
async fn integrations_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
    after_id: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_list_frame("integrations.list", &task_id, after_id, limit),
    )
    .await
}
#[tauri::command]
async fn integrations_cancel(
    runtime: tauri::State<'_, Runtime>,
    integration_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integrations.cancel",
            "integration_id",
            &integration_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
    )
    .await
}
#[tauri::command]
async fn integrations_reconcile(
    runtime: tauri::State<'_, Runtime>,
    integration_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    integration_call(
        &runtime,
        integration_reference_frame(
            "integrations.reconcile",
            "integration_id",
            &integration_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
    )
    .await
}

fn factory_scope_proposal_frame(selection: &Value, key: &str) -> Value {
    json!({"method":"factory_scopes.propose","params":selection,"idempotency_key":key})
}
fn factory_scope_frame(
    method: &str,
    scope_id: &str,
    revision: Option<u64>,
    key: Option<&str>,
) -> Value {
    let mut value = json!({"method":method,"params":{"scope_id":scope_id}});
    if let Some(revision) = revision {
        value["expected_revision"] = json!(revision);
    }
    if let Some(key) = key {
        value["idempotency_key"] = json!(key);
    }
    value
}
#[tauri::command]
async fn factory_scope_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                factory_scope_proposal_frame(&selection, &idempotency_key),
            )
            .map_err(|e| e.to_string())?;
            approval::confirm_factory_and_finalize(
                proposal,
                &selection,
                |message| parent.confirm("AgentBaton — Independent Factory approval", message),
                |frame| parent.finalize(|| ipc::call(&directory, frame).map_err(|e| e.to_string())),
            )
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn factory_scopes_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_scope_frame("factory_scopes.get", &scope_id, None, None),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_scopes_list(
    runtime: tauri::State<'_, Runtime>,
    root_task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"factory_scopes.list","params":{"root_task_id":root_task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_scopes_revoke(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_scope_frame(
            "factory_scopes.revoke",
            &scope_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_runs_start(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_scope_frame(
            "factory_runs.start",
            &scope_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_runs_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_scope_frame("factory_runs.get", &scope_id, None, None),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_runs_cancel(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_scope_frame(
            "factory_runs.cancel",
            &scope_id,
            Some(expected_revision),
            Some(&idempotency_key),
        ),
        false,
    )
    .await
}

// Fixed Owner-side plan storage/read routes. No general Goal or Agent-method
// selector is exposed, and no per-plan Native approval/finalize is invented.
fn factory_child_plan_query_frame(
    method: &str,
    scope_id: &str,
    task_id: &str,
    plan_revision: Option<u64>,
) -> Value {
    let mut frame = json!({"method":method,"params":{"scope_id":scope_id,"task_id":task_id}});
    if let Some(revision) = plan_revision {
        frame["params"]["plan_revision"] = json!(revision);
    }
    frame
}
fn factory_child_plan_submit_frame(
    scope_id: &str,
    task_id: &str,
    plan_json: &str,
    expected_plan_revision: u64,
    expected_revision: u64,
    idempotency_key: &str,
) -> Value {
    json!({"method":"factory_child_plans.submit","params":{"scope_id":scope_id,"task_id":task_id,"plan_json":plan_json,"expected_plan_revision":expected_plan_revision},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
#[tauri::command]
async fn factory_child_plans_context(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_plan_query_frame("factory_child_plans.context", &scope_id, &task_id, None),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_plans_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
    plan_revision: Option<u64>,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_plan_query_frame(
            "factory_child_plans.get",
            &scope_id,
            &task_id,
            plan_revision,
        ),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_plans_list(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
) -> Result<Value, String> {
    // Core returns bounded ascending references, not every raw plan revision.
    call(
        &runtime,
        factory_child_plan_query_frame("factory_child_plans.list", &scope_id, &task_id, None),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_plans_submit(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
    plan_json: String,
    expected_plan_revision: u64,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_plan_submit_frame(
            &scope_id,
            &task_id,
            &plan_json,
            expected_plan_revision,
            expected_revision,
            &idempotency_key,
        ),
        false,
    )
    .await
}

// Fixed Owner child-Goal routes. The inner request uses Core's frozen typed
// snake_case DTO; it is not an authorization/caller/token or method selector.
fn factory_child_goal_start_frame(
    scope_id: &str,
    request: &agentbaton_core::factory::FactoryChildGoalSubmission,
    expected_revision: u64,
    key: &str,
) -> Value {
    json!({"method":"factory_child_goals.start","params":{"scope_id":scope_id,"request":request},"expected_revision":expected_revision,"idempotency_key":key})
}
fn factory_child_goal_get_frame(scope_id: &str, task_id: &str, goal_run_id: Option<&str>) -> Value {
    json!({"method":"factory_child_goals.get","params":{"scope_id":scope_id,"task_id":task_id,"goal_run_id":goal_run_id}})
}
fn factory_child_goal_list_frame(scope_id: &str, task_id: &str) -> Value {
    json!({"method":"factory_child_goals.list","params":{"scope_id":scope_id,"task_id":task_id,"goal_run_id":null}})
}
fn factory_child_goal_cancel_frame(
    scope_id: &str,
    task_id: &str,
    goal_run_id: &str,
    expected_goal_run_revision: u64,
    expected_revision: u64,
    key: &str,
) -> Value {
    json!({"method":"factory_child_goals.cancel","params":{"scope_id":scope_id,"task_id":task_id,"goal_run_id":goal_run_id,"expected_goal_run_revision":expected_goal_run_revision},"expected_revision":expected_revision,"idempotency_key":key})
}
fn factory_child_goal_advance_frame(
    scope_id: &str,
    goal_run_id: &str,
    request: &agentbaton_core::factory::FactoryChildGoalSubmission,
    expected_goal_run_revision: u64,
    expected_revision: u64,
    key: &str,
) -> Value {
    json!({"method":"factory_child_goals.advance_plan","params":{"scope_id":scope_id,"goal_run_id":goal_run_id,"request":request,"expected_goal_run_revision":expected_goal_run_revision},"expected_revision":expected_revision,"idempotency_key":key})
}
#[tauri::command]
async fn factory_child_goals_start(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    request: agentbaton_core::factory::FactoryChildGoalSubmission,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_goal_start_frame(&scope_id, &request, expected_revision, &idempotency_key),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_goals_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
    goal_run_id: Option<String>,
) -> Result<Value, String> {
    let response = call(
        &runtime,
        factory_child_goal_get_frame(&scope_id, &task_id, goal_run_id.as_deref()),
        false,
    )
    .await?;
    approval::check_factory_child_goal_view(response, &scope_id, &task_id, goal_run_id.as_deref())
}
#[tauri::command]
async fn factory_child_goals_list(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_goal_list_frame(&scope_id, &task_id),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_goals_cancel(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    task_id: String,
    goal_run_id: String,
    expected_goal_run_revision: u64,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_goal_cancel_frame(
            &scope_id,
            &task_id,
            &goal_run_id,
            expected_goal_run_revision,
            expected_revision,
            &idempotency_key,
        ),
        false,
    )
    .await
}
#[tauri::command]
async fn factory_child_goals_advance_plan(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    goal_run_id: String,
    request: agentbaton_core::factory::FactoryChildGoalSubmission,
    expected_goal_run_revision: u64,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        factory_child_goal_advance_frame(
            &scope_id,
            &goal_run_id,
            &request,
            expected_goal_run_revision,
            expected_revision,
            &idempotency_key,
        ),
        false,
    )
    .await
}

#[tauri::command]
async fn goal_graph_run_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                goal_graph_proposal_frame(&selection, &idempotency_key),
            )
            .map_err(|error| error.to_string())?;
            approval::confirm_goal_graph_and_finalize(
                proposal,
                &selection,
                |message| parent.confirm("AgentBaton — Review Goal graph execution", message),
                |command| {
                    parent.finalize(|| {
                        ipc::call(&directory, command).map_err(|error| error.to_string())
                    })
                },
            )
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn goal_node_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                goal_node_proposal_frame(&selection, expected_revision, &idempotency_key),
            )
            .map_err(|error| error.to_string())?;
            approval::confirm_goal_node_and_finalize(
                proposal,
                &selection,
                expected_revision,
                |message| parent.confirm("AgentBaton — Review exact Goal node result", message),
                |command| {
                    parent.finalize(|| {
                        ipc::call(&directory, command).map_err(|error| error.to_string())
                    })
                },
            )
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn goal_node_reject(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                goal_node_proposal_frame(&selection, expected_revision, &idempotency_key),
            )
            .map_err(|error| error.to_string())?;
            approval::confirm_goal_node_rejection_and_decline(
                proposal,
                &selection,
                expected_revision,
                |message| parent.confirm("AgentBaton — Reject exact Goal node result", message),
                |command| {
                    parent.finalize(|| {
                        ipc::call(&directory, command).map_err(|error| error.to_string())
                    })
                },
            )
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn goal_graph_runs_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_graph_runs.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_graph_runs_get(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_graph_runs_cancel(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        goal_graph_cancel_frame(&run_id, expected_revision, &idempotency_key),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_graph_run_advance_plan(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
    expected_plan_revision: u64,
    expected_ledger_revision: u64,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        goal_graph_advance_frame(
            &run_id,
            expected_plan_revision,
            expected_ledger_revision,
            expected_revision,
            &idempotency_key,
        ),
        false,
    )
    .await
}

fn goal_run_proposal_frame(selection: &Value, idempotency_key: &str) -> Value {
    json!({"method":"goal_runs.propose","params":selection,"idempotency_key":idempotency_key})
}
fn goal_run_cancel_frame(run_id: &str, expected_revision: u64, idempotency_key: &str) -> Value {
    json!({"method":"goal_runs.cancel","params":{"run_id":run_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
fn goal_run_advance_frame(
    run_id: &str,
    expected_plan_revision: u64,
    expected_ledger_revision: u64,
    expected_revision: u64,
    idempotency_key: &str,
) -> Value {
    json!({"method":"goal_runs.advance_plan","params":{"run_id":run_id,"expected_plan_revision":expected_plan_revision,"expected_ledger_revision":expected_ledger_revision},"expected_revision":expected_revision,"idempotency_key":idempotency_key})
}
#[tauri::command]
async fn goal_run_advance_plan(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
    expected_plan_revision: u64,
    expected_ledger_revision: u64,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        goal_run_advance_frame(
            &run_id,
            expected_plan_revision,
            expected_ledger_revision,
            expected_revision,
            &idempotency_key,
        ),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_run_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        native_parent::with_caller(window, |parent| {
            let proposal = ipc::call(
                &directory,
                goal_run_proposal_frame(&selection, &idempotency_key),
            )
            .map_err(|error| error.to_string())?;
            approval::confirm_goal_execution_and_finalize(
                proposal,
                &selection,
                |message| parent.confirm("AgentBaton — Native Goal execution approval", message),
                |command| {
                    parent.finalize(|| {
                        ipc::call(&directory, command).map_err(|error| error.to_string())
                    })
                },
            )
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn goal_runs_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_runs.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_runs_get(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_runs.get","params":{"run_id":run_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_runs_cancel(
    runtime: tauri::State<'_, Runtime>,
    run_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(
        &runtime,
        goal_run_cancel_frame(&run_id, expected_revision, &idempotency_key),
        false,
    )
    .await
}

#[tauri::command]
async fn goal_scope_approve(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        window,
        approval::ApprovalKind::Goal,
        selection,
        idempotency_key,
    )
    .await
}
#[tauri::command]
async fn goal_scopes_list(
    runtime: tauri::State<'_, Runtime>,
    task_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_scopes.list","params":{"task_id":task_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_scopes_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_scopes.get","params":{"scope_id":scope_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_scopes_revoke(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"goal_scopes.revoke","params":{"scope_id":scope_id},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),false).await
}
// Raw plan JSON survives this bridge intact. Parsing it into Value first would
// silently erase duplicate fields before the Core's strict typed deserializer.
fn goal_plan_frame(
    revise: bool,
    scope_id: String,
    plan_json: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, Value> {
    if plan_json.len() > 262_144 {
        return Err(
            json!({"request_id":null,"error":{"code":"INVALID_REQUEST","message":"Raw Goal plan exceeds256KiB; nothing was submitted","retryable":false,"side_effect_state":"not_started"},"revision":0,"warnings":[]}),
        );
    }
    Ok(
        json!({"method":if revise{"goal_plans.revise"}else{"goal_plans.propose"},"params":{"scope_id":scope_id,"plan_json":plan_json},"expected_revision":expected_revision,"idempotency_key":idempotency_key}),
    )
}
#[tauri::command]
async fn goal_plans_propose(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    plan_json: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    match goal_plan_frame(
        false,
        scope_id,
        plan_json,
        expected_revision,
        idempotency_key,
    ) {
        Ok(frame) => call(&runtime, frame, false).await,
        Err(refusal) => Ok(refusal),
    }
}
#[tauri::command]
async fn goal_plans_revise(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    plan_json: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    match goal_plan_frame(
        true,
        scope_id,
        plan_json,
        expected_revision,
        idempotency_key,
    ) {
        Ok(frame) => call(&runtime, frame, false).await,
        Err(refusal) => Ok(refusal),
    }
}
#[tauri::command]
async fn goal_plans_get(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_plans.get","params":{"scope_id":scope_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_plans_list(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"goal_plans.list","params":{"scope_id":scope_id}}),
        false,
    )
    .await
}
#[tauri::command]
async fn goal_plans_ready_preview(
    runtime: tauri::State<'_, Runtime>,
    scope_id: String,
    expected_revision: u64,
) -> Result<Value, String> {
    call(&runtime,json!({"method":"goal_plans.ready_preview","params":{"scope_id":scope_id},"expected_revision":expected_revision}),false).await
}

#[tauri::command]
async fn events_subscribe(
    runtime: tauri::State<'_, Runtime>,
    cursor: u64,
    limit: u32,
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"events.subscribe","params":{"cursor":cursor,"limit":limit.min(100)}}),
        false,
    )
    .await
}

fn daemon_path(executable: &Path) -> Result<PathBuf, String> {
    let parent = executable
        .parent()
        .ok_or("Desktop executable has no parent directory")?;
    Ok(parent.join("batond"))
}

fn restartable(error: ErrorKind) -> bool {
    matches!(error, ErrorKind::NotFound | ErrorKind::ConnectionRefused)
}

fn requires_start(runtime: &Path) -> Result<bool, String> {
    // Socket existence is not liveness: a killed daemon leaves its socket.
    // Only absence/refusal can start another daemon. Authentication is checked
    // once after a connection exists; protocol/auth errors never trigger spawn.
    match UnixStream::connect(runtime.join("batond.sock")) {
        Ok(stream) => {
            drop(stream);
            Ok(false)
        }
        Err(error) if restartable(error.kind()) => Ok(true),
        Err(error) => Err(format!("Cannot access local daemon socket: {error}")),
    }
}

fn before_deadline<T: Send + 'static>(
    deadline: Instant,
    operation: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or("Daemon readiness deadline expired; no task was dispatched.")?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    // The single read-only authentication probe has the core's own bounded
    // socket timeout. If our shorter deadline expires its worker may finish
    // later, but no task write, repeated probe, or further spawn can occur.
    std::thread::spawn(move || {
        let _ = sender.send(operation());
    });
    receiver
        .recv_timeout(remaining)
        .map_err(|_| "Daemon readiness deadline expired; no task was dispatched.".into())
}

fn authenticated_snapshot(runtime: &Path, deadline: Instant) -> Result<Value, String> {
    let runtime = runtime.to_path_buf();
    let response = before_deadline(deadline, move || {
        ipc::call(&runtime, json!({"method":"system.snapshot","params":{}}))
    })?
    .map_err(|e| e.to_string())?;
    if let Some(error) = response.get("error") {
        return Err(error.to_string());
    }
    require_current_database_schema(&response)?;
    Ok(response)
}

fn require_current_database_schema(response: &Value) -> Result<(), String> {
    if response.get("error").is_some()
        || response["result"]["api_version"].as_u64() != Some(1)
        || response["result"]["database_schema_version"].as_u64() != Some(17)
    {
        return Err("Authenticated daemon requires exact database schema17; old/future/unknown schema is unsupported".into());
    }
    Ok(())
}

fn ensure_daemon(runtime: &Path) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    if !requires_start(runtime)? {
        return authenticated_snapshot(runtime, deadline);
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let daemon = daemon_path(&executable)?;
    if !daemon.is_file() {
        return Err(
            "Bundled batond not found. For development, start batond separately, then retry."
                .into(),
        );
    }
    // Only the installed sibling binary is executable here. Renderer input
    // never selects paths or arguments. A raced startup is rejected by the
    // daemon's exclusive writer lock; this client never removes the socket.
    let mut child = Command::new(daemon)
        .env("BATON_RUNTIME_DIR", runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start bundled daemon: {e}"))?;
    loop {
        if Instant::now() >= deadline {
            return Err("Daemon readiness deadline expired; no task was dispatched.".into());
        }
        if !requires_start(runtime)? {
            // Once a socket is live, authenticate exactly once. Surface any
            // error immediately rather than retrying a hung or incompatible server.
            return authenticated_snapshot(runtime, deadline);
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            // Another desktop can win the startup race. Authenticate the winner.
            if !requires_start(runtime)? {
                return authenticated_snapshot(runtime, deadline);
            }
            return Err(format!("Bundled daemon exited during startup ({status}). Inspect runtime permissions and database health."));
        }
        std::thread::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

pub fn dispatch_production(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        integration_policy_approve,
        integration_targets_list,
        integration_context_get,
        integration_policies_get,
        integration_policies_list,
        integration_policies_revoke,
        integrations_start,
        integrations_get,
        integrations_list,
        integrations_cancel,
        integrations_reconcile,
        factory_scope_approve,
        factory_scopes_get,
        factory_scopes_list,
        factory_scopes_revoke,
        factory_runs_start,
        factory_runs_get,
        factory_runs_cancel,
        factory_child_plans_context,
        factory_child_plans_get,
        factory_child_plans_list,
        factory_child_plans_submit,
        factory_child_goals_start,
        factory_child_goals_get,
        factory_child_goals_list,
        factory_child_goals_cancel,
        factory_child_goals_advance_plan,
        goal_graph_run_approve,
        goal_graph_runs_list,
        goal_graph_runs_get,
        goal_graph_runs_cancel,
        goal_graph_run_advance_plan,
        goal_node_approve,
        goal_node_reject,
        goal_run_approve,
        goal_run_advance_plan,
        goal_runs_list,
        goal_runs_get,
        goal_runs_cancel,
        goal_scope_approve,
        goal_scopes_list,
        goal_scopes_get,
        goal_scopes_revoke,
        goal_plans_propose,
        goal_plans_revise,
        goal_plans_get,
        goal_plans_list,
        goal_plans_ready_preview,
        command_validation_policies_list,
        command_validation_policy_approve,
        command_validation_policies_revoke,
        command_validations_run,
        command_validations_list,
        command_validations_get,
        command_validations_cancel,
        data_validation_policies_list,
        data_validation_policy_approve,
        data_validations_list,
        data_validations_run,
        data_validation_policies_revoke,
        validation_policies_list,
        validation_policy_approve,
        validations_list,
        validations_run,
        validation_policies_revoke,
        system_snapshot,
        tasks_create,
        tasks_cancel,
        events_subscribe,
        projects_list,
        projects_import,
        projects_archive,
        projects_reassociate,
        workspaces_create,
        workspaces_list,
        workspaces_resolve,
        open_workspace_terminal,
        workspaces_prepare_task_git,
        tasks_get,
        data_workspace_previews_start,
        data_workspace_previews_list,
        data_workspace_previews_get,
        data_workspace_previews_cancel,
        data_workspace_approve,
        data_workspaces_list,
        data_workspaces_get,
        data_workspaces_cancel,
        connections_list,
        connections_register,
        grants_list,
        grants_create,
        grants_revoke,
        sessions_list,
        sessions_get,
        sessions_create,
        sessions_send,
        sessions_cancel
    ];
    handler(invoke)
}

/// Shared by the shipping entry point and the separate native integration binary.
/// This builder exposes only the production restricted command dispatcher.
pub fn desktop_builder(directory: PathBuf) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(native_parent::on_window_event)
        .manage(Runtime { directory })
        .invoke_handler(dispatch_production)
}

pub fn run() {
    let directory = match ipc::default_runtime_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Cannot resolve runtime directory: {error}");
            std::process::exit(1);
        }
    };
    desktop_builder(directory)
        .run(tauri::generate_context!())
        .expect("Desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daemon_is_fixed_sibling_not_path_from_renderer() {
        assert_eq!(
            daemon_path(Path::new(
                "/Applications/AgentBaton.app/Contents/MacOS/AgentBaton"
            ))
            .unwrap(),
            Path::new("/Applications/AgentBaton.app/Contents/MacOS/batond")
        );
        assert!(daemon_path(Path::new("/")).is_err());
    }
    #[test]
    fn stale_socket_is_restartable_but_permission_and_live_failure_are_not() {
        assert!(restartable(ErrorKind::NotFound));
        assert!(restartable(ErrorKind::ConnectionRefused));
        assert!(!restartable(ErrorKind::PermissionDenied));
        assert!(!restartable(ErrorKind::TimedOut));
        assert!(!restartable(ErrorKind::InvalidData));
    }
    #[test]
    fn stalled_read_only_probe_cannot_extend_readiness_deadline() {
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let result = before_deadline(Instant::now() + Duration::from_millis(20), move || {
            release_receiver.recv().unwrap();
            "late response"
        });
        assert!(result.unwrap_err().contains("deadline expired"));
        // Release the simulated stalled probe only after the caller returned.
        release_sender.send(()).unwrap();
    }
    #[test]
    fn goal_plan_bridge_preserves_original_duplicate_fields_whitespace_and_utf8() {
        let raw = " {\"revision\":1,\"revision\":2,\"goal\":\"目标\"}\n";
        for revise in [false, true] {
            let frame =
                goal_plan_frame(revise, "scope".into(), raw.into(), 0, "same-key".into()).unwrap();
            assert_eq!(frame["params"], json!({"scope_id":"scope","plan_json":raw}));
            assert!(frame["params"].get("proposal").is_none());
            assert_eq!(frame["expected_revision"], 0);
            assert_eq!(frame["idempotency_key"], "same-key");
        }
        let refusal =
            goal_plan_frame(false, "s".into(), "中".repeat(87_382), 0, "k".into()).unwrap_err();
        assert_eq!(refusal["error"]["code"], "INVALID_REQUEST");
        assert_eq!(refusal["error"]["side_effect_state"], "not_started");
    }
    #[test]
    fn data_copy_bridge_carries_only_preview_reference_with_outer_cas() {
        assert_eq!(
            data_workspace_proposal_frame("preview", 7, "same-key"),
            json!({"method":"data_workspaces.propose","params":{"preview_id":"preview"},"expected_revision":7,"idempotency_key":"same-key"})
        );
    }
    #[test]
    fn goal_execution_bridge_preserves_exact_selected_cas_and_private_action_boundaries() {
        assert_eq!(
            approval::ApprovalKind::GoalExecution.proposal_method(),
            "goal_runs.propose"
        );
        let selected =
            json!({"scope_id":"scope","expected_plan_revision":3,"expected_ledger_revision":7});
        assert_eq!(
            goal_run_proposal_frame(&selected, "native-key"),
            json!({"method":"goal_runs.propose","params":selected,"idempotency_key":"native-key"})
        );
        assert_eq!(
            goal_run_cancel_frame("run", 9, "cancel-key"),
            json!({"method":"goal_runs.cancel","params":{"run_id":"run"},"expected_revision":9,"idempotency_key":"cancel-key"})
        );
        assert_eq!(
            goal_run_advance_frame("run", 4, 12, 9, "advance-key"),
            json!({"method":"goal_runs.advance_plan","params":{"run_id":"run","expected_plan_revision":4,"expected_ledger_revision":12},"expected_revision":9,"idempotency_key":"advance-key"})
        );
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("async fn approvals_finalize"));
        assert!(!production.contains("async fn goal_runs_dispatch"));
    }
}

#[cfg(test)]
mod graph_bridge_tests {
    use super::*;
    #[test]
    fn graph_and_owner_frames_keep_fixed_kinds_separate_and_preserve_exact_cas() {
        let graph =
            json!({"scope_id":"scope","expected_plan_revision":4,"expected_ledger_revision":91});
        assert_eq!(
            goal_graph_proposal_frame(&graph, "graph-key"),
            json!({"method":"goal_graph_runs.propose","params":graph,"idempotency_key":"graph-key"})
        );
        let node = json!({"run_id":"run","node_id":"owner","expected_plan_revision":4,"expected_ledger_revision":91});
        assert_eq!(
            goal_node_proposal_frame(&node, 8, "owner-key"),
            json!({"method":"goal_node_approvals.propose","params":node,"expected_revision":8,"idempotency_key":"owner-key"})
        );
        assert_eq!(
            goal_graph_cancel_frame("run", 8, "cancel-key"),
            json!({"method":"goal_graph_runs.cancel","params":{"run_id":"run"},"expected_revision":8,"idempotency_key":"cancel-key"})
        );
        assert_eq!(
            goal_graph_advance_frame("run", 4, 91, 8, "advance-key"),
            json!({"method":"goal_graph_runs.advance_plan","params":{"run_id":"run","expected_plan_revision":4,"expected_ledger_revision":91},"expected_revision":8,"idempotency_key":"advance-key"})
        );
        assert_ne!(
            goal_graph_proposal_frame(&graph, "key")["method"],
            goal_run_proposal_frame(&graph, "key")["method"]
        );
    }
}

#[cfg(test)]
mod factory_bridge_tests {
    use super::*;
    #[test]
    fn factory_owner_frames_keep_only_scope_reference_and_original_cas_key() {
        for method in [
            "factory_scopes.revoke",
            "factory_runs.start",
            "factory_runs.cancel",
        ] {
            assert_eq!(
                factory_scope_frame(method, "scope", Some(7), Some("same-key")),
                json!({"method":method,"params":{"scope_id":"scope"},"expected_revision":7,"idempotency_key":"same-key"})
            );
        }
        assert_eq!(
            factory_scope_frame("factory_runs.get", "scope", None, None),
            json!({"method":"factory_runs.get","params":{"scope_id":"scope"}})
        );
    }
    #[test]
    fn factory_child_plan_read_frames_are_fixed_narrow_queries() {
        for method in [
            "factory_child_plans.context",
            "factory_child_plans.get",
            "factory_child_plans.list",
        ] {
            assert_eq!(
                factory_child_plan_query_frame(method, "scope", "child", None),
                json!({"method":method,"params":{"scope_id":"scope","task_id":"child"}})
            );
        }
        assert_eq!(
            factory_child_plan_query_frame("factory_child_plans.get", "scope", "child", Some(4)),
            json!({"method":"factory_child_plans.get","params":{"scope_id":"scope","task_id":"child","plan_revision":4}})
        );
    }
    #[test]
    fn factory_child_plan_submit_keeps_raw_text_both_cas_and_exact_key() {
        let raw = " {\n\"goal\":\"中文🧭\",\"nodes\": [] }\n";
        let frame = factory_child_plan_submit_frame("scope", "child", raw, 0, 7, "same-key");
        assert_eq!(
            frame,
            json!({"method":"factory_child_plans.submit","params":{"scope_id":"scope","task_id":"child","plan_json":raw,"expected_plan_revision":0},"expected_revision":7,"idempotency_key":"same-key"})
        );
        assert!(frame.get("approved").is_none());
        assert!(frame.get("caller").is_none());
        assert!(frame.get("token").is_none());
        assert_eq!(frame["params"].as_object().unwrap().len(), 4);
    }
    #[test]
    fn factory_selection_never_adds_agent_tool_token_or_owner_approval_flags() {
        let selection = json!({"root_task_id":"task","grant_id":"grant"});
        let frame = factory_scope_proposal_frame(&selection, "key");
        assert_eq!(
            frame,
            json!({"method":"factory_scopes.propose","params":selection,"idempotency_key":"key"})
        );
        assert!(frame.get("caller").is_none());
        assert!(frame.get("token").is_none());
        assert!(frame["params"].get("approved").is_none());
    }
    fn child_goal_request_fixture() -> agentbaton_core::factory::FactoryChildGoalSubmission {
        serde_json::from_value(json!({"task_id":"child","plan":{"id":"saved","scope_id":"factory","run_id":"factory-run","task_id":"child","revision":2,"plan_digest":"a".repeat(64),"raw_sha256":"b".repeat(64),"receipt_sequence":12},"expected_factory_run_revision":9})).unwrap()
    }
    #[test]
    fn child_goal_fixed_frames_preserve_three_separate_cas_values_original_key_and_inner_snake_case(
    ) {
        let request = child_goal_request_fixture();
        assert_eq!(
            factory_child_goal_start_frame("factory", &request, 7, "same-key"),
            json!({"method":"factory_child_goals.start","params":{"scope_id":"factory","request":request},"expected_revision":7,"idempotency_key":"same-key"})
        );
        assert_eq!(
            factory_child_goal_advance_frame("factory", "goal-run", &request, 4, 7, "same-key"),
            json!({"method":"factory_child_goals.advance_plan","params":{"scope_id":"factory","goal_run_id":"goal-run","request":request,"expected_goal_run_revision":4},"expected_revision":7,"idempotency_key":"same-key"})
        );
        assert_eq!(
            factory_child_goal_cancel_frame("factory", "child", "goal-run", 4, 7, "same-key"),
            json!({"method":"factory_child_goals.cancel","params":{"scope_id":"factory","task_id":"child","goal_run_id":"goal-run","expected_goal_run_revision":4},"expected_revision":7,"idempotency_key":"same-key"})
        );
    }
    #[test]
    fn child_goal_read_routes_have_explicit_nullable_run_and_no_cas_or_mutation_key() {
        assert_eq!(
            factory_child_goal_get_frame("factory", "child", None),
            json!({"method":"factory_child_goals.get","params":{"scope_id":"factory","task_id":"child","goal_run_id":null}})
        );
        assert_eq!(
            factory_child_goal_get_frame("factory", "child", Some("run")),
            json!({"method":"factory_child_goals.get","params":{"scope_id":"factory","task_id":"child","goal_run_id":"run"}})
        );
        assert_eq!(
            factory_child_goal_list_frame("factory", "child"),
            json!({"method":"factory_child_goals.list","params":{"scope_id":"factory","task_id":"child","goal_run_id":null}})
        );
    }
    #[test]
    fn child_goal_typed_request_does_not_accept_authority_caller_tokens_or_float_revisions() {
        for key in [
            "approved",
            "token",
            "caller",
            "root_grant_id",
            "scope_id",
            "execution_binding",
        ] {
            let mut value = json!(child_goal_request_fixture());
            value[key] = json!(true);
            assert!(
                serde_json::from_value::<agentbaton_core::factory::FactoryChildGoalSubmission>(
                    value
                )
                .is_err()
            );
        }
        for value in [json!(true), json!(1.0), json!(null)] {
            let mut request = json!(child_goal_request_fixture());
            request["expected_factory_run_revision"] = value;
            assert!(
                serde_json::from_value::<agentbaton_core::factory::FactoryChildGoalSubmission>(
                    request
                )
                .is_err()
            );
        }
    }
}

#[cfg(test)]
mod workspace_resolution_bridge_tests {
    use super::*;

    #[test]
    fn fixed_workspace_read_preserves_only_four_selection_fields() {
        let selection = agentbaton_core::CodeWorkspacePreparationRequest {
            task_id: "task".into(),
            expected_task_revision: 3,
            expected_project_revision: 7,
            host_id: "local".into(),
        };
        assert_eq!(
            workspace_resolution_frame(&selection),
            json!({
                "method":"workspaces.resolve", "params":{
                    "task_id":"task", "expected_task_revision":3,
                    "expected_project_revision":7, "host_id":"local"
                }
            })
        );
        let mut extra = serde_json::to_value(&selection).unwrap();
        extra["destination"] = json!("/unapproved");
        assert!(
            serde_json::from_value::<agentbaton_core::CodeWorkspacePreparationRequest>(extra)
                .is_err()
        );
        let production = include_str!("lib.rs").split("#[cfg(test)]").next().unwrap();
        let wrapper = production
            .split("async fn workspaces_resolve(")
            .nth(1)
            .unwrap()
            .split("#[tauri::command]")
            .next()
            .unwrap();
        assert!(wrapper.contains("workspace_resolution_frame(&selection), false"));
        assert!(!wrapper.contains("confirm_proposed"));
        assert!(!wrapper.contains("ensure_daemon"));
    }
}

#[cfg(test)]
mod task_workspace_binding_bridge_tests {
    use super::*;
    #[test]
    fn typed_git_preparation_maps_six_parameters_and_original_task_cas_without_numeric_fs_loss() {
        let selection = agentbaton_core::TaskGitWorkspacePreparation {
            task_id: "task".into(),
            expected_task_revision: 3,
            expected_spec_revision: 2,
            expected_project_revision: 7,
            host_id: "local".into(),
            expected_source_pin: agentbaton_core::GitWorkspacePin {
                canonical_root: "/source".into(),
                device: 9_007_199_254_740_993,
                inode: u64::MAX,
                common_dir: "/source/.git".into(),
                head: "a".repeat(40),
            },
            preparation_key: "code-prep:".to_owned() + &"b".repeat(64),
        };
        let frame = task_workspace_preparation_frame(&selection, "original-key");
        assert_eq!(
            frame,
            json!({"method":"workspaces.prepare_task_git","params":{
            "task_id":"task","expected_spec_revision":2,"expected_project_revision":7,"host_id":"local",
            "expected_source_pin":{"canonical_root":"/source","device":"9007199254740993","inode":"18446744073709551615","common_dir":"/source/.git","head":"a".repeat(40)},
            "preparation_key":"code-prep:".to_owned()+&"b".repeat(64)
        },"expected_revision":3,"idempotency_key":"original-key"})
        );
        assert!(frame["params"].get("expected_task_revision").is_none());
        assert_eq!(
            task_get_frame("task"),
            json!({"method":"tasks.get","params":{"task_id":"task"}})
        );
        let mut numeric = serde_json::to_value(&selection).unwrap();
        numeric["expected_source_pin"]["device"] = json!(1);
        assert!(
            serde_json::from_value::<agentbaton_core::TaskGitWorkspacePreparation>(numeric)
                .is_err()
        );
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        let wrapper = production
            .split("async fn workspaces_prepare_task_git(")
            .nth(1)
            .unwrap()
            .split("#[tauri::command]")
            .next()
            .unwrap();
        assert!(wrapper.contains("false"));
        assert!(!wrapper.contains("confirm_proposed"));
        assert!(!wrapper.contains("ensure_daemon"));
    }
}

#[cfg(test)]
mod integration_bridge_tests {
    use super::*;
    #[test]
    fn integration_reads_and_writes_preserve_fixed_reference_cas_and_key() {
        for (method, field) in [
            ("integration_policies.revoke", "policy_id"),
            ("integrations.start", "policy_id"),
            ("integrations.cancel", "integration_id"),
            ("integrations.reconcile", "integration_id"),
        ] {
            let frame = integration_reference_frame(
                method,
                field,
                "exact-id",
                Some(73),
                Some("original-key"),
            );
            assert_eq!(frame["method"], method);
            assert_eq!(frame["params"], json!({field:"exact-id"}));
            assert_eq!(frame["expected_revision"], 73);
            assert_eq!(frame["idempotency_key"], "original-key");
            assert_eq!(frame.as_object().unwrap().len(), 4);
        }
        assert_eq!(
            integration_reference_frame("integrations.get", "integration_id", "id", None, None),
            json!({"method":"integrations.get","params":{"integration_id":"id"}})
        );
        assert_eq!(
            integration_list_frame(
                "integration_policies.list",
                "task",
                Some("cursor".into()),
                Some(7)
            ),
            json!({"method":"integration_policies.list","params":{"task_id":"task","after_id":"cursor","limit":7}})
        );
        assert_eq!(
            integration_context_frame("task", "scope"),
            json!({"method":"integration_context.get","params":{"task_id":"task","goal_scope_id":"scope"}})
        );
        assert_eq!(
            integration_targets_frame("task", Some("refs/heads/last".into()), Some(2)),
            json!({"method":"integration_targets.list","params":{"task_id":"task","after_ref":"refs/heads/last","limit":2}})
        );
    }
    #[test]
    fn current_schema_requires_authenticated_exact_typed_seventeen() {
        let good = json!({"result":{"api_version":1,"database_schema_version":17}});
        assert!(require_current_database_schema(&good).is_ok());
        for bad in [
            Value::Null,
            json!(16),
            json!(18),
            json!(17.0),
            json!(true),
            json!("17"),
        ] {
            let mut response = good.clone();
            response["result"]["database_schema_version"] = bad;
            assert!(require_current_database_schema(&response).is_err());
        }
        let mut error = good.clone();
        error["error"] = Value::Null;
        assert!(require_current_database_schema(&error).is_err());
        assert!(
            require_current_database_schema(&json!({"result":{"database_schema_version":17}}))
                .is_err()
        );
    }
    #[test]
    fn generic_owner_path_cannot_finalize_integration() {
        let response=approval::confirm_and_finalize(json!({"result":{"id":"p","revision":1,"kind":"integration","consumed":false,"snapshot":{}}}),approval::ApprovalKind::Integration, |_|panic!("no generic dialog"), |_|panic!("no generic finalization")).unwrap();
        assert!(response.get("error").is_some());
        assert_eq!(response["error"]["side_effect_state"], "not_started");
    }
}
