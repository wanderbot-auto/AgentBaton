//! Separate debug-only own-process OS dialog test, never shipped or exposed by
//! the production dispatcher. Fixed Grant/Factory fixtures use the real
//! grants_create/factory_scope_approve bridges; no host Factory finalization.
#[cfg(not(debug_assertions))]
compile_error!("native owner dialog smoke must never ship in release");
#[cfg(any(target_os = "macos", test))]
#[path = "../native_owner_dialog_test/controller.rs"]
mod controller;
#[cfg(all(target_os = "macos", feature = "native-smoke"))]
#[path = "../native_owner_dialog_test/macos.rs"]
mod macos;

#[cfg(all(target_os = "macos", feature = "native-smoke"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    macos::run()
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("NOT_RUN: this harness requires a real macOS GUI session");
    std::process::exit(2);
}
