//! Fixed-reference, fixed-operation history export handoff.
use crate::Runtime;
use agentbaton_core::{
    history_export::{self, Reference, Selection},
    ipc,
};
use serde_json::{json, Value};

#[tauri::command]
pub(super) async fn history_export_preview(
    runtime: tauri::State<'_, Runtime>,
    selection: Selection,
    idempotency_key: String,
) -> Result<Value, String> {
    crate::call(
        &runtime,
        json!({"method":"history_exports.preview","params":selection,"idempotency_key":idempotency_key}),
        false,
    )
    .await
}
#[tauri::command]
pub(super) async fn history_export_publish(
    runtime: tauri::State<'_, Runtime>,
    reference: Reference,
) -> Result<Value, String> {
    crate::call(
        &runtime,
        json!({"method":"history_exports.publish","params":reference}),
        false,
    )
    .await
}
#[tauri::command]
pub(super) async fn history_export_get(
    runtime: tauri::State<'_, Runtime>,
    reference: Reference,
) -> Result<Value, String> {
    crate::call(
        &runtime,
        json!({"method":"history_exports.get","params":reference}),
        false,
    )
    .await
}
#[tauri::command]
pub(super) async fn history_export_reveal(
    runtime: tauri::State<'_, Runtime>,
    window: tauri::WebviewWindow,
    reference: Reference,
) -> Result<Value, String> {
    let directory = runtime.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::native_parent::with_caller(window, |parent| {
            let observed = ipc::call(
                &directory,
                json!({"method":"history_exports.get","params":reference}),
            )
            .map_err(|_| "Cannot reconcile export".to_string())?;
            if observed.get("error").is_some() {
                return Ok(observed);
            }
            parent.finalize(|| {
                let path = match history_export::published_path(
                    &directory.join("history-exports"),
                    &reference,
                ) {
                    Ok(path) => path,
                    Err(error) => return Ok(json!({"error":error})),
                };
                reveal(&path)
            })
        })
    })
    .await
    .map_err(|_| "Native export handoff interrupted; reconcile before retrying".to_string())?
}
#[cfg(target_os = "macos")]
fn reveal(path: &std::path::Path) -> Result<Value, String> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut child = Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Finder handoff failed".to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(json!({"result":{"status":"requested"}}))
                } else {
                    Err("Finder refused export handoff".into())
                }
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Finder handoff outcome unknown".into());
            }
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn reveal(_path: &std::path::Path) -> Result<Value, String> {
    Err("Native history export reveal is supported on macOS".into())
}
