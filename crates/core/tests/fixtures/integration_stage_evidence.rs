//! Failure-only observations of existing records after the original Actor join.
//! No Store/recovery/RPC, authority, process-stop proof or failure-instant claim.
use super::super::ActorJoinOutcome;
use agentbaton_core::{
    execution::ExecutionBinding,
    integration::{IntegrationAttemptView, IntegrationRun},
    CoreError,
};
use rusqlite::{params_from_iter, Connection, OpenFlags};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{json, Map, Value};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, Metadata, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_ROWS: usize = 32;
const MAX_EVENTS: usize = 64;
const MAX_DOCUMENT: usize = 65_536;
const MAX_INPUT: usize = 262_144;
const MAX_OUTPUT: usize = 16_384;
const MAX_DATABASE: u64 = 64 * 1024 * 1024;
const PREFIX: &[u8] = b"INTEGRATION_STAGE_FAILURE ";
const UNKNOWN: &str = "integration.stage_unknown";
const PREPARATION: &str = "integration.preparation_io_observed";

type Checked<T> = Result<T, ()>;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    id: String,
    task: String,
    policy: String,
}
thread_local! {
    // Test thread only. Keys and binding strings never enter public output.
    static OBSERVED: RefCell<BTreeMap<PathBuf, Option<Binding>>> = const { RefCell::new(BTreeMap::new()) };
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
fn binding(value: &Value) -> Option<Binding> {
    let b = Binding {
        id: value.get("id")?.as_str()?.into(),
        task: value.get("task_id")?.as_str()?.into(),
        policy: value.get("policy_id")?.as_str()?.into(),
    };
    (identifier(&b.id) && identifier(&b.task) && identifier(&b.policy)).then_some(b)
}
pub(super) fn observe_reply(runtime: &Path, method: &str, reply: &Value) {
    if reply.get("error").is_some() {
        return;
    }
    let observed = match method {
        "integrations.start" | "integrations.get" => reply
            .get("result")
            .and_then(|v| v.get("run"))
            .and_then(binding),
        "integrations.list" => reply
            .get("result")
            .and_then(Value::as_array)
            .filter(|v| v.len() == 1)
            .and_then(|v| binding(&v[0])),
        _ => return,
    };
    let Some(next) = observed else {
        return;
    };
    OBSERVED.with(|items| {
        let mut items = items.borrow_mut();
        if !items.contains_key(runtime) && items.len() >= MAX_ROWS {
            return;
        }
        items
            .entry(runtime.into())
            .and_modify(|old| {
                if old.as_ref() != Some(&next) {
                    *old = None;
                }
            })
            .or_insert(Some(next));
    });
}
fn envelope(status: &str, actor: &str, rows: Value) -> Value {
    json!({"contract":"agentbaton.integration-stage-failure.v1","status":status,
        "actor_join":actor,"snapshot_time":"post_owned_shutdown_not_failure_instant",
        "measurement_not_authority":true,"process_stop_proven":false,"rows":rows})
}
pub(super) fn finish(
    runtime: &Path,
    actor: Option<ActorJoinOutcome>,
    task: Option<&str>,
    failed: bool,
) {
    let observed = OBSERVED.with(|items| items.borrow_mut().remove(runtime).flatten());
    if !failed {
        return;
    }
    let value = diagnostic(actor, observed, task, |b| collect(runtime, b));
    // Never mask the original failure with a diagnostic write/serialization error.
    if let Ok(bytes) = serde_json::to_vec(&value) {
        if bytes.len() + PREFIX.len() + 1 <= MAX_OUTPUT {
            let mut line = Vec::with_capacity(bytes.len() + PREFIX.len() + 1);
            line.extend_from_slice(PREFIX);
            line.extend_from_slice(&bytes);
            line.push(b'\n');
            let mut stream = std::io::stdout().lock();
            let _ = stream.write_all(&line);
        }
    }
}
fn diagnostic(
    actor: Option<ActorJoinOutcome>,
    observed: Option<Binding>,
    task: Option<&str>,
    read: impl FnOnce(&Binding) -> Checked<Value>,
) -> Value {
    let actor_label = match actor {
        Some(ActorJoinOutcome::Ok) => "ok",
        Some(ActorJoinOutcome::Error) => "error",
        Some(ActorJoinOutcome::Panic) => "panic",
        None => "unobserved",
    };
    if actor != Some(ActorJoinOutcome::Ok) {
        envelope("unobserved_actor_join", actor_label, json!([]))
    } else if let Some(b) = observed.filter(|b| Some(b.task.as_str()) == task) {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read(&b))) {
            Ok(Ok(value)) => value,
            _ => envelope("unobserved_records", actor_label, json!([])),
        }
    } else {
        envelope("unobserved_run_binding", actor_label, json!([]))
    }
}

