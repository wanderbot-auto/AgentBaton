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
            || policy.bindings.goal_scope_expires_at_ms <= now
            || scope["scope_digest"] != policy.bindings.goal_scope_digest
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
    if version != 18 {
        return Err(changed());
    }
    crate::conversations::validate_schema(db)?;
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

/// Checked in the same immediate transaction as integration admission. A second
/// managed worktree is still a writer to the same physical Git repository.
pub(super) fn check_conversation_writers(
    db: &Connection,
    source: &IntegrationGitPin,
) -> Result<()> {
    crate::conversations::validate_schema(db)?;
    let (count, bytes): (i64, i64) = db.query_row(
        "SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(project_id)+octet_length(workspace_id)+octet_length(connection_id)+octet_length(grant_id)+octet_length(state)),0) FROM conversations",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(db_error)?;
    if !(0..=100_000).contains(&count) || !(0..=4_194_304).contains(&bytes) {
        return Err(limited());
    }
    // Validate mirrors before filtering: a moved SQL state must not hide a writer.
    let mut query = db.prepare("SELECT id,project_id,workspace_id,connection_id,grant_id,state,document FROM conversations ORDER BY id").map_err(db_error)?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(db_error)?;
    let mut workspace_bytes = 4_194_304_i64;
    for row in rows {
        let (id, project, workspace, connection, grant, state, document) = row.map_err(db_error)?;
        let c: crate::conversations::Conversation = crate::storage::decode(&document)?;
        if c.id != id
            || c.project_id != project
            || c.workspace_id != workspace
            || c.connection_id != connection
            || c.grant_id != grant
            || c.state != state
        {
            return Err(changed());
        }
        if matches!(state.as_str(), "stopped" | "failed") {
            continue;
        }
        let size: i64 = db
            .query_row(
                "SELECT octet_length(document) FROM managed_workspaces WHERE id=?1",
                [&workspace],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if size < 0 || size > workspace_bytes {
            return Err(limited());
        }
        workspace_bytes -= size;
        let managed: crate::registry::ManagedWorkspace = raw(db, "managed_workspaces", &workspace)?;
        let identity = managed.identity.as_ref().ok_or_else(changed)?;
        if managed.id != workspace
            || managed.project_id != project
            || managed.root != identity.canonical_root
        {
            return Err(changed());
        }
        let WorkspaceKind::Git { common_dir, .. } = &identity.kind else {
            return Err(changed());
        };
        // Compare admission pins first. Even a renamed/deleted directory cannot
        // turn an owned writer to the original repository into absence.
        if c.git_common_device.to_string() == source.common_device
            && c.git_common_inode.to_string() == source.common_inode
        {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "This physical repository still has an active or unknown conversation writer",
            ));
        }
        if super::fs::directory_identity(&identity.canonical_root)?
            != (identity.device, identity.inode)
            || super::fs::directory_identity(common_dir)?
                != (c.git_common_device, c.git_common_inode)
        {
            return Err(changed());
        }
    }
    Ok(())
}

#[cfg(test)]
mod conversation_writer_tests {
    use super::*;
    use crate::{
        conversations::Conversation,
        registry::ManagedWorkspace,
        storage::{encode, Store},
    };
    use rusqlite::params;

