use agentbaton_core::ipc::{default_runtime_dir, Daemon};
use std::sync::{atomic::AtomicBool, Arc};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> agentbaton_core::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, stop.clone()).map_err(|_| {
            agentbaton_core::CoreError::new(
                "INTERNAL_ERROR",
                "Cannot register daemon shutdown signals",
            )
        })?;
    }
    let runtime = default_runtime_dir()?;
    let daemon = Daemon::bind(&runtime)?;
    eprintln!("batond API v1 ready at {}", runtime.display());
    daemon.run(stop)
}
