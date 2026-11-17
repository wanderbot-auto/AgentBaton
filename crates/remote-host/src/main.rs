use agentbaton_remote_host::{
    protocol::*,
    store::{load_config, Store},
    transport::{input_ready, nonblocking, read_frame, write_frame},
};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn reap(store: &mut Store, children: &mut Vec<(String, Child)>, wait: bool) -> Result<()> {
    let mut first = None;
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let mut i = 0;
        while i < children.len() {
            if let Some(status) = children[i].1.try_wait()? {
                let (id, _) = children.remove(i);
                if let Err(e) = store.child_exited(&id, status.success()) {
                    if first.is_none() {
                        first = Some(e);
                    }
                }
            } else {
                i += 1;
            }
        }
        if !wait || children.is_empty() {
            break;
        }
        if Instant::now() >= deadline {
            for (id, child) in children.iter_mut() {
                let _ = child.kill();
                let status = child.wait()?;
                if let Err(e) = store.child_exited(id, status.success()) {
                    if first.is_none() {
                        first = Some(e);
                    }
                }
            }
            children.clear();
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    first.map_or(Ok(()), Err)
}
fn serve(path: &Path) -> Result<()> {
    let config = load_config(path)?;
    let mut store = Store::open(config, true)?;
    let read_fd = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    let write_fd = unsafe { libc::fcntl(libc::STDOUT_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    require(read_fd >= 0 && write_fd >= 0, "io")?;
    let mut input = unsafe { std::fs::File::from_raw_fd(read_fd) };
    let mut output = unsafe { std::fs::File::from_raw_fd(write_fd) };
    nonblocking(input.as_raw_fd())?;
    nonblocking(output.as_raw_fd())?;
    let mut children = Vec::new();
    let start = Instant::now();
    let limit = Duration::from_millis(
        store
            .config
            .grant
            .expires_ms
            .saturating_sub(now_ms()?)
            .max(0) as u64,
    );
    let result = (|| {
        loop {
            reap(&mut store, &mut children, false)?;
            if !input_ready(input.as_raw_fd())? {
                if start.elapsed() >= limit {
                    store.db_pause()?;
                }
                continue;
            }
            let frame = match read_frame(&mut input) {
                Ok(frame) => frame,
                Err(e) if e.code == "disconnected" => break,
                Err(e) => {
                    let response = Response {
                        version: VERSION,
                        request_id: String::new(),
                        result: Err(e.clone()),
                    };
                    let _ = write_frame(&mut output, &serde_json::to_vec(&response)?);
                    return Err(e);
                }
            };
            let request: Request = match serde_json::from_slice(&frame) {
                Ok(request) => request,
                Err(_) => {
                    let response = Response {
                        version: VERSION,
                        request_id: String::new(),
                        result: Err(Error::new("protocol")),
                    };
                    write_frame(&mut output, &serde_json::to_vec(&response)?)?;
                    continue;
                }
            };
            // A monotonic deadline can only shorten the wall deadline within this process.
            if start.elapsed() >= limit {
                store.db_pause()?;
            }
            let reply = store.handle(&request);
            if let Ok(Reply::Accepted { job }) = &reply {
                if store.needs_spawn(job)? && !children.iter().any(|(id, _)| id == job) {
                    match Command::new(std::env::current_exe()?)
                        .arg("--worker")
                        .arg(path)
                        .arg(job)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(child) => children.push((job.clone(), child)),
                        Err(_) => {
                            store.spawn_failed(job)?;
                        }
                    }
                }
            }
            write_frame(
                &mut output,
                &serde_json::to_vec(&Response {
                    version: VERSION,
                    request_id: request.request_id,
                    result: reply,
                })?,
            )?;
        }
        Ok(())
    })();
    // EOF is a transport loss, not a cancellation; already approved work remains bounded.
    let cleanup = reap(&mut store, &mut children, true);
    result.and(cleanup)
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let result = if args.len() == 3 && args[1] == "--serve" {
        serve(Path::new(&args[2]))
    } else if args.len() == 4 && args[1] == "--worker" {
        (|| {
            let config = load_config(Path::new(&args[2]))?;
            let job = args[3].to_str().ok_or_else(|| Error::new("job"))?;
            require(identifier(job), "job")?;
            agentbaton_remote_host::worker::run(Store::open(config, false)?, job)
        })()
    } else {
        Err(Error::new("usage"))
    };
    if let Err(error) = result {
        eprintln!("baton-host-companion: {}", error.code);
        std::process::exit(1);
    }
}
