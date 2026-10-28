//! A single bounded domain I/O worker; no SQLite or external process in threads.
use crate::{
    data_workspaces::{
        data_error, DataWorkspaceCreation, DataWorkspacePreview, DataWorkspaceSelection,
    },
    registry::Project,
    storage::Store,
    CoreError, Result,
};
use agentbaton_workspaces::data_workspace::{
    create_data_workspace_controlled, verify_dataset_controlled, ApprovedDataSelection,
    ApprovedManagedRoot, CopyControl,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
const IO_BUDGET: Duration = Duration::from_secs(30);
type ReadResult<T> = Box<Result<T>>;
type DataChecked = (
    crate::data_validation::CheckedDataValidation,
    crate::data_workspaces::DataInputWitness,
);
type IntegrityChecked = (
    crate::validation::CheckedArtifactValidation,
    crate::data_workspaces::DataInputWitness,
);
enum Output {
    Preview(ReadResult<agentbaton_workspaces::data_workspace::DatasetSnapshotView>),
    Copy(ReadResult<agentbaton_workspaces::data_workspace::DataWorkspaceCreation>),
    Admission(ReadResult<crate::data_workspaces::DataInputWitness>),
    Validation(ReadResult<DataChecked>),
    Integrity(ReadResult<IntegrityChecked>),
}
impl Output {
    fn preview(result: Result<agentbaton_workspaces::data_workspace::DatasetSnapshotView>) -> Self {
        Self::Preview(Box::new(result))
    }
    fn copy(result: Result<agentbaton_workspaces::data_workspace::DataWorkspaceCreation>) -> Self {
        Self::Copy(Box::new(result))
    }
    fn admission(result: Result<crate::data_workspaces::DataInputWitness>) -> Self {
        Self::Admission(Box::new(result))
    }
    fn validation(result: Result<DataChecked>) -> Self {
        Self::Validation(Box::new(result))
    }
    fn integrity(result: Result<IntegrityChecked>) -> Self {
        Self::Integrity(Box::new(result))
    }
}
pub(crate) enum CompletedDataRequest {
    Input(
        crate::ipc::ReadyRequest,
        ReadResult<crate::data_workspaces::DataInputWitness>,
    ),
    Validation(crate::ipc::ReadyRequest, ReadResult<DataChecked>),
    Integrity(crate::ipc::ReadyRequest, ReadResult<IntegrityChecked>),
}
struct Worker {
    id: String,
    cancel: Arc<AtomicBool>,
    started: Instant,
    preview: bool,
    pending: Option<crate::ipc::ReadyRequest>,
    projects: Vec<(String, u64)>,
    validation: Option<(String, String, u64, bool)>,
    thread: JoinHandle<Output>,
}
pub(crate) struct DataWorkspaceWorkers {
    runtime_root: std::path::PathBuf,
    runtime_directory: std::fs::File,
    worker: Option<Worker>,
    retained: Option<(String, Output, bool, Option<crate::ipc::ReadyRequest>)>,
}
fn cancelled() -> CoreError {
    CoreError::new(
        "CANCEL_REQUESTED",
        "Data I/O was cancelled before copying; no retry was scheduled",
    )
}
impl DataWorkspaceWorkers {
    pub(crate) fn new(runtime: &std::path::Path) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let runtime_directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK)
                    .bits() as i32,
            )
            .open(runtime)
            .map_err(|_| {
                CoreError::new("PERMISSION_DENIED", "Cannot pin private runtime directory")
            })?;
        let workers = Self {
            runtime_root: runtime.into(),
            runtime_directory,
            worker: None,
            retained: None,
        };
        workers.runtime_privacy_pin()?;
        Ok(workers)
    }
    fn runtime_privacy_pin(&self) -> Result<(u64, u64)> {
        use std::os::unix::fs::MetadataExt;
        let held = self.runtime_directory.metadata().map_err(|_| {
            CoreError::new("PERMISSION_DENIED", "Private runtime identity unavailable")
        })?;
        let current = std::fs::symlink_metadata(&self.runtime_root).map_err(|_| {
            CoreError::new(
                "PERMISSION_DENIED",
                "Private runtime moved; no ordinary dataset reads authorized",
            )
        })?;
        if !held.is_dir()
            || !current.is_dir()
            || (held.dev(), held.ino()) != (current.dev(), current.ino())
            || current.mode() & 0o7777 != 0o700
            || std::fs::canonicalize(&self.runtime_root).ok().as_deref()
                != Some(self.runtime_root.as_path())
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Private runtime directory changed; no ordinary dataset reads authorized",
            ));
        }
        Ok((held.dev(), held.ino()))
    }
    pub(crate) fn has_workers(&self) -> bool {
        self.worker.is_some() || self.retained.is_some()
    }
    pub(crate) fn cancel(&self, id: &str) {
        if let Some(w) = self.worker.as_ref().filter(|w| w.id == id) {
            w.cancel.store(true, Ordering::Relaxed)
        }
    }
    pub(crate) fn preview(
        &mut self,
        store: &mut Store,
        selection: &DataWorkspaceSelection,
        revision: u64,
        key: &str,
    ) -> Result<DataWorkspacePreview> {
        if let Some(old) = store.data_preview_replay(selection, revision, key)? {
            return Ok(old);
        }
        if self.has_workers() {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "The bounded data I/O worker is occupied; read status or cancel its operation",
            ));
        }
        let excluded_runtime = self.runtime_privacy_pin()?;
        let source = store.get_project(&selection.project_id)?;
        if (source.identity.device, source.identity.inode) == excluded_runtime
            || source
                .identity
                .canonical_root
                .starts_with(&self.runtime_root)
            || selection.files.iter().any(|f| {
                source
                    .identity
                    .canonical_root
                    .join(&f.relative_path)
                    .starts_with(&self.runtime_root)
            })
        {
            return Err(CoreError::new("PERMISSION_DENIED","Baton private runtime state is not a dataset source; use separately authorized artifact references for managed outputs"));
        }
        let record = store.prepare_data_preview(selection, revision, key)?;
        if record.state != "checking" {
            return Ok(record);
        }
        let project = store.get_project(&record.project_id)?;
        let project_reference = (project.id.clone(), project.revision);
        let chosen = record.selection.clone();
        let approval = record.id.clone();
        let scheduled = self.spawn(&record.id, true, move |cancel, deadline| {
            Output::preview((|| {
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                let approved = ApprovedDataSelection::from_verified_host(
                    &project.identity.canonical_root,
                    approval,
                    chosen.files,
                    chosen.limits,
                )
                .map_err(data_error)?;
                let contract = approved.contract_view();
                if contract.source.device != project.identity.device
                    || contract.source.inode != project.identity.inode
                {
                    return Err(CoreError::new(
                        "INPUT_CHANGED",
                        "Registered data source was moved or replaced",
                    ));
                }
                let control = CopyControl::new(cancel, deadline)
                    .exclude_directory(excluded_runtime.0, excluded_runtime.1);
                let snapshot =
                    verify_dataset_controlled(&approved, &control).map_err(data_error)?;
                Ok(snapshot.view())
            })())
        });
        if let Err(error) = scheduled {
            store.finish_data_preview(&record.id, Err(error.clone()), false)?;
            return Err(error);
        }
        self.worker
            .as_mut()
            .unwrap()
            .projects
            .push(project_reference);
        Ok(record)
    }
    pub(crate) fn copy(&mut self, store: &mut Store, id: &str) -> Result<()> {
        let record = store.get_data_workspace_creation(id)?;
        if record.state != "creating" || self.worker.as_ref().is_some_and(|w| w.id == id) {
            return Ok(());
        }
        if self.has_workers() {
            return Err(CoreError{side_effect_state:"confirmed".into(),details_ref:Some(record.id),..CoreError::new("WORKSPACE_BUSY","Owner-confirmed copy intent is recorded; the bounded data worker is occupied, reconcile this operation before retrying")});
        }
        let (record, project) = match store.authorized_data_copy_context(id) {
            Ok(context) => context,
            Err(error) => {
                store.finish_data_copy(id, &Err(error.clone()), false)?;
                return Err(error);
            }
        };
        let excluded_runtime = match self.runtime_privacy_pin() {
            Ok(pin) => pin,
            Err(error) => {
                store.finish_data_copy(id, &Err(error.clone()), false)?;
                return Err(error);
            }
        };
        let project_reference = (project.id.clone(), project.revision);
        let result = self.spawn(id, false, move |cancel, deadline| {
            Output::copy(copy_exact(
                &record,
                &project,
                cancel,
                deadline,
                excluded_runtime,
            ))
        });
        if let Err(error) = &result {
            store.finish_data_copy(id, &Err(error.clone()), false)?;
        }
        if result.is_ok() {
            self.worker
                .as_mut()
                .unwrap()
                .projects
                .push(project_reference);
        }
        result
    }
    fn spawn(
        &mut self,
        id: &str,
        preview: bool,
        f: impl FnOnce(&Arc<AtomicBool>, Instant) -> Output + Send + 'static,
    ) -> Result<()> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let started = Instant::now();
        let deadline = started + IO_BUDGET;
        let thread=std::thread::Builder::new().name("baton-data-io".into()).spawn(move||f(&flag, deadline)).map_err(|_|CoreError{side_effect_state:"confirmed".into(),details_ref:Some(id.into()),..CoreError::new("RESOURCE_LIMIT","Data operation intent is recorded but no I/O thread started; reconcile without a new-key replay")})?;
        self.worker = Some(Worker {
            id: id.into(),
            cancel,
            started,
            preview,
            pending: None,
            projects: Vec::new(),
            validation: None,
            thread,
        });
        Ok(())
    }
    pub(crate) fn tick(&mut self, store: &mut Store) -> Result<()> {
        if let Some(w) = &self.worker {
            let project_changed = w.projects.iter().any(|(id, revision)| {
                store
                    .get_project(id)
                    .map_or(true, |p| p.archived || p.revision != *revision)
            });
            if project_changed {
                w.cancel.store(true, Ordering::Relaxed);
                if w.pending.is_none() {
                    if w.preview {
                        let record = store.get_data_workspace_preview(&w.id)?;
                        if record.state == "checking" {
                            store.cancel_data_workspace_preview(
                                &w.id,
                                record.revision,
                                "system-project-changed",
                            )?;
                        }
                    } else {
                        let record = store.get_data_workspace_creation(&w.id)?;
                        if record.state == "creating" {
                            store.cancel_data_workspace_creation(
                                &w.id,
                                record.revision,
                                "system-project-changed",
                            )?;
                        }
                    }
                }
            }
            if let Some((policy, _, revision, data)) = &w.validation {
                let invalid = if *data {
                    store
                        .get_data_validation_policy(policy)
                        .map_or(true, |p| p.revoked || p.revision != *revision)
                } else {
                    store
                        .get_validation_policy(policy)
                        .map_or(true, |p| p.revoked || p.revision != *revision)
                };
                if invalid {
                    w.cancel.store(true, Ordering::Relaxed);
                }
            }
            if w.started.elapsed() >= IO_BUDGET {
                w.cancel.store(true, Ordering::Relaxed);
            }
            if !w.thread.is_finished() {
                return Ok(());
            }
        }
        if let Some(w) = self.worker.take() {
            let stopped = w.cancel.load(Ordering::Relaxed);
            let mut output = w.thread.join().unwrap_or_else(|_| {
                let error = CoreError {
                    side_effect_state: "unknown".into(),
                    ..CoreError::new(
                        "OUTCOME_UNKNOWN",
                        "Data I/O worker panicked; partial observations require reconciliation",
                    )
                };
                if w.pending.is_some() {
                    Output::admission(Err(error))
                } else if w.preview {
                    Output::preview(Err(error))
                } else {
                    Output::copy(Err(error))
                }
            });
            if stopped {
                if matches!(output, Output::Validation(_)) {
                    output = Output::validation(Err(cancelled()));
                } else if matches!(output, Output::Integrity(_)) {
                    output = Output::integrity(Err(cancelled()));
                }
            }
            self.retained = Some((w.id, output, stopped, w.pending));
        }
        if let Some((id, output, stopped, _)) = &self.retained {
            match output {
                Output::Preview(result) => {
                    store.finish_data_preview(id, result.as_ref().clone(), *stopped)?;
                }
                Output::Copy(result) => {
                    store.finish_data_copy(id, result, true)?;
                }
                Output::Admission(_) | Output::Validation(_) | Output::Integrity(_) => {
                    return Ok(())
                }
            }
            // Retain the actual root/result until its journal transaction commits.
            self.retained = None;
        }
        Ok(())
    }
    pub(crate) fn defer(
        &mut self,
        contexts: Vec<crate::data_workspaces::DataInputContext>,
        graph: Option<crate::goal_store::GraphReadContext>,
        job: crate::ipc::ReadyRequest,
    ) -> std::result::Result<(), Box<(CoreError, crate::ipc::ReadyRequest)>> {
        if self.has_workers() {
            return Err(Box::new((CoreError::new("WORKSPACE_BUSY","The bounded data I/O worker is occupied; cancellation and status remain available"),job)));
        }
        let hash = agentbaton_workspaces::validation::sha256(job.raw.as_bytes());
        let id = format!("data-read:{}", uuid::Uuid::new_v4());
        let request_deadline = job.deadline;
        let result=self.spawn(&id,false,move|cancel,deadline|Output::admission((||{
            if cancel.load(Ordering::Relaxed) { return Err(cancelled()); }
            let mut witness=crate::data_workspaces::DataInputWitness::restore(contexts,hash,request_deadline.min(deadline))?;
            if let Some(graph)=graph {witness.graph=Some(graph.execute(cancel.clone(),request_deadline.min(deadline))?);}
            if cancel.load(Ordering::Relaxed) { return Err(cancelled()); }
            if Instant::now()>=request_deadline.min(deadline) { return Err(CoreError::new("DEADLINE_EXCEEDED","The data read observation arrived after admission deadline; no new action was admitted")); }
            Ok(witness)
        })()));
        if let Err(mut error) = result {
            error.side_effect_state = "not_started".into();
            return Err(Box::new((error, job)));
        }
        self.worker.as_mut().unwrap().pending = Some(job);
        Ok(())
    }
    pub(crate) fn validate_data(
        &mut self,
        prepared: Box<crate::data_validation::PreparedDataValidation>,
        witness: crate::data_workspaces::DataInputWitness,
        job: crate::ipc::ReadyRequest,
    ) -> std::result::Result<(), Box<(CoreError, crate::ipc::ReadyRequest)>> {
        if self.has_workers() {
            return Err(Box::new((
                CoreError::new("WORKSPACE_BUSY", "The bounded data worker is occupied"),
                job,
            )));
        }
        let id = format!("data-validation:{}", uuid::Uuid::new_v4());
        let policy = (
            prepared.policy.id.clone(),
            prepared.policy.task_id.clone(),
            prepared.policy.revision,
            true,
        );
        let result = self.spawn(&id, false, move |cancel, _deadline| {
            Output::validation((|| {
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                let checked = prepared.execute()?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                let witness = witness.refresh()?;
                if cancel.load(Ordering::Relaxed) || Instant::now() >= witness.deadline() {
                    return Err(CoreError::new(
                        "DEADLINE_EXCEEDED",
                        "Data validation observation expired or was cancelled; no result committed",
                    ));
                }
                Ok((checked, witness))
            })())
        });
        if let Err(mut error) = result {
            error.side_effect_state = "not_started".into();
            return Err(Box::new((error, job)));
        }
        let worker = self.worker.as_mut().unwrap();
        worker.pending = Some(job);
        worker.validation = Some(policy);
        Ok(())
    }
    pub(crate) fn validate_integrity(
        &mut self,
        prepared: Box<crate::validation::PreparedArtifactValidation>,
        witness: crate::data_workspaces::DataInputWitness,
        job: crate::ipc::ReadyRequest,
    ) -> std::result::Result<(), Box<(CoreError, crate::ipc::ReadyRequest)>> {
        if self.has_workers() {
            return Err(Box::new((
                CoreError::new("WORKSPACE_BUSY", "The bounded data worker is occupied"),
                job,
            )));
        }
        let id = format!("integrity-validation:{}", uuid::Uuid::new_v4());
        let policy = (
            prepared.policy.id.clone(),
            prepared.policy.task_id.clone(),
            prepared.policy.revision,
            false,
        );
        let result = self.spawn(&id, false, move |cancel, _deadline| {
            Output::integrity((|| {
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                let checked = prepared.execute()?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                let witness = witness.refresh()?;
                if cancel.load(Ordering::Relaxed) || Instant::now() >= witness.deadline() {
                    return Err(CoreError::new(
                        "DEADLINE_EXCEEDED",
                        "Integrity observations expired or were cancelled",
                    ));
                }
                Ok((checked, witness))
            })())
        });
        if let Err(mut error) = result {
            error.side_effect_state = "not_started".into();
            return Err(Box::new((error, job)));
        }
        let worker = self.worker.as_mut().unwrap();
        worker.pending = Some(job);
        worker.validation = Some(policy);
        Ok(())
    }
    pub(crate) fn cancel_validation_policy(&self, id: &str) {
        if let Some(w) = &self.worker {
            if w.validation
                .as_ref()
                .is_some_and(|(policy, _, _, _)| policy == id)
            {
                w.cancel.store(true, Ordering::Relaxed);
            }
        }
    }
    pub(crate) fn cancel_task(&self, id: &str) {
        if let Some(w) = &self.worker {
            if w.validation
                .as_ref()
                .is_some_and(|(_, task, _, _)| task == id)
            {
                w.cancel.store(true, Ordering::Relaxed);
            }
        }
    }
    pub(crate) fn take_admission(&mut self) -> Option<CompletedDataRequest> {
        if !self.retained.as_ref().is_some_and(|(_, out, _, _)| {
            matches!(
                out,
                Output::Admission(_) | Output::Validation(_) | Output::Integrity(_)
            )
        }) {
            return None;
        }
        let (_, out, _, pending) = self.retained.take().unwrap();
        let job = pending.expect("owned data read request");
        Some(match out {
            Output::Admission(result) => CompletedDataRequest::Input(job, result),
            Output::Validation(result) => CompletedDataRequest::Validation(job, result),
            Output::Integrity(result) => CompletedDataRequest::Integrity(job, result),
            _ => unreachable!(),
        })
    }
    pub(crate) fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        if let Some(w) = &self.worker {
            w.cancel.store(true, Ordering::Relaxed)
        }
        while self.has_workers() {
            self.tick(store)?;
            if let Some(completed) = self.take_admission() {
                let job = match completed {
                    CompletedDataRequest::Input(job, _)
                    | CompletedDataRequest::Validation(job, _)
                    | CompletedDataRequest::Integrity(job, _) => job,
                };
                let _=job.reply.try_send(serde_json::json!({"request_id":null,"error":CoreError::new("CANCEL_REQUESTED","Daemon shutdown ended the readonly data preflight before admission"),"revision":0,"warnings":[]}));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    pub(crate) fn reap_owned_workers(&mut self) {
        if let Some(w) = self.worker.take() {
            w.cancel.store(true, Ordering::Relaxed);
            let _ = w.thread.join();
        }
        self.retained = None;
    }
}
fn copy_exact(
    record: &DataWorkspaceCreation,
    project: &Project,
    cancel: &AtomicBool,
    deadline: Instant,
    excluded_runtime: (u64, u64),
) -> Result<agentbaton_workspaces::data_workspace::DataWorkspaceCreation> {
    if cancel.load(Ordering::Relaxed) {
        return Err(cancelled());
    }
    if project.archived || project.revision != record.project_revision {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Data project changed before its approved copy",
        ));
    }
    let selection = &record.snapshot.contract;
    let approved = ApprovedDataSelection::from_verified_host(
        &project.identity.canonical_root,
        selection.approval_reference.clone(),
        selection.files.clone(),
        selection.limits.clone(),
    )
    .map_err(data_error)?;
    let control = CopyControl::new(cancel, deadline)
        .exclude_directory(excluded_runtime.0, excluded_runtime.1);
    let snapshot = verify_dataset_controlled(&approved, &control).map_err(data_error)?;
    if snapshot.view() != record.snapshot {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "The approved dataset changed; it was not silently resnapshotted",
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(cancelled());
    }
    let root = ApprovedManagedRoot::from_verified_host(&record.managed_root.canonical_root)
        .map_err(data_error)?;
    if root.view() != record.managed_root {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Host-managed root was replaced before copying",
        ));
    }
    create_data_workspace_controlled(&snapshot, &root, &record.id, &control).map_err(data_error)
}
