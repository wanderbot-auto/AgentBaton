//! Repository fences are physical host/common-dir identities, not caller paths
//! or a project label. Only this domain's actual private stage may pass its own.
use super::{authority::*, budget, records, runtime, *};
use crate::{storage::db_error, Result};
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use rusqlite::Connection;
use serde_json::json;

pub(crate) struct IntegrationFencePermit {
    integration_id: String,
    attempt_id: String,
    repository_digest: String,
}
pub(super) fn for_stage(
    db: &Connection,
    ticket: &runtime::IntegrationStageTicket,
) -> Result<IntegrationFencePermit> {
    for_reference(db, &ticket.read_reference())
}
pub(super) fn for_reference(
    db: &Connection,
    reference: &runtime::IntegrationStageReadRef,
) -> Result<IntegrationFencePermit> {
    let record = budget::attempt(db, &reference.attempt_id)?;
    if record.integration_id != reference.run_id
        || record.view.settled
        || record.policy_id != reference.policy_id
    {
        return Err(denied());
    }
    let policy = records::policy(db, &record.policy_id)?;
    let permit = IntegrationFencePermit {
        integration_id: record.integration_id,
        attempt_id: record.view.attempt_id,
        repository_digest: policy.bindings.repository_digest,
    };
    permit.verify(db)?;
    Ok(permit)
}

impl IntegrationFencePermit {
    fn verify(&self, db: &Connection) -> Result<()> {
        let record = budget::attempt(db, &self.attempt_id)?;
        let run = records::run(db, &self.integration_id)?;
        let policy = records::policy(db, &run.policy_id)?;
        let scope: serde_json::Value = raw(db, "goal_scopes", &policy.selection.goal_scope_id)?;
        let fence = runtime::fence(db, &run)?;
        let now = ms()?;
        if policy.revoked
            || policy.expires_at_ms <= ms()?
            || scope["revoked"] != false
            || scope["revision"] != policy.bindings.goal_scope_revision
            || record.view.settled
            || !matches!(record.view.state.as_str(), "prepared" | "running")
            || scope["expires_at_ms"]
                .as_i64()
                .is_none_or(|expiry| expiry <= now)
            || record.integration_id != run.id
            || run.cancel_requested
            || run.state == IntegrationState::OutcomeUnknown
            || self.repository_digest != policy.bindings.repository_digest
            || fence.state != "held"
            || fence.active_attempt_id.as_deref() != Some(self.attempt_id.as_str())
            || run.active_stage != Some(record.view.kind)
        {
            return Err(denied());
        }
        crate::policy::checked_lineage(db, &policy.bindings.root_grant_id)?;
        if let Some(id) = &policy.bindings.factory_scope_id {
            let factory = crate::factory::checked_child_goal_scope(db, id)?;
            if factory.revoked || factory.expires_at_ms <= ms()? {
                return Err(denied());
            }
        }
        Ok(())
    }
}
pub(super) fn repository_digest(device: &str, inode: &str) -> Result<String> {
    // The pathname/project remains fully approved in static bindings, but must
    // never create another fence key for the same physical repository.
    sha(&json!({"host_id":"local","common_device":device,"common_inode":inode}))
}
pub(crate) fn check_workspace_fences(
    db: &Connection,
    workspace: &WorkspaceIdentity,
    own: Option<&IntegrationFencePermit>,
) -> Result<()> {
    let WorkspaceKind::Git { common_dir, .. } = &workspace.kind else {
        return Ok(());
    };
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version < 17 {
        let count:i64=db.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN('integration_policies','integrations','integration_attempts','integration_ref_fences')",[],|r|r.get(0)).map_err(db_error)?;
        if count == 0 {
            return Ok(());
        }
        return Err(changed());
    }
    if version != 17 {
        return Err(changed());
    }
    let (device, inode) = super::fs::directory_identity(common_dir)?;
    let digest = repository_digest(&device.to_string(), &inode.to_string())?;
    for current in checked_fences(db)? {
        let repository = &current.repository_digest;
        let id = &current.integration_id;
        let state = &current.state;
        if repository != &digest || state == "released" {
            continue;
        }
        let allowed = own.is_some_and(|permit| {
            permit.repository_digest == digest
                && permit.integration_id == *id
                && current.state == "held"
        });
        if !allowed {
            return Err(CoreError::new("WORKSPACE_BUSY", "This physical repository remains fenced by an owned or unknown integration; paths, new projects and lease expiry cannot bypass it"));
        }
        own.ok_or_else(denied)?.verify(db)?;
    }
    Ok(())
}

pub(super) fn checked_fences(db: &Connection) -> Result<Vec<runtime::RefFence>> {
    // Authenticate every stored row before any filter or replacement write.
    // A malformed/moved SQL key must not turn an existing fence into absence.
    let (count, bytes): (i64, i64) = db.query_row(
        "SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(integration_id)+octet_length(repository_digest)+octet_length(target_ref)),0) FROM integration_ref_fences", [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(db_error)?;
    if !(0..=100_000).contains(&count) || !(0..=4_194_304).contains(&bytes) {
        return Err(limited());
    }
    let mut statement = db.prepare("SELECT repository_digest,target_ref,integration_id,revision,state,document FROM integration_ref_fences ORDER BY repository_digest,target_ref").map_err(db_error)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let mut checked = Vec::with_capacity(rows.len());
    for (repository, target, id, revision, state, document) in rows {
        let run = records::run(db, &id)?;
        let current = runtime::fence(db, &run)?;
        let stored: runtime::RefFence = crate::storage::decode(&document)?;
        if current.repository_digest != repository
            || current.target_ref != target
            || current.integration_id != id
            || current.revision != u64::try_from(revision).map_err(|_| changed())?
            || current.state != state
            || !matches!(state.as_str(), "held" | "unknown" | "released")
            || sha(&stored)? != sha(&current)?
        {
            return Err(changed());
        }
        checked.push(current);
    }
    Ok(checked)
}
