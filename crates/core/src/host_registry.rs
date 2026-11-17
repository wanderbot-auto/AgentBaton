//! Immutable Host inventory only. No contact, execution authority or migration.
use crate::storage::{db_error, encode, now, Store};
use crate::{CoreError, Result};
use agentbaton_remote_host::registration::HostRegistration;
use rusqlite::{params, types::ValueRef, Connection, Row, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const READ_RECEIPTS_SQL: &str = "SELECT CASE WHEN typeof(key)='text' AND octet_length(key)<=256 THEN key END,CASE WHEN typeof(payload)='text' AND octet_length(payload)<=16384 THEN payload END,CASE WHEN typeof(result)='text' AND octet_length(result)<=24576 THEN result END FROM idempotency WHERE scope=?1 ORDER BY key LIMIT 129";
const READ_HOSTS_SQL: &str = "SELECT CASE WHEN typeof(id)='text' AND octet_length(id)<=36 THEN id END,CASE WHEN typeof(name)='text' AND octet_length(name)<=256 THEN name END,CASE WHEN typeof(name_key)='text' AND octet_length(name_key)<=256 THEN name_key END,CASE WHEN typeof(revision)='integer' THEN revision END,CASE WHEN typeof(created_sequence)='integer' THEN created_sequence END,CASE WHEN typeof(document)='text' AND octet_length(document)<=24576 THEN document END FROM host_registrations ORDER BY created_sequence LIMIT 129";
const READ_EVENT_SQL: &str = "SELECT CASE WHEN typeof(document)='text' AND octet_length(document)<=4096 THEN document END FROM events WHERE sequence=?1";

const SCOPE: &str = "hosts.register:v1";
const MAX_HOSTS: usize = 128;
const MAX_METADATA: usize = 16 * 1024;
const MAX_DOCUMENT: usize = 24 * 1024;
const MAX_EVENT: usize = 4 * 1024;
const MAX_RESPONSE: usize = 512 * 1024;
// The later IPC adapter must measure its actual complete envelope as well.
const FRAME_RESERVE: usize = 2048;
const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;
// Match the composed migration's existing main-schema metadata policy, not a
// lower arbitrary limit on lawful opaque names/tables.
const MAX_SCHEMA_OBJECTS: usize = 256;
const MAX_SCHEMA_BYTES: usize = 1_048_576;
const MAX_AUDIT_BYTES: usize = MAX_HOSTS * (MAX_DOCUMENT * 2 + MAX_METADATA + MAX_EVENT + 1024);
const TABLE_SQL: &str = "CREATE TABLE host_registrations(\n id TEXT PRIMARY KEY,\n name TEXT NOT NULL,\n name_key TEXT NOT NULL,\n revision INTEGER NOT NULL CHECK(revision=1),\n created_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),\n document TEXT NOT NULL\n)";
const INDEX_SQL: &str =
    "CREATE INDEX host_registrations_name_id ON host_registrations(name_key,id)";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostCreationFacts {
    local_validation: String,
    references: String,
    remote_probe: String,
    remote_connection: String,
    remote_capabilities: String,
    execution_enabled: bool,
}
impl HostCreationFacts {
    fn unverified() -> Self {
        Self {
            local_validation: "configuration_syntax_validated".into(),
            references: "unresolved".into(),
            remote_probe: "not_probed".into(),
            remote_connection: "not_contacted".into(),
            remote_capabilities: "unknown".into(),
            execution_enabled: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostRegistrationRecord {
    pub record_version: u32,
    pub id: String,
    pub revision: u64,
    pub created_sequence: u64,
    pub created_at: String,
    pub creation_key: String,
    pub registration: HostRegistration,
    pub creation_facts: HostCreationFacts,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostCreationReceipt {
    pub receipt_version: u32,
    pub host_id: String,
    pub revision: u64,
    pub created_sequence: u64,
    pub created_at: String,
    pub registration: HostRegistration,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct HostEventPayload {
    payload_version: u32,
    host_id: String,
    revision: u64,
    creation_key: String,
    registration_sha256: String,
}
// Typed payload preserves canonical bytes without a generic Value map. This is
// an ordinary Event envelope, never a fabricated Task aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct HostEvent {
    sequence: u64,
    event_id: String,
    aggregate_id: String,
    revision: u64,
    kind: String,
    occurred_at: String,
    payload: HostEventPayload,
    source: String,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostRegistrationView {
    pub view_version: u32,
    pub record: HostRegistrationRecord,
    pub registration: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostListRequest {
    pub limit: u32,
    pub after_created_sequence: Option<u64>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostRegistrationPage {
    pub view_version: u32,
    pub hosts: Vec<HostRegistrationView>,
    pub next_created_sequence: Option<u64>,
}
struct AuditedHost {
    record: HostRegistrationRecord,
    receipt: HostCreationReceipt,
}
#[derive(Default)]
struct Budget(usize);
impl Budget {
    fn add(&mut self, bytes: usize) -> Result<()> {
        self.0 = self.0.checked_add(bytes).ok_or_else(integrity)?;
        if self.0 > MAX_AUDIT_BYTES {
            return Err(integrity());
        }
        Ok(())
    }
}
fn integrity() -> CoreError {
    let mut error = CoreError::new(
        "OUTCOME_UNKNOWN",
        "Host inventory integrity is unknown; reconcile or restore before retrying writes",
    );
    error.side_effect_state = "unknown".into();
    error
}
fn unsupported() -> CoreError {
    CoreError::new(
        "VERSION_UNSUPPORTED",
        "Host inventory schema or record version is unsupported",
    )
}
fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Invalid or oversized Host metadata request",
    )
}
fn key_valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 256
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
fn uuid_valid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|u| {
        u.to_string() == id && u.get_version_num() == 4 && u.as_bytes()[8] & 0xc0 == 0x80
    })
}
fn sequence_valid(sequence: u64) -> bool {
    (1..=MAX_SEQUENCE).contains(&sequence)
}
// storage::now emits UTC RFC3339. No new time feature or clock read on queries.
fn timestamp_valid(value: &str) -> bool {
    let b = value.as_bytes();
    if !(20..=30).contains(&b.len()) || b.last() != Some(&b'Z') {
        return false;
    }
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return false;
    }
    let number = |a: usize, z: usize| -> Option<u32> {
        let digits = &b[a..z];
        digits
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| digits.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    year > 0
        && (1..=days).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60
        && (b.len() == 20
            || (b.len() >= 22
                && b[19] == b'.'
                && b[20..b.len() - 1].iter().all(u8::is_ascii_digit)))
}
fn canonical_registration(registration: &HostRegistration) -> Result<String> {
    registration.validate().map_err(|_| invalid())?;
    let raw = encode(registration)?;
    if raw.len() > MAX_METADATA {
        return Err(invalid());
    }
    Ok(raw)
}
fn decode_canonical<T: DeserializeOwned + Serialize>(raw: &str, bound: usize) -> Result<T> {
    if raw.len() > bound {
        return Err(integrity());
    }
    let value: T = serde_json::from_str(raw).map_err(|_| integrity())?;
    if encode(&value)? != raw {
        return Err(integrity());
    }
    Ok(value)
}
// SQL projects invalid/oversized values as NULL rather than omitting the row.
// This post-projection guard still rejects wrong SQLite types/UTF-8 and checks
// cumulative Rust decoding bytes; it is not a whole-engine memory sandbox.
fn read_text(row: &Row<'_>, column: usize, bound: usize, budget: &mut Budget) -> Result<String> {
    match row.get_ref(column).map_err(db_error)? {
        ValueRef::Text(bytes) if bytes.len() <= bound => {
            budget.add(bytes.len())?;
            Ok(std::str::from_utf8(bytes)
                .map_err(|_| integrity())?
                .to_owned())
        }
        _ => Err(integrity()),
    }
}
fn read_positive(row: &Row<'_>, column: usize) -> Result<u64> {
    match row.get_ref(column).map_err(db_error)? {
        ValueRef::Integer(n) if n > 0 && n <= MAX_SEQUENCE as i64 => Ok(n as u64),
        _ => Err(integrity()),
    }
}
fn validate_schema(db: &Connection) -> Result<()> {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version != 19 {
        return Err(unsupported());
    }
    let foreign_keys: i64 = db
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .map_err(db_error)?;
    if foreign_keys != 1 {
        return Err(integrity());
    }
    let mut statement=db.prepare("SELECT CASE WHEN typeof(type)='text' AND octet_length(type)<=32 THEN type END,CASE WHEN typeof(name)='text' AND octet_length(name)<=128 THEN name END,CASE WHEN typeof(tbl_name)='text' AND octet_length(tbl_name)<=128 THEN tbl_name END,CASE WHEN typeof(sql)='null' THEN NULL WHEN typeof(sql)='text' AND octet_length(sql)<=16384 THEN sql ELSE x'00' END FROM main.sqlite_schema WHERE lower(tbl_name)='host_registrations' OR lower(name) IN ('host_registrations','host_registrations_name_id') ORDER BY name LIMIT 9").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut actual = BTreeMap::new();
    let mut budget = Budget::default();
    while let Some(row) = rows.next().map_err(db_error)? {
        if actual.len() >= 4 {
            return Err(unsupported());
        }
        let kind = read_text(row, 0, 32, &mut budget)?;
        let name = read_text(row, 1, 128, &mut budget)?;
        let table = read_text(row, 2, 128, &mut budget)?;
        let sql = match row.get_ref(3).map_err(db_error)? {
            ValueRef::Null => None,
            _ => Some(read_text(row, 3, 16 * 1024, &mut budget)?),
        };
        actual.insert(name, (kind, table, sql));
    }
    let expected: BTreeMap<_, _> = [
        (
            "host_registrations",
            ("table", "host_registrations", Some(TABLE_SQL)),
        ),
        (
            "host_registrations_name_id",
            ("index", "host_registrations", Some(INDEX_SQL)),
        ),
        (
            "sqlite_autoindex_host_registrations_1",
            ("index", "host_registrations", None),
        ),
        (
            "sqlite_autoindex_host_registrations_2",
            ("index", "host_registrations", None),
        ),
    ]
    .into_iter()
    .map(|(n, (k, t, s))| {
        (
            n.to_owned(),
            (k.to_owned(), t.to_owned(), s.map(str::to_owned)),
        )
    })
    .collect();
    if actual != expected {
        return Err(unsupported());
    }
    // Inspect dependency metadata only; ordinary opaque pragma_* tables may
    // shadow table-valued PRAGMAs. Direct main PRAGMA syntax cannot name them
    // as a query source, and it never evaluates their rows/CHECK expressions.
    if has_incoming_host_fk(db)? {
        return Err(unsupported());
    }
    Ok(())
}
fn has_incoming_host_fk(db: &Connection) -> Result<bool> {
    let (objects, bytes): (i64, i64) = db.query_row(
        "SELECT count(*),coalesce(sum(octet_length(type)+octet_length(name)+octet_length(tbl_name)+coalesce(octet_length(sql),0)),0) FROM main.sqlite_schema",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(db_error)?;
    if !(1..=MAX_SCHEMA_OBJECTS as i64).contains(&objects)
        || !(1..=MAX_SCHEMA_BYTES as i64).contains(&bytes)
    {
        return Err(unsupported());
    }
    // Do not filter malformed metadata into absence. All main objects remain
    // in this N+1 projection; only their validated table names select a PRAGMA.
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(type)='text' AND type IN ('table','index','view','trigger') THEN type END,CASE WHEN typeof(name)='text' AND octet_length(name)<=?1 THEN name END,CASE WHEN typeof(tbl_name)='text' AND octet_length(tbl_name)<=?1 THEN tbl_name END FROM main.sqlite_schema ORDER BY type,name LIMIT ?2",
    ).map_err(db_error)?;
    let mut rows = statement
        .query(params![
            MAX_SCHEMA_BYTES as i64,
            (MAX_SCHEMA_OBJECTS + 1) as i64
        ])
        .map_err(db_error)?;
    let mut seen = 0;
    let mut fk_rows = 0usize;
    let mut budget = Budget::default();
    while let Some(row) = rows.next().map_err(db_error)? {
        if seen >= MAX_SCHEMA_OBJECTS {
            return Err(unsupported());
        }
        seen += 1;
        let kind = read_text(row, 0, 7, &mut budget)?;
        let name = read_text(row, 1, MAX_SCHEMA_BYTES, &mut budget)?;
        let table = read_text(row, 2, MAX_SCHEMA_BYTES, &mut budget)?;
        if name.contains('\0') || table.contains('\0') {
            return Err(integrity());
        }
        if kind != "table" {
            continue;
        }
        if name != table {
            return Err(integrity());
        }
        // PRAGMA arguments use SQLite's quoted-identifier syntax. The source is
        // bounded main-schema metadata, never a caller command or SQL fragment.
        let quoted = name.replace('"', "\"\"");
        let mut keys = db
            .prepare(&format!("PRAGMA main.foreign_key_list(\"{quoted}\")"))
            .map_err(db_error)?;
        let mut keys = keys.query([]).map_err(db_error)?;
        while let Some(key) = keys.next().map_err(db_error)? {
            // Every FK result requires source syntax within the existing1MiB
            // schema budget. This finite ceiling adds no smaller opaque limit;
            // accepted ordinary opaque tables have no FK declarations at all.
            if fk_rows >= MAX_SCHEMA_BYTES {
                return Err(unsupported());
            }
            fk_rows += 1;
            let target = read_text(key, 2, MAX_SCHEMA_BYTES, &mut budget)?;
            if target.eq_ignore_ascii_case("host_registrations") {
                return Ok(true);
            }
        }
    }
    if seen as i64 != objects {
        return Err(integrity());
    }
    Ok(false)
}

impl HostRegistrationRecord {
    fn receipt(&self) -> HostCreationReceipt {
        HostCreationReceipt {
            receipt_version: 1,
            host_id: self.id.clone(),
            revision: self.revision,
            created_sequence: self.created_sequence,
            created_at: self.created_at.clone(),
            registration: self.registration.clone(),
        }
    }
    fn validate(&self) -> Result<()> {
        if self.record_version != 1 {
            return Err(unsupported());
        }
        if !uuid_valid(&self.id)
            || self.revision != 1
            || !sequence_valid(self.created_sequence)
            || !timestamp_valid(&self.created_at)
            || !key_valid(&self.creation_key)
            || self.creation_facts != HostCreationFacts::unverified()
        {
            return Err(integrity());
        }
        canonical_registration(&self.registration).map_err(|_| integrity())?;
        Ok(())
    }
}
impl HostCreationReceipt {
    fn validate(&self) -> Result<()> {
        if self.receipt_version != 1 {
            return Err(unsupported());
        }
        if !uuid_valid(&self.host_id)
            || self.revision != 1
            || !sequence_valid(self.created_sequence)
            || !timestamp_valid(&self.created_at)
        {
            return Err(integrity());
        }
        canonical_registration(&self.registration).map_err(|_| integrity())?;
        Ok(())
    }
}
fn digest(raw: &str) -> String {
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}
fn audit(db: &Connection) -> Result<Vec<AuditedHost>> {
    validate_schema(db)?;
    let mut budget = Budget::default();
    let mut receipts = BTreeMap::new();
    {
        let mut statement = db.prepare(READ_RECEIPTS_SQL).map_err(db_error)?;
        let mut rows = statement.query([SCOPE]).map_err(db_error)?;
        while let Some(row) = rows.next().map_err(db_error)? {
            if receipts.len() >= MAX_HOSTS {
                return Err(integrity());
            }
            let key = read_text(row, 0, 256, &mut budget)?;
            if !key_valid(&key) {
                return Err(integrity());
            }
            let payload = read_text(row, 1, MAX_METADATA, &mut budget)?;
            let raw = read_text(row, 2, MAX_DOCUMENT, &mut budget)?;
            let receipt: HostCreationReceipt = decode_canonical(&raw, MAX_DOCUMENT)?;
            receipt.validate()?;
            if canonical_registration(&receipt.registration)? != payload {
                return Err(integrity());
            }
            receipts.insert(key, (payload, receipt));
        }
    }
    let mut statement = db.prepare(READ_HOSTS_SQL).map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut audited = Vec::new();
    let mut keys = BTreeSet::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        if audited.len() >= MAX_HOSTS {
            return Err(integrity());
        }
        let id = read_text(row, 0, 36, &mut budget)?;
        let name = read_text(row, 1, 256, &mut budget)?;
        let name_key = read_text(row, 2, 256, &mut budget)?;
        let revision = read_positive(row, 3)?;
        let sequence = read_positive(row, 4)?;
        let raw = read_text(row, 5, MAX_DOCUMENT, &mut budget)?;
        let record: HostRegistrationRecord = decode_canonical(&raw, MAX_DOCUMENT)?;
        record.validate()?;
        if id != record.id
            || revision != record.revision
            || sequence != record.created_sequence
            || name != record.registration.name
            || name_key != name
            || !keys.insert(record.creation_key.clone())
        {
            return Err(integrity());
        }
        let (payload, receipt) = receipts
            .remove(&record.creation_key)
            .ok_or_else(integrity)?;
        if receipt != record.receipt() {
            return Err(integrity());
        }
        let mut event_statement = db.prepare(READ_EVENT_SQL).map_err(db_error)?;
        let mut event_rows = event_statement.query([sequence as i64]).map_err(db_error)?;
        let event_row = event_rows.next().map_err(db_error)?.ok_or_else(integrity)?;
        let raw_event = read_text(event_row, 0, MAX_EVENT, &mut budget)?;
        let event: HostEvent = decode_canonical(&raw_event, MAX_EVENT)?;
        if event.payload.payload_version != 1 {
            return Err(unsupported());
        }
        if event.sequence != 0
            || !uuid_valid(&event.event_id)
            || event.aggregate_id != id
            || event.revision != 1
            || event.kind != "host.registered"
            || event.occurred_at != record.created_at
            || event.source != "user_action"
            || event.payload.host_id != id
            || event.payload.revision != 1
            || event.payload.creation_key != record.creation_key
            || event.payload.registration_sha256 != digest(&payload)
        {
            return Err(integrity());
        }
        audited.push(AuditedHost { record, receipt });
    }
    if !receipts.is_empty() {
        return Err(integrity());
    }
    Ok(audited)
}
fn page(
    entries: Vec<AuditedHost>,
    request: &HostListRequest,
    byte_limit: usize,
) -> Result<HostRegistrationPage> {
    let candidates: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            request
                .after_created_sequence
                .is_none_or(|c| e.record.created_sequence > c)
        })
        .collect();
    let available = candidates.len();
    let mut page = HostRegistrationPage {
        view_version: 1,
        hosts: Vec::new(),
        next_created_sequence: None,
    };
    for entry in candidates.into_iter().take(request.limit as usize) {
        let sequence = entry.record.created_sequence;
        page.hosts.push(HostRegistrationView {
            view_version: 1,
            record: entry.record,
            registration: "registered".into(),
        });
        page.next_created_sequence = Some(sequence);
        if encode(&page)?
            .len()
            .checked_add(FRAME_RESERVE)
            .ok_or_else(integrity)?
            > byte_limit
        {
            page.hosts.pop();
            if page.hosts.is_empty() {
                return Err(CoreError::new(
                    "RESOURCE_LIMIT",
                    "Host response exceeds bounded page size",
                ));
            }
            break;
        }
    }
    page.next_created_sequence = if page.hosts.len() < available {
        page.hosts.last().map(|h| h.record.created_sequence)
    } else {
        None
    };
    Ok(page)
}
impl Store {
    pub fn register_host(
        &mut self,
        registration: &HostRegistration,
        key: &str,
    ) -> Result<HostCreationReceipt> {
        if !key_valid(key) {
            return Err(invalid());
        }
        let payload = canonical_registration(registration)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let entries = audit(&tx)?;
        if let Some(existing) = entries.iter().find(|e| e.record.creation_key == key) {
            if canonical_registration(&existing.receipt.registration)? != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Host creation key already belongs to a different request",
                ));
            }
            let receipt = existing.receipt.clone();
            tx.commit().map_err(db_error)?;
            return Ok(receipt);
        }
        if entries.len() >= MAX_HOSTS {
            return Err(CoreError::new("RESOURCE_LIMIT", "Host inventory is full"));
        }
        let id = Uuid::new_v4().to_string();
        let created_at = now()?;
        let event = HostEvent {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: id.clone(),
            revision: 1,
            kind: "host.registered".into(),
            occurred_at: created_at.clone(),
            payload: HostEventPayload {
                payload_version: 1,
                host_id: id.clone(),
                revision: 1,
                creation_key: key.into(),
                registration_sha256: digest(&payload),
            },
            source: "user_action".into(),
        };
        let raw_event = encode(&event)?;
        if raw_event.len() > MAX_EVENT {
            return Err(invalid());
        }
        tx.execute("INSERT INTO events(document) VALUES(?1)", [raw_event])
            .map_err(db_error)?;
        let inserted = tx.last_insert_rowid();
        if inserted <= 0 || !sequence_valid(inserted as u64) {
            return Err(integrity());
        }
        let record = HostRegistrationRecord {
            record_version: 1,
            id,
            revision: 1,
            created_sequence: inserted as u64,
            created_at,
            creation_key: key.into(),
            registration: registration.clone(),
            creation_facts: HostCreationFacts::unverified(),
        };
        record.validate()?;
        let receipt = record.receipt();
        let raw_record = encode(&record)?;
        let raw_receipt = encode(&receipt)?;
        if raw_record.len() > MAX_DOCUMENT || raw_receipt.len() > MAX_DOCUMENT {
            return Err(invalid());
        }
        tx.execute("INSERT INTO host_registrations(id,name,name_key,revision,created_sequence,document) VALUES(?1,?2,?3,?4,?5,?6)",
            params![record.id,registration.name,registration.name,1,inserted,raw_record]).map_err(db_error)?;
        tx.execute(
            "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
            params![SCOPE, key, payload, raw_receipt],
        )
        .map_err(db_error)?;
        tx.commit().map_err(|error| {
            let mut error = db_error(error);
            error.code = "OUTCOME_UNKNOWN".into();
            error
        })?;
        Ok(receipt)
    }
    pub fn find_host_registration(&mut self, key: &str) -> Result<Option<HostCreationReceipt>> {
        if !key_valid(key) {
            return Err(invalid());
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        let result = audit(&tx)?
            .into_iter()
            .find(|e| e.record.creation_key == key)
            .map(|e| e.receipt);
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub fn get_host_registration(&mut self, id: &str) -> Result<HostRegistrationView> {
        if !uuid_valid(id) {
            return Err(invalid());
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        let entry = audit(&tx)?
            .into_iter()
            .find(|e| e.record.id == id)
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Host registration does not exist"))?;
        tx.commit().map_err(db_error)?;
        Ok(HostRegistrationView {
            view_version: 1,
            record: entry.record,
            registration: "registered".into(),
        })
    }
    pub fn list_host_registrations(
        &mut self,
        request: &HostListRequest,
    ) -> Result<HostRegistrationPage> {
        if !(1..=25).contains(&request.limit)
            || request
                .after_created_sequence
                .is_some_and(|c| !sequence_valid(c))
        {
            return Err(invalid());
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        let result = page(audit(&tx)?, request, MAX_RESPONSE)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}
#[cfg(test)]
#[path = "host_registry_tests.rs"]
mod tests;
