//! Owner-only directory observations, never task/execution or terminal authority.
//! No Git inspection: a dirty checkout is still a legitimate directory to open.
use crate::registry::{ManagedWorkspace, Project};
use crate::storage::{db_error, Store};
use crate::{CoreError, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

const MAX_DOCUMENT_BYTES: i64 = 262_144;
const MAX_PATH_BYTES: usize = 32_768;
const MAX_DIRECTORY_IDENTITIES: usize = 129;

fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Select a registered local directory using exact IDs and revisions",
    )
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Registered directory identity changed or is unavailable; refresh before opening",
    )
}
fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Directory observation exceeds its bounded resources",
    )
}
fn valid_id(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().to_string() == value)
}
fn uuid_field<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<String, D::Error> {
    let value = String::deserialize(decoder)?;
    if !valid_id(&value) {
        return Err(serde::de::Error::custom("Expected a canonical UUID"));
    }
    Ok(value)
}
fn revision_field<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<u64, D::Error> {
    let value = serde_json::Value::deserialize(decoder)?;
    value
        .as_u64()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| serde::de::Error::custom("Expected a positive exact u64 integer"))
}
fn decimal_identity(value: &str) -> Option<u64> {
    let number = value.parse::<u64>().ok()?;
    (number.to_string() == value).then_some(number)
}
fn identity_field<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<String, D::Error> {
    let value = String::deserialize(decoder)?;
    if decimal_identity(&value).is_none() {
        return Err(serde::de::Error::custom(
            "Expected canonical decimal u64 text",
        ));
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceOpenSelection {
    ProjectSource {
        #[serde(deserialize_with = "uuid_field")]
        project_id: String,
        #[serde(deserialize_with = "revision_field")]
        expected_project_revision: u64,
    },
    ManagedWorkspace {
        #[serde(deserialize_with = "uuid_field")]
        project_id: String,
        #[serde(deserialize_with = "revision_field")]
        expected_project_revision: u64,
        #[serde(deserialize_with = "uuid_field")]
        workspace_id: String,
        #[serde(deserialize_with = "revision_field")]
        expected_workspace_revision: u64,
    },
}
impl WorkspaceOpenSelection {
    pub fn validate(&self) -> Result<()> {
        let (project, revision) = self.project();
        if !valid_id(project) || revision == 0 {
            return Err(invalid());
        }
        if let Self::ManagedWorkspace {
            workspace_id,
            expected_workspace_revision,
            ..
        } = self
        {
            if !valid_id(workspace_id) || *expected_workspace_revision == 0 {
                return Err(invalid());
            }
        }
        Ok(())
    }
    fn project(&self) -> (&str, u64) {
        match self {
            Self::ProjectSource {
                project_id,
                expected_project_revision,
            }
            | Self::ManagedWorkspace {
                project_id,
                expected_project_revision,
                ..
            } => (project_id, *expected_project_revision),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceOpenRequest {
    pub selection: WorkspaceOpenSelection,
}

fn forbidden_read_field<'de, D: Deserializer<'de>, T>(
    decoder: D,
) -> std::result::Result<Option<T>, D::Error> {
    let _ = serde::de::IgnoredAny::deserialize(decoder)?;
    Err(serde::de::Error::custom(
        "Directory context accepts no write envelope fields",
    ))
}

// Match the existing Owner Request envelope exactly, retaining typed params
// through raw decoding so serde_json::Value cannot hide duplicate nested keys.
// The new read refuses even explicit-null CAS/idempotency fields. Older routes
// continue to use their original decoder and behavior.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceOpenEnvelope {
    #[serde(rename = "api_version")]
    _api_version: u32,
    #[serde(rename = "request_id")]
    _request_id: String,
    #[serde(rename = "token")]
    _token: String,
    #[serde(rename = "method")]
    _method: String,
    params: WorkspaceOpenRequest,
    #[serde(
        rename = "expected_revision",
        default,
        deserialize_with = "forbidden_read_field"
    )]
    _expected_revision: Option<u64>,
    #[serde(
        rename = "idempotency_key",
        default,
        deserialize_with = "forbidden_read_field"
    )]
    _idempotency_key: Option<String>,
}

