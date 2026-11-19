use super::{
    bounded_id, changed, plan_inventory_cleanup, HistoryCleanupPlan, HistoryRecordKind,
    HistoryRetentionPolicy, HistorySource, HistoryUse, InventoryRecord, InventoryUse,
};
use crate::{
    conversations::{self, Conversation},
    goal_store::planning,
    sessions::ManagedSession,
    storage::{db_error, decode, Store, Task, TaskState},
    CoreError, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{btree_map::Entry, BTreeMap};

const MAX_TURNS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistorySnapshotGap {
    RecordPins,
    BodyCaptureTimestamp,
    DerivedSourceInventory,
    MixedJournalAndUnclassifiedArtifacts,
    ProjectRetentionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectHistoryRetentionPreview {
    pub plan: HistoryCleanupPlan,
    pub gaps: Vec<HistorySnapshotGap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TurnKind {
    Conversation,
    Session,
}

struct Turn {
    id: String,
    owner: (TurnKind, String),
    state: String,
    kind: HistoryRecordKind,
}

enum Owner {
    Conversation(Conversation),
    Session {
        session: ManagedSession,
        task: Box<Task>,
    },
}

impl Owner {
    fn source(&self) -> HistorySource<'_> {
        match self {
            Self::Conversation(conversation) => HistorySource::Conversation(conversation),
            Self::Session { session, task } => HistorySource::Session { session, task },
        }
    }
}

struct OwnerObservation {
    owner: Owner,
    usage: HistoryUse,
    planning_turn_id: Option<String>,
}

impl Store {
    /// Preview project-owned turn details using one deferred SQLite read
    /// transaction. Policy is an explicit evaluation input. The returned gaps
    /// describe which evidence this Store cannot supply for eventual cleanup.
    pub fn preview_project_history_retention(
        &self,
        project_id: &str,
        policy: HistoryRetentionPolicy,
        evaluated_at_ms: i64,
    ) -> Result<ProjectHistoryRetentionPreview> {
        if !bounded_id(project_id) {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Invalid history project ID",
            ));
        }
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        if self.get_project(project_id)?.id != project_id {
            return Err(changed("Stored history project identity disagrees"));
        }

        let turns = read_turns(&tx, project_id)?;
        let mut owners = BTreeMap::new();
        for turn in &turns {
            if let Entry::Vacant(entry) = owners.entry(&turn.owner) {
                entry.insert(observe_owner(self, &tx, project_id, &turn.owner)?);
            }
            if owners[&turn.owner]
                .planning_turn_id
                .as_ref()
                .is_some_and(|id| id != &turn.id)
            {
                return Err(changed("Goal planning turn and source link disagree"));
            }
        }
        let records = turns.iter().map(|turn| {
            let observation = &owners[&turn.owner];
            let settled = match turn.owner.0 {
                TurnKind::Conversation => "completed",
                TurnKind::Session => "ended",
            };
            InventoryRecord {
                id: &turn.id,
                kind: turn.kind,
                source: observation.owner.source(),
                captured_at_ms: None,
                pinned: None,
                active_use: InventoryUse::Observed(combined(
                    observation.usage,
                    activity(Some(&turn.state), &[settled], &["pending"]),
                )),
            }
        });
        let plan = plan_inventory_cleanup(project_id, policy, evaluated_at_ms, records, None)?;
        tx.commit().map_err(db_error)?;
        Ok(ProjectHistoryRetentionPreview {
            plan,
            gaps: vec![
                HistorySnapshotGap::RecordPins,
                HistorySnapshotGap::BodyCaptureTimestamp,
                HistorySnapshotGap::DerivedSourceInventory,
                HistorySnapshotGap::MixedJournalAndUnclassifiedArtifacts,
                HistorySnapshotGap::ProjectRetentionPolicy,
            ],
        })
    }
}

