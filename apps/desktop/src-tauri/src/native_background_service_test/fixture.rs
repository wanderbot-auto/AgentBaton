// Included only inside the macOS + native-smoke + debug engine module.
use std::cell::RefCell;
use std::os::unix::fs::DirBuilderExt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) enum Mode {
    Production,
    // None is an explicitly invalid fixture mode, NEVER Production.
    Fixture(Option<Arc<Fixture>>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Status,
    Enable,
    Disable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    Normal,
    Race,
    Conflict,
}
impl Case {
    pub(crate) fn parse(s: &str) -> std::result::Result<Self, String> {
        match s {
            "normal" => Ok(Self::Normal),
            "race" => Ok(Self::Race),
            "conflict" => Ok(Self::Conflict),
            _ => Err("Unknown fixed fixture case".into()),
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Race => "race",
            Self::Conflict => "conflict",
        }
    }
}
struct Scope {
    fixture: Arc<Fixture>,
    runtime: PathBuf,
}
thread_local! { static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) }; }
struct ScopeOwner;
impl Drop for ScopeOwner {
    fn drop(&mut self) {
        SCOPE.with(|s| {
            s.borrow_mut().take();
        });
    }
}
pub(super) fn resolve(runtime: &Path) -> Option<Result<Layout>> {
    SCOPE.with(|slot| {
        slot.borrow().as_ref().map(|s| {
            if runtime != s.runtime {
                return Err(Code::StateChanged);
            }
            s.fixture.resolve_layout(runtime)
        })
    })
}
pub(crate) fn invoke(
    fixture: Option<Arc<Fixture>>,
    runtime: &Path,
    op: Operation,
    original: impl FnOnce() -> Value,
) -> Value {
    let Some(fixture) = fixture else {
        return refusal(Code::StateChanged);
    };
    if fixture.begin(runtime, op).is_err() {
        fixture.fail("unexpected_or_unbound_command");
        return refusal(Code::StateChanged);
    }
    let occupied = SCOPE.with(|s| s.borrow().is_some());
    if occupied {
        fixture.fail("nested_fixture_scope");
        return refusal(Code::Busy);
    }
    SCOPE.with(|s| {
        *s.borrow_mut() = Some(Scope {
            fixture: fixture.clone(),
            runtime: runtime.into(),
        })
    });
    let _owner = ScopeOwner;
    let response = original(); // Real command, Native confirmation, CAS and filesystem code.
    if fixture.finish(op, &response).is_err() {
        fixture.fail("response_or_filesystem_mismatch");
    }
    response
}
struct Progress {
    index: usize,
    launched: bool,
    inflight: bool,
    completed: bool,
    failed: Option<&'static str>,
    before: Option<Snapshot>,
    pages: usize,
    ui_requested: bool,
    ui_seen: bool,
    prior_sheet: Option<isize>,
    raced: bool,
    receipts: Vec<Value>,
    ui_reports: usize,
}
pub(crate) struct Fixture {
    // Not serializable. No command/environment/path setter can construct this.
    root: Directory,
    root_path: PathBuf,
    core: Directory,
    core_path: PathBuf,
    runtime: Directory,
    layout: Layout,
    case: Case,
    progress: Mutex<Progress>,
    conflict_leaf: Option<Leaf>,
    real_home_disjoint: bool,
    abi_home_agrees: bool,
    production_custom_runtime_refused: bool,
}
impl Fixture {
    pub(crate) fn create(case: Case) -> std::result::Result<Arc<Self>, String> {
        Self::create_checked(case).map_err(|c| format!("Generated fixture refused: {}", c.text()))
    }
    fn create_checked(case: Case) -> Result<Arc<Self>> {
        let (uid, account_home) = system_home()?; // Actual Darwin ABI, read-only.
        let abi_home_agrees = std::env::var_os("HOME").as_deref() == Some(account_home.as_os_str());
        if !abi_home_agrees {
            return Err(Code::HomeMismatch);
        }
        // Fixed Darwin root-owned sticky parent, not env::temp_dir/HOME input.
        let parent_path = Path::new("/private/tmp");
        let mut changed = false;
        let parent = Directory::open(parent_path, uid, false, false, &mut changed)?
            .ok_or(Code::UnsafePath)?;
        if changed {
            return Err(Code::StateChanged);
        }
        parent.recheck()?;
        let name = format!(
            "abg-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let c = c_name(name.as_ref())?;
        if unsafe { libc::mkdirat(parent.fd(), c.as_ptr(), 0o700) } != 0 {
            return Err(Code::IoFailed);
        }
        parent.recheck()?;
        let root_path = parent_path.join(name);
        if root_path.starts_with(&account_home) || account_home.starts_with(&root_path) {
            return Err(Code::UnsafePath);
        }
        let root =
            Directory::open(&root_path, uid, false, true, &mut changed)?.ok_or(Code::UnsafePath)?;
        // Read-only physical identity, not only lexical prefix disjointness.
        // Never enumerate/create the real Home or its Library/LaunchAgents.
        let account = Directory::open(&account_home, uid, false, false, &mut changed)?
            .ok_or(Code::HomeUnavailable)?;
        let account_identity = account
            .pins
            .last()
            .ok_or(Code::HomeUnavailable)?
            .directory_identity();
        let root_identity = root
            .pins
            .last()
            .ok_or(Code::UnsafePath)?
            .directory_identity();
        if account.pins.last().ok_or(Code::HomeUnavailable)?.uid != uid
            || root.identities().contains(&account_identity)
            || account.identities().contains(&root_identity)
        {
            return Err(Code::UnsafePath);
        }

        let home = root_path.join("h");
        let core_path = root_path.join("c");
        let runtime_path = home.join("Library/Application Support/dev.agentbaton/runtime");
        // Keep BOTH original Unix socket bounds, including registration layout.
        if [&runtime_path, &core_path]
            .iter()
            .any(|p| p.join("batond.sock").as_os_str().as_bytes().len() >= 104)
        {
            return Err(Code::UnsafePath);
        }
        let executable = home.join("Applications/Test.app/Contents/MacOS/agentbaton-desktop");
        for path in [
            &home,
            &core_path,
            &runtime_path,
            executable.parent().ok_or(Code::UnsafePath)?,
        ] {
            Directory::open(path, uid, true, true, &mut changed)?
                .ok_or(Code::UnsafePath)?
                .recheck()?;
        }
        // Generated installation fixtures are never executed, not an installed-app claim.
        for path in [
            &executable,
            &executable.parent().ok_or(Code::UnsafePath)?.join("batond"),
        ] {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o700)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|_| Code::IoFailed)?;
            file.write_all(b"generated registration-only binary fixture; never executed\n")
                .map_err(|_| Code::IoFailed)?;
            file.sync_all().map_err(|_| Code::IoFailed)?;
        }
        let production_custom_runtime_refused =
            matches!(Layout::current(&core_path), Err(Code::CustomRuntime));
        if !production_custom_runtime_refused {
            return Err(Code::StateChanged);
        }
        let layout = Layout::from_paths(uid, home, executable, runtime_path.clone())?;
        let runtime = Directory::open(&runtime_path, uid, false, true, &mut changed)?
            .ok_or(Code::UnsafePath)?;
        let core =
            Directory::open(&core_path, uid, false, true, &mut changed)?.ok_or(Code::UnsafePath)?;
        let mut conflict_leaf = None;
        if case == Case::Conflict {
            let agents = Directory::open(&layout.agents, uid, true, false, &mut changed)?
                .ok_or(Code::UnsafePath)?;
            create_known(
                &agents,
                PLIST,
                b"generated unknown contents: must remain unchanged",
                uid,
                &mut changed,
            )?;
            conflict_leaf = read_leaf(&agents, PLIST, uid)?;
        }
        root.recheck()?;
        Ok(Arc::new(Self {
            root,
            root_path,
            core,
            core_path,
            runtime,
            layout,
            case,
            progress: Mutex::new(Progress {
                index: 0,
                launched: false,
                inflight: false,
                completed: false,
                failed: None,
                before: None,
                pages: 0,
                ui_requested: false,
                ui_seen: false,
                prior_sheet: None,
                raced: false,
                receipts: vec![],
                ui_reports: 0,
            }),
            conflict_leaf,
            real_home_disjoint: true,
            abi_home_agrees,
            production_custom_runtime_refused,
        }))
    }
    pub(crate) fn core_runtime(&self) -> PathBuf {
        self.core_path.clone()
    }
    fn binding(&self, runtime: &Path) -> Result<()> {
        if runtime != self.core_path {
            return Err(Code::CustomRuntime);
        }
        self.root.recheck()?;
        self.core.recheck()?;
        if !self.real_home_disjoint || !self.abi_home_agrees {
            return Err(Code::UnsafePath);
        }
        Ok(())
    }
    fn resolve_layout(&self, runtime: &Path) -> Result<Layout> {
        self.binding(runtime)?;
        self.runtime.recheck()?;
        if self
            .layout
            .runtime
            .join("batond.sock")
            .as_os_str()
            .as_bytes()
            .len()
            >= 104
        {
            return Err(Code::UnsafePath);
        }
        Ok(self.layout.clone())
    }
    fn expected(&self, index: usize) -> Option<Operation> {
        use Operation::*;
        match self.case {
            Case::Normal => [
                Status, Enable, Status, Enable, Status, Disable, Status, Disable, Status,
            ]
            .get(index)
            .copied(),
            Case::Race => [Status, Enable, Status].get(index).copied(),
            Case::Conflict => [Status, Enable].get(index).copied(),
        }
    }
    pub(crate) fn fail(&self, code: &'static str) {
        if let Ok(mut s) = self.progress.lock() {
            s.failed.get_or_insert(code);
        }
    }
    fn begin(&self, runtime: &Path, op: Operation) -> Result<()> {
        self.binding(runtime)?;
        let mut s = self.progress.lock().map_err(|_| Code::Busy)?;
        if s.failed.is_some()
            || s.inflight
            || s.completed
            || !s.launched
            || self.expected(s.index) != Some(op)
        {
            return Err(Code::StateChanged);
        }
        s.inflight = true;
        s.pages = 0;
        s.prior_sheet = None;
        s.before = if self.case == Case::Conflict || s.raced {
            None
        } else {
            Some(inspect(&self.layout)?)
        };
        Ok(())
    }
    fn expected_receipt(&self, index: usize) -> Value {
        if self.case == Case::Conflict {
            return refusal(Code::ContentConflict);
        }
        if self.case == Case::Race && index > 0 {
            return refusal(Code::StateChanged);
        }
        if self.case == Case::Normal && [1, 5].contains(&index) {
            return value("unknown", "cancelled", "cancelled", "not_started");
        }
        let enabled = self.case == Case::Normal && (3..=6).contains(&index);
        let mutating = self.case == Case::Normal && [3, 7].contains(&index);
        value(
            if enabled {
                "enabled_next_login"
            } else {
                "disabled"
            },
            if mutating { "applied" } else { "observed" },
            "ok",
            if mutating { "confirmed" } else { "not_started" },
        )
    }
    fn finish(&self, op: Operation, response: &Value) -> Result<()> {
        let mut s = self.progress.lock().map_err(|_| Code::Busy)?;
        if !s.inflight
            || self.expected(s.index) != Some(op)
            || *response != self.expected_receipt(s.index)
        {
            return Err(Code::StateChanged);
        }
        let need_sheet = op != Operation::Status && self.case != Case::Conflict;
        if need_sheet {
            let wanted = if self.case == Case::Normal && [1, 5].contains(&s.index) {
                1
            } else {
                self.pages_for(op)?.len()
            };
            if s.pages != wanted {
                return Err(Code::StateChanged);
            }
        } else if s.pages != 0 {
            return Err(Code::StateChanged);
        }
        if self.case == Case::Conflict {
            let mut changed = false;
            let dir = Directory::open(
                &self.layout.agents,
                self.layout.uid,
                false,
                false,
                &mut changed,
            )?
            .ok_or(Code::StateChanged)?;
            if self.conflict_leaf.is_none()
                || read_leaf(&dir, PLIST, self.layout.uid)?.as_ref() != self.conflict_leaf.as_ref()
                || Directory::open(
                    &self.layout.state,
                    self.layout.uid,
                    false,
                    true,
                    &mut changed,
                )?
                .is_some()
            {
                return Err(Code::StateChanged);
            }
        } else if s.raced {
            if self.runtime.recheck().is_ok() {
                return Err(Code::StateChanged);
            }
            self.no_registration()?;
        } else {
            let after = inspect(&self.layout)?;
            if [Operation::Status].contains(&op)
                || self.case == Case::Normal && [1, 5].contains(&s.index)
            {
                if s.before.as_ref() != Some(&after) {
                    return Err(Code::StateChanged);
                }
            }
            if !(self.case == Case::Normal && [1, 5].contains(&s.index))
                && after.enabled != (response["login_registration"] == "enabled_next_login")
            {
                return Err(Code::StateChanged);
            }
            if after.enabled
                && (after
                    .plist
                    .as_ref()
                    .is_none_or(|p| p.bytes != self.layout.plist)
                    || after
                        .receipt
                        .as_ref()
                        .is_none_or(|p| p.bytes != self.layout.receipt))
            {
                return Err(Code::StateChanged);
            }
            if self.case == Case::Normal
                && s.index == 7
                && s.before
                    .as_ref()
                    .is_none_or(|before| before.receipt.as_ref() != after.receipt.as_ref())
            {
                return Err(Code::StateChanged);
            }
            if self.case == Case::Normal
                && s.index >= 7
                && (after.plist.is_some()
                    || after
                        .receipt
                        .as_ref()
                        .is_none_or(|p| p.bytes != self.layout.receipt))
            {
                return Err(Code::StateChanged);
            }
        }
        self.root.recheck()?;
        self.core.recheck()?;
        s.inflight = false;
        s.completed = true;
        s.receipts.push(response.clone());
        Ok(())
    }
    fn no_registration(&self) -> Result<()> {
        let mut changed = false;
        for (path, name) in [(&self.layout.agents, PLIST), (&self.layout.state, RECEIPT)] {
            if let Some(dir) = Directory::open(path, self.layout.uid, false, false, &mut changed)? {
                if read_leaf(&dir, name, self.layout.uid)?.is_some() {
                    return Err(Code::StateChanged);
                }
            }
        }
        Ok(())
    }
    fn pages_for(&self, op: Operation) -> Result<Vec<String>> {
        let verb = if op == Operation::Enable {
            "Register the background daemon for NEXT LOGIN"
        } else {
            "Remove NEXT LOGIN registration only"
        };
        let layout = &self.layout;
        let message=format!("{verb}.\n\nNo current daemon or task will be started/stopped by this action. This is not a Grant, Agent dispatch, complete exit, sleep guarantee or crash takeover. The operating system's effective startup state remains unobserved.\n\nProgram: {}\nRuntime: {}\nRegistration: {}\n\n仅修改下次登录的后台注册，不停止当前后台或任务，不新授予权限；本次设置动作不派发Agent、不强开界面。下次登录后台仍遵守原授权、预算和启动对账。系统实际登录启动效果尚未验证。",layout.program.display(),layout.runtime.display(),layout.agents.join(PLIST).display());
        crate::native_background_service_test::presentation::review_prompts(&message)
            .map_err(|_| Code::UnsafePath)
    }
    // Called by Rust after exact real attached-sheet observation, never by renderer.
    pub(crate) fn before_click(
        &self,
        index: usize,
        page: usize,
        number: isize,
    ) -> std::result::Result<bool, String> {
        (|| -> Result<bool> {
            let mut s = self.progress.lock().map_err(|_| Code::Busy)?;
            let op = self.expected(s.index).ok_or(Code::StateChanged)?;
            if s.index != index
                || !s.inflight
                || op == Operation::Status
                || self.case == Case::Conflict
                || page != s.pages
                || s.prior_sheet == Some(number)
            {
                return Err(Code::StateChanged);
            }
            if s.before.as_ref() != Some(&inspect(&self.layout)?) {
                return Err(Code::StateChanged);
            }
            let cancel = self.case == Case::Normal && [1, 5].contains(&index);
            if self.case == Case::Race && page + 1 == self.pages_for(op)?.len() {
                self.runtime.recheck()?;
                std::fs::rename(&self.layout.runtime, self.root_path.join("old-runtime"))
                    .map_err(|_| Code::IoFailed)?;
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&self.layout.runtime)
                    .map_err(|_| Code::IoFailed)?;
                s.raced = true;
            }
            s.pages += 1;
            s.prior_sheet = Some(number);
            Ok(cancel)
        })()
        .map_err(|_| "Unbound real sheet or generated preconfirm check failed".into())
    }
    pub(crate) fn launch(&self) -> std::result::Result<Option<(usize, Operation, bool)>, String> {
        let mut s = self.progress.lock().map_err(|_| "Phase lock")?;
        if let Some(failed) = s.failed {
            return Err(failed.into());
        }
        if s.launched {
            return Ok(None);
        }
        let Some(op) = self.expected(s.index) else {
            return Ok(None);
        };
        s.launched = true;
        Ok(Some((
            s.index,
            op,
            self.case == Case::Conflict && op == Operation::Enable,
        )))
    }
    pub(crate) fn sheet(&self) -> std::result::Result<Option<(usize, usize, Vec<String>)>, String> {
        let s = self.progress.lock().map_err(|_| "Phase lock")?;
        let Some(op) = self.expected(s.index) else {
            return Ok(None);
        };
        if !s.inflight || op == Operation::Status || self.case == Case::Conflict {
            return Ok(None);
        }
        Ok(Some((
            s.index,
            s.pages,
            self.pages_for(op).map_err(|_| "Presentation failed")?,
        )))
    }
    pub(crate) fn ui_report(&self, index: usize, seen: bool) -> std::result::Result<(), String> {
        let mut s = self.progress.lock().map_err(|_| "Phase lock")?;
        if !seen || index != s.index || !s.completed || s.inflight || s.failed.is_some() {
            s.failed.get_or_insert("stale_or_unbound_renderer_report");
            return Err("Unbound renderer observation".into());
        }
        if s.ui_seen || !s.ui_requested {
            return Err("Duplicate/unrequested renderer observation".into());
        }
        s.ui_reports += 1;
        s.ui_seen = true;
        // Observation only: no phase/action/Native click is armed by this handler.
        Ok(())
    }
    pub(crate) fn advance_observed(&self) -> std::result::Result<(), String> {
        self.binding(&self.core_path)
            .map_err(|_| "Fixture binding changed")?;
        let mut s = self.progress.lock().map_err(|_| "Phase lock")?;
        if s.completed && s.ui_seen && !s.inflight && s.failed.is_none() {
            // Only the fixed Rust controller advances its predeclared case.
            s.index += 1;
            s.launched = false;
            s.completed = false;
            s.before = None;
            s.ui_requested = false;
            s.ui_seen = false;
        }
        Ok(())
    }
    pub(crate) fn observe_ui(
        &self,
    ) -> std::result::Result<Option<(usize, &'static str, bool)>, String> {
        let mut s = self.progress.lock().map_err(|_| "Phase lock")?;
        if !s.completed || s.ui_requested {
            return Ok(None);
        }
        s.ui_requested = true;
        let response = self.expected_receipt(s.index);
        let text = if response["login_registration"] == "conflict" {
            "Another registration needs attention"
        } else if response["code"] != "ok" {
            "Login startup is unknown"
        } else if response["login_registration"] == "enabled_next_login" {
            "Enabled for next login"
        } else {
            "Disabled for next login"
        };
        Ok(Some((s.index, text, response["code"] != "ok")))
    }
    pub(crate) fn done(&self) -> bool {
        self.progress
            .lock()
            .is_ok_and(|s| s.failed.is_none() && self.expected(s.index).is_none())
    }
    pub(crate) fn evidence(&self) -> Value {
        let Ok(s) = self.progress.lock() else {
            return json!({"status":"failed","code":"phase_lock"});
        };
        json!({"status":if self.done_without_lock(&s){"passed"}else{"failed"},"case":self.case.name(),
            "dimension":"real_macos_webview_ipc_native_sheet_generated_paths","failed":s.failed,"completed_operations":s.receipts.len(),"renderer_observations_untrusted":s.ui_reports,
            "receipts":s.receipts,"root_private_and_bound":self.root.recheck().is_ok(),"core_runtime_bound":self.core.recheck().is_ok(),"registration_runtime_socket_bounded":self.layout.runtime.join("batond.sock").as_os_str().as_bytes().len()<104,
            "core_runtime_socket_bounded":self.core_path.join("batond.sock").as_os_str().as_bytes().len()<104,"real_home_disjoint":self.real_home_disjoint,"actual_darwin_home_abi_agrees":self.abi_home_agrees,
            "production_custom_runtime_refused_readonly":self.production_custom_runtime_refused,"actual_os_login":"not_run","installed_layout":"generated_not_supported_installation_proof","receipt_before_plist":"source_order_only_not_terminal_chronology","current_daemon_action":"none","effective_login_start":"not_observed",
            "generated_fixture_root":self.root_path,"fixture_retention":"retained_private_no_unknown_data_deleted","conflict_mutation_path":if self.case==Case::Conflict{"explicit_bridge_only_ui_remains_disabled"}else{"production_dom_buttons"},"contract":"abg-registration-fixture-v1","provider_model_calls":"not_run"})
    }
    fn done_without_lock(&self, s: &Progress) -> bool {
        s.failed.is_none() && self.expected(s.index).is_none()
    }
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    #[test]
    fn absent_fixture_never_executes_original_or_resolves_home() {
        let response = invoke(None, Path::new("/not-a-runtime"), Operation::Status, || {
            panic!("must not execute")
        });
        assert_eq!(response, refusal(Code::StateChanged));
    }
    #[test]
    fn wrong_overlong_stale_and_concurrent_bindings_never_execute_original() {
        for mode in 0..4 {
            let f = Fixture::create(Case::Normal).unwrap();
            f.launch().unwrap();
            let runtime = if mode == 0 {
                f.root_path.join("wrong")
            } else if mode == 1 {
                PathBuf::from(format!("/{}", "x".repeat(120)))
            } else {
                f.core_runtime()
            };
            if mode == 2 {
                std::fs::rename(&f.root_path, f.root_path.with_extension("retained-old")).unwrap();
            }
            if mode == 3 {
                f.progress.lock().unwrap().inflight = true;
            }
            let response = invoke(Some(f), &runtime, Operation::Status, || {
                panic!("must not execute")
            });
            assert_eq!(response, refusal(Code::StateChanged));
        }
    }
    #[test]
    fn renderer_cannot_arm_complete_or_replay_a_phase() {
        let f = Fixture::create(Case::Normal).unwrap();
        assert!(f.ui_report(0, true).is_err());
        assert!(!f.done());
        let f = Fixture::create(Case::Normal).unwrap();
        f.launch().unwrap();
        assert!(f.ui_report(1, true).is_err());
        assert!(!f.done());
    }
    #[test]
    fn genuinely_concurrent_second_invocation_refuses_before_original() {
        let f = Fixture::create(Case::Normal).unwrap();
        f.launch().unwrap();
        let (entered, received) = std::sync::mpsc::sync_channel(1);
        let (release, until_release) = std::sync::mpsc::sync_channel(1);
        let worker = f.clone();
        let runtime = f.core_runtime();
        let thread = std::thread::spawn(move || {
            invoke(Some(worker), &runtime, Operation::Status, || {
                entered.send(()).unwrap();
                until_release
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                value("disabled", "observed", "ok", "not_started")
            })
        });
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let response = invoke(
            Some(f.clone()),
            &f.core_runtime(),
            Operation::Status,
            || panic!("concurrent invocation must not execute"),
        );
        release.send(()).unwrap();
        assert_eq!(response, refusal(Code::StateChanged));
        assert_eq!(
            thread.join().unwrap(),
            value("disabled", "observed", "ok", "not_started")
        );
        assert!(!f.done());
    }
    #[test]
    fn panic_scope_does_not_leave_an_override_or_retry_permission() {
        let f = Fixture::create(Case::Normal).unwrap();
        f.launch().unwrap();
        let runtime = f.core_runtime();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| invoke(
                Some(f.clone()),
                &runtime,
                Operation::Status,
                || panic!("controlled test panic")
            )))
            .is_err()
        );
        assert!(resolve(&runtime).is_none());
        assert!(matches!(
            Layout::current(&runtime),
            Err(Code::CustomRuntime)
        ));
        assert_eq!(
            invoke(Some(f), &runtime, Operation::Status, || panic!("no replay")),
            refusal(Code::StateChanged)
        );
    }
    #[test]
    fn real_status_and_dom_observation_cannot_dispatch_or_advance_a_write() {
        let f = Fixture::create(Case::Normal).unwrap();
        f.launch().unwrap();
        let runtime = f.core_runtime();
        let value = invoke(Some(f.clone()), &runtime, Operation::Status, || {
            observe(&runtime)
        });
        assert_eq!(
            value,
            super::value("disabled", "observed", "ok", "not_started")
        );
        assert!(f.observe_ui().unwrap().is_some());
        f.ui_report(0, true).unwrap();
        assert_eq!(f.progress.lock().unwrap().index, 0);
        assert_eq!(f.progress.lock().unwrap().pages, 0);
        f.no_registration().unwrap();
        f.advance_observed().unwrap();
        assert_eq!(f.progress.lock().unwrap().index, 1);
        assert!(!f.progress.lock().unwrap().inflight);
        assert!(f.ui_report(0, true).is_err());
    }
}