/// Invoked only for the already authenticated Owner's new method. This adds
/// duplicate-key protection to this read; it never authenticates a caller.
pub(crate) fn validate_workspace_open_frame(raw: &str) -> Result<()> {
    let frame: WorkspaceOpenEnvelope = serde_json::from_str(raw).map_err(|_| invalid())?;
    frame.params.selection.validate()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDirectoryIdentity {
    #[serde(deserialize_with = "identity_field")]
    pub device: String,
    #[serde(deserialize_with = "identity_field")]
    pub inode: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceOpenContext {
    pub selection: WorkspaceOpenSelection,
    pub canonical_path: String,
    pub directory_identity: Vec<WorkspaceDirectoryIdentity>,
}

fn components(path: &Path) -> Result<Vec<std::ffi::OsString>> {
    if !path.is_absolute() || path.to_str().is_none() {
        return Err(changed());
    }
    if path.as_os_str().len() > MAX_PATH_BYTES {
        return Err(limited());
    }
    let mut names = vec![];
    let mut exact = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                if names.len() >= MAX_DIRECTORY_IDENTITIES - 1 {
                    return Err(limited());
                }
                names.push(name.to_owned());
                exact.push(name);
            }
            _ => return Err(changed()),
        }
    }
    // Component iteration normalizes '.', repeated/trailing separators. Never
    // treat those spellings, or a symlink resolution, as a canonical fallback.
    if exact.as_os_str() != path.as_os_str() {
        return Err(changed());
    }
    Ok(names)
}
fn file_identity(file: &File) -> Result<WorkspaceDirectoryIdentity> {
    let metadata = file.metadata().map_err(|_| changed())?;
    if !metadata.is_dir() {
        return Err(changed());
    }
    Ok(WorkspaceDirectoryIdentity {
        device: metadata.dev().to_string(),
        inode: metadata.ino().to_string(),
    })
}
fn directory_flags() -> rustix::fs::OFlags {
    use rustix::fs::OFlags;
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}

/// Holds an observed chain, not a lease, sandbox or continuous pathname lock.
/// Callers must obtain the context from the authenticated Owner channel; this
/// helper validates filesystem correspondence, not caller authority.
pub struct WorkspaceOpenDirectoryPin {
    path: PathBuf,
    names: Vec<std::ffi::OsString>,
    files: Vec<File>,
    identities: Vec<WorkspaceDirectoryIdentity>,
}
impl WorkspaceOpenDirectoryPin {
    fn capture(path: &Path) -> Result<Self> {
        let names = components(path)?;
        let root: File = rustix::fs::open("/", directory_flags(), rustix::fs::Mode::empty())
            .map_err(|_| changed())?
            .into();
        let mut identities = vec![file_identity(&root)?];
        let mut files = vec![root];
        for name in &names {
            let file: File = rustix::fs::openat(
                files.last().ok_or_else(changed)?,
                name,
                directory_flags(),
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| changed())?
            .into();
            identities.push(file_identity(&file)?);
            files.push(file);
        }
        let pin = Self {
            path: path.into(),
            names,
            files,
            identities,
        };
        pin.recheck()?;
        Ok(pin)
    }
    pub fn recheck(&self) -> Result<()> {
        if components(&self.path)? != self.names || self.files.len() != self.identities.len() {
            return Err(changed());
        }
        let mut current: File = rustix::fs::open("/", directory_flags(), rustix::fs::Mode::empty())
            .map_err(|_| changed())?
            .into();
        if file_identity(&current)? != self.identities[0] {
            return Err(changed());
        }
        for (index, name) in self.names.iter().enumerate() {
            if file_identity(&self.files[index])? != self.identities[index] {
                return Err(changed());
            }
            current =
                rustix::fs::openat(&current, name, directory_flags(), rustix::fs::Mode::empty())
                    .map_err(|_| changed())?
                    .into();
            if file_identity(&current)? != self.identities[index + 1] {
                return Err(changed());
            }
        }
        for (file, identity) in self.files.iter().zip(&self.identities) {
            if &file_identity(file)? != identity {
                return Err(changed());
            }
        }
        Ok(())
    }
}

/// Reopen and hold every directory named by a fresh Owner context, comparing
/// canonical decimal identities without floating-point/JS numeric conversion.
pub fn pin_open_context(context: &WorkspaceOpenContext) -> Result<WorkspaceOpenDirectoryPin> {
    context.selection.validate()?;
    let names = components(Path::new(&context.canonical_path))?;
    if context.directory_identity.len() != names.len() + 1 {
        return Err(changed());
    }
    for identity in &context.directory_identity {
        if decimal_identity(&identity.device).is_none()
            || decimal_identity(&identity.inode).is_none()
        {
            return Err(invalid());
        }
    }
    let pin = WorkspaceOpenDirectoryPin::capture(Path::new(&context.canonical_path))?;
    if pin.identities != context.directory_identity {
        return Err(changed());
    }
    pin.recheck()?;
    Ok(pin)
}

