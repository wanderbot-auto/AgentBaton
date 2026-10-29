//! Controlled test-only inverse from current schema16 to generated predecessors.
//! This does not make a genuine historical14 fixture and never runs on live data.
use rusqlite::{Connection, Transaction};
use std::collections::BTreeSet;

pub const FACTORY_TABLES: [&str; 12] = [
    "factory_child_goal_runs",
    "factory_child_plan_heads",
    "factory_child_plan_revisions",
    "factory_scoped_handles",
    "factory_handoffs",
    "factory_attempts",
    "factory_budget_reservations",
    "factory_delegations",
    "factory_task_bindings",
    "factory_task_lineage",
    "factory_runs",
    "factory_scopes",
];
const SCOPE15: &str = "CREATE TABLE goal_scopes(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),document TEXT NOT NULL)";
const RUN15: &str = "CREATE TABLE goal_runs(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES goal_scopes(id),project_id TEXT NOT NULL REFERENCES projects(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL,document TEXT NOT NULL)";
const SCOPE_INDEX: &str =
    "CREATE UNIQUE INDEX goal_one_active_scope_per_task ON goal_scopes(task_id) WHERE revoked=0";
const RUN_INDEX: &str = "CREATE UNIQUE INDEX goal_one_running_scope ON goal_runs(scope_id) WHERE state IN('queued','running','cancel_requested','outcome_unknown')";

fn check_old_task_semantics(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
    // Production16 added no table: it added Task semantics. The inverse may
    // accept only rows already representable before that semantic increment.
    // Never clear a binding (including explicit null), serialize a parsed Task
    // back over raw JSON, or treat malformed/unknown raw fields as an old Task.
    let source = include_str!("../../src/storage.rs");
    let expected = source
        .split_once("CREATE TABLE tasks(")
        .and_then(|(_, tail)| {
            tail.split_once(';')
                .map(|(sql, _)| format!("CREATE TABLE tasks({sql}"))
        })
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let actual: String = transaction.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='tasks'",
        [],
        |row| row.get(0),
    )?;
    if actual != expected || transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE tbl_name='tasks' AND type IN('index','trigger') AND sql IS NOT NULL)",
        [], |row| row.get::<_, bool>(0),
    )? { return Err(rusqlite::Error::InvalidQuery); }
    let mut statement = transaction.prepare(
        "SELECT id,length(CAST(document AS BLOB)),CASE WHEN length(CAST(document AS BLOB))<=262144 THEN document END FROM tasks",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let size: i64 = row.get(1)?;
        let raw: Option<String> = row.get(2)?;
        if size <= 0 || size > 262_144 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let raw = raw.ok_or(rusqlite::Error::InvalidQuery)?;
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|_| rusqlite::Error::InvalidQuery)?;
        if !value.is_object() || value.get("code_workspace_binding").is_some() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let task: agentbaton_core::storage::Task =
            serde_json::from_str(&raw).map_err(|_| rusqlite::Error::InvalidQuery)?;
        if task.id != id
            || task.revision == 0
            || task.spec_revision == 0
            || task.code_workspace_binding.is_some()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

