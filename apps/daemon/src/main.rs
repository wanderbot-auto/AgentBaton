use agentbaton_core::ipc::{default_runtime_dir, Daemon};
use std::sync::{atomic::AtomicBool, Arc};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> agentbaton_core::Result<()> {
    let runtime = default_runtime_dir()?;
    let daemon = Daemon::bind(&runtime)?;
    eprintln!("batond API v1 ready at {}", runtime.display());
    daemon.run(Arc::new(AtomicBool::new(false)))
}
