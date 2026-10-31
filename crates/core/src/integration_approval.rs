//! The one native-approval boundary. Serializable snapshots never substitute for
//! the request-owned observation or for the private authenticated finalizer.
use super::{authority::*, reads::*, records, *};
use crate::{
    sessions::ApprovalProposal,
    storage::{db_error, encode, now, Event, Store},
    Result,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

fn frame<T: serde::Serialize>(value: &T) -> Result<()> {
    // Bound the full envelope with the most expensive allowed escaped request id.
    let envelope = json!({"api_version":1,"request_id":"\0".repeat(128),"sequence":u64::MAX,"revision":u64::MAX,"result":value,"warnings":[]});
    if serde_json::to_vec(&envelope).map_err(|_| invalid())?.len() > MAX_INTEGRATION_BYTES {
        return Err(limited());
    }
    Ok(())
}
pub(super) fn selected_bindings(
    context: &IntegrationContextView,
    s: &IntegrationPolicySelection,
) -> Result<IntegrationStaticBindings> {
    selection_shape(s)?;
    if s.task_id != context.bindings.task_id
        || s.goal_scope_id != context.bindings.goal_scope_id
        || s.expected_task_revision != context.task_revision
        || s.expected_spec_revision != context.bindings.spec_revision
        || s.expected_scope_revision != context.bindings.goal_scope_revision
    {
        return Err(CoreError::new(
            "STALE_REVISION",
            "Integration proposal no longer matches the observed Task and Scope",
        ));
    }
    let mut bindings = context.bindings.clone();
    let mut validators = Vec::new();
    for selected in &s.stage_reservations.validators {
        let original = context
            .bindings
            .validators
            .iter()
            .chain(&context.available_validators)
            .find(|v| v.reference == selected.validator)
            .ok_or_else(denied)?;
        validators.push(original.clone());
    }
    if !bindings
        .outputs
        .iter()
        .flat_map(|o| &o.validators)
        .all(|r| validators.iter().any(|v| v.reference == *r))
        || !validators.iter().any(|v| {
            matches!(
                v.reference.kind,
                crate::goal_store::GoalValidatorKind::Data
                    | crate::goal_store::GoalValidatorKind::Command
            )
        })
    {
        return Err(CoreError::new("AUTH_REQUIRED","Integration requires complete original output coverage and an explicitly approved correctness validator"));
    }
    let mut runtime = 0u64;
    let mut output = 0u64;
    for reservation in std::iter::once(&s.stage_reservations.preparation)
        .chain(
            s.stage_reservations
                .validators
                .iter()
                .map(|v| &v.reservation),
        )
        .chain(std::iter::once(&s.stage_reservations.cas))
    {
        for limits in [&bindings.root_budget, &bindings.project_budget] {
            if reservation.runtime_ms > limits.runtime_ms
                || reservation.output_bytes > limits.output_bytes
            {
                return Err(limited());
            }
        }
        runtime = runtime
            .checked_add(reservation.runtime_ms)
            .ok_or_else(limited)?;
        output = output
            .checked_add(reservation.output_bytes)
            .ok_or_else(limited)?;
    }
    let attempts = u64::try_from(validators.len())
        .map_err(|_| limited())?
        .checked_add(2)
        .and_then(|n| n.checked_mul(u64::from(s.max_rounds)))
        .ok_or_else(limited)?;
    runtime = runtime
        .checked_mul(u64::from(s.max_rounds))
        .ok_or_else(limited)?;
    output = output
        .checked_mul(u64::from(s.max_rounds))
        .ok_or_else(limited)?;
    for limits in [&bindings.root_budget, &bindings.project_budget] {
        if attempts > u64::from(limits.attempts)
            || runtime > limits.runtime_ms
            || output > limits.output_bytes
        {
            return Err(limited());
        }
    }
    bindings.validators = validators;
    bindings.binding_digest.clear();
    bindings.binding_digest = sha(&bindings)?;
    Ok(bindings)
}
fn guarantees() -> IntegrationGuarantees {
    IntegrationGuarantees {
        local_unchecked_out_ref_only: true,
        merged_candidate_revalidation_required: true,
        original_index_and_working_files_unchanged: true,
        output_subject_rebinding_same_paths_only: true,
        fixed_inputs_and_scripts_unchanged: true,
        os_filesystem_sandbox: false,
        os_network_sandbox: false,
        whole_filesystem_output_quota: false,
        remote_delivery_authorized: false,
    }
}
impl Store {
    /// Select this domain only from a bounded, strictly typed stored proposal.
    /// Absence, another domain, or undecodable bytes return to the original
    /// owner-request guards; this probe never grants or finalizes authority.
    pub(crate) fn integration_proposal_for_request(
        &self,
        id: &str,
    ) -> Result<Option<ApprovalProposal>> {
        let row: Option<Option<String>> = self.conn.query_row(
            "SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM approval_proposals WHERE id=?1",
            [id],
            |r| r.get(0),
        ).optional().map_err(db_error)?;
        let Some(Some(text)) = row else {
            return Ok(None);
        };
        // Decode the original text, never a Value that discards duplicate keys.
        let Ok(proposal) = serde_json::from_str::<ApprovalProposal>(&text) else {
            return Ok(None);
        };
        if proposal.kind != "integration" {
            return Ok(None);
        }
        // Preserve the original Integration ID/size/object/typed checks after
        // domain selection. The actor owns this connection throughout routing.
        self.integration_proposal_record(id).map(Some)
    }
    pub(crate) fn integration_proposal_record(&self, id: &str) -> Result<ApprovalProposal> {
        raw(&self.conn, "approval_proposals", id)
    }
    /// Exact request history is resolved before scheduling any read/hash work.
    pub(crate) fn integration_proposal_replay(
        &self,
        s: &IntegrationPolicySelection,
        key_id: &str,
    ) -> Result<Option<ApprovalProposal>> {
        selection_shape(s)?;
        records::replay(&self.conn, "integration:propose", key_id, &encode(s)?)
    }
    pub(crate) fn propose_integration_policy_observed(
        &mut self,
        s: &IntegrationPolicySelection,
        key_id: &str,
        proof: IntegrationReadProof,
        raw_request: &str,
        deadline: Instant,
    ) -> Result<ApprovalProposal> {
        key(key_id)?;
        let payload = encode(s)?;
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = records::replay(&tx, "integration:propose", key_id, &payload)? {
            return Ok(old);
        }
        if !matches!(&proof.intent,IntegrationReadIntent::Proposal(original) if original.as_ref()==s)
        {
            return Err(denied());
        }
        proof.verify(self, raw_request, deadline)?;
        let bindings = selected_bindings(&proof.context, s)?;
        let snapshot=IntegrationPolicySnapshot{intent:s.clone(),bindings,as_of_sequence:proof.context.as_of_sequence,shared_usage:proof.context.shared_usage.clone(),authority_kind:"authorized_local_integration".into(),guarantees:guarantees(),warning:"Independent permission for local unchecked-out branch integration only. Original selected outputs are captured without changing the source index or files; actual merged-candidate correctness is checked anew. Existing lineage, TTL and shared budgets still apply. No OS filesystem/network sandbox, whole-filesystem quota, remote delivery or production permission is granted.".into()};
        let proposal = ApprovalProposal {
            id: Uuid::new_v4().to_string(),
            kind: "integration".into(),
            revision: 1,
            snapshot: serde_json::to_value(snapshot).map_err(|_| invalid())?,
            created_at: now()?,
            expires_at_ms: ms()?.checked_add(300_000).ok_or_else(invalid)?,
            consumed: false,
        };
        frame(&proposal)?;
        tx.execute(
            "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
            params![proposal.id, encode(&proposal)?],
        )
        .map_err(db_error)?;
        records::event(
            &tx,
            &proposal.id,
            1,
            "approval.proposed",
            &json!({"kind":"integration","snapshot":proposal.snapshot}),
        )?;
        records::remember(&tx, "integration:propose", key_id, &payload, &proposal)?;
        // All business rows and held observations are still checked under this
        // immediate transaction, after constructing the exact review frame.
        proof.verify(self, raw_request, deadline)?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(proposal)
    }
    pub(crate) fn integration_finalization_replay(
        &self,
        id: &str,
        revision: u64,
    ) -> Result<Option<Value>> {
        let p: ApprovalProposal = raw(&self.conn, "approval_proposals", id)?;
        if p.kind != "integration" {
            return Ok(None);
        }
        if p.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration proposal changed",
            ));
        }
        if !p.consumed {
            return Ok(None);
        }
        let policy = records::policy(&self.conn, id)?;
        let result:Option<String>=self.conn.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM approval_proposals WHERE id=?1",[id],|r|r.get(0)).map_err(db_error)?;
        let value: Value = crate::storage::decode(&result.ok_or_else(denied)?)?;
        if value["id"] != policy.id {
            return Err(denied());
        }
        Ok(Some(value))
    }
    /// Only the existing private native-confirmed finalize dispatch calls this.
    pub(crate) fn finalize_integration_policy_observed(
        &mut self,
        id: &str,
        revision: u64,
        proof: IntegrationReadProof,
        raw_request: &str,
        deadline: Instant,
    ) -> Result<Value> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = self.integration_finalization_replay(id, revision)? {
            return Ok(old);
        }
        let mut proposal: ApprovalProposal = raw(&tx, "approval_proposals", id)?;
        if proposal.kind != "integration" || proposal.consumed {
            return Err(denied());
        }
        if proposal.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration proposal changed",
            ));
        }
        if proposal.expires_at_ms <= ms()? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Integration proposal expired before native confirmation",
            ));
        }
        if !matches!(&proof.intent,IntegrationReadIntent::Finalize{proposal_id,revision:r} if proposal_id==id&&*r==revision)
        {
            return Err(denied());
        }
        proof.verify(self, raw_request, deadline)?;
        let snapshot: IntegrationPolicySnapshot =
            serde_json::from_value(proposal.snapshot.clone()).map_err(|_| changed())?;
        let mut current = proof.context.clone();
        // Runtime Task revision changes are not a spec/Goal permission upgrade.
        // Compare the exact original static bindings, not dynamic as-of usage.
        current.task_revision = snapshot.intent.expected_task_revision;
        let bindings = selected_bindings(&current, &snapshot.intent)?;
        if sha(&bindings)? != sha(&snapshot.bindings)?
            || snapshot.authority_kind != "authorized_local_integration"
            || snapshot.guarantees != guarantees()
        {
            return Err(changed());
        }
        let mut expiry = ms()?
            .checked_add(i64::try_from(snapshot.intent.ttl_ms).map_err(|_| invalid())?)
            .ok_or_else(invalid)?
            .min(bindings.goal_scope_expires_at_ms);
        for grant in &bindings.grant_lineage {
            expiry = expiry.min(grant.expires_at_ms);
        }
        if expiry <= ms()? {
            return Err(denied());
        }
        let policy = IntegrationPolicy {
            id: id.into(),
            revision: 1,
            approval_id: id.into(),
            selection: snapshot.intent,
            bindings,
            snapshot_digest: sha(&proposal.snapshot)?,
            created_at: now()?,
            expires_at_ms: expiry,
            revoked: false,
        };
        frame(&policy)?;
        proposal.consumed = true;
        if tx.execute("UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",params![encode(&proposal)?,encode(&policy)?,id]).map_err(db_error)?!=1{return Err(changed());}
        let owner = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: id.into(),
            revision,
            kind: "approval.confirmed".into(),
            occurred_at: now()?,
            payload: json!({"kind":"integration","proposal_id":id,"consumed":true}),
            source: "local_owner_confirmation".into(),
        };
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&owner)?])
            .map_err(db_error)?;
        let owner_sequence = records::sequence(&tx)?;
        tx.execute("INSERT INTO integration_policies(id,task_id,spec_revision,goal_scope_id,approval_id,revision,document) VALUES(?1,?2,?3,?4,?1,1,?5)",params![id,policy.selection.task_id,sql_num(policy.selection.expected_spec_revision)?,policy.selection.goal_scope_id,encode(&policy)?]).map_err(db_error)?;
        records::event(
            &tx,
            id,
            1,
            "integration.policy_approved",
            &json!({"snapshot_digest":policy.snapshot_digest,"owner_sequence":owner_sequence,"scope_digest":policy.bindings.goal_scope_digest,"binding_digest":policy.bindings.binding_digest,"automatic_watch_armed":true}),
        )?;
        proof.seal_observation()?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        serde_json::to_value(policy).map_err(|_| invalid())
    }
}