fn read_turns(db: &Connection, project_id: &str) -> Result<Vec<Turn>> {
    // The turn ID, explicit parent and exact Core body shape define these
    // records. Journal events and replay payloads are outside this inventory.
    let mut statement = db
        .prepare(
            "WITH owned_turns(kind,id,owner_id,state,document) AS (
            SELECT 0,t.id,c.id,t.state,t.document
            FROM conversation_turns t JOIN conversations c ON c.id=t.conversation_id
            WHERE c.project_id=?1
            UNION ALL
            SELECT 1,t.id,s.id,t.state,t.document
            FROM session_turns t JOIN sessions s ON s.id=t.session_id
            WHERE s.project_id=?1
        )
        SELECT kind,id,owner_id,state,
               CASE WHEN octet_length(document)<=524288 AND json_valid(document)
                    THEN COALESCE(json_type(document)='object'
                         AND json_extract(document,'$.turn_id')=id
                         AND json_type(document,'$.text')='text'
                         AND (SELECT count(*) FROM json_each(document))=2,0)
                    ELSE 0 END
        FROM owned_turns LIMIT ?2",
        )
        .map_err(db_error)?;
    let mut rows = statement
        .query(params![project_id, (MAX_TURNS + 1) as i64])
        .map_err(db_error)?;
    let mut turns = Vec::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        if turns.len() == MAX_TURNS {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Project history preview exceeds the turn limit",
            ));
        }
        let id: String = row.get(1).map_err(db_error)?;
        let owner_id: String = row.get(2).map_err(db_error)?;
        if !bounded_id(&id) || !bounded_id(&owner_id) {
            return Err(changed("Stored history turn or owner identity is invalid"));
        }
        let owner_kind = match row.get::<_, i64>(0).map_err(db_error)? {
            0 => TurnKind::Conversation,
            _ => TurnKind::Session,
        };
        turns.push(Turn {
            id,
            owner: (owner_kind, owner_id),
            state: row.get(3).map_err(db_error)?,
            kind: if row.get(4).map_err(db_error)? {
                HistoryRecordKind::Body
            } else {
                HistoryRecordKind::Unclassified
            },
        });
    }
    Ok(turns)
}

