//! Actual generated-tree IPC and own-process Native confirmation, never OS login.
#[cfg(not(debug_assertions))]
compile_error!("native background service smoke must never ship in release");
#[cfg(all(feature = "native-smoke", debug_assertions, target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    agentbaton_desktop::native_background_service_test::run()
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("NOT_RUN: real macOS WebView/Native sheet required");
    std::process::exit(2);
}
