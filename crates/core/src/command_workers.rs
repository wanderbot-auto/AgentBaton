//! One owned command worker. SQLite remains exclusively on the daemon actor.
//! Critical results travel through the retained JoinHandle, never a lossy log queue.
use crate::{
    command_validation::{command_error, has_unreaped_child, CommandValidationRecord},
    storage::{Store, TaskState},
    CoreError, Result,
};
use agentbaton_workspaces::command_validation::{run_approved_command, CommandRun};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};
struct Worker {
    policy_id: String,
    task_id: String,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<std::result::Result<CommandRun, CoreError>>,
}
#[derive(Default)]
pub struct CommandWorkers {
    workers: HashMap<String, Worker>,
    retained: HashMap<String, CommandRun>,
}
impl CommandWorkers {
    pub fn has_workers(&self) -> bool {
        !self.workers.is_empty() || !self.retained.is_empty()
    }
    pub fn owns(&self, id: &str) -> bool {
        self.workers.contains_key(id) || self.retained.contains_key(id)
    }
    pub fn cancel(&self, id: &str) {
        if let Some(w) = self.workers.get(id) {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }
    /// Infallible post-commit control using only metadata fixed from the actual
    /// persisted prepared run. Corrupt history cannot prevent a stop request or
    /// turn a committed revocation into a not_started response.
    pub fn cancel_policy(&self, policy: &str) {
        for worker in self
            .workers
            .values()
            .filter(|worker| worker.policy_id == policy)
        {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn cancel_task(&self, task: &str) {
        for worker in self
            .workers
            .values()
            .filter(|worker| worker.task_id == task)
        {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn start(
        &mut self,
        store: &mut Store,
        policy: &str,
        revision: u64,
        key: &str,
    ) -> Result<CommandValidationRecord> {
        if let Some(old) = store.command_replay(policy, revision, key)? {
            return Ok(old);
        }
        if self.has_workers() {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Stop or reconcile the owned command before starting another",
            ));
        }
        let (mut record, rule) = store.prepare_command_validation(policy, revision, key)?;
        let prepared = record.clone();
        record.state = "running".into();
        store.save_command_record(&mut record, "command_validation.worker_starting", false)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        match std::thread::Builder::new()
            .name("baton-fixed-validator".into())
            .spawn(move || run_approved_command(&rule, &flag).map_err(command_error))
        {
            Ok(thread) => {
                self.workers.insert(
                    record.id,
                    Worker {
                        policy_id: record.policy_id.clone(),
                        task_id: record.task_id.clone(),
                        cancel,
                        thread,
                    },
                );
            }
            Err(_) => store.command_failed_before_receipt(
                &record.id,
                CoreError::new(
                    "RESOURCE_LIMIT",
                    "Could not create the owned command worker; no program started",
                ),
                false,
            )?,
        }
        Ok(prepared)
    }
    pub fn tick(&mut self, store: &mut Store) -> Result<()> {
        for (id, w) in &self.workers {
            let r = store.get_command_validation_record(id)?;
            let p = store.get_command_validation_policy(&r.policy_id)?;
            let task = store.get(&r.task_id)?;
            let project = store.get_project(&p.project_id)?;
            if p.revoked
                || project.archived
                || Some(task.revision) != p.refs["task_revision"].as_u64()
                || matches!(
                    task.state,
                    TaskState::CancelRequested | TaskState::Cancelled
                )
                || r.state == "cancel_requested"
            {
                w.cancel.store(true, Ordering::Relaxed);
            }
        }
        let done: Vec<_> = self
            .workers
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in done {
            let worker = self.workers.remove(&id).unwrap();
            match worker.thread.join() {
                Ok(Ok(run)) => {
                    let pending = has_unreaped_child(&run);
                    if pending {
                        self.retained.insert(id.clone(), run);
                        store.finish_command_validation(&id, &self.retained[&id])?;
                    } else {
                        store.finish_command_validation(&id, &run)?;
                    }
                }
                Ok(Err(error)) => store.command_failed_before_receipt(&id, error, false)?,
                Err(_) => store.command_failed_before_receipt(
                    &id,
                    CoreError::new(
                        "OUTCOME_UNKNOWN",
                        "Command worker panicked; process outcome requires reconciliation",
                    ),
                    true,
                )?,
            }
        }
        let ids: Vec<_> = self.retained.keys().cloned().collect();
        for id in ids {
            let run = self.retained.get_mut(&id).unwrap();
            if run.try_reap_owned_leader().unwrap_or(false) {
                store.finish_command_validation(&id, run)?;
                self.retained.remove(&id);
            }
        }
        Ok(())
    }
    pub fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed);
        }
        while self.has_workers() {
            self.tick(store)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    /// Fatal actor paths still retain writer authority until the original Child is
    /// reaped. A kernel-stalled owned process can delay shutdown; never detach it.
    pub(crate) fn reap_owned_workers(&mut self) {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed);
        }
        for (id, w) in self.workers.drain() {
            if let Ok(Ok(run)) = w.thread.join() {
                if has_unreaped_child(&run) {
                    self.retained.insert(id, run);
                }
            }
        }
        while !self.retained.is_empty() {
            self.retained
                .retain(|_, run| !run.try_reap_owned_leader().unwrap_or(false));
            if !self.retained.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
impl Drop for CommandWorkers {
    fn drop(&mut self) {
        self.reap_owned_workers();
    }
}
