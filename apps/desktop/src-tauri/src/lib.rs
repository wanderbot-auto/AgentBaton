#[cfg(all(feature = "native-smoke", not(debug_assertions)))]
compile_error!("native-smoke is debug-only and cannot be enabled in release application builds");

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
mod approval;

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
    app: tauri::AppHandle,
    kind: approval::ApprovalKind,
    params: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let proposal = ipc::call(&directory, json!({"method":kind.proposal_method(),"params":params,"idempotency_key":idempotency_key})).map_err(|error| error.to_string())?;
        approval::confirm_and_finalize(proposal, kind, |message| {
            // This is a Rust-native dialog. No dialog plugin permission, receipt
            // or approvals.finalize command is granted to the web renderer.
            app.dialog().message(message).title("AgentBaton — Native owner approval")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancel)
                .blocking_show()
        }, |command| ipc::call(&directory, command).map_err(|error| error.to_string()))
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
    app: tauri::AppHandle,
    name: String,
    executable: String,
    arguments: Vec<String>,
    provider: String,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        app,
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
    app: tauri::AppHandle,
    project_id: String,
    workspace_id: String,
    connection_id: String,
    max_attempts: u32,
    max_concurrency: u32,
    max_duration_ms: u64,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(&runtime, app, approval::ApprovalKind::Grant, json!({"project_id":project_id,"workspace_id":workspace_id,"connection_id":connection_id,"max_attempts":max_attempts,"max_concurrency":max_concurrency,"max_duration_ms":max_duration_ms}), idempotency_key).await
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
    app: tauri::AppHandle,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        app,
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
    app: tauri::AppHandle,
    selection: Value,
    idempotency_key: String,
) -> Result<Value, String> {
    confirm_proposed(
        &runtime,
        app,
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
    Ok(response)
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
}