// Reject duplicate object keys at every depth, before typed deserialization.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|v| Unique(Value::Number(v)))
                    .ok_or_else(|| E::custom("number refused"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_none<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(v)) = seq.next_element()? {
                    if values.len() >= 1024 {
                        return Err(de::Error::custom("array bound"));
                    }
                    values.push(v);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                let mut values = Map::new();
                while let Some(k) = map.next_key::<String>()? {
                    if values.len() >= 128 || values.contains_key(&k) {
                        return Err(de::Error::custom("object refused"));
                    }
                    let Unique(v) = map.next_value()?;
                    values.insert(k, v);
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(V)
    }
}
fn shape(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| shape(v, other)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| shape(a, b))
        }
        _ => true,
    }
}
fn decode<T: for<'a> Deserialize<'a> + serde::Serialize>(value: Value) -> Checked<T> {
    let typed: T = serde_json::from_value(value.clone()).map_err(|_| ())?;
    require(shape(
        &value,
        &serde_json::to_value(&typed).map_err(|_| ())?,
    ))?;
    Ok(typed)
}
fn parse<T: for<'a> Deserialize<'a> + serde::Serialize>(text: &str) -> Checked<T> {
    if text.is_empty() || text.len() > MAX_DOCUMENT {
        return Err(());
    }
    let Unique(value) = serde_json::from_str::<Unique>(text).map_err(|_| ())?;
    decode(value)
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    view: IntegrationAttemptView,
    integration_id: String,
    policy_id: String,
    policy_snapshot_digest: String,
    root_task_id: String,
    root_grant_id: String,
    project_id: String,
    task_id: String,
    factory_scope_id: Option<String>,
    ancestors: Vec<String>,
    intent_id: String,
    binding: ExecutionBinding,
    prepared_at_ms: i64,
    queued_ms: u64,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Event {
    sequence: u64,
    event_id: String,
    aggregate_id: String,
    revision: u64,
    kind: String,
    occurred_at: String,
    payload: Value,
    source: String,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct JoinedError {
    code: String,
    retryable: bool,
    side_effect_state: String,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct UnknownEvent {
    attempt_id: String,
    observed_runtime_lower_bound_ms: u64,
    historical_cas_confirmed: bool,
    worker_thread_joined: bool,
    reservation_refunded: bool,
    effects_admitted: bool,
    joined_error: Option<JoinedError>,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PreparationIo {
    elapsed_ms: u64,
    git_calls_attempted: u64,
    git_calls_complete: u64,
    git_stdout_observed_bytes: Option<u64>,
    git_stderr_observed_bytes: Option<u64>,
    effects_may_have_started: bool,
    interrupted: bool,
    io_complete: bool,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PreparationEvent {
    attempt_id: String,
    io: PreparationIo,
    measurement_not_authority: bool,
    whole_filesystem_output: Option<Value>,
}
#[derive(Clone)]
struct AttemptRow {
    id: String,
    run: String,
    round: i64,
    kind: String,
    state: String,
    authority: String,
    root_task: String,
    root_grant: String,
    project: String,
    document: String,
}
fn known_code(code: &str) -> &'static str {
    match code {
        "DEADLINE_EXCEEDED" => "DEADLINE_EXCEEDED",
        "INPUT_CHANGED" => "INPUT_CHANGED",
        "INVALID_REQUEST" => "INVALID_REQUEST",
        "INVALID_CONTRACT" => "INVALID_CONTRACT",
        "RESOURCE_LIMIT" => "RESOURCE_LIMIT",
        "PERMISSION_DENIED" => "PERMISSION_DENIED",
        "AUTH_REQUIRED" => "AUTH_REQUIRED",
        "STALE_REVISION" => "STALE_REVISION",
        "WORKSPACE_BUSY" => "WORKSPACE_BUSY",
        "CONFIG_CONFLICT" => "CONFIG_CONFLICT",
        "CAPABILITY_UNSUPPORTED" => "CAPABILITY_UNSUPPORTED",
        "ENFORCEMENT_UNAVAILABLE" => "ENFORCEMENT_UNAVAILABLE",
        "CANCEL_REQUESTED" => "CANCEL_REQUESTED",
        "OUTCOME_UNKNOWN" => "OUTCOME_UNKNOWN",
        "INTERNAL_ERROR" => "INTERNAL_ERROR",
        "BUDGET_UNKNOWN" => "BUDGET_UNKNOWN",
        _ => "unrecognized",
    }
}
fn error_projection(error: Option<&CoreError>) -> Value {
    error.map(|e| json!({"code":known_code(&e.code),"retryable":e.retryable,"side_effect_state":match e.side_effect_state.as_str() { "not_started"=>"not_started", "unknown"=>"unknown", _=>"unrecognized" }})).unwrap_or(Value::Null)
}
fn require(condition: bool) -> Checked<()> {
    if condition {
        Ok(())
    } else {
        Err(())
    }
}
fn project(
    b: &Binding,
    run_text: &str,
    records: &[AttemptRow],
    events: &[(i64, String)],
) -> Checked<Value> {
    require(!records.is_empty() && records.len() <= MAX_ROWS && events.len() <= MAX_EVENTS)?;
    let total = records
        .iter()
        .map(|r| r.document.len())
        .chain(events.iter().map(|e| e.1.len()))
        .try_fold(run_text.len(), |a, n| a.checked_add(n))
        .ok_or(())?;
    require(total <= MAX_INPUT)?;
    let run: IntegrationRun = parse(run_text)?;
    require(
        run.id == b.id
            && run.task_id == b.task
            && run.policy_id == b.policy
            && (1..=8).contains(&run.round),
    )?;
    let mut attempts = BTreeMap::new();
    for row in records {
        let a: Attempt = parse(&row.document)?;
        let v = &a.view;
        let kind = serde_json::to_value(v.kind).map_err(|_| ())?;
        require(
            identifier(&row.id)
                && row.id == v.attempt_id
                && a.integration_id == b.id
                && row.run == b.id
                && a.policy_id == b.policy
                && a.task_id == b.task
                && row.authority == "integration"
                && row.round == i64::from(v.round)
                && v.round >= 1
                && v.round <= run.round
                && kind.as_str() == Some(row.kind.as_str())
                && (v.validator.is_some() == (row.kind == "validator"))
                && row.state == v.state
                && matches!(
                    v.state.as_str(),
                    "prepared" | "running" | "cancel_requested" | "finished" | "outcome_unknown"
                )
                && a.root_task_id == row.root_task
                && a.root_grant_id == row.root_grant
                && a.project_id == row.project,
        )?;
        // Read the complete production shape; none of these private bindings are published.
        require(
            identifier(&a.policy_snapshot_digest)
                && identifier(&a.intent_id)
                && a.prepared_at_ms >= 0
                && a.ancestors.len() <= 32
                && a.factory_scope_id.as_deref().is_none_or(identifier)
                && identifier(&a.binding.connection_id),
        )?;
        let _ = a.queued_ms;
        require(attempts.insert(row.id.clone(), a).is_none())?;
    }
    let mut unknown = BTreeMap::new();
    let mut preparation = BTreeMap::new();
    let mut sequences = BTreeSet::new();
    for (sequence, text) in events {
        let e: Event = parse(text)?;
        require(
            *sequence > 0
                && sequences.insert(*sequence)
                && e.sequence == 0
                && identifier(&e.event_id)
                && !e.occurred_at.is_empty()
                && e.aggregate_id == b.id
                && e.source == "integration_core"
                && e.revision >= 1
                && e.revision <= run.revision,
        )?;
        if e.kind == UNKNOWN {
            let mut payload: UnknownEvent = decode(e.payload)?;
            let a = attempts.get(&payload.attempt_id).ok_or(())?;
            require(a.view.state == "outcome_unknown" && !payload.reservation_refunded)?;
            if let Some(error) = payload.joined_error.as_mut() {
                error.code = known_code(&error.code).into();
                error.side_effect_state = match error.side_effect_state.as_str() {
                    "not_started" => "not_started",
                    "unknown" => "unknown",
                    _ => "unrecognized",
                }
                .into();
            }
            require(
                unknown
                    .insert(payload.attempt_id.clone(), payload)
                    .is_none(),
            )?;
        } else if e.kind == PREPARATION {
            let p: PreparationEvent = decode(e.payload)?;
            let a = attempts.get(&p.attempt_id).ok_or(())?;
            require(
                a.view.kind == agentbaton_core::integration::IntegrationStageKind::Preparation
                    && p.measurement_not_authority
                    && p.whole_filesystem_output.is_none()
                    && p.io.git_calls_complete <= p.io.git_calls_attempted
                    && (!p.io.io_complete || p.io.git_calls_complete == p.io.git_calls_attempted)
                    && (p.io.io_complete
                        || (p.io.git_stdout_observed_bytes.is_none()
                            && p.io.git_stderr_observed_bytes.is_none()))
                    && (p.io.git_stdout_observed_bytes.is_some()
                        == (p.io.io_complete && p.io.git_calls_attempted > 0))
                    && (p.io.git_stderr_observed_bytes.is_some()
                        == (p.io.io_complete && p.io.git_calls_attempted > 0)),
            )?;
            require(preparation.insert(p.attempt_id, p.io).is_none())?;
        } else {
            return Err(());
        }
    }
    let mut rows = Vec::new();
    for (ordinal, (id, a)) in attempts.iter().enumerate() {
        let mut u = unknown
            .remove(id)
            .map(|u| serde_json::to_value(u).map_err(|_| ()))
            .transpose()?
            .unwrap_or(Value::Null);
        if let Some(object) = u.as_object_mut() {
            object.remove("attempt_id");
        }
        rows.push(json!({"ordinal":ordinal+1,"round":a.view.round,"kind":a.view.kind,"state":a.view.state,
            "declared_runtime_ms":a.view.reservation.runtime_ms,"declared_output_bytes":a.view.reservation.output_bytes,
            "actual_runtime_ms":a.view.actual_runtime_ms,"actual_output_bytes":a.view.actual_output_bytes,
            "settled":a.view.settled,"io_completion":a.view.io_completion,"last_error":error_projection(a.view.last_error.as_ref()),
            "stage_unknown":u,"preparation_io":preparation.remove(id)}));
    }
    let mut result = envelope("collected", "ok", json!(rows));
    result["schema"] = json!(17);
    result["run_state"] = serde_json::to_value(run.state).map_err(|_| ())?;
    result["run_round"] = json!(run.round);
    result["active_stage"] = serde_json::to_value(run.active_stage).map_err(|_| ())?;
    result["run_last_error"] = error_projection(run.last_error.as_ref());
    require(serde_json::to_vec(&result).map_err(|_| ())?.len() + PREFIX.len() + 1 <= MAX_OUTPUT)?;
    Ok(result)
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    (
        a.dev(),
        a.ino(),
        a.uid(),
        a.gid(),
        a.mode(),
        a.nlink(),
        a.len(),
        a.mtime(),
        a.mtime_nsec(),
        a.ctime(),
        a.ctime_nsec(),
    ) == (
        b.dev(),
        b.ino(),
        b.uid(),
        b.gid(),
        b.mode(),
        b.nlink(),
        b.len(),
        b.mtime(),
        b.mtime_nsec(),
        b.ctime(),
        b.ctime_nsec(),
    )
}
struct Pin {
    path: PathBuf,
    file: File,
    metadata: Metadata,
}
impl Pin {
    fn open(path: &Path, directory: bool) -> Checked<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(
                libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC
                    | if directory { libc::O_DIRECTORY } else { 0 },
            )
            .open(path)
            .map_err(|_| ())?;
        let m = file.metadata().map_err(|_| ())?;
        require(
            m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o7777 == if directory { 0o700 } else { 0o600 }
                && if directory {
                    m.is_dir()
                } else {
                    m.is_file() && m.nlink() == 1 && m.len() >= 100 && m.len() <= MAX_DATABASE
                },
        )?;
        let pin = Self {
            path: path.into(),
            file,
            metadata: m,
        };
        pin.check()?;
        Ok(pin)
    }
    fn check(&self) -> Checked<()> {
        require(
            same(&self.metadata, &self.file.metadata().map_err(|_| ())?)
                && same(
                    &self.metadata,
                    &fs::symlink_metadata(&self.path).map_err(|_| ())?,
                ),
        )
    }
}
fn no_journal(runtime: &Path) -> Checked<()> {
    // immutable read ignores journals: refuse any journal instead of omitting it.
    for name in [
        "baton.sqlite3-wal",
        "baton.sqlite3-shm",
        "baton.sqlite3-journal",
    ] {
        match fs::symlink_metadata(runtime.join(name)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => return Err(()),
        }
    }
    Ok(())
}
fn uri(path: &Path) -> Checked<String> {
    use std::os::unix::ffi::OsStrExt;
    require(path.is_absolute())?;
    let mut text = String::from("file:");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            text.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            write!(&mut text, "%{byte:02X}").map_err(|_| ())?;
        }
    }
    text.push_str("?mode=ro&immutable=1");
    Ok(text)
}
fn collect(runtime: &Path, b: &Binding) -> Checked<Value> {
    require(
        identifier(&b.id)
            && identifier(&b.task)
            && identifier(&b.policy)
            && fs::canonicalize(runtime).map_err(|_| ())? == runtime,
    )?;
    let directory = Pin::open(runtime, true)?;
    let database = Pin::open(&runtime.join("baton.sqlite3"), false)?;
    no_journal(runtime)?;
    let db = Connection::open_with_flags(
        uri(&database.path)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| ())?;
    db.busy_timeout(Duration::ZERO).map_err(|_| ())?;
    db.execute_batch("PRAGMA query_only=ON; BEGIN;")
        .map_err(|_| ())?;
    let schema: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| ())?;
    require(schema == 17)?;
    let run_matches: i64 = db.query_row("SELECT count(*) FROM (SELECT 1 FROM integrations WHERE id=?1 OR json_extract(document,'$.id')=?1 LIMIT 2)",[&b.id],|r|r.get(0)).map_err(|_| ())?;
    require(run_matches == 1)?;
    let (id,task,policy,state,revision,text):(String,String,String,String,i64,String) = db.query_row("SELECT id,task_id,policy_id,state,revision,CASE WHEN length(CAST(document AS BLOB)) BETWEEN 1 AND 65536 THEN document END FROM integrations WHERE id=?1",[&b.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_| ())?;
    let run: IntegrationRun = parse(&text)?;
    require(
        id == b.id
            && task == b.task
            && policy == b.policy
            && run.id == id
            && run.task_id == task
            && run.policy_id == policy
            && i64::try_from(run.revision).map_err(|_| ())? == revision
            && serde_json::to_value(run.state).map_err(|_| ())?.as_str() == Some(state.as_str()),
    )?;
    let records = {
        let mut statement = db.prepare("SELECT attempt_id,integration_id,round,kind,state,authority_kind,root_task_id,root_grant_id,project_id,CASE WHEN length(CAST(document AS BLOB)) BETWEEN 1 AND 65536 THEN document END FROM integration_attempts WHERE integration_id=?1 OR json_extract(document,'$.integration_id')=?1 ORDER BY round,attempt_id LIMIT 33").map_err(|_| ())?;
        let records = statement
            .query_map([&b.id], |r| {
                Ok(AttemptRow {
                    id: r.get(0)?,
                    run: r.get(1)?,
                    round: r.get(2)?,
                    kind: r.get(3)?,
                    state: r.get(4)?,
                    authority: r.get(5)?,
                    root_task: r.get(6)?,
                    root_grant: r.get(7)?,
                    project: r.get(8)?,
                    document: r.get(9)?,
                })
            })
            .map_err(|_| ())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ())?;
        records
    };
    require(records.len() <= MAX_ROWS)?;
    for row in &records {
        require(identifier(&row.id))?;
        let matches: i64 = db.query_row("SELECT count(*) FROM (SELECT 1 FROM integration_attempts WHERE attempt_id=?1 OR json_extract(document,'$.view.attempt_id')=?1 LIMIT 2)",[&row.id],|r|r.get(0)).map_err(|_| ())?;
        require(matches == 1)?;
    }
    let mut values = vec![b.id.clone()];
    values.extend(records.iter().map(|r| r.id.clone()));
    let places = (0..records.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let reverse = if records.is_empty() {
        String::from("0")
    } else {
        format!("json_extract(document,'$.payload.attempt_id') IN ({places})")
    };
    let sql = format!("SELECT sequence,CASE WHEN length(CAST(document AS BLOB)) BETWEEN 1 AND 65536 THEN document END FROM events WHERE (json_extract(document,'$.aggregate_id')=? OR {reverse}) AND json_extract(document,'$.kind') IN ('integration.stage_unknown','integration.preparation_io_observed') ORDER BY sequence LIMIT 65");
    let events = {
        let mut statement = db.prepare(&sql).map_err(|_| ())?;
        let events = statement
            .query_map(params_from_iter(values), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|_| ())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ())?;
        events
    };
    let value = project(b, &text, &records, &events)?;
    db.execute_batch("ROLLBACK;").map_err(|_| ())?;
    drop(db);
    database.check()?;
    directory.check()?;
    no_journal(runtime)?;
    require(fs::canonicalize(runtime).map_err(|_| ())? == runtime)?;
    Ok(value)
}

#[cfg(test)]
#[path = "integration_stage_evidence_tests.rs"]
mod tests;