fn check_empty_factory(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != 16 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    check_old_task_semantics(transaction)?;
    // Every named domain must exist AND be empty before the first DDL. Neither
    // missing tables nor malformed nonempty history are treated as zero.
    for table in FACTORY_TABLES {
        let count: i64 =
            transaction.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
        if count != 0 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        // These are the exact CREATE statements compiled into this checkout,
        // not inferred arbitrary DDL. Refuse unknown columns/FKs/constraints or
        // extra user indexes/triggers even on an otherwise empty named table.
        let sources = [
            include_str!("../../src/factory_store.rs"),
            include_str!("../../src/factory_child_plan.rs"),
            include_str!("../../src/factory_child_goal.rs"),
        ];
        let prefix = format!("CREATE TABLE {table}(");
        let expected = sources
            .iter()
            .find_map(|source| source.find(&prefix).map(|offset| &source[offset..]))
            .and_then(|tail| tail.split_once(';').map(|(ddl, _)| ddl))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let actual: String = transaction.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        if actual != expected || expected.contains('\\') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        // Require the exact full named-index set, not just that each remaining
        // object is recognized. A missing known index is not an old fixture.
        let mut expected_indexes = BTreeSet::new();
        for source in sources {
            for (offset, _) in source.match_indices("CREATE ") {
                let tail = &source[offset..];
                if !(tail.starts_with("CREATE INDEX ") || tail.starts_with("CREATE UNIQUE INDEX "))
                {
                    continue;
                }
                let Some((sql, _)) = tail.split_once(';') else {
                    continue;
                };
                if sql.contains(&format!(" ON {table}(")) {
                    expected_indexes.insert(sql.to_owned());
                }
            }
        }
        let mut actual_indexes = BTreeSet::new();
        let mut objects = transaction.prepare("SELECT sql FROM sqlite_master WHERE tbl_name=?1 AND type IN('index','trigger') AND sql IS NOT NULL")?;
        for sql in objects.query_map([table], |row| row.get::<_, String>(0))? {
            let sql = sql?;
            actual_indexes.insert(sql);
        }
        if actual_indexes != expected_indexes {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

// Inner only so a caller can atomically synthesize13 as well as12. The outer
// caller must own its isolated connection and restore foreign_keys afterwards.
pub fn remove_empty_child_goal_increment(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
    if transaction.is_autocommit() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    check_empty_factory(transaction)?;
    let foreign_keys: bool =
        transaction.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if foreign_keys
        || transaction
            .prepare("PRAGMA foreign_key_check")?
            .exists([])?
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let marked: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM goal_scopes WHERE json_type(document,'$.factory_origin') IS NOT NULL AND json_type(document,'$.factory_origin')!='null')", [], |row|row.get(0))?;
    if marked {
        return Err(rusqlite::Error::InvalidQuery);
    }
    // sqlite_master quotes the name after the production ALTER TABLE RENAME.
    // Normalize exactly that spelling, not arbitrary unknown schema changes.
    for (name, expected) in [
        ("goal_scopes", SCOPE15),
        ("goal_runs", RUN15),
        ("goal_one_active_scope_per_task", SCOPE_INDEX),
        ("goal_one_running_scope", RUN_INDEX),
    ] {
        let actual: String = transaction.query_row(
            "SELECT sql FROM sqlite_master WHERE name=?1",
            [name],
            |row| row.get(0),
        )?;
        let actual = actual.replace(
            &format!("CREATE TABLE \"{name}\""),
            &format!("CREATE TABLE {name}"),
        );
        if actual != expected {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    let extra: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE tbl_name IN('goal_scopes','goal_runs') AND (type='trigger' OR (type='index' AND sql IS NOT NULL AND name NOT IN('goal_one_active_scope_per_task','goal_one_running_scope'))))", [], |row| row.get(0))?;
    if extra {
        return Err(rusqlite::Error::InvalidQuery);
    }
    // Exact inverse of the two known production15 rebuilds: copy rowid and ALL
    // original columns/raw documents. Native ids again reference real proposals;
    // incoming FK table names stay unchanged. No native capability FK is removed.
    transaction.execute_batch("CREATE TABLE goal_scopes_pre_factory(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),task_id TEXT NOT NULL REFERENCES tasks(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),document TEXT NOT NULL);
INSERT INTO goal_scopes_pre_factory(rowid,id,task_id,root_grant_id,revision,revoked,document) SELECT rowid,* FROM goal_scopes;
CREATE TABLE goal_runs_pre_factory(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),scope_id TEXT NOT NULL REFERENCES goal_scopes(id),project_id TEXT NOT NULL REFERENCES projects(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL,document TEXT NOT NULL);
INSERT INTO goal_runs_pre_factory(rowid,id,scope_id,project_id,task_id,revision,state,document) SELECT rowid,* FROM goal_runs;
DROP TABLE factory_child_goal_runs;
DROP TABLE goal_runs;
DROP TABLE goal_scopes;
ALTER TABLE goal_scopes_pre_factory RENAME TO goal_scopes;
ALTER TABLE goal_runs_pre_factory RENAME TO goal_runs;
CREATE UNIQUE INDEX goal_one_active_scope_per_task ON goal_scopes(task_id) WHERE revoked=0;
CREATE UNIQUE INDEX goal_one_running_scope ON goal_runs(scope_id) WHERE state IN('queued','running','cancel_requested','outcome_unknown');
PRAGMA user_version=14;")?;
    if transaction
        .prepare("PRAGMA foreign_key_check")?
        .exists([])?
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub fn remove_empty_factory_increment(connection: &Connection) -> rusqlite::Result<()> {
    if !connection.is_autocommit() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let foreign_keys: bool =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    connection.pragma_update(None, "foreign_keys", false)?;
    let result = (|| {
        let transaction = connection.unchecked_transaction()?;
        remove_empty_child_goal_increment(&transaction)?;
        transaction.execute_batch(
            "DROP TABLE factory_child_plan_heads;
DROP TABLE factory_child_plan_revisions;
DROP TABLE factory_scoped_handles;
DROP TABLE factory_handoffs;
DROP TABLE factory_attempts;
DROP TABLE factory_budget_reservations;
DROP TABLE factory_delegations;
DROP TABLE factory_task_bindings;
DROP TABLE factory_task_lineage;
DROP TABLE factory_runs;
DROP TABLE factory_scopes;
PRAGMA user_version=12;",
        )?;
        if transaction
            .prepare("PRAGMA foreign_key_check")?
            .exists([])?
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.commit()
    })();
    let restore = connection.pragma_update(None, "foreign_keys", foreign_keys);
    match (result, restore) {
        (Err(error), _) => Err(error),
        (Ok(()), restored) => restored,
    }
}