    // Synthetic persisted owner records test admission boundaries, not Provider
    // execution. Separate directories model distinct managed worktree roots.
    fn fixture() -> (tempfile::TempDir, Store, IntegrationGitPin, Conversation) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state.sqlite3")).unwrap();
        store
            .conn
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        let common = dir.path().join("common");
        let root = dir.path().join("other-worktree");
        std::fs::create_dir(&common).unwrap();
        std::fs::create_dir(&root).unwrap();
        let (device, inode) = super::super::fs::directory_identity(&root).unwrap();
        let (cd, ci) = super::super::fs::directory_identity(&common).unwrap();
        let w = ManagedWorkspace {
            id: "other-workspace".into(),
            project_id: "project".into(),
            identity: Some(WorkspaceIdentity {
                canonical_root: root.clone(),
                device,
                inode,
                kind: WorkspaceKind::Git {
                    common_dir: common.clone(),
                    head: "a".repeat(40),
                    dirty: false,
                },
            }),
            root: root.clone(),
            base_revision: "a".repeat(40),
            created_at: "now".into(),
            state: "ready".into(),
            revision: 1,
        };
        store.conn.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,payload,root) VALUES(?1,?2,?3,'ready','fixture','{}',?4)",params![w.id,w.project_id,encode(&w).unwrap(),root.to_str().unwrap()]).unwrap();
        let c:Conversation = serde_json::from_value(json!({"id":"conversation","project_id":"project","title":"fixture","workspace_id":w.id,"connection_id":"connection","grant_id":"grant","state":"ready","revision":1,"native_session_id":null,"created_at":"now","updated_at":"now","last_error":null,"turn_state":null,"runtime_snapshot":null,"attempt_id":"attempt","fencing_token":1,"git_common_device":cd,"git_common_inode":ci})).unwrap();
        store.conn.execute("INSERT INTO conversations(id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload) VALUES(?1,?2,?3,?4,?5,'grant',?6,?7,'fixture','{}')",params![c.id,c.project_id,c.workspace_id,c.connection_id,c.grant_id,c.state,encode(&c).unwrap()]).unwrap();
        let pin = IntegrationGitPin {
            canonical_root: dir.path().join("source-worktree"),
            device: device.to_string(),
            inode: "different-root".into(),
            common_dir: common,
            common_device: cd.to_string(),
            common_inode: ci.to_string(),
            head: "a".repeat(40),
        };
        (dir, store, pin, c)
    }
    fn state(store: &Store, c: &mut Conversation, value: &str) {
        c.state = value.into();
        store
            .conn
            .execute(
                "UPDATE conversations SET state=?1,document=?2",
                params![value, encode(c).unwrap()],
            )
            .unwrap();
    }
    #[test]
    fn another_worktree_same_physical_repository_blocks_every_owned_state() {
        let (_dir, store, mut pin, mut c) = fixture();
        // A caller spelling is irrelevant: admission uses physical pins.
        pin.common_dir = "/unrelated/spelling".into();
        for active in [
            "starting",
            "ready",
            "running",
            "cancel_requested",
            "outcome_unknown",
        ] {
            state(&store, &mut c, active);
            assert_eq!(
                check_conversation_writers(&store.conn, &pin)
                    .unwrap_err()
                    .code,
                "WORKSPACE_BUSY"
            );
        }
        for terminal in ["stopped", "failed"] {
            state(&store, &mut c, terminal);
            check_conversation_writers(&store.conn, &pin).unwrap();
        }
    }
    #[test]
    fn separate_repository_allowed_but_replaced_identity_fails_closed() {
        let (dir, store, mut pin, _c) = fixture();
        let other = dir.path().join("other-repository");
        std::fs::create_dir(&other).unwrap();
        let (device, inode) = super::super::fs::directory_identity(&other).unwrap();
        pin.common_dir = other;
        pin.common_device = device.to_string();
        pin.common_inode = inode.to_string();
        check_conversation_writers(&store.conn, &pin).unwrap();
        std::fs::rename(
            dir.path().join("common"),
            dir.path().join("original-common"),
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("common")).unwrap();
        assert_eq!(
            check_conversation_writers(&store.conn, &pin)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
    }
    #[test]
    fn hidden_writer_mirror_and_missing_workspace_fail_closed() {
        let (_dir, store, pin, _c) = fixture();
        store
            .conn
            .execute("UPDATE conversations SET state='stopped'", [])
            .unwrap();
        assert_eq!(
            check_conversation_writers(&store.conn, &pin)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        store
            .conn
            .execute("UPDATE conversations SET state='ready'", [])
            .unwrap();
        store
            .conn
            .execute("DELETE FROM managed_workspaces", [])
            .unwrap();
        assert!(check_conversation_writers(&store.conn, &pin).is_err());
    }
    #[test]
    fn real_conversation_reservation_excludes_another_managed_worktree() {
        let mut f = crate::conversation_tests::fixture();
        let second = f
            .store
            .create_workspace(
                &f.project.id,
                f.project.revision,
                &f.workspace.base_revision,
                f.workspace.root.parent().unwrap(),
                "integration-source",
            )
            .unwrap();
        assert_ne!(second.id, f.workspace.id);
        let identity = second.identity.as_ref().unwrap();
        let WorkspaceKind::Git {
            common_dir, head, ..
        } = &identity.kind
        else {
            panic!("Git fixture");
        };
        let (device, inode) = super::super::fs::directory_identity(common_dir).unwrap();
        let pin = IntegrationGitPin {
            canonical_root: identity.canonical_root.clone(),
            device: identity.device.to_string(),
            inode: identity.inode.to_string(),
            common_dir: common_dir.clone(),
            common_device: device.to_string(),
            common_inode: inode.to_string(),
            head: head.clone(),
        };
        check_conversation_writers(&f.store.conn, &pin).unwrap();
        let c = f.create("real-conversation");
        assert_eq!(c.state, "starting");
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let tx = f
            .store
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        assert_eq!(
            check_conversation_writers(&tx, &pin).unwrap_err().code,
            "WORKSPACE_BUSY"
        );
    }

    #[test]
    fn bounded_history_and_changed_schema_rejected() {
        let (_dir, store, pin, _c) = fixture();
        store
            .conn
            .execute("UPDATE conversations SET document=zeroblob(4194305)", [])
            .unwrap();
        assert_eq!(
            check_conversation_writers(&store.conn, &pin)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        store
            .conn
            .execute("DROP TABLE conversation_turns", [])
            .unwrap();
        assert_eq!(
            check_conversation_writers(&store.conn, &pin)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
    }
}
