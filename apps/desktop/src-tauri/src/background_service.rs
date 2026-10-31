//! Explicit, per-user next-login registration only. No service command or Core
//! request is issued here. Current daemon/tasks, sleep and crash ownership are
//! outside this module; a launchd effective state is never inferred from a file.
#[cfg(not(target_os = "macos"))]
use serde_json::json;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Enable,
    Disable,
}

#[cfg(not(target_os = "macos"))]
fn unsupported() -> Value {
    json!({"api_version":1,"platform":"unsupported","mechanism":"unsupported",
        "login_registration":"unknown","outcome":"unsupported","code":"unsupported_platform",
        "side_effect_state":"not_started","current_daemon_action":"none","effective_login_start":"not_observed"})
}

pub(crate) fn status(runtime: &Path) -> Value {
    #[cfg(target_os = "macos")]
    {
        return engine::observe(runtime);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = runtime;
        unsupported()
    }
}

pub(crate) fn change(window: tauri::WebviewWindow, runtime: PathBuf, action: Action) -> Value {
    #[cfg(target_os = "macos")]
    {
        return engine::change(window, runtime, action);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, runtime, action);
        unsupported()
    }
}

pub(crate) fn observation_failed() -> Value {
    #[cfg(target_os = "macos")]
    {
        return engine::failure_value(false);
    }
    #[cfg(not(target_os = "macos"))]
    {
        unsupported()
    }
}