fn observe_owner(
    store: &Store,
    db: &Connection,
    project_id: &str,
    (kind, id): &(TurnKind, String),
) -> Result<OwnerObservation> {
    match kind {
        TurnKind::Conversation => {
            let conversation = conversations::read(db, id)?;
            if conversation.project_id != project_id {
                return Err(changed("Conversation belongs to a different project"));
            }
            let dispatch = db
                .query_row(
                    "SELECT state FROM conversation_dispatches WHERE conversation_id=?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(db_error)?;
            let mut usage = combined(
                dispatch_activity(dispatch.as_deref()),
                owner_activity(&conversation.state, conversation.turn_state.as_deref()),
            );
            let purpose = planning::purpose(db, id)?;
            if let Some(purpose) = &purpose {
                let task = read_task(store, &purpose.prepared.source.task_id, project_id)?;
                usage = combined(usage, task_activity(&task));
            }
            Ok(OwnerObservation {
                owner: Owner::Conversation(conversation),
                usage,
                planning_turn_id: purpose.map(|purpose| purpose.turn_id),
            })
        }
        TurnKind::Session => {
            let (document, task_id, state): (Option<String>, String, String) = db
                .query_row(
                    "SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,
                            task_id,state FROM sessions WHERE id=?1 AND project_id=?2",
                    params![id, project_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(db_error)?;
            let session: ManagedSession = decode(
                &document
                    .ok_or_else(|| changed("Stored history session exceeds the read bound"))?,
            )?;
            if session.id != *id
                || session.project_id != project_id
                || session.task_id != task_id
                || session.state != state
                || !bounded_id(&session.attempt_id)
                || !bounded_id(&session.intent_id)
            {
                return Err(changed(
                    "Stored history session ownership or state disagrees",
                ));
            }
            let task = read_task(store, &task_id, project_id)?;
            let dispatch = db
                .query_row(
                    "SELECT d.state FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id
                     WHERE d.id=?1 AND a.id=?2 AND a.task_id=?3",
                    params![session.intent_id, session.attempt_id, task.id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(db_error)?;
            let usage = [
                dispatch_activity(dispatch.as_deref()),
                owner_activity(&session.state, session.turn_state.as_deref()),
                task_activity(&task),
                linked_work(db, &session)?,
            ]
            .into_iter()
            .fold(HistoryUse::Inactive, combined);
            Ok(OwnerObservation {
                usage,
                owner: Owner::Session {
                    session,
                    task: Box::new(task),
                },
                planning_turn_id: None,
            })
        }
    }
}

fn read_task(store: &Store, id: &str, project_id: &str) -> Result<Task> {
    if !bounded_id(id) {
        return Err(changed("Stored history task identity is invalid"));
    }
    // The caller holds the read transaction on this exact Store connection.
    let task = store.get(id)?;
    if task.id != id || task.project_id != project_id {
        return Err(changed("Stored history task belongs to a different owner"));
    }
    Ok(task)
}

fn linked_work(db: &Connection, session: &ManagedSession) -> Result<HistoryUse> {
    let mut statement = db
        .prepare(
            "SELECT 0,r.project_id,r.state,n.session_id,n.attempt_id,NULL
         FROM goal_run_nodes n LEFT JOIN goal_runs r ON r.id=n.run_id
         WHERE n.session_id=?1 OR n.attempt_id=?2
         UNION ALL
         SELECT 1,r.project_id,r.state,a.session_id,a.attempt_id,a.task_id
         FROM factory_attempts a LEFT JOIN factory_runs r ON r.id=a.run_id
         WHERE a.session_id=?1 OR a.attempt_id=?2
         LIMIT ?3",
        )
        .map_err(db_error)?;
    let mut rows = statement
        .query(params![
            session.id,
            session.attempt_id,
            (MAX_TURNS + 1) as i64
        ])
        .map_err(db_error)?;
    let mut usage = HistoryUse::Inactive;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(db_error)? {
        count += 1;
        if count > MAX_TURNS {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Too many linked history activities",
            ));
        }
        let project_id: Option<String> = row.get(1).map_err(db_error)?;
        let session_id: Option<String> = row.get(3).map_err(db_error)?;
        let attempt_id: Option<String> = row.get(4).map_err(db_error)?;
        let task_id: Option<String> = row.get(5).map_err(db_error)?;
        if project_id
            .as_deref()
            .is_some_and(|id| id != session.project_id)
            || session_id.as_deref().is_some_and(|id| id != session.id)
            || attempt_id
                .as_deref()
                .is_some_and(|id| id != session.attempt_id)
            || task_id.as_deref().is_some_and(|id| id != session.task_id)
        {
            return Err(changed(
                "Linked history activity and session ownership disagree",
            ));
        }
        let state: Option<String> = row.get(2).map_err(db_error)?;
        let linked = match row.get::<_, i64>(0).map_err(db_error)? {
            0 => activity(
                state.as_deref(),
                &["accepted", "cancelled"],
                &["queued", "running", "cancel_requested"],
            ),
            _ => activity(
                state.as_deref(),
                &["completed", "cancelled"],
                &[
                    "queued",
                    "starting",
                    "running",
                    "waiting_child",
                    "resuming",
                    "cancel_requested",
                ],
            ),
        };
        usage = combined(usage, linked);
    }
    Ok(usage)
}

fn dispatch_activity(state: Option<&str>) -> HistoryUse {
    activity(state, &["stopped"], &["prepared", "sent"])
}

fn owner_activity(state: &str, turn_state: Option<&str>) -> HistoryUse {
    let process = activity(
        Some(state),
        &["stopped", "failed"],
        &["starting", "ready", "running", "cancel_requested"],
    );
    let turn = turn_state.map_or(HistoryUse::Inactive, |state| {
        activity(
            Some(state),
            &["completed", "failed", "cancelled"],
            &["running"],
        )
    });
    combined(process, turn)
}

fn task_activity(task: &Task) -> HistoryUse {
    match task.state {
        TaskState::Accepted | TaskState::Cancelled => HistoryUse::Inactive,
        TaskState::OutcomeUnknown => HistoryUse::Unknown,
        _ => HistoryUse::Active,
    }
}

fn activity(state: Option<&str>, settled: &[&str], active: &[&str]) -> HistoryUse {
    match state {
        Some(state) if settled.contains(&state) => HistoryUse::Inactive,
        Some(state) if active.contains(&state) => HistoryUse::Active,
        _ => HistoryUse::Unknown,
    }
}

fn combined(a: HistoryUse, b: HistoryUse) -> HistoryUse {
    use HistoryUse::{Active, Inactive, Unknown};
    match (a, b) {
        (Active, _) | (_, Active) => Active,
        (Unknown, _) | (_, Unknown) => Unknown,
        (Inactive, Inactive) => Inactive,
    }
}
