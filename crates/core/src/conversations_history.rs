//! Bounded reverse keyset pagination of the existing journal; no schema change.
//! Revision continuity proves only retained normalized local events, never full
//! Provider capture. Cursors are read positions, not execution authority.
use crate::{
    storage::{db_error, decode, Event, Store},
    CoreError, Result,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_ROWS: u32 = 2048;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const EVENT_BYTES: usize = MAX_BYTES - 16 * 1024;
const SAFE_INTEGER: u64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHistoryRequest {
    pub conversation_id: String,
    pub cursor: Option<ConversationHistoryCursor>,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryAnchor {
    pub sequence: u64,
    pub event_id: String,
    pub revision: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHistoryCursor {
    pub version: u32,
    pub conversation_id: String,
    pub attempt_id: String,
    pub snapshot_sequence: u64,
    pub snapshot_revision: u64,
    pub before_sequence: u64,
    pub expected_revision: u64,
    pub anchor: Option<HistoryAnchor>,
    pub seal: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryCoverage {
    Partial,
    Complete,
    Gap,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryGap {
    pub expected_revision: u64,
    pub observed_revision: Option<u64>,
}
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHistoryPage {
    pub conversation_id: String,
    pub attempt_id: String,
    pub snapshot_sequence: u64,
    pub snapshot_revision: u64,
    pub events: Vec<Event>,
    pub next_cursor: Option<ConversationHistoryCursor>,
    pub has_more: bool,
    pub scan_limited: bool,
    pub byte_limited: bool,
    pub scanned_rows: u32,
    pub scanned_bytes: usize,
    /// Exact serialized events array length; metadata has a separate 16 KiB reserve.
    pub returned_bytes: usize,
    pub coverage: HistoryCoverage,
    pub gap: Option<HistoryGap>,
    pub capture: String,
}
fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Invalid bounded conversation history request",
    )
}
fn stale() -> CoreError {
    CoreError::new(
        "STALE_REVISION",
        "Conversation history cursor changed or expired; reload history",
    )
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Conversation history record changed or is inconsistent",
    )
}
fn limited() -> CoreError {
    CoreError::new("RESOURCE_LIMIT","Conversation history record or cursor verification exceeds the bounded page; no record was skipped")
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn sha(raw: &[u8]) -> String {
    hex(&Sha256::digest(raw))
}
// A bounded process-local registry authenticates read positions without new
// keys, dependencies or persistent schema. Random UUID pairs are opaque tokens,
// not an encryption scheme. Eviction/restart requires a fresh history snapshot.
struct CursorRegistration {
    token: String,
    document: Vec<u8>,
    created: Instant,
}
fn registrations() -> &'static Mutex<VecDeque<CursorRegistration>> {
    static REGISTRY: OnceLock<Mutex<VecDeque<CursorRegistration>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(VecDeque::new()))
}
fn cursor_document(cursor: &ConversationHistoryCursor) -> Result<Vec<u8>> {
    if cursor.seal.len() > 64
        || cursor.anchor.as_ref().is_some_and(|a| {
            !bounded_id(&a.event_id)
                || a.sha256.len() != 64
                || !a.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err(stale());
    }
    let mut unsigned = cursor.clone();
    unsigned.seal.clear();
    let document = serde_json::to_vec(&unsigned).map_err(|_| changed())?;
    if document.len() > 2048 {
        return Err(stale());
    }
    Ok(document)
}
fn seal(cursor: &ConversationHistoryCursor) -> Result<String> {
    let document = cursor_document(cursor)?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut registry = registrations().lock().map_err(|_| stale())?;
    registry.retain(|entry| entry.created.elapsed() < Duration::from_secs(1800));
    register(
        &mut registry,
        CursorRegistration {
            token: token.clone(),
            document,
            created: Instant::now(),
        },
    );
    Ok(token)
}
fn register(registry: &mut VecDeque<CursorRegistration>, entry: CursorRegistration) {
    while registry.len() >= 1024 {
        registry.pop_front();
    }
    registry.push_back(entry);
}
fn valid_seal(cursor: &ConversationHistoryCursor) -> Result<bool> {
    if cursor.seal.len() != 64 || !cursor.seal.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(false);
    }
    let document = cursor_document(cursor)?;
    let mut registry = registrations().lock().map_err(|_| stale())?;
    registry.retain(|entry| entry.created.elapsed() < Duration::from_secs(1800));
    Ok(registry
        .iter()
        .any(|entry| entry.token == cursor.seal && entry.document == document))
}
fn bounded_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
fn parse_event(raw: &str, sequence: u64) -> Result<Event> {
    let mut e: Event = decode(raw)?;
    e.sequence = sequence;
    Ok(e)
}
fn checked_sequence(value: i64) -> Result<u64> {
    let value = u64::try_from(value).map_err(|_| changed())?;
    if value >= SAFE_INTEGER {
        return Err(limited());
    }
    Ok(value)
}
impl Store {
    pub fn conversation_history(
        &self,
        request: &ConversationHistoryRequest,
    ) -> Result<ConversationHistoryPage> {
        if !bounded_id(&request.conversation_id) || request.limit == 0 || request.limit > 100 {
            return Err(invalid());
        }
        // Shared &self access uses the same connection and this read snapshot; no
        // filesystem reads, Provider IO, or writes occur while the transaction lives.
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let c = self.get_conversation(&request.conversation_id)?;
        if !bounded_id(&c.id)
            || !bounded_id(&c.attempt_id)
            || c.revision == 0
            || c.revision > SAFE_INTEGER
        {
            return Err(changed());
        }
        let maximum = checked_sequence(
            tx.query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(db_error)?,
        )?;
        let mut cursor = if let Some(cursor) = &request.cursor {
            if cursor.version != 1
                || cursor.conversation_id != c.id
                || cursor.attempt_id != c.attempt_id
                || cursor.snapshot_sequence > maximum
                || cursor.snapshot_sequence >= SAFE_INTEGER
                || cursor.snapshot_revision == 0
                || cursor.snapshot_revision > c.revision
                || cursor.before_sequence == 0
                || cursor.before_sequence > cursor.snapshot_sequence + 1
                || cursor.expected_revision == 0
                || cursor.expected_revision > cursor.snapshot_revision
                || !valid_seal(cursor)?
            {
                return Err(stale());
            }
            cursor.clone()
        } else {
            ConversationHistoryCursor {
                version: 1,
                conversation_id: c.id.clone(),
                attempt_id: c.attempt_id.clone(),
                snapshot_sequence: maximum,
                snapshot_revision: c.revision,
                before_sequence: maximum + 1,
                expected_revision: c.revision,
                anchor: None,
                seal: String::new(),
            }
        };
        let initial_before = cursor.before_sequence;
        let mut scanned_rows = 0;
        let mut scanned_bytes = 0usize;
        if let Some(anchor) = &cursor.anchor {
            if anchor.sequence == 0
                || anchor.sequence > cursor.snapshot_sequence
                || anchor.sequence < cursor.before_sequence
                || anchor.revision != cursor.expected_revision + 1
                || anchor.revision > cursor.snapshot_revision
                || !bounded_id(&anchor.event_id)
                || anchor.sha256.len() != 64
            {
                return Err(stale());
            }
            let length: Option<i64> = tx
                .query_row(
                    "SELECT octet_length(document) FROM events WHERE sequence=?1",
                    [i64::try_from(anchor.sequence).map_err(|_| stale())?],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db_error)?;
            let length = usize::try_from(length.ok_or_else(stale)?).map_err(|_| changed())?;
            if length > MAX_BYTES {
                return Err(limited());
            }
            let raw: String = tx
                .query_row(
                    "SELECT document FROM events WHERE sequence=?1",
                    [anchor.sequence as i64],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if raw.len() != length || sha(raw.as_bytes()) != anchor.sha256 {
                return Err(stale());
            }
            let e = parse_event(&raw, anchor.sequence)?;
            if e.aggregate_id != c.id
                || e.event_id != anchor.event_id
                || e.revision != anchor.revision
            {
                return Err(stale());
            }
            scanned_rows += 1;
            scanned_bytes += length;
        } else if cursor.expected_revision != cursor.snapshot_revision {
            return Err(stale());
        }
        let mut events = Vec::new();
        let mut event_bytes = 2usize;
        let mut gap = None;
        let mut scan_limited = false;
        let mut byte_limited = false;
        let mut complete = false;
        {
            let mut metadata=tx.prepare("SELECT sequence,octet_length(document) FROM events WHERE sequence<?1 AND sequence<=?2 ORDER BY sequence DESC LIMIT ?3").map_err(db_error)?;
            let mut rows = metadata
                .query(params![
                    cursor.before_sequence as i64,
                    cursor.snapshot_sequence as i64,
                    MAX_ROWS - scanned_rows
                ])
                .map_err(db_error)?;
            let mut documents = tx
                .prepare_cached("SELECT document FROM events WHERE sequence=?1")
                .map_err(db_error)?;
            loop {
                if events.len() >= request.limit as usize {
                    break;
                }
                if scanned_rows >= MAX_ROWS {
                    scan_limited = true;
                    break;
                }
                let Some(row) = rows.next().map_err(db_error)? else {
                    gap = Some(HistoryGap {
                        expected_revision: cursor.expected_revision,
                        observed_revision: None,
                    });
                    break;
                };
                scanned_rows += 1;
                let sequence = checked_sequence(row.get(0).map_err(db_error)?)?;
                if sequence == 0 {
                    return Err(changed());
                }
                let length = usize::try_from(row.get::<_, i64>(1).map_err(db_error)?)
                    .map_err(|_| changed())?;
                if length > MAX_BYTES {
                    return Err(limited());
                }
                if length > MAX_BYTES.saturating_sub(scanned_bytes) {
                    byte_limited = true;
                    break;
                }
                let raw: String = documents
                    .query_row([sequence as i64], |r| r.get(0))
                    .map_err(db_error)?;
                if raw.len() != length {
                    return Err(changed());
                }
                scanned_bytes += length;
                let e = parse_event(&raw, sequence)?;
                if e.aggregate_id == c.id {
                    if !bounded_id(&e.event_id)
                        || e.revision == 0
                        || e.revision > SAFE_INTEGER
                        || e.source != "conversation_core"
                        || !e.kind.starts_with("conversation.")
                    {
                        return Err(changed());
                    }
                    if e.revision > cursor.expected_revision {
                        return Err(changed());
                    }
                    if e.revision < cursor.expected_revision {
                        gap = Some(HistoryGap {
                            expected_revision: cursor.expected_revision,
                            observed_revision: Some(e.revision),
                        });
                        break;
                    }
                    if e.revision == 1
                        && (e.kind != "conversation.launch_reserved"
                            || e.payload
                                .get("attempt_id")
                                .and_then(serde_json::Value::as_str)
                                != Some(c.attempt_id.as_str()))
                    {
                        return Err(changed());
                    }
                    let bytes = serde_json::to_vec(&e).map_err(|_| changed())?.len()
                        + usize::from(!events.is_empty());
                    if bytes > EVENT_BYTES.saturating_sub(event_bytes) {
                        if events.is_empty() {
                            return Err(limited());
                        }
                        byte_limited = true;
                        break;
                    }
                    event_bytes += bytes;
                    cursor.anchor = Some(HistoryAnchor {
                        sequence,
                        event_id: e.event_id.clone(),
                        revision: e.revision,
                        sha256: sha(raw.as_bytes()),
                    });
                    cursor.expected_revision -= 1;
                    events.push(e);
                }
                cursor.before_sequence = sequence;
                if cursor.expected_revision == 0 {
                    complete = true;
                    break;
                }
            }
        }
        let has_more = !complete && gap.is_none();
        if has_more && cursor.before_sequence >= initial_before {
            return Err(limited());
        }
        events.reverse();
        let returned_bytes = serde_json::to_vec(&events).map_err(|_| changed())?.len();
        let coverage = if complete {
            HistoryCoverage::Complete
        } else if gap.is_some() {
            HistoryCoverage::Gap
        } else {
            HistoryCoverage::Partial
        };
        let snapshot_sequence = cursor.snapshot_sequence;
        let snapshot_revision = cursor.snapshot_revision;
        let next_cursor = if has_more {
            cursor.seal = seal(&cursor)?;
            Some(cursor)
        } else {
            None
        };
        let page = ConversationHistoryPage {
            conversation_id: c.id,
            attempt_id: c.attempt_id,
            snapshot_sequence,
            snapshot_revision,
            events,
            next_cursor,
            has_more,
            scan_limited,
            byte_limited,
            scanned_rows,
            scanned_bytes,
            returned_bytes,
            coverage,
            gap,
            capture: "normalized_partial".into(),
        };
        if serde_json::to_vec(&page).map_err(|_| changed())?.len() > MAX_BYTES {
            return Err(limited());
        }
        tx.commit().map_err(db_error)?;
        Ok(page)
    }
}
#[cfg(test)]
#[path = "conversations_history_tests.rs"]
mod tests;