#[cfg(test)]
mod request_domain_tests {
    use super::*;

    fn proposal(id: &str, kind: &str) -> String {
        serde_json::to_string(&ApprovalProposal {
            id: id.into(),
            kind: kind.into(),
            revision: 1,
            snapshot: json!({}),
            created_at: "2026-10-31T00:00:00Z".into(),
            expires_at_ms: i64::MAX,
            consumed: false,
        })
        .unwrap()
    }
    fn put(store: &Store, id: &str, text: &str) {
        store
            .conn
            .execute(
                "INSERT INTO approval_proposals(id,document,result) VALUES(?1,?2,NULL)",
                params![id, text],
            )
            .unwrap();
    }
    fn unchanged(store: &Store, sequence: u64, documents: &[(String, String)]) {
        assert_eq!(store.sequence().unwrap(), sequence);
        assert_eq!(store.integration_record_counts().unwrap().policies, 0);
        assert_eq!(store.integration_record_counts().unwrap().runs, 0);
        let rows: Vec<(String, String)> = store
            .conn
            .prepare("SELECT id,document FROM approval_proposals WHERE result IS NULL ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(rows, documents);
    }
    fn documents(store: &Store) -> Vec<(String, String)> {
        store
            .conn
            .prepare("SELECT id,document FROM approval_proposals ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn optional_domain_probe_leaves_missing_and_original_kinds_to_original_owner_guards() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(&root.path().join("baton.sqlite3")).unwrap();
        assert!(store
            .integration_proposal_for_request("unknown-owner-proposal")
            .unwrap()
            .is_none());
        for kind in [
            "grant",
            "factory",
            "goal_execution",
            "goal_graph_execution",
            "goal_node_acceptance",
        ] {
            put(&store, kind, &proposal(kind, kind));
            assert!(
                store
                    .integration_proposal_for_request(kind)
                    .unwrap()
                    .is_none(),
                "{kind}"
            );
        }
        put(
            &store,
            "integration-only",
            &proposal("integration-only", "integration"),
        );
        let selected = store
            .integration_proposal_for_request("integration-only")
            .unwrap()
            .unwrap();
        assert_eq!(selected.id, "integration-only");
        assert_eq!(selected.kind, "integration");
        let before = documents(&store);
        let sequence = store.sequence().unwrap();
        assert!(store
            .integration_proposal_for_request("unknown-owner-proposal")
            .unwrap()
            .is_none());
        unchanged(&store, sequence, &before);
    }

    #[test]
    fn malformed_duplicate_unknown_and_oversized_integration_fallback_never_finalize() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(&root.path().join("baton.sqlite3")).unwrap();
        for id in [
            "malformed",
            "duplicate-kind",
            "unknown-authority",
            "oversized",
        ] {
            // Each original document matches its row: only the named corruption
            // may explain refusal, never an unrelated record-identity mismatch.
            let original = proposal(id, "integration");
            let text = match id {
                "malformed" => original[..original.len() - 1].to_owned(),
                "duplicate-kind" => original.replace(
                    "\"kind\":\"integration\"",
                    "\"kind\":\"integration\",\"kind\":\"grant\"",
                ),
                "unknown-authority" => original.replacen('{', "{\"caller_authorized\":true,", 1),
                "oversized" => format!(
                    "{original}{}",
                    " ".repeat(MAX_INTEGRATION_BYTES + 1 - original.len())
                ),
                _ => unreachable!(),
            };
            put(&store, id, &text);
            let before = documents(&store);
            let sequence = store.sequence().unwrap();
            assert!(
                store
                    .integration_proposal_for_request(id)
                    .unwrap()
                    .is_none(),
                "{id}"
            );
            let error = store.finalize_proposal(id, 1).unwrap_err();
            if id == "oversized" {
                assert_eq!(error.code, "RESOURCE_LIMIT", "{id}: {error:?}");
                assert_eq!(error.message, "Integration record or shared resource bound exceeded; no history was truncated");
            } else {
                assert_eq!(error.code, "INTERNAL_ERROR", "{id}: {error:?}");
                assert_eq!(
                    error.message,
                    "Stored record is invalid; restore or export the database"
                );
            }
            assert_eq!(error.side_effect_state, "not_started", "{id}: {error:?}");
            unchanged(&store, sequence, &before);
        }
    }

    #[test]
    fn exact_byte_bound_retains_strict_integration_id_and_native_finalizer_checks() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(&root.path().join("baton.sqlite3")).unwrap();
        let original = proposal("bounded", "integration");
        let bounded = format!(
            "{original}{}",
            " ".repeat(MAX_INTEGRATION_BYTES - original.len())
        );
        put(&store, "bounded", &bounded);
        assert!(store
            .integration_proposal_for_request("bounded")
            .unwrap()
            .is_some());
        let before = documents(&store);
        let sequence = store.sequence().unwrap();
        assert_eq!(
            store.finalize_proposal("bounded", 1).unwrap_err().code,
            "AUTH_REQUIRED"
        );
        unchanged(&store, sequence, &before);
        put(&store, "invalid id", &proposal("invalid id", "integration"));
        assert_eq!(
            store
                .integration_proposal_for_request("invalid id")
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }
}
