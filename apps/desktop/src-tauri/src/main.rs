#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
) -> Result<Value, String> {
    call(
        &runtime,
        json!({"method":"tasks.create","params":{"goal":goal},"idempotency_key":idempotency_key}),
        false,
    )
    .await
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

fn main() {
    let directory = match ipc::default_runtime_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Cannot resolve runtime directory: {error}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .manage(Runtime { directory })
        .invoke_handler(tauri::generate_handler![
            system_snapshot,
            tasks_create,
            tasks_cancel,
            events_subscribe
        ])
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
