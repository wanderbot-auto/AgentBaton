//! Main-window hiding keeps the same WebView alive. This module never exits the
//! application, signals a daemon, creates a replacement window or reloads UI.
use crate::{
    background_policy::{hide_existing, restore_existing, RestoreStep},
    native_parent,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, Window, WindowEvent};

#[derive(Default)]
pub(crate) struct BackgroundWindow {
    restoration_ready: AtomicBool,
}
pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::TrayIconBuilder,
    };
    let restore = MenuItem::with_id(
        app,
        "restore-main",
        "Show AgentBaton / 显示主窗口",
        true,
        None::<&str>,
    );
    let tray = restore
        .and_then(|item| Menu::with_items(app, &[&item]))
        .and_then(|menu| {
            let mut builder = TrayIconBuilder::with_id("background-restore")
                .menu(&menu)
                .tooltip("AgentBaton — Show / 显示主窗口")
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "restore-main" {
                        if let Err(error) = restore_main(app) {
                            eprintln!("Cannot restore AgentBaton main window: {error}");
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            builder.build(app)
        });
    match tray {
        Ok(_) => app
            .state::<BackgroundWindow>()
            .restoration_ready
            .store(true, Ordering::Release),
        Err(error) => {
            eprintln!("Background menu unavailable; main window will remain visible: {error}")
        }
    }
    // Failure disables hiding, never destroys the current window or blocks the
    // rest of the application from opening normally.
    Ok(())
}
pub(crate) fn restore_main(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Original main window is unavailable; no replacement created")?;
    restore_existing(window.label(), |step| {
        match step {
            RestoreStep::Unminimize => window.unminimize(),
            RestoreStep::Show => window.show(),
            RestoreStep::Focus => window.set_focus(),
        }
        .map_err(|e| e.to_string())
    })
}
pub(crate) fn hide_main(
    window: &tauri::WebviewWindow,
) -> Result<(), crate::background_policy::HideFailure> {
    use crate::background_policy::HideFailure;
    let ready = window
        .state::<BackgroundWindow>()
        .restoration_ready
        .load(Ordering::Acquire)
        && window
            .app_handle()
            .tray_by_id("background-restore")
            .is_some();
    // Policy failures are proven before native hiding begins; a native API
    // failure itself does not prove that visibility remained unchanged.
    hide_existing(
        window.label(),
        ready,
        native_parent::background_hide_blocked(window.label()),
        || Ok(()),
    )
    .map_err(HideFailure::not_started)?;
    window
        .hide()
        .map_err(|e| HideFailure::unknown(e.to_string()))
}
pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) {
    // Preserve existing close invalidation and the already-started private
    // approval completion contract before deciding whether hiding is safe.
    native_parent::on_window_event(window, event);
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" {
            api.prevent_close();
            let ready = window
                .state::<BackgroundWindow>()
                .restoration_ready
                .load(Ordering::Acquire)
                && window
                    .app_handle()
                    .tray_by_id("background-restore")
                    .is_some();
            if let Err(error) = hide_existing(
                window.label(),
                ready,
                native_parent::background_hide_blocked(window.label()),
                || window.hide().map_err(|e| e.to_string()),
            ) {
                eprintln!("Main window remains visible: {error}");
            }
        }
    }
}
