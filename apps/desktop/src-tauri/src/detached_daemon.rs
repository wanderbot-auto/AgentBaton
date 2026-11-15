//! A bundled daemon has an independent Unix session. Only an owned Child is
//! waited/reaped; no PID discovery, signal, kill or shutdown is performed here.
use std::os::unix::process::CommandExt;
use std::{
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
};
unsafe extern "C" {
    fn setsid() -> i32;
}

pub(crate) struct DetachedDaemon {
    child: Option<Child>,
}
impl DetachedDaemon {
    pub(crate) fn spawn(executable: &Path, runtime: &Path) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command
            .env("BATON_RUNTIME_DIR", runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        require_detached_session(&mut command);
        Ok(Self {
            child: Some(command.spawn()?),
        })
    }
    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.as_mut().expect("owned child").try_wait()
    }
}
fn require_detached_session(command: &mut Command) {
    // setsid is async-signal-safe; the child performs no allocation or locks
    // in pre_exec. Failure rejects startup rather than sharing the UI session.
    unsafe {
        command.pre_exec(|| {
            if setsid() == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

impl Drop for DetachedDaemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // A detached service may outlive this UI. wait never requests its stop.
            // If thread creation fails, dropping Child also does not kill it; the
            // operating system reaps it once this UI process actually exits.
            let _ = std::thread::Builder::new()
                .name("baton-daemon-reaper".into())
                .spawn(move || {
                    let _ = child.wait();
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        time::{Duration, Instant},
    };
    #[test]
    #[ignore = "subprocess-only helper; exercised by the actual parent-exit test"]
    fn detached_parent_helper() {
        let Some(root) = std::env::var_os("BACKGROUND_GENERATED_PARENT") else {
            return;
        };
        let root = PathBuf::from(root);
        let child = DetachedDaemon::spawn(&root.join("generated-service"), &root).unwrap();
        // Drop only reaps asynchronously; exiting this generated client must not stop
        // the service. This is an OS process contract, not a batond/macOS UI test.
        drop(child);
        std::process::exit(0);
    }
    fn wait_for(path: &Path) -> Vec<u8> {
        let start = Instant::now();
        loop {
            if let Ok(value) = fs::read(path) {
                if !value.is_empty() {
                    return value;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "generated service deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn a_generated_service_uses_its_own_session_and_outlives_its_parent_client() {
        let root = std::env::temp_dir().join(format!(
            "baton-background-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::write(self.0.join("finish"), b"stop generated fixture only");
            }
        }
        let _cleanup = Cleanup(root.clone());
        let python = Command::new("python3")
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let program = format!(
            "#!{}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            r#"import os,json,time
from pathlib import Path
p=Path(os.environ['BATON_RUNTIME_DIR'])
def publish(name,value):
 tmp=p/(name+'.tmp');tmp.write_text(json.dumps(value));tmp.replace(p/name)
publish('ready.json',{'pid':os.getpid(),'sid':os.getsid(0),'parent':os.getppid()})
started=time.monotonic();count=0
while time.monotonic()-started<4 and not (p/'finish').exists():
 count+=1;publish('progress.json',{'count':count,'sid':os.getsid(0)})
 time.sleep(.03)
publish('finished.json',{'count':count})
"#
        );
        fs::write(root.join("generated-service"), program).unwrap();
        fs::set_permissions(
            root.join("generated-service"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let module = module_path!().split_once("::").unwrap().1;
        let helper = format!("{module}::detached_parent_helper");
        let mut parent = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", helper.as_str(), "--nocapture"])
            .env("BACKGROUND_GENERATED_PARENT", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let parent_pid = parent.id();
        let exit = parent.wait().unwrap();
        assert!(exit.success());
        let ready: Value = serde_json::from_slice(&wait_for(&root.join("ready.json"))).unwrap();
        assert_eq!(ready["pid"], ready["sid"]);
        assert_ne!(ready["sid"], parent_pid);
        let first: Value = serde_json::from_slice(&wait_for(&root.join("progress.json"))).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let second: Value = serde_json::from_slice(&wait_for(&root.join("progress.json"))).unwrap();
        assert!(second["count"].as_u64().unwrap() > first["count"].as_u64().unwrap());
        assert_eq!(second["sid"], ready["sid"]);
        fs::write(root.join("finish"), b"generated fixture finish").unwrap();
        let finished: Value =
            serde_json::from_slice(&wait_for(&root.join("finished.json"))).unwrap();
        assert!(finished["count"].as_u64().unwrap() >= second["count"].as_u64().unwrap());
    }
    #[test]
    fn failed_detached_program_start_does_not_fallback_to_another_launcher() {
        let root = std::env::temp_dir();
        assert!(
            DetachedDaemon::spawn(&root.join("nonexistent-baton-background-binary"), &root)
                .is_err()
        );
    }
    #[test]
    fn failed_session_detachment_refuses_spawn_without_an_attached_fallback() {
        let configured = || {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .arg("--help")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0);
            command
        };
        // Prove that the generated child may become its own process-group
        // leader; that setup itself is not the error under test.
        assert!(configured().spawn().unwrap().wait().unwrap().success());
        let mut command = configured();
        require_detached_session(&mut command);
        // POSIX rejects setsid for a process-group leader. An attached command
        // must not be run as a fallback after this mandatory pre_exec fails.
        let error = command
            .spawn()
            .err()
            .expect("mandatory detachment must fail");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