fn load_project(db: &Connection, id: &str) -> Result<Project> {
    let row = db.query_row(
        "SELECT CASE WHEN length(CAST(document AS BLOB)) BETWEEN 1 AND ?2 THEN document END, CASE WHEN length(CAST(root AS BLOB))<=32768 THEN root END, CASE WHEN length(device)<=20 THEN device END, CASE WHEN length(inode)<=20 THEN inode END FROM projects WHERE id=?1",
        rusqlite::params![id, MAX_DOCUMENT_BYTES],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?)),
    ).optional().map_err(db_error)?.ok_or_else(|| CoreError::new("NOT_FOUND", "Registered project does not exist"))?;
    let project: Project =
        serde_json::from_str(&row.0.ok_or_else(limited)?).map_err(|_| changed())?;
    if project.id != id
        || row.1.as_deref() != project.identity.canonical_root.to_str()
        || row.2.as_deref() != Some(project.identity.device.to_string().as_str())
        || row.3.as_deref() != Some(project.identity.inode.to_string().as_str())
    {
        return Err(changed());
    }
    Ok(project)
}
fn load_workspace(db: &Connection, id: &str) -> Result<ManagedWorkspace> {
    let row = db.query_row(
        "SELECT CASE WHEN length(CAST(document AS BLOB)) BETWEEN 1 AND ?2 THEN document END, CASE WHEN length(CAST(root AS BLOB))<=32768 THEN root END, CASE WHEN length(project_id)=36 THEN project_id END, state FROM managed_workspaces WHERE id=?1",
        rusqlite::params![id, MAX_DOCUMENT_BYTES],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?)),
    ).optional().map_err(db_error)?.ok_or_else(|| CoreError::new("NOT_FOUND", "Registered workspace does not exist"))?;
    let workspace: ManagedWorkspace =
        serde_json::from_str(&row.0.ok_or_else(limited)?).map_err(|_| changed())?;
    if workspace.id != id
        || row.1.as_deref() != workspace.root.to_str()
        || row.2.as_deref() != Some(workspace.project_id.as_str())
        || row.3 != workspace.state
    {
        return Err(changed());
    }
    Ok(workspace)
}

impl Store {
    /// Strict Owner read: registration and all directory observations only.
    /// No Git/status/HEAD checks, task assignment, approvals or data writes.
    pub fn workspace_open_context(
        &self,
        selection: &WorkspaceOpenSelection,
    ) -> Result<WorkspaceOpenContext> {
        selection.validate()?;
        let (project_id, expected_project_revision) = selection.project();
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let project = load_project(&tx, project_id)?;
        if project.revision != expected_project_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Project changed; refresh before opening",
            ));
        }
        if project.archived {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Archived project cannot open a new external tool",
            ));
        }
        let identity = match selection {
            WorkspaceOpenSelection::ProjectSource { .. } => project.identity,
            WorkspaceOpenSelection::ManagedWorkspace {
                workspace_id,
                expected_workspace_revision,
                ..
            } => {
                let workspace = load_workspace(&tx, workspace_id)?;
                if workspace.project_id != project_id {
                    return Err(CoreError::new(
                        "PERMISSION_DENIED",
                        "Workspace does not belong to the selected project",
                    ));
                }
                if workspace.revision != *expected_workspace_revision {
                    return Err(CoreError::new(
                        "STALE_REVISION",
                        "Workspace changed; refresh before opening",
                    ));
                }
                if workspace.state != "ready" {
                    return Err(changed());
                }
                let identity = workspace.identity.ok_or_else(changed)?;
                if identity.canonical_root != workspace.root {
                    return Err(changed());
                }
                identity
            }
        };
        let pin = WorkspaceOpenDirectoryPin::capture(&identity.canonical_root)?;
        let leaf = pin.identities.last().ok_or_else(changed)?;
        if leaf.device != identity.device.to_string() || leaf.inode != identity.inode.to_string() {
            return Err(changed());
        }
        let context = WorkspaceOpenContext {
            selection: selection.clone(),
            canonical_path: identity
                .canonical_root
                .to_str()
                .ok_or_else(changed)?
                .to_owned(),
            directory_identity: pin.identities.clone(),
        };
        pin.recheck()?;
        tx.commit().map_err(db_error)?;
        Ok(context)
    }
}

#[cfg(test)]
#[path = "workspace_open_tests.rs"]
mod tests;
