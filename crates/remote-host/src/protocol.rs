use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 32768;
pub const MAX_CONTENT: usize = 4096;
pub const MAX_EVENTS: usize = 64;
pub const MAX_JOBS: usize = 32;
pub const MAX_RECEIPTS: usize = 4096;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: String,
}
impl Error {
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::new("io")
    }
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::new("journal")
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::new("protocol")
    }
}
pub fn require(ok: bool, code: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::new(code))
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub fn now_ms() -> Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::new("clock"))?
        .as_millis()
        .try_into()
        .map_err(|_| Error::new("clock"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub host: String,
    pub project: String,
    pub connection: String,
    pub attempt: String,
    pub input_revision: String,
    pub configuration: String,
    pub identity_ref: String,
    pub mapping: String,
    pub recipient: String,
    pub sensitivity: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub binding: Binding,
    pub controller: String,
    pub fence: i64,
    pub credential_hash: String,
    pub issued_ms: i64,
    pub expires_ms: i64,
    pub max_operations: i64,
    pub max_bytes: i64,
    pub allow_record: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: u32,
    pub host_identity: String,
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub grant: Grant,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        let g = &self.grant;
        require(self.schema == VERSION, "version")?;
        require(
            identifier(&self.host_identity)
                && identifier(&g.controller)
                && g.fence > 0
                && g.fence < i64::MAX,
            "configuration",
        )?;
        let b = &g.binding;
        require(
            [
                &b.host,
                &b.project,
                &b.connection,
                &b.attempt,
                &b.input_revision,
                &b.configuration,
                &b.identity_ref,
                &b.mapping,
                &b.recipient,
                &b.sensitivity,
            ]
            .iter()
            .all(|s| identifier(s)),
            "binding",
        )?;
        require(
            b.host == self.host_identity && b.recipient == b.host,
            "identity",
        )?;
        require(
            g.issued_ms >= 0
                && g.expires_ms > g.issued_ms
                && g.expires_ms - g.issued_ms <= 300_000
                && g.expires_ms < i64::MAX,
            "lease",
        )?;
        require(
            g.max_operations >= 0
                && g.max_bytes >= 0
                && g.max_operations <= MAX_JOBS as i64
                && g.max_bytes <= 524288
                && g.credential_hash.len() == 64
                && g.credential_hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "budget",
        )?;
        require(
            self.root.is_absolute() && self.state_dir.is_absolute(),
            "mapping",
        )
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: String,
    pub host_identity: String,
    pub binding: Binding,
    pub controller: String,
    pub fence: i64,
    pub credential: String,
    pub action: Action,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Action {
    Reconcile {
        cursor: i64,
    },
    Record {
        name: String,
        content: String,
        delay_ms: i64,
    },
    Cancel,
    Revoke,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub sequence: i64,
    pub kind: String,
    pub job: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub digest: String,
    pub state: String,
    pub stop_evidence: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub binding: Binding,
    pub fence: i64,
    pub events: Vec<Event>,
    pub cursor: i64,
    pub more: bool,
    pub jobs: Vec<Job>,
    pub cancel_requested: bool,
    pub confirmed_stopped: bool,
    pub paused: bool,
    pub revoked: bool,
    pub used_operations: i64,
    pub used_bytes: i64,
    pub history_owner: String,
    pub cache_policy: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Reply {
    Accepted { job: String },
    CancelRequested,
    Revoked,
    Snapshot { snapshot: Box<Snapshot> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u32,
    pub request_id: String,
    pub result: std::result::Result<Reply, Error>,
}
