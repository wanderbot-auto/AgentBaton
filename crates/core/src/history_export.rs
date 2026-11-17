//! Bounded, project-owned history snapshots. No database backup or restore authority.
use crate::{
    storage::{db_error, Store},
    CoreError, Result,
};
use rustix::fs::{linkat, mkdirat, open, openat, AtFlags, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;
const MAX_ROWS: usize = 10_000;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_PACKAGE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub version: u32,
    pub project_id: String,
    pub from_sequence: u64,
    pub to_sequence: Option<u64>,
    pub tasks: bool,
    pub messages: bool,
    pub events: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub preview_id: String,
    pub selection_digest: String,
    pub package_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    #[serde(flatten)]
    pub reference: Reference,
    pub selection: Selection,
    pub snapshot_sequence: u64,
    pub counts: BTreeMap<String, usize>,
    pub excluded_events: usize,
    pub missing_references: usize,
    pub redacted_fields: usize,
    pub bytes: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub preview: Preview,
    pub state: String,
    pub filename: String,
}
fn err(code: &str, text: &str) -> CoreError {
    CoreError::new(code, text)
}
fn limited() -> CoreError {
    err(
        "RESOURCE_LIMIT",
        "History export exceeds bounded capture; narrow the sequence range or categories",
    )
}
fn invalid() -> CoreError {
    err(
        "INVALID_REQUEST",
        "Invalid history export selection or reference",
    )
}
fn io_error() -> CoreError {
    let mut error = err(
        "EXPORT_IO",
        "Cannot securely read or publish local history package; reconcile the original reference",
    );
    error.side_effect_state = "unknown".into();
    error
}
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| invalid())
}
fn secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "authorization",
        "credential",
        "cookie",
        "api_key",
        "apikey",
        "private_key",
        "auth",
        "config",
        "environment",
        "raw",
        "identity",
    ]
    .iter()
    .any(|s| key.contains(s))
}
// Conservative line suppression avoids retaining a credential's trailing words.
// This is deliberately an export-local detector, never a zero-secret guarantee.
fn scrub(value: &mut Value, count: &mut usize) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if secret_key(key) {
                    *value = json!("[REDACTED]");
                    *count += 1;
                } else {
                    scrub(value, count);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                scrub(value, count);
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if [
                "bearer ",
                "authorization",
                "api_key",
                "api-key",
                "apikey",
                "password",
                "secret",
                "token",
                "credential",
                "private key",
                "sk-",
                "ghp_",
                "github_pat_",
                "xoxb-",
                "xoxp-",
                "-----begin",
            ]
            .iter()
            .any(|s| lower.contains(s))
            {
                *text = "[REDACTED: credential-like text]".into();
                *count += 1;
            }
        }
        _ => {}
    }
}
fn project_fields(value: &Value, fields: &[&str]) -> Value {
    let mut map = serde_json::Map::new();
    for field in fields {
        if let Some(value) = value.get(*field) {
            map.insert((*field).into(), value.clone());
        }
    }
    Value::Object(map)
}
struct Bound {
    started: Instant,
    rows: usize,
    bytes: usize,
}
impl Bound {
    fn row(&mut self, raw: Option<String>) -> Result<Value> {
        self.rows += 1;
        let raw = raw.ok_or_else(limited)?;
        self.bytes += raw.len();
        if self.rows > MAX_ROWS
            || self.bytes > MAX_BYTES
            || self.started.elapsed() > Duration::from_secs(2)
        {
            return Err(limited());
        }
        serde_json::from_str(&raw).map_err(|_| err("INPUT_CHANGED", "History record is invalid"))
    }
}
impl Store {
    pub fn history_export_preview(&self, root: &Path, selection: &Selection) -> Result<Preview> {
        self.capture_history_export(selection)?.persist(root)
    }
    pub(crate) fn capture_history_export(&self, selection: &Selection) -> Result<Capture> {
        if selection.version != 1
            || selection.project_id.is_empty()
            || selection.project_id.len() > 128
            || !(selection.tasks || selection.messages || selection.events)
            || selection.from_sequence > 9_007_199_254_740_991
            || selection
                .to_sequence
                .is_some_and(|n| n > 9_007_199_254_740_991 || n < selection.from_sequence)
        {
            return Err(invalid());
        }
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let project: Option<String> = tx.query_row("SELECT CASE WHEN octet_length(document)<=262144 THEN document END FROM projects WHERE id=?1", [&selection.project_id], |r| r.get(0)).map_err(|_| err("NOT_FOUND", "Project unavailable"))?;
        let mut bound = Bound {
            started: Instant::now(),
            rows: 0,
            bytes: 0,
        };
        let project = bound.row(project)?;
        if project["id"] != selection.project_id {
            return Err(err("INPUT_CHANGED", "Project ownership changed"));
        }
        let cutoff: i64 = tx
            .query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get(0)
            })
            .map_err(db_error)?;
        let cutoff = u64::try_from(cutoff).map_err(|_| invalid())?;
        let end = selection.to_sequence.unwrap_or(cutoff);
        if end > cutoff || selection.from_sequence > end {
            return Err(err(
                "STALE_REVISION",
                "Selected journal range is ahead of available history",
            ));
        }
        // Bound global entity scanning before JSON predicates; no unbounded full journal scan.
        let mut task_statement = tx.prepare("SELECT id, CASE WHEN octet_length(document)<=262144 THEN document END FROM tasks ORDER BY id LIMIT 10001").map_err(db_error)?;
        let mut rows = task_statement.query([]).map_err(db_error)?;
        let mut task_ids = HashSet::new();
        let mut all_ids = HashSet::new();
        let mut namespaces = HashMap::new();
        let mut ambiguous = HashSet::new();
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().map_err(db_error)? {
            let value = bound.row(row.get(1).map_err(db_error)?)?;
            let id: String = row.get(0).map_err(db_error)?;
            if namespaces.insert(id.clone(), "task.").is_some() {
                ambiguous.insert(id.clone());
            }
            if value["project_id"] == selection.project_id {
                if value["id"] != id {
                    return Err(err("INPUT_CHANGED", "Task ownership mirror mismatch"));
                }
                task_ids.insert(id.clone());
                all_ids.insert(id);
                if selection.tasks {
                    tasks.push(project_fields(
                        &value,
                        &[
                            "id",
                            "project_id",
                            "goal",
                            "state",
                            "spec_revision",
                            "revision",
                            "created_at",
                            "updated_at",
                        ],
                    ));
                }
            }
        }
        drop(rows);
        drop(task_statement);
        let mut sessions = Vec::new();
        for (table, fields) in [
            (
                "sessions",
                &[
                    "id",
                    "task_id",
                    "project_id",
                    "state",
                    "revision",
                    "attempt_id",
                    "created_at",
                    "updated_at",
                ][..],
            ),
            (
                "conversations",
                &[
                    "id",
                    "project_id",
                    "title",
                    "state",
                    "revision",
                    "attempt_id",
                    "created_at",
                    "updated_at",
                ][..],
            ),
        ] {
            if !(selection.messages || selection.events) {
                continue;
            }
            let task_column = if table == "sessions" {
                "task_id"
            } else {
                "NULL"
            };
            let sql = format!("SELECT id,project_id,CASE WHEN octet_length(document)<=262144 THEN document END,{task_column} FROM {table} ORDER BY id LIMIT 10001");
            let mut statement = tx.prepare(&sql).map_err(db_error)?;
            let mut rows = statement.query([]).map_err(db_error)?;
            while let Some(row) = rows.next().map_err(db_error)? {
                let value = bound.row(row.get(2).map_err(db_error)?)?;
                let project_id: String = row.get(1).map_err(db_error)?;
                let row_id: String = row.get(0).map_err(db_error)?;
                if namespaces
                    .insert(
                        row_id.clone(),
                        if table == "sessions" {
                            "session."
                        } else {
                            "conversation."
                        },
                    )
                    .is_some()
                {
                    ambiguous.insert(row_id.clone());
                }
                if project_id != selection.project_id {
                    continue;
                }
                if table == "sessions"
                    && value["task_id"].as_str()
                        != row
                            .get::<_, Option<String>>(3)
                            .map_err(db_error)?
                            .as_deref()
                {
                    return Err(err("INPUT_CHANGED", "Session task mirror mismatch"));
                }
                if value["id"] != row_id {
                    return Err(err("INPUT_CHANGED", "Session id mirror mismatch"));
                }
                if value["project_id"] != selection.project_id
                    || (table == "sessions"
                        && !value["task_id"]
                            .as_str()
                            .is_some_and(|id| task_ids.contains(id)))
                {
                    return Err(err("INPUT_CHANGED", "Session ownership mirror mismatch"));
                }
                let id = value["id"].as_str().ok_or_else(invalid)?.to_owned();
                all_ids.insert(id);
                if selection.messages || selection.events {
                    sessions.push(project_fields(&value, fields));
                }
            }
        }
        let mut events = Vec::new();
        let mut messages = Vec::new();
        let mut excluded = 0;
        let mut missing = 0;
        if selection.messages || selection.events {
            let mut statement = tx.prepare("SELECT sequence, CASE WHEN octet_length(document)<=262144 THEN document END FROM events WHERE sequence>?1 AND sequence<=?2 ORDER BY sequence LIMIT 10001").map_err(db_error)?;
            let mut rows = statement
                .query(rusqlite::params![
                    selection.from_sequence as i64,
                    end as i64
                ])
                .map_err(db_error)?;
            while let Some(row) = rows.next().map_err(db_error)? {
                let mut value = bound.row(row.get(1).map_err(db_error)?)?;
                let aggregate = value["aggregate_id"].as_str().ok_or_else(invalid)?;
                if !all_ids.contains(aggregate)
                    || ambiguous.contains(aggregate)
                    || !namespaces.get(aggregate).is_some_and(|prefix| {
                        value["kind"]
                            .as_str()
                            .is_some_and(|kind| kind.starts_with(prefix))
                    })
                    || value["payload"]
                        .get("project_id")
                        .is_some_and(|p| p != &selection.project_id)
                {
                    excluded += 1;
                    continue;
                }
                value["sequence"] = json!(row.get::<_, i64>(0).map_err(db_error)?);
                if value["payload"].get("raw_ref").is_some()
                    || value["payload"].get("artifact_id").is_some()
                {
                    missing += 1;
                }
                let mut envelope = project_fields(
                    &value,
                    &[
                        "sequence",
                        "event_id",
                        "aggregate_id",
                        "revision",
                        "kind",
                        "occurred_at",
                        "source",
                    ],
                );
                let payload = &value["payload"];
                let update = payload.get("update").unwrap_or(&Value::Null);
                // Only normalized local text and tool/turn identifiers; raw Provider
                // protocol, tool input/output objects, config and unknown fields excluded.
                let mut safe_payload = project_fields(
                    payload,
                    &[
                        "turn_id",
                        "text",
                        "state",
                        "reason",
                        "stop_evidence",
                        "tool_call_id",
                    ],
                );
                if update.is_object() {
                    safe_payload["update"] =
                        project_fields(update, &["kind", "text", "tool_call_id", "state"]);
                }
                if let Some(kind) = update.get("kind").filter(|v| v.is_object()) {
                    safe_payload["update"]["kind"] =
                        project_fields(kind, &["kind", "text", "tool_call_id"]);
                }
                envelope["payload"] = safe_payload;
                let is_message = payload.get("text").is_some()
                    || (update.get("text").is_some() || update["kind"].get("text").is_some());
                if is_message && selection.messages {
                    messages.push(envelope);
                } else if selection.events {
                    envelope["payload"].as_object_mut().unwrap().remove("text");
                    if let Some(update) = envelope["payload"]
                        .get_mut("update")
                        .and_then(Value::as_object_mut)
                    {
                        update.remove("text");
                        if let Some(kind) = update.get_mut("kind").and_then(Value::as_object_mut) {
                            kind.remove("text");
                        }
                    }
                    events.push(envelope);
                }
            }
            drop(rows);
            drop(statement);
        }
        tx.commit().map_err(db_error)?;
        let mut redacted = 0;
        let mut files = BTreeMap::new();
        let mut counts = BTreeMap::new();
        for (name, records) in [
            ("tasks.json", tasks),
            ("sessions.json", sessions),
            ("messages.json", messages),
            ("events.json", events),
        ] {
            counts.insert(name.to_string(), records.len());
            let mut value = json!(records);
            scrub(&mut value, &mut redacted);
            let content = String::from_utf8(encoded(&value)?).map_err(|_| invalid())?;
            files.insert(
                name.to_string(),
                json!({"sha256":sha(content.as_bytes()),"content":content}),
            );
        }
        let id = Uuid::new_v4().to_string();
        let digest = sha(&encoded(selection)?);
        let warnings = vec![
            "history_only_not_restorable".into(), "entity_metadata_current_at_snapshot_journal_range_only".into(),
            "normalized_local_capture_only_provider_completeness_unknown".into(), "free_text_may_contain_unrecognized_secrets_review_before_sharing".into(),
            "excluded_raw_protocol_tool_arguments_outputs_configuration_identities_source_files_artifacts_recovery_and_unowned_events".into(),
        ];
        let package = json!({"manifest":{"format":"agentbaton-history","schema_version":1,"export_id":id,"source":"AgentBaton local SQLite normalized history","project":project_fields(&project,&["id","name","revision"]),"selection":selection,"selection_digest":digest,"snapshot_sequence":cutoff,"range_end_sequence":end,"counts":counts,"restorable":false},"files":files,"report":{"redacted_fields":redacted,"excluded_events":excluded,"missing_references":missing,"warnings":warnings}});
        let mut package = package;
        scrub(&mut package["manifest"]["project"]["name"], &mut redacted);
        package["report"]["redacted_fields"] = json!(redacted);
        let bytes = encoded(&package)?;
        if bytes.len() > MAX_PACKAGE {
            return Err(limited());
        }
        let preview = Preview {
            reference: Reference {
                preview_id: id.clone(),
                selection_digest: digest,
                package_sha256: sha(&bytes),
            },
            selection: selection.clone(),
            snapshot_sequence: cutoff,
            counts,
            excluded_events: excluded,
            missing_references: missing,
            redacted_fields: redacted,
            bytes: bytes.len(),
            warnings,
        };
        Ok(Capture { preview, bytes })
    }
}
pub(crate) struct Capture {
    preview: Preview,
    bytes: Vec<u8>,
}
impl Capture {
    fn persist(self, root: &Path) -> Result<Preview> {
        let directory = ExportDirectory::open(root)?;
        if std::fs::read_dir(root)
            .map_err(|_| io_error())?
            .take(65)
            .count()
            >= 64
        {
            return Err(err(
                "EXPORT_CAPACITY",
                "Retained history export storage is full; no files were removed",
            ));
        }
        directory.write_new(
            &format!("{}.snapshot", self.preview.reference.preview_id),
            &self.bytes,
        )?;
        Ok(self.preview)
    }
}
// Only filesystem work crosses this worker boundary. SQLite capture stays in a
// bounded Actor read transaction; fsync never blocks the daemon service loop.
pub(crate) enum Operation {
    Preview(Capture),
    Publish(Reference),
    Get(Reference),
}
#[derive(Default)]
pub(crate) struct Workers {
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Workers {
    pub(crate) fn poll(&mut self) {
        if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
            let _ = self.thread.take().unwrap().join();
        }
    }
    pub(crate) fn busy(&self) -> bool {
        self.thread.is_some()
    }
    pub(crate) fn start(
        &mut self,
        root: PathBuf,
        operation: Operation,
        request_id: String,
        revision: u64,
        job: crate::ipc::ReadyRequest,
    ) {
        let fallback = job.reply.clone();
        let fallback_id = request_id.clone();
        let spawned=std::thread::Builder::new().name("baton-history-export".into()).spawn(move || {
            let result = if Instant::now() >= job.deadline {
                Err(err(
                    "DEADLINE_EXCEEDED",
                    "Export request expired before filesystem work",
                ))
            } else {
                match operation {
                    Operation::Preview(c) => c.persist(&root).map(|p| json!(p)),
                    Operation::Publish(r) => publish(&root, &r).map(|r| json!(r)),
                    Operation::Get(r) => get(&root, &r).map(|r| json!(r)),
                }
            };
            let wire = match result {
                Ok(value) => {
                    json!({"request_id":request_id,"result":value,"revision":revision,"warnings":[]})
                }
                Err(error) => {
                    json!({"request_id":request_id,"error":error,"revision":revision,"warnings":[]})
                }
            };
            let _ = job.reply.try_send(wire);
        });
        match spawned {
            Ok(thread) => self.thread = Some(thread),
            Err(_) => {
                let _=fallback.try_send(json!({"request_id":fallback_id,"error":err("RESOURCE_LIMIT","History export filesystem worker could not start"),"revision":revision,"warnings":[]}));
            }
        }
    }
    pub(crate) fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.join();
    }
}
// All operations below are relative to an opened, private no-follow directory.
// Paths returned to Finder are rechecked against that anchored inode.
struct ExportDirectory {
    file: File,
    root: PathBuf,
}
fn private(file: &File, directory: bool) -> Result<()> {
    let m = file.metadata().map_err(|_| io_error())?;
    if m.uid() != rustix::process::geteuid().as_raw()
        || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        return Err(err(
            "PERMISSION_DENIED",
            "Export state ownership, type or permissions changed",
        ));
    }
    Ok(())
}
impl ExportDirectory {
    fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute()
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(invalid());
        }
        let parent = root.parent().ok_or_else(invalid)?;
        // Walk each component from filesystem root with NOFOLLOW; no symlink parent races.
        let mut fd = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| io_error())?;
        for part in parent.components() {
            if let Component::Normal(part) = part {
                fd = openat(
                    &fd,
                    part,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| io_error())?;
            }
        }
        let parent_file = File::from(fd);
        private(&parent_file, true)?;
        let name = root.file_name().ok_or_else(invalid)?;
        match mkdirat(&parent_file, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => (),
            Err(e) if e == rustix::io::Errno::EXIST => (),
            Err(_) => return Err(io_error()),
        }
        let file = File::from(
            openat(
                &parent_file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| io_error())?,
        );
        private(&file, true)?;
        Ok(Self {
            file,
            root: root.into(),
        })
    }
    fn write_new(&self, name: &str, bytes: &[u8]) -> Result<()> {
        private(&self.file, true)?;
        // The partial name cannot be read or published. Existing partials are
        // retained as failure evidence; no cleanup or overwrite takes place.
        let temporary = format!("{name}.writing");
        let mut file = File::from(
            openat(
                &self.file,
                temporary.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| io_error())?,
        );
        private(&file, false)?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| io_error())?;
        private(&self.file, true)?;
        linkat(
            &self.file,
            temporary.as_str(),
            &self.file,
            name,
            AtFlags::empty(),
        )
        .map_err(|_| io_error())?;
        rustix::fs::unlinkat(&self.file, temporary.as_str(), AtFlags::empty())
            .map_err(|_| io_error())?;
        self.file.sync_all().map_err(|_| io_error())
    }
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        private(&self.file, true)?;
        let fd = match openat(
            &self.file,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
            Err(_) => return Err(io_error()),
        };
        let file = File::from(fd);
        private(&file, false)?;
        if file.metadata().map_err(|_| io_error())?.len() > MAX_PACKAGE as u64 {
            return Err(limited());
        }
        let mut bytes = Vec::new();
        file.take((MAX_PACKAGE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| io_error())?;
        if bytes.len() > MAX_PACKAGE {
            return Err(limited());
        }
        Ok(Some(bytes))
    }
}
fn validate_reference(reference: &Reference) -> Result<()> {
    if Uuid::parse_str(&reference.preview_id)
        .map(|id| id.to_string() != reference.preview_id)
        .unwrap_or(true)
        || [&reference.selection_digest, &reference.package_sha256]
            .iter()
            .any(|v| {
                v.len() != 64
                    || !v
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
    {
        return Err(invalid());
    }
    Ok(())
}
fn preview_from(bytes: &[u8], reference: &Reference) -> Result<Preview> {
    if sha(bytes) != reference.package_sha256 {
        return Err(err("INPUT_CHANGED", "Export package checksum changed"));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let m = &value["manifest"];
    let report = &value["report"];
    if m["export_id"] != reference.preview_id || m["selection_digest"] != reference.selection_digest
    {
        return Err(err("INPUT_CHANGED", "Export selection changed"));
    }
    let selection: Selection =
        serde_json::from_value(m["selection"].clone()).map_err(|_| invalid())?;
    if sha(&encoded(&selection)?) != reference.selection_digest {
        return Err(invalid());
    }
    Ok(Preview {
        reference: reference.clone(),
        selection,
        snapshot_sequence: m["snapshot_sequence"].as_u64().ok_or_else(invalid)?,
        counts: serde_json::from_value(m["counts"].clone()).map_err(|_| invalid())?,
        excluded_events: report["excluded_events"].as_u64().ok_or_else(invalid)? as usize,
        missing_references: report["missing_references"].as_u64().ok_or_else(invalid)? as usize,
        redacted_fields: report["redacted_fields"].as_u64().ok_or_else(invalid)? as usize,
        bytes: bytes.len(),
        warnings: serde_json::from_value(report["warnings"].clone()).map_err(|_| invalid())?,
    })
}
pub fn get(root: &Path, reference: &Reference) -> Result<Receipt> {
    validate_reference(reference)?;
    let directory = ExportDirectory::open(root)?;
    let filename = format!("{}.history.json", reference.preview_id);
    let published = directory.read(&filename)?;
    let ready = published.is_some();
    let bytes = match published {
        Some(bytes) => bytes,
        None => directory
            .read(&format!("{}.snapshot", reference.preview_id))?
            .ok_or_else(|| err("NOT_FOUND", "Export preview is unavailable; preview again"))?,
    };
    let preview = preview_from(&bytes, reference)?;
    Ok(Receipt {
        preview,
        state: if ready { "ready" } else { "preview" }.into(),
        filename,
    })
}
#[cfg(test)]
thread_local! { static FAIL_AFTER_LINK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub fn publish(root: &Path, reference: &Reference) -> Result<Receipt> {
    let current = get(root, reference)?;
    if current.state == "ready" {
        return Ok(current);
    }
    let directory = ExportDirectory::open(root)?;
    private(&directory.file, true)?;
    let source = format!("{}.snapshot", reference.preview_id);
    match linkat(
        &directory.file,
        source.as_str(),
        &directory.file,
        current.filename.as_str(),
        AtFlags::empty(),
    ) {
        Ok(()) => (),
        Err(e) if e == rustix::io::Errno::EXIST => (),
        Err(_) => return Err(io_error()),
    }
    #[cfg(test)]
    if FAIL_AFTER_LINK.with(|flag| flag.replace(false)) {
        return Err(io_error());
    }
    directory.file.sync_all().map_err(|_| io_error())?;
    let receipt = get(root, reference)?;
    if receipt.state != "ready" {
        return Err(io_error());
    }
    Ok(receipt)
}
/// Restricted native handoff: only the verified, server-named published package.
pub fn published_path(root: &Path, reference: &Reference) -> Result<PathBuf> {
    let receipt = get(root, reference)?;
    if receipt.state != "ready" {
        return Err(err("NOT_FOUND", "Export has not been published"));
    }
    let directory = ExportDirectory::open(root)?;
    let observed = std::fs::symlink_metadata(&directory.root).map_err(|_| io_error())?;
    let pinned = directory.file.metadata().map_err(|_| io_error())?;
    if observed.dev() != pinned.dev() || observed.ino() != pinned.ino() || !observed.is_dir() {
        return Err(io_error());
    }
    Ok(root.join(receipt.filename))
}
#[cfg(test)]
#[path = "history_export_tests.rs"]
mod tests;
