//! Handoff between the existing Factory planner and the same derived Graph.
//! No new planner identity, automatic unknown replay or replacement root Grant.
use super::*;

impl Store {
    pub(super) fn reconcile_factory_child_goals(&mut self, scope_id: &str) -> Result<()> {
        let s = scope(&self.conn, scope_id)?;
        if !s.selection.capabilities.child_goal_execution {
            return Ok(());
        }
        // A genuine accepted result is immutable history, even when accepting
        // this Factory root made new live admission unavailable. This never
        // authorizes a new worker or failed-Goal recovery under an expired,
        // revoked or terminal scope.
        let live = base_live(&self.conn, &s).is_ok();
        let current = run(&self.conn, scope_id)?;
        if current.cancel_requested
            || current.active_session_id.is_some()
            || matches!(
                current.state.as_str(),
                "completed" | "cancelled" | "blocked" | "outcome_unknown"
            )
        {
            return Ok(());
        }
        for b in bindings(&self.conn, &current)? {
            let Some(goal_id) = &b.child_goal_run_id else {
                continue;
            };
            if b.handle.cancel_requested
                || b.handle.result_available
                || !matches!(b.handle.state.as_str(), "waiting_child" | "waiting_goal")
            {
                continue;
            }
            // Private Goal summary authenticates the issuer without requiring a
            // new live I/O read or conflating a UI state with an owned stop.
            let g = self.goal_graph_summary(goal_id)?;
            if g.run.task_id != b.handle.task_id {
                return Err(changed());
            }
            if g.task_accepted && g.run.state == "accepted" {
                self.finish_factory_child_goal(scope_id, &b.handle.task_id, goal_id)?;
            } else if live
                && matches!(g.run.state.as_str(), "blocked" | "failed")
                && self.confirm_factory_child_goal_failure(goal_id)?
            {
                self.prepare_factory_goal_recovery(scope_id, &b.handle.task_id, goal_id)?;
            } else if live && g.run.state == "outcome_unknown" {
                let tx = self
                    .conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(db_error)?;
                let mut r = run(&tx, scope_id)?;
                let mut actual = binding(&tx, &r.id, &b.handle.task_id)?;
                actual.handle.state = "outcome_unknown".into();
                actual.handle.confirmed_stop = false;
                save_binding(&tx, &mut actual)?;
                r.state = "outcome_unknown".into();
                save_run(
                    &tx,
                    &mut r,
                    "factory.child_goal_unknown",
                    json!({"task_id":b.handle.task_id,"goal_run_id":goal_id,"automatic_retry":false,"reservation_refunded":false,"all_graph_attempts_stopped":false}),
                )?;
                tx.commit().map_err(db_error)?;
            }
        }
        Ok(())
    }
    fn prepare_factory_goal_recovery(
        &mut self,
        scope_id: &str,
        task_id: &str,
        goal_id: &str,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let s = scope(&tx, scope_id)?;
        base_live(&tx, &s)?;
        let mut r = run(&tx, scope_id)?;
        let mut b = binding(&tx, &r.id, task_id)?;
        if r.cancel_requested
            || r.active_session_id.is_some()
            || b.child_goal_run_id.as_deref() != Some(goal_id)
            || b.handle.cancel_requested
            || !matches!(b.handle.state.as_str(), "waiting_child" | "waiting_goal")
        {
            return Err(denied());
        }
        let task = Store::factory_child_read_task(&tx, task_id)?;
        // Reusing the planner is permitted only after its own exact normal
        // internal handoff; a Graph failure cannot repair an unknown planner.
        if b.session_id.is_some() || b.handle.attempt_id.is_some() {
            child_plan::require_normal_factory_handoff(&tx, &s, &b, &task)?;
        } else {
            let prior: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM factory_attempts WHERE run_id=?1 AND task_id=?2)",
                    params![r.id, task_id],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if prior || b.handle.state != "waiting_goal" {
                return Err(denied());
            }
        }
        if self.factory_recovery_queue_epochs.len() >= 1000 {
            return Err(limited());
        }
        // The old delegation queue may have handed ownership directly to a
        // Graph without ever launching a Factory planner. This is a NEW
        // planner attempt after authenticated all-stop failure, not a restart
        // of that old queue interval. Include this preparation and every later
        // wait/preflight in its own monotonic interval; never renew Scope TTL.
        let queued_at = std::time::Instant::now();
        let context = match crate::goal_store::reserve_factory_child_goal_rework(&tx, goal_id) {
            Ok(context) => context,
            Err(mut e) if matches!(e.code.as_str(), "RESOURCE_LIMIT" | "BUDGET_UNKNOWN") => {
                e.side_effect_state = "confirmed".into();
                b.handle.state = "blocked".into();
                save_binding(&tx, &mut b)?;
                r.state = "blocked".into();
                r.last_error = Some(e);
                save_run(
                    &tx,
                    &mut r,
                    "factory.child_goal_rework_blocked",
                    json!({"task_id":task_id,"goal_run_id":goal_id,"planner_attempt_started":false,"reservation_refunded":false}),
                )?;
                tx.commit().map_err(db_error)?;
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        let old_session = b.session_id.take();
        b.turn_sent = false;
        b.stop_reason = None;
        b.round = b.round.checked_add(1).ok_or_else(limited)?;
        b.child_goal_recovery = Some(context.clone());
        b.handle.state = "resuming".into();
        // This remains the proof of the *old* stopped planner. The real next
        // reservation replaces session/attempt/fence and clears stop-confirmed.
        b.handle.result_available = false;
        task_prompt(&tx, &s, &b)?;
        save_binding(&tx, &mut b)?;
        r.state = "resuming".into();
        save_run(
            &tx,
            &mut r,
            "factory.child_goal_planner_recovery_prepared",
            json!({"task_id":task_id,"goal_run_id":goal_id,"previous_planner_session_id":old_session,"failure_context":context,"context_continuation":"new_attempt_explicit_handoff_only","same_original_connection":b.request.connection_id,"new_permissions":false,"reservation_refunded":false,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)?;
        // Publish only after the original live/normal-stop/rework guards and
        // all writes commit. Failed/unknown/cancelled recovery emits no epoch;
        // resuming state prevents a replay from creating the same round again.
        self.factory_recovery_queue_epochs
            .push((task_id.to_owned(), b.round, queued_at));
        Ok(())
    }
    fn finish_factory_child_goal(
        &mut self,
        scope_id: &str,
        task_id: &str,
        goal_id: &str,
    ) -> Result<()> {
        let view = self.get_factory_child_goal(&FactoryChildGoalQuery {
            scope_id: scope_id.into(),
            task_id: task_id.into(),
            goal_run_id: Some(goal_id.into()),
        })?;
        let accepted = view.details.task_acceptance.as_ref().ok_or_else(denied)?;
        if !view.details.task_accepted
            || !view.summary.goal.task_accepted
            || accepted.run_id != goal_id
            || accepted.task_id != task_id
        {
            return Err(denied());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let s = scope(&tx, scope_id)?;
        let mut r = run(&tx, scope_id)?;
        let mut b = binding(&tx, &r.id, task_id)?;
        if r.cancel_requested
            || r.active_session_id.is_some()
            || b.child_goal_run_id.as_deref() != Some(goal_id)
            || b.handle.cancel_requested
            || !matches!(b.handle.state.as_str(), "waiting_child" | "waiting_goal")
            || !planner_stop_confirmed(&tx, &b)?
            || !crate::goal_store::factory_child_goal_accepted_stop(&tx, goal_id)?
        {
            return Err(denied());
        }
        // The accepted Graph receipt is a genuine Task acceptance, not the
        // earlier Factory output snapshot or its pre-Graph bytecount.
        b.handle.state = "result_available".into();
        b.handle.result_available = true;
        // Derived Graph all-history stop proof above is distinct from the
        // original planner's stop. Project it before fixing the child result
        // and joining its parent; do not fabricate a Factory worker receipt.
        b.handle.confirmed_stop = true;
        b.child_goal_recovery = None;
        b.result = Some(FactoryTaskResult {
            task: b.handle.clone(),
            artifacts: accepted
                .artifact_refs
                .iter()
                .map(|a| FactoryArtifactReference {
                    artifact_id: a.artifact_id.clone(),
                    attempt_id: a.producer_attempt_id.clone(),
                    relative_path: a.relative_path.clone(),
                    sha256: a.sha256.clone(),
                    bytes: a.bytes,
                })
                .collect(),
            validation_refs: accepted
                .validation_ids
                .iter()
                .cloned()
                .chain(std::iter::once(format!(
                    "goal-task-acceptance:{}",
                    accepted.id
                )))
                .collect(),
            acceptance_state: "task_accepted".into(),
            context_continuation: "new_attempt_explicit_handoff_only".into(),
            handoff_digest: None,
            child_plan: Some(view.summary.plan),
        });
        save_binding(&tx, &mut b)?;
        r.state = if b.handle.parent_task_id.is_none() {
            "completed"
        } else {
            "running"
        }
        .into();
        save_run(
            &tx,
            &mut r,
            "factory.child_goal_accepted",
            json!({"task_id":task_id,"goal_run_id":goal_id,"task_acceptance_id":accepted.id,"acceptance_sequence":accepted.sequence,"task_accepted":true,"new_grant":false,"reservation_refunded":false}),
        )?;
        if let Some(parent) = &b.handle.parent_task_id {
            join_parent(&tx, &mut r, parent)?;
        }
        // No new I/O occurs here: the Goal's accepted history already binds
        // original inputs/root, candidate FilePins/SHA, validators and Owner.
        let _ = s;
        tx.commit().map_err(db_error)
    }
}

pub(super) fn planner_stop_confirmed(db: &Connection, b: &TaskBinding) -> Result<bool> {
    let Some(attempt) = b.handle.attempt_id.as_deref() else {
        return Ok(b.session_id.is_none());
    };
    db.query_row("SELECT EXISTS(SELECT 1 FROM factory_attempts a JOIN sessions s ON s.id=a.session_id JOIN events e WHERE a.attempt_id=?1 AND a.run_id=?2 AND a.scope_id=?3 AND a.task_id=?4 AND (s.id=?5 OR ?5 IS NULL) AND json_extract(a.document,'$.stop_confirmed')=1 AND json_extract(s.document,'$.id')=s.id AND json_extract(s.document,'$.attempt_id')=?1 AND json_extract(s.document,'$.task_id')=?4 AND json_extract(e.document,'$.source')='factory_core' AND json_extract(e.document,'$.kind')='factory.attempt_stopped' AND json_extract(e.document,'$.aggregate_id')=?2 AND json_extract(e.document,'$.payload.session_id')=s.id AND json_extract(e.document,'$.payload.attempt_id')=?1 AND json_extract(e.document,'$.payload.task_id')=?4 AND (json_extract(e.document,'$.payload.owned_reaped')=1 OR json_extract(e.document,'$.payload.never_started')=1))",params![attempt,b.run_id,b.scope_id,b.handle.task_id,b.session_id],|row|row.get(0)).map_err(db_error)
}