pub(crate) fn mutation_unknown() -> Value {
    #[cfg(target_os = "macos")]
    {
        return engine::failure_value(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        unsupported()
    }
}

// Other Unix platforms exercise only generated-directory tests of these helpers.
// The production commands on those platforms return before any filesystem call.
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(all(test, not(target_os = "macos")), allow(dead_code))]
mod engine {
    use super::Action;
    use serde_json::{json, Value};
    use std::ffi::CString;
    #[cfg(target_os = "macos")]
    use std::ffi::OsString;
    use std::fs::{File, Metadata, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::ffi::OsStrExt;
    #[cfg(target_os = "macos")]
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Component, Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    const LABEL: &str = "dev.agentbaton.daemon";
    const PLIST: &str = "dev.agentbaton.daemon.plist";
    const RECEIPT: &str = "owner.json";
    const MAX_FILE: usize = 64 * 1024;
    const MAX_PATH: usize = 4096;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static MUTATING: AtomicBool = AtomicBool::new(false);
    #[cfg(test)]
    thread_local! {
        // Private in-process filesystem-race control; absent from normal/release
        // binaries and never reachable from a command or renderer input.
        static AFTER_RECEIPT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Code {
        UnsupportedInstallation,
        HomeUnavailable,
        HomeMismatch,
        CustomRuntime,
        UnsafePath,
        OwnershipConflict,
        ContentConflict,
        InstallationChanged,
        StateChanged,
        Busy,
        NativeParentUnavailable,
        IoFailed,
        ReconcileRequired,
    }
    impl Code {
        fn text(self) -> &'static str {
            match self {
                Self::UnsupportedInstallation => "unsupported_installation",
                Self::HomeUnavailable => "home_unavailable",
                Self::HomeMismatch => "home_mismatch",
                Self::CustomRuntime => "custom_runtime",
                Self::UnsafePath => "unsafe_path",
                Self::OwnershipConflict => "ownership_conflict",
                Self::ContentConflict => "content_conflict",
                Self::InstallationChanged => "installation_changed",
                Self::StateChanged => "state_changed",
                Self::Busy => "busy",
                Self::NativeParentUnavailable => "native_parent_unavailable",
                Self::IoFailed => "io_failed",
                Self::ReconcileRequired => "reconcile_required",
            }
        }
    }
    type Result<T> = std::result::Result<T, Code>;

    fn value(registration: &str, outcome: &str, code: &str, effect: &str) -> Value {
        json!({"api_version":1,"platform":"macos","mechanism":"per_user_launchagent",
            "login_registration":registration,"outcome":outcome,"code":code,
            "side_effect_state":effect,"current_daemon_action":"none","effective_login_start":"not_observed"})
    }
    fn refusal(code: Code) -> Value {
        let registration = if matches!(
            code,
            Code::OwnershipConflict | Code::ContentConflict | Code::InstallationChanged
        ) {
            "conflict"
        } else {
            "unknown"
        };
        value(
            registration,
            if code == Code::UnsupportedInstallation {
                "unsupported"
            } else {
                "refused"
            },
            code.text(),
            "not_started",
        )
    }
    pub(super) fn failure_value(mutating: bool) -> Value {
        if mutating {
            value("unknown", "unknown", "reconcile_required", "unknown")
        } else {
            refusal(Code::IoFailed)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Layout {
        uid: u32,
        home: PathBuf,
        executable: PathBuf,
        program: PathBuf,
        runtime: PathBuf,
        agents: PathBuf,
        state: PathBuf,
        plist: Vec<u8>,
        receipt: Vec<u8>,
    }

    fn path_text(path: &Path) -> Result<&str> {
        let text = path.to_str().ok_or(Code::UnsafePath)?;
        if !path.is_absolute()
            || text.len() > MAX_PATH
            || text.as_bytes().contains(&0)
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(Code::UnsafePath);
        }
        Ok(text)
    }
    fn xml(text: &str) -> Result<String> {
        let mut out = String::new();
        for c in text.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                '\n' => out.push_str("&#10;"),
                '\r' => out.push_str("&#13;"),
                '\t' => out.push_str("&#9;"),
                c if c < '\u{20}' || matches!(c, '\u{fffe}' | '\u{ffff}') => {
                    return Err(Code::UnsafePath);
                }
                c => out.push(c),
            }
        }
        Ok(out)
    }
    impl Layout {
        fn from_paths(
            uid: u32,
            home: PathBuf,
            executable: PathBuf,
            runtime: PathBuf,
        ) -> Result<Self> {
            path_text(&home)?;
            path_text(&executable)?;
            path_text(&runtime)?;
            let expected_runtime = home.join("Library/Application Support/dev.agentbaton/runtime");
            if runtime != expected_runtime {
                return Err(Code::CustomRuntime);
            }
            let macos = executable.parent().ok_or(Code::UnsupportedInstallation)?;
            let contents = macos.parent().ok_or(Code::UnsupportedInstallation)?;
            let bundle = contents.parent().ok_or(Code::UnsupportedInstallation)?;
            if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS")
                || contents.file_name().and_then(|s| s.to_str()) != Some("Contents")
                || !bundle
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.ends_with(".app"))
                || executable
                    .components()
                    .any(|c| c.as_os_str() == "AppTranslocation")
            {
                return Err(Code::UnsupportedInstallation);
            }
            let program = macos.join("batond");
            let program_text = path_text(&program)?;
            let home_text = path_text(&home)?;
            let runtime_text = path_text(&runtime)?;
            // Exact, shell-free, owned configuration. It is not loaded now and
            // KeepAlive is deliberately false: no crash/PID takeover is promised.
            let plist = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\n<!-- AgentBaton owned next-login registration v1 -->\n<key>Label</key><string>{LABEL}</string>\n<key>ProgramArguments</key><array><string>{}</string></array>\n<key>EnvironmentVariables</key><dict><key>HOME</key><string>{}</string><key>BATON_RUNTIME_DIR</key><string>{}</string></dict>\n<key>RunAtLoad</key><true/><key>KeepAlive</key><false/>\n<key>LimitLoadToSessionType</key><string>Aqua</string>\n<key>ProcessType</key><string>Background</string>\n</dict></plist>\n", xml(program_text)?, xml(home_text)?, xml(runtime_text)?).into_bytes();
            let receipt = serde_json::to_vec(&json!({"schema":1,"label":LABEL,"program":program_text,
                "home":home_text,"runtime":runtime_text,"plist":String::from_utf8(plist.clone()).map_err(|_|Code::UnsafePath)?})).map_err(|_|Code::UnsafePath)?;
            if plist.len() > MAX_FILE || receipt.len() > MAX_FILE {
                return Err(Code::UnsafePath);
            }
            let agents = home.join("Library/LaunchAgents");
            let state = home.join("Library/Application Support/dev.agentbaton/background-service");
            Ok(Self {
                uid,
                home,
                executable,
                program,
                runtime,
                agents,
                state,
                plist,
                receipt,
            })
        }
        #[cfg(target_os = "macos")]
        fn current(runtime: &Path) -> Result<Self> {
            let (uid, home) = system_home()?;
            if std::env::var_os("HOME").as_deref() != Some(home.as_os_str()) {
                return Err(Code::HomeMismatch);
            }
            if std::env::var_os("BATON_RUNTIME_DIR").is_some_and(|p| {
                PathBuf::from(p) != home.join("Library/Application Support/dev.agentbaton/runtime")
            }) {
                return Err(Code::CustomRuntime);
            }
            if runtime.join("batond.sock").as_os_str().as_bytes().len() >= 104 {
                return Err(Code::UnsafePath);
            }
            Self::from_paths(
                uid,
                home,
                std::env::current_exe().map_err(|_| Code::UnsupportedInstallation)?,
                runtime.into(),
            )
        }
    }

    #[cfg(target_os = "macos")]
    fn system_home() -> Result<(u32, PathBuf)> {
        // libc supplies Darwin's real passwd ABI. No hand-written structure,
        // unbounded C string scan, credential lookup or renderer path is used.
        let uid = unsafe { libc::geteuid() };
        if uid == 0 {
            return Err(Code::OwnershipConflict);
        }
        let mut size = 1024;
        while size <= 64 * 1024 {
            let mut buffer = vec![0u8; size];
            let mut passwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
            let mut found = std::ptr::null_mut();
            let rc = unsafe {
                libc::getpwuid_r(
                    uid,
                    passwd.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                )
            };
            if rc == libc::ERANGE {
                size *= 2;
                continue;
            }
            if rc != 0 || found.is_null() || found != passwd.as_mut_ptr() {
                return Err(Code::HomeUnavailable);
            }
            let passwd = unsafe { passwd.assume_init() };
            if passwd.pw_uid != uid {
                return Err(Code::OwnershipConflict);
            }
            let start = passwd.pw_dir as usize;
            let base = buffer.as_ptr() as usize;
            if start < base || start >= base + buffer.len() {
                return Err(Code::HomeUnavailable);
            }
            let bytes = &buffer[start - base..];
            let end = bytes
                .iter()
                .position(|b| *b == 0)
                .ok_or(Code::HomeUnavailable)?;
            if end == 0 || end > MAX_PATH {
                return Err(Code::HomeUnavailable);
            }
            let home = PathBuf::from(OsString::from_vec(bytes[..end].to_vec()));
            path_text(&home)?;
            return Ok((uid, home));
        }
        Err(Code::HomeUnavailable)
    }

    type DirectoryIdentity = (u64, u64, u32, u32);
    type DirectoryChainIdentity = Vec<DirectoryIdentity>;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Pin {
        device: u64,
        inode: u64,
        uid: u32,
        mode: u32,
        links: u64,
        bytes: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    }
    impl Pin {
        fn from(m: &Metadata) -> Self {
            Self {
                device: m.dev(),
                inode: m.ino(),
                uid: m.uid(),
                mode: m.mode(),
                links: m.nlink(),
                bytes: m.len(),
                modified: (m.mtime(), m.mtime_nsec()),
                changed: (m.ctime(), m.ctime_nsec()),
            }
        }
        fn directory_identity(&self) -> DirectoryIdentity {
            (self.device, self.inode, self.uid, self.mode)
        }
    }
    fn c_name(name: &std::ffi::OsStr) -> Result<CString> {
        CString::new(name.as_bytes()).map_err(|_| Code::UnsafePath)
    }
    fn open_at(fd: RawFd, name: &CString, flags: i32) -> Result<Option<File>> {
        // Owned descriptors and live CStrings remain valid across this call.
        let raw = unsafe {
            libc::openat(
                fd,
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        if raw < 0 {
            return if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                Ok(None)
            } else {
                Err(Code::UnsafePath)
            };
        }
        Ok(Some(unsafe { File::from_raw_fd(raw) }))
    }
    struct Directory {
        files: Vec<File>,
        names: Vec<CString>,
        pins: Vec<Pin>,
    }
    impl Directory {
        fn open(
            path: &Path,
            uid: u32,
            create: bool,
            private: bool,
            changed: &mut bool,
        ) -> Result<Option<Self>> {
            path_text(path)?;
            let root = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open("/")
                .map_err(|_| Code::UnsafePath)?;
            let mut result = Self {
                pins: vec![Pin::from(&root.metadata().map_err(|_| Code::IoFailed)?)],
                files: vec![root],
                names: vec![],
            };
            for part in path.components().filter_map(|c| {
                if let Component::Normal(v) = c {
                    Some(v)
                } else {
                    None
                }
            }) {
                let name = c_name(part)?;
                let fd = result.files.last().ok_or(Code::UnsafePath)?.as_raw_fd();
                let mut next = open_at(fd, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
                if next.is_none() && create {
                    let rc = unsafe { libc::mkdirat(fd, name.as_ptr(), 0o700) };
                    if rc == 0 {
                        *changed = true;
                    } else if std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                        return Err(Code::IoFailed);
                    }
                    next = open_at(fd, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
                }
                let Some(next) = next else { return Ok(None) };
                let meta = next.metadata().map_err(|_| Code::IoFailed)?;
                let sticky_root = meta.uid() == 0 && meta.mode() & 0o1000 != 0;
                if !meta.is_dir()
                    || ![0, uid].contains(&meta.uid())
                    || meta.mode() & 0o022 != 0 && !sticky_root
                {
                    return Err(Code::OwnershipConflict);
                }
                result.pins.push(Pin::from(&meta));
                result.files.push(next);
                result.names.push(name);
            }
            let last = result.pins.last().ok_or(Code::UnsafePath)?;
            if private && (last.uid != uid || last.mode & 0o7777 != 0o700) {
                return Err(Code::OwnershipConflict);
            }
            result.recheck()?;
            Ok(Some(result))
        }
        fn fd(&self) -> RawFd {
            self.files[self.files.len() - 1].as_raw_fd()
        }
        fn recheck(&self) -> Result<()> {
            for (i, name) in self.names.iter().enumerate() {
                let now = open_at(
                    self.files[i].as_raw_fd(),
                    name,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                )?
                .ok_or(Code::StateChanged)?;
                let meta = now.metadata().map_err(|_| Code::IoFailed)?;
                if Pin::from(&meta).directory_identity() != self.pins[i + 1].directory_identity() {
                    return Err(Code::StateChanged);
                }
            }
            Ok(())
        }
        fn identities(&self) -> DirectoryChainIdentity {
            self.pins.iter().map(Pin::directory_identity).collect()
        }
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Leaf {
        pin: Pin,
        bytes: Vec<u8>,
    }
    impl Leaf {
        fn after_owned_rename(&self, before: &Self) -> bool {
            // rename can change ctime itself; every content/identity/mode/mtime
            // field still has to match the held pre-rename observation.
            self.bytes == before.bytes
                && self.pin.device == before.pin.device
                && self.pin.inode == before.pin.inode
                && self.pin.uid == before.pin.uid
                && self.pin.mode == before.pin.mode
                && self.pin.links == before.pin.links
                && self.pin.bytes == before.pin.bytes
                && self.pin.modified == before.pin.modified
        }
    }
    fn read_leaf(dir: &Directory, name: &str, uid: u32) -> Result<Option<Leaf>> {
        dir.recheck()?;
        let name = c_name(name.as_ref())?;
        let Some(mut file) = open_at(dir.fd(), &name, libc::O_RDONLY)? else {
            return Ok(None);
        };
        let meta = file.metadata().map_err(|_| Code::IoFailed)?;
        if !meta.is_file()
            || meta.uid() != uid
            || meta.mode() & 0o7777 != 0o600
            || meta.nlink() != 1
            || meta.len() > MAX_FILE as u64
        {
            return Err(Code::OwnershipConflict);
        }
        let pin = Pin::from(&meta);
        let mut bytes = Vec::new();
        (&mut file)
            .take((MAX_FILE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Code::IoFailed)?;
        let named = open_at(dir.fd(), &name, libc::O_RDONLY)?.ok_or(Code::StateChanged)?;
        if bytes.len() as u64 != pin.bytes
            || bytes.len() > MAX_FILE
            || Pin::from(&file.metadata().map_err(|_| Code::IoFailed)?) != pin
            || Pin::from(&named.metadata().map_err(|_| Code::IoFailed)?) != pin
        {
            return Err(Code::StateChanged);
        }
        dir.recheck()?;
        Ok(Some(Leaf { pin, bytes }))
    }
    fn binary_pin(path: &Path, uid: u32) -> Result<Pin> {
        let mut untouched = false;
        let dir = Directory::open(
            path.parent().ok_or(Code::UnsafePath)?,
            uid,
            false,
            false,
            &mut untouched,
        )?
        .ok_or(Code::UnsupportedInstallation)?;
        let name = c_name(path.file_name().ok_or(Code::UnsafePath)?)?;
        let file =
            open_at(dir.fd(), &name, libc::O_RDONLY)?.ok_or(Code::UnsupportedInstallation)?;
        let meta = file.metadata().map_err(|_| Code::IoFailed)?;
        if !meta.is_file()
            || ![0, uid].contains(&meta.uid())
            || meta.mode() & 0o6022 != 0
            || meta.mode() & 0o111 == 0
            || meta.nlink() != 1
            || meta.len() == 0
            || meta.len() > 128 * 1024 * 1024
        {
            return Err(Code::UnsupportedInstallation);
        }
        let pin = Pin::from(&meta);
        dir.recheck()?;
        Ok(pin)
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Snapshot {
        enabled: bool,
        receipt: Option<Leaf>,
        plist: Option<Leaf>,
        directories: Vec<Option<DirectoryChainIdentity>>,
        runtime_directory: DirectoryChainIdentity,
        binaries: Vec<Pin>,
    }
    impl Snapshot {
        fn registration(&self) -> &'static str {
            if self.enabled {
                "enabled_next_login"
            } else {
                "disabled"
            }
        }
    }
    fn inspect(layout: &Layout) -> Result<Snapshot> {
        let mut untouched = false;
        let home = Directory::open(&layout.home, layout.uid, false, false, &mut untouched)?
            .ok_or(Code::HomeUnavailable)?;
        if home.pins.last().ok_or(Code::UnsafePath)?.uid != layout.uid {
            return Err(Code::OwnershipConflict);
        }
        // Registration must reference the existing private default runtime. Do
        // not create or repair daemon state from a login-start setting operation.
        let runtime = Directory::open(&layout.runtime, layout.uid, false, true, &mut untouched)?
            .ok_or(Code::UnsafePath)?;
        let state = Directory::open(&layout.state, layout.uid, false, true, &mut untouched)?;
        let agents = Directory::open(&layout.agents, layout.uid, false, false, &mut untouched)?;
        if agents
            .as_ref()
            .is_some_and(|d| d.pins[d.pins.len() - 1].uid != layout.uid)
        {
            return Err(Code::OwnershipConflict);
        }
        let receipt = state
            .as_ref()
            .map(|d| read_leaf(d, RECEIPT, layout.uid))
            .transpose()?
            .flatten();
        let plist = agents
            .as_ref()
            .map(|d| read_leaf(d, PLIST, layout.uid))
            .transpose()?
            .flatten();
        if let Some(r) = &receipt {
            if r.bytes != layout.receipt {
                let old = serde_json::from_slice::<Value>(&r.bytes).ok();
                return Err(
                    if old.as_ref().is_some_and(|v| {
                        v["schema"] == 1
                            && v["label"] == LABEL
                            && v["home"].as_str() == layout.home.to_str()
                            && v["program"].as_str() != layout.program.to_str()
                    }) {
                        Code::InstallationChanged
                    } else {
                        Code::ContentConflict
                    },
                );
            }
        }
        if let Some(p) = &plist {
            if p.bytes != layout.plist || receipt.is_none() {
                return Err(Code::ContentConflict);
            }
        }
        Ok(Snapshot {
            enabled: plist.is_some(),
            receipt,
            plist,
            directories: vec![
                Some(home.identities()),
                state.as_ref().map(Directory::identities),
                agents.as_ref().map(Directory::identities),
            ],
            runtime_directory: runtime.identities(),
            binaries: vec![
                binary_pin(&layout.executable, layout.uid)?,
                binary_pin(&layout.program, layout.uid)?,
            ],
        })
    }
    fn temporary() -> String {
        format!(
            ".agentbaton-owned-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }
    fn rename_exclusive(from: &Directory, old: &str, to: &Directory, new: &str) -> Result<()> {
        from.recheck()?;
        to.recheck()?;
        let old = c_name(old.as_ref())?;
        let new = c_name(new.as_ref())?;
        #[cfg(target_os = "macos")]
        let rc = unsafe {
            libc::renameatx_np(
                from.fd(),
                old.as_ptr(),
                to.fd(),
                new.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        #[cfg(target_os = "linux")]
        let rc = unsafe {
            libc::renameat2(
                from.fd(),
                old.as_ptr(),
                to.fd(),
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let rc = -1;
        if rc != 0 {
            return Err(
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                    Code::StateChanged
                } else {
                    Code::IoFailed
                },
            );
        }
        Ok(())
    }
    fn unlink_known(dir: &Directory, name: &str, expected: &Leaf, uid: u32) -> Result<()> {
        // Only private, uniquely named staging/quarantine files can be removed.
        // The registered fixed path is never unlinked after a stale pathname check.
        if read_leaf(dir, name, uid)?.as_ref() != Some(expected) {
            return Err(Code::ReconcileRequired);
        }
        dir.recheck()?;
        let name = c_name(name.as_ref())?;
        if unsafe { libc::unlinkat(dir.fd(), name.as_ptr(), 0) } != 0 {
            return Err(Code::IoFailed);
        }
        Ok(())
    }
    fn create_known(
        dir: &Directory,
        name: &str,
        bytes: &[u8],
        uid: u32,
        changed: &mut bool,
    ) -> Result<()> {
        dir.recheck()?;
        let temp = temporary();
        let leaf = c_name(temp.as_ref())?;
        let raw = unsafe {
            libc::openat(
                dir.fd(),
                leaf.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if raw < 0 {
            return Err(Code::IoFailed);
        }
        *changed = true;
        let mut file = unsafe { File::from_raw_fd(raw) };
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Code::IoFailed)?;
        let expected = read_leaf(dir, &temp, uid)?.ok_or(Code::StateChanged)?;
        if expected.bytes != bytes {
            return Err(Code::StateChanged);
        }
        rename_exclusive(dir, &temp, dir, name)?;
        if !read_leaf(dir, name, uid)?.is_some_and(|moved| moved.after_owned_rename(&expected)) {
            return Err(Code::ReconcileRequired);
        }
        dir.files
            .last()
            .ok_or(Code::IoFailed)?
            .sync_all()
            .map_err(|_| Code::IoFailed)?;
        Ok(())
    }
    fn remove_known(
        agents: &Directory,
        state: &Directory,
        expected: &Leaf,
        uid: u32,
        changed: &mut bool,
    ) -> Result<()> {
        if read_leaf(agents, PLIST, uid)?.as_ref() != Some(expected) {
            return Err(Code::StateChanged);
        }
        let quarantine = temporary();
        rename_exclusive(agents, PLIST, state, &quarantine)?;
        *changed = true;
        let moved = match read_leaf(state, &quarantine, uid) {
            Ok(Some(moved)) if moved.after_owned_rename(expected) => moved,
            _ => {
                // Never delete a raced/unknown file. Restore without clobbering,
                // or retain it privately if another file now owns the fixed name.
                let _ = rename_exclusive(state, &quarantine, agents, PLIST);
                return Err(Code::ReconcileRequired);
            }
        };
        unlink_known(state, &quarantine, &moved, uid)?;
        agents
            .files
            .last()
            .ok_or(Code::IoFailed)?
            .sync_all()
            .map_err(|_| Code::IoFailed)?;
        state
            .files
            .last()
            .ok_or(Code::IoFailed)?
            .sync_all()
            .map_err(|_| Code::IoFailed)?;
        Ok(())
    }
    fn apply(layout: &Layout, before: &Snapshot, action: Action) -> Value {
        let mut changed = false;
        let result = (|| -> Result<()> {
            if inspect(layout)? != *before {
                return Err(Code::StateChanged);
            }
            let wanted = action == Action::Enable;
            if before.enabled == wanted {
                return Ok(());
            }
            let state = Directory::open(&layout.state, layout.uid, true, true, &mut changed)?
                .ok_or(Code::IoFailed)?;
            let agents = Directory::open(&layout.agents, layout.uid, wanted, false, &mut changed)?
                .ok_or(Code::StateChanged)?;
            if agents.pins.last().ok_or(Code::UnsafePath)?.uid != layout.uid {
                return Err(Code::OwnershipConflict);
            }
            if wanted {
                match read_leaf(&state, RECEIPT, layout.uid)? {
                    Some(r) if r.bytes == layout.receipt => {}
                    Some(_) => return Err(Code::ContentConflict),
                    None => {
                        create_known(&state, RECEIPT, &layout.receipt, layout.uid, &mut changed)?
                    }
                }
                #[cfg(test)]
                AFTER_RECEIPT.with(|hook| {
                    if let Some(hook) = hook.borrow_mut().take() {
                        hook();
                    }
                });
                state.recheck()?;
                create_known(&agents, PLIST, &layout.plist, layout.uid, &mut changed)?;
            } else {
                remove_known(
                    &agents,
                    &state,
                    before.plist.as_ref().ok_or(Code::StateChanged)?,
                    layout.uid,
                    &mut changed,
                )?;
                // Keep the private receipt as ownership evidence for a later
                // explicit enable. Its presence alone never enables startup.
            }
            let after = inspect(layout)?;
            if after.enabled != wanted
                || after.binaries != before.binaries
                || after.runtime_directory != before.runtime_directory
            {
                return Err(Code::ReconcileRequired);
            }
            Ok(())
        })();
        match result {
            Ok(()) => value(
                if action == Action::Enable {
                    "enabled_next_login"
                } else {
                    "disabled"
                },
                if changed { "applied" } else { "observed" },
                "ok",
                if changed { "confirmed" } else { "not_started" },
            ),
            Err(Code::ReconcileRequired) => {
                value("unknown", "unknown", "reconcile_required", "unknown")
            }
            Err(_) if changed => value("unknown", "partial", "partial_commit", "confirmed"),
            Err(code) => refusal(code),
        }
    }
    #[cfg(target_os = "macos")]
    pub(super) fn observe(runtime: &Path) -> Value {
        match Layout::current(runtime).and_then(|layout| inspect(&layout)) {
            Ok(snapshot) => value(snapshot.registration(), "observed", "ok", "not_started"),
            Err(code) => refusal(code),
        }
    }
    #[cfg(target_os = "macos")]
    pub(super) fn change(window: tauri::WebviewWindow, runtime: PathBuf, action: Action) -> Value {
        struct Gate;
        impl Drop for Gate {
            fn drop(&mut self) {
                MUTATING.store(false, Ordering::Release);
            }
        }
        if MUTATING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return refusal(Code::Busy);
        }
        let _gate = Gate;
        let result = crate::native_parent::with_caller(window, |parent| {
            let layout = match Layout::current(&runtime) {
                Ok(v) => v,
                Err(e) => return Ok(refusal(e)),
            };
            let before = match inspect(&layout) {
                Ok(v) => v,
                Err(e) => return Ok(refusal(e)),
            };
            let verb = if action == Action::Enable {
                "Register the background daemon for NEXT LOGIN"
            } else {
                "Remove NEXT LOGIN registration only"
            };
            let message=format!("{verb}.\n\nNo current daemon or task will be started/stopped by this action. This is not a Grant, Agent dispatch, complete exit, sleep guarantee or crash takeover. The operating system's effective startup state remains unobserved.\n\nProgram: {}\nRuntime: {}\nRegistration: {}\n\n仅修改下次登录的后台注册，不停止当前后台或任务，不新授予权限；本次设置动作不派发Agent、不强开界面。下次登录后台仍遵守原授权、预算和启动对账。系统实际登录启动效果尚未验证。",layout.program.display(),layout.runtime.display(),layout.agents.join(PLIST).display());
            if !parent.confirm("AgentBaton — Login background registration", &message) {
                return Ok(value("unknown", "cancelled", "cancelled", "not_started"));
            }
            parent.finalize(|| {
                let fresh = match Layout::current(&runtime) {
                    Ok(v) => v,
                    Err(e) => return Ok(refusal(e)),
                };
                if fresh != layout {
                    return Ok(refusal(Code::StateChanged));
                }
                Ok(apply(&layout, &before, action))
            })
        });
        match result {
            Ok(report) if report["api_version"] == 1 => report,
            _ => refusal(Code::NativeParentUnavailable),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use std::os::unix::fs::{symlink, PermissionsExt};

        struct Fixture {
            root: PathBuf,
            layout: Layout,
        }
        impl Fixture {
            fn new() -> Self {
                let uid = unsafe { libc::geteuid() };
                // Cloud /tmp may be world-writable and owned by an unrelated
                // uid. Never relax production owner checks for a test fixture.
                // macOS keeps its canonical per-user temporary directory; Linux
                // uses the actual source directory, validated without symlinks.
                #[cfg(target_os = "macos")]
                let parent_path = fs::canonicalize(std::env::temp_dir()).unwrap();
                #[cfg(not(target_os = "macos"))]
                let parent_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                let mut untouched = false;
                let parent = Directory::open(&parent_path, uid, false, false, &mut untouched)
                    .unwrap()
                    .unwrap();
                assert_eq!(parent.pins.last().unwrap().uid, uid);
                assert!(!untouched);
                parent.recheck().unwrap();
                let name = format!(
                    "ab-bg-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::Relaxed)
                );
                let c = c_name(name.as_ref()).unwrap();
                // Exclusive descriptor-relative creation never replaces a stale
                // or unknown fixture and establishes mode0700 from first create.
                assert_eq!(unsafe { libc::mkdirat(parent.fd(), c.as_ptr(), 0o700) }, 0);
                parent.recheck().unwrap();
                let root = parent_path.join(name);
                let generated = Directory::open(&root, uid, false, true, &mut untouched)
                    .unwrap()
                    .unwrap();
                generated.recheck().unwrap();
                let home = root.join("home");
                fs::create_dir(&home).unwrap();
                fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
                let executable =
                    home.join("Applications/Agent Baton.app/Contents/MacOS/agentbaton-desktop");
                fs::create_dir_all(executable.parent().unwrap()).unwrap();
                for path in [&executable, &executable.parent().unwrap().join("batond")] {
                    fs::write(path, b"generated test executable, never run").unwrap();
                    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
                }
                let runtime = home.join("Library/Application Support/dev.agentbaton/runtime");
                // This is generated fixture setup, never registration behavior.
                fs::create_dir_all(&runtime).unwrap();
                fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
                let layout =
                    Layout::from_paths(unsafe { libc::geteuid() }, home, executable, runtime)
                        .unwrap();
                Self { root, layout }
            }
            fn enable(&self) -> Value {
                let before = inspect(&self.layout).unwrap();
                apply(&self.layout, &before, Action::Enable)
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                if !std::thread::panicking() {
                    let _ = fs::remove_dir_all(&self.root);
                }
            }
        }

        #[test]
        fn default_observation_is_disabled_and_creates_nothing() {
            let f = Fixture::new();
            let before = fs::read_dir(&f.layout.home).unwrap().count();
            let snapshot = inspect(&f.layout).unwrap();
            assert!(!snapshot.enabled);
            assert!(!f.layout.agents.exists());
            assert!(!f.layout.state.exists());
            assert_eq!(fs::read_dir(&f.layout.home).unwrap().count(), before);
        }
        #[test]
        fn next_login_registration_and_revocation_never_execute_a_program() {
            let f = Fixture::new();
            let enabled = f.enable();
            assert_eq!(enabled["outcome"], "applied");
            assert_eq!(enabled["login_registration"], "enabled_next_login");
            assert_eq!(enabled["current_daemon_action"], "none");
            assert_eq!(enabled["effective_login_start"], "not_observed");
            assert_eq!(
                fs::read(f.layout.agents.join(PLIST)).unwrap(),
                f.layout.plist
            );
            let current = inspect(&f.layout).unwrap();
            assert!(current.enabled);
            let repeated = apply(&f.layout, &current, Action::Enable);
            assert_eq!(repeated["outcome"], "observed");
            assert_eq!(repeated["side_effect_state"], "not_started");
            let disabled = apply(&f.layout, &current, Action::Disable);
            assert_eq!(disabled["outcome"], "applied");
            assert!(!f.layout.agents.join(PLIST).exists());
            assert_eq!(
                fs::read(f.layout.state.join(RECEIPT)).unwrap(),
                f.layout.receipt
            );
            assert!(!inspect(&f.layout).unwrap().enabled);
        }
        #[test]
        fn registration_is_fixed_shell_free_and_xml_escaped() {
            let f = Fixture::new();
            let text = String::from_utf8(f.layout.plist.clone()).unwrap();
            assert!(text.contains("<key>RunAtLoad</key><true/><key>KeepAlive</key><false/>"));
            assert!(text.contains("<key>ProgramArguments</key><array><string>"));
            assert!(!text.contains("/bin/sh"));
            assert!(!text.contains("launchctl"));
            assert_eq!(
                xml("<&>\"'\n\r\t").unwrap(),
                "&lt;&amp;&gt;&quot;&apos;&#10;&#13;&#9;"
            );
            assert_eq!(xml("\0"), Err(Code::UnsafePath));
        }
        #[test]
        fn xml_and_layout_reject_xml10_forbidden_scalars_without_rejecting_unicode() {
            let f = Fixture::new();
            assert_eq!(xml("中文😀").unwrap(), "中文😀");
            for forbidden in ['\u{fffe}', '\u{ffff}'] {
                assert_eq!(
                    xml(&format!("private{forbidden}text")),
                    Err(Code::UnsafePath)
                );
                let home = f.root.join(format!("home{forbidden}"));
                assert_eq!(
                    Layout::from_paths(
                        f.layout.uid,
                        home.clone(),
                        f.layout.executable.clone(),
                        home.join("Library/Application Support/dev.agentbaton/runtime"),
                    )
                    .unwrap_err(),
                    Code::UnsafePath
                );
                assert_eq!(
                    Layout::from_paths(
                        f.layout.uid,
                        f.layout.home.clone(),
                        f.layout.home.join(format!(
                            "Applications/Bad{forbidden}.app/Contents/MacOS/desktop"
                        )),
                        f.layout.runtime.clone(),
                    )
                    .unwrap_err(),
                    Code::UnsafePath
                );
            }
            let home = f.root.join("home中文😀");
            assert!(Layout::from_paths(
                f.layout.uid,
                home.clone(),
                home.join("Applications/合法😀.app/Contents/MacOS/desktop"),
                home.join("Library/Application Support/dev.agentbaton/runtime"),
            )
            .is_ok());
        }
        #[test]
        fn missing_symlink_or_nonprivate_runtime_is_refused_without_repair() {
            let f = Fixture::new();
            let before = inspect(&f.layout).unwrap();
            let saved = f.root.join("runtime-saved");
            fs::rename(&f.layout.runtime, &saved).unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::UnsafePath);
            let report = apply(&f.layout, &before, Action::Enable);
            assert_eq!(report["outcome"], "refused");
            assert_eq!(report["side_effect_state"], "not_started");
            assert!(!f.layout.runtime.exists());
            assert!(!f.layout.state.exists());
            assert!(!f.layout.agents.exists());
            symlink(&saved, &f.layout.runtime).unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::UnsafePath);
            assert_eq!(
                apply(&f.layout, &before, Action::Enable)["outcome"],
                "refused"
            );
            assert!(fs::symlink_metadata(&f.layout.runtime)
                .unwrap()
                .file_type()
                .is_symlink());
            fs::remove_file(&f.layout.runtime).unwrap();
            fs::rename(&saved, &f.layout.runtime).unwrap();
            fs::set_permissions(&f.layout.runtime, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::OwnershipConflict);
            assert_eq!(
                apply(&f.layout, &before, Action::Enable)["outcome"],
                "refused"
            );
            assert_eq!(
                fs::metadata(&f.layout.runtime).unwrap().mode() & 0o7777,
                0o755
            );
            assert!(!f.layout.state.exists());
            assert!(!f.layout.agents.exists());
        }
        #[test]
        fn runtime_replacement_invalidates_preconfirmed_enable_and_disable() {
            for action in [Action::Enable, Action::Disable] {
                let f = Fixture::new();
                if action == Action::Disable {
                    assert_eq!(f.enable()["outcome"], "applied");
                }
                let before = inspect(&f.layout).unwrap();
                fs::rename(&f.layout.runtime, f.root.join("runtime-saved")).unwrap();
                fs::create_dir(&f.layout.runtime).unwrap();
                fs::set_permissions(&f.layout.runtime, fs::Permissions::from_mode(0o700)).unwrap();
                assert_ne!(
                    inspect(&f.layout).unwrap().runtime_directory,
                    before.runtime_directory
                );
                let report = apply(&f.layout, &before, action);
                assert_eq!(report["outcome"], "refused");
                assert_eq!(report["code"], "state_changed");
                assert_eq!(report["side_effect_state"], "not_started");
                assert_eq!(
                    f.layout.agents.join(PLIST).exists(),
                    action == Action::Disable
                );
                assert_eq!(
                    f.layout.state.join(RECEIPT).exists(),
                    action == Action::Disable
                );
            }
        }
        #[test]
        fn runtime_replacement_after_receipt_is_never_reported_as_applied() {
            let f = Fixture::new();
            let before = inspect(&f.layout).unwrap();
            let runtime = f.layout.runtime.clone();
            let saved = f.root.join("runtime-saved");
            AFTER_RECEIPT.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    fs::rename(&runtime, saved).unwrap();
                    fs::create_dir(&runtime).unwrap();
                    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
                }))
            });
            let report = apply(&f.layout, &before, Action::Enable);
            assert_eq!(report["outcome"], "unknown");
            assert_eq!(report["code"], "reconcile_required");
            assert_eq!(report["side_effect_state"], "unknown");
            assert_eq!(
                fs::read(f.layout.agents.join(PLIST)).unwrap(),
                f.layout.plist
            );
            assert_eq!(
                fs::read(f.layout.state.join(RECEIPT)).unwrap(),
                f.layout.receipt
            );
        }
        #[test]
        fn nonroot_owned_world_writable_sticky_ancestor_is_refused() {
            let f = Fixture::new();
            // The supported per-user installation does not run as uid0.
            assert_ne!(f.layout.uid, 0);
            let unsafe_parent = f.root.join("untrusted-sticky");
            fs::create_dir(&unsafe_parent).unwrap();
            fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o1777)).unwrap();
            let mut changed = false;
            assert!(matches!(
                Directory::open(&unsafe_parent, f.layout.uid, false, false, &mut changed),
                Err(Code::OwnershipConflict)
            ));
            assert!(!changed);
        }
        #[test]
        fn custom_runtime_relative_and_unbundled_installations_are_refused() {
            let f = Fixture::new();
            assert_eq!(
                Layout::from_paths(
                    f.layout.uid,
                    f.layout.home.clone(),
                    f.layout.executable.clone(),
                    f.root.join("other")
                )
                .unwrap_err(),
                Code::CustomRuntime
            );
            assert_eq!(
                Layout::from_paths(
                    f.layout.uid,
                    f.layout.home.clone(),
                    f.root.join("batond"),
                    f.layout.runtime.clone()
                )
                .unwrap_err(),
                Code::UnsupportedInstallation
            );
            assert_eq!(
                Layout::from_paths(
                    f.layout.uid,
                    PathBuf::from("relative"),
                    f.layout.executable.clone(),
                    f.layout.runtime.clone()
                )
                .unwrap_err(),
                Code::UnsafePath
            );
        }
        #[test]
        fn unknown_existing_plist_or_receipt_is_not_replaced_or_removed() {
            let f = Fixture::new();
            assert_eq!(f.enable()["outcome"], "applied");
            let path = f.layout.agents.join(PLIST);
            fs::write(&path, b"unrelated user configuration").unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::ContentConflict);
            assert_eq!(fs::read(&path).unwrap(), b"unrelated user configuration");
            fs::write(&path, &f.layout.plist).unwrap();
            fs::write(f.layout.state.join(RECEIPT), b"unknown owner").unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::ContentConflict);
            assert_eq!(fs::read(&path).unwrap(), f.layout.plist);
        }
        #[test]
        fn symlink_leaf_ancestor_and_nonprivate_modes_are_refused() {
            let f = Fixture::new();
            f.enable();
            let path = f.layout.agents.join(PLIST);
            fs::rename(&path, f.root.join("saved")).unwrap();
            symlink(f.root.join("saved"), &path).unwrap();
            assert!(inspect(&f.layout).is_err());
            fs::remove_file(&path).unwrap();
            fs::rename(f.root.join("saved"), &path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::OwnershipConflict);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::rename(&f.layout.agents, f.root.join("agents-saved")).unwrap();
            symlink(f.root.join("agents-saved"), &f.layout.agents).unwrap();
            assert!(inspect(&f.layout).is_err());
        }
        #[test]
        fn hardlink_and_replaced_same_content_inode_do_not_gain_ownership() {
            let f = Fixture::new();
            f.enable();
            let before = inspect(&f.layout).unwrap();
            let path = f.layout.agents.join(PLIST);
            fs::hard_link(&path, f.root.join("link")).unwrap();
            assert_eq!(inspect(&f.layout).unwrap_err(), Code::OwnershipConflict);
            fs::remove_file(f.root.join("link")).unwrap();
            fs::remove_file(&path).unwrap();
            fs::write(&path, &f.layout.plist).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let result = apply(&f.layout, &before, Action::Disable);
            assert_eq!(result["outcome"], "refused");
            assert_eq!(result["code"], "state_changed");
            assert!(path.exists());
        }
        #[test]
        fn changed_directory_or_executable_blocks_a_preconfirmed_snapshot() {
            let f = Fixture::new();
            f.enable();
            let before = inspect(&f.layout).unwrap();
            fs::rename(&f.layout.agents, f.root.join("old-agents")).unwrap();
            fs::create_dir(&f.layout.agents).unwrap();
            assert_eq!(
                apply(&f.layout, &before, Action::Disable)["code"],
                "state_changed"
            );
            assert!(f.root.join("old-agents").join(PLIST).exists());
            let current = inspect(&f.layout).unwrap();
            fs::write(&f.layout.program, b"changed package").unwrap();
            assert_eq!(
                apply(&f.layout, &current, Action::Enable)["code"],
                "state_changed"
            );
        }
        #[test]
        fn receipt_only_is_disabled_and_explicit_enable_can_complete_it() {
            let f = Fixture::new();
            let mut changed = false;
            let dir = Directory::open(&f.layout.state, f.layout.uid, true, true, &mut changed)
                .unwrap()
                .unwrap();
            create_known(&dir, RECEIPT, &f.layout.receipt, f.layout.uid, &mut changed).unwrap();
            assert!(!inspect(&f.layout).unwrap().enabled);
            assert_eq!(f.enable()["outcome"], "applied");
        }
        #[test]
        fn directory_replacement_after_receipt_reports_real_partial_and_never_registers() {
            let f = Fixture::new();
            let before = inspect(&f.layout).unwrap();
            let old = f.layout.state.clone();
            let moved = f.root.join("owned-receipt-retained");
            let retained = moved.clone();
            AFTER_RECEIPT.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || fs::rename(old, moved).unwrap()))
            });
            let result = apply(&f.layout, &before, Action::Enable);
            assert_eq!(result["outcome"], "partial");
            assert_eq!(result["code"], "partial_commit");
            assert_eq!(result["side_effect_state"], "confirmed");
            assert!(!f.layout.agents.join(PLIST).exists());
            assert_eq!(fs::read(retained.join(RECEIPT)).unwrap(), f.layout.receipt);
        }
        #[test]
        fn unknown_file_arriving_after_receipt_is_preserved_and_partial_is_not_success() {
            let f = Fixture::new();
            let before = inspect(&f.layout).unwrap();
            let target = f.layout.agents.join(PLIST);
            let raced = target.clone();
            AFTER_RECEIPT.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    fs::write(&raced, b"new external configuration").unwrap();
                    fs::set_permissions(raced, fs::Permissions::from_mode(0o600)).unwrap();
                }))
            });
            let result = apply(&f.layout, &before, Action::Enable);
            assert_eq!(result["outcome"], "partial");
            assert_eq!(result["current_daemon_action"], "none");
            assert_eq!(fs::read(target).unwrap(), b"new external configuration");
            assert_eq!(
                fs::read(f.layout.state.join(RECEIPT)).unwrap(),
                f.layout.receipt
            );
        }
        #[test]
        fn create_never_clobbers_an_existing_unknown_file() {
            let f = Fixture::new();
            let mut changed = false;
            let dir = Directory::open(&f.layout.state, f.layout.uid, true, true, &mut changed)
                .unwrap()
                .unwrap();
            create_known(
                &dir,
                RECEIPT,
                b"first private file",
                f.layout.uid,
                &mut changed,
            )
            .unwrap();
            assert!(create_known(&dir, RECEIPT, b"second", f.layout.uid, &mut changed).is_err());
            assert_eq!(
                read_leaf(&dir, RECEIPT, f.layout.uid)
                    .unwrap()
                    .unwrap()
                    .bytes,
                b"first private file"
            );
        }
        #[test]
        fn detached_directory_descriptor_cannot_write_into_a_replaced_binding() {
            let f = Fixture::new();
            let mut changed = false;
            let dir = Directory::open(&f.layout.state, f.layout.uid, true, true, &mut changed)
                .unwrap()
                .unwrap();
            fs::rename(&f.layout.state, f.root.join("detached")).unwrap();
            fs::create_dir(&f.layout.state).unwrap();
            assert!(
                create_known(&dir, RECEIPT, b"never write", f.layout.uid, &mut changed).is_err()
            );
            assert!(!f.root.join("detached").join(RECEIPT).exists());
            assert!(!f.layout.state.join(RECEIPT).exists());
        }
        #[test]
        fn report_contains_only_closed_state_and_never_private_paths() {
            let f = Fixture::new();
            let report = f.enable();
            let text = report.to_string();
            assert!(!text.contains(f.layout.home.to_str().unwrap()));
            assert_eq!(report.as_object().unwrap().len(), 9);
            assert_eq!(
                refusal(Code::NativeParentUnavailable)["side_effect_state"],
                "not_started"
            );
            assert_eq!(failure_value(true)["side_effect_state"], "unknown");
        }
    }
}
