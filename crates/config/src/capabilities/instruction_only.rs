//! Explicit one-file Codex instruction source. Text remains untrusted data;
//! reading it never executes instructions or grants tools/permissions.
use super::{directory_names, parse_skill, ParseStatus, SkillDialect};
use crate::{error, io_error, sha256, ConfigError, ErrorCode};
use rustix::fs::{self as rfs, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, Metadata},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};
type Result<T> = std::result::Result<T, ConfigError>;
pub const INSTRUCTION_ONLY_SKILL_SOURCE: &str = "baton-managed-codex-skill";
pub const INSTRUCTION_ONLY_SKILL_MAX_BYTES: usize = 32_768;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstructionOnlySkillMetadata {
    pub directory_name: String,
    pub name: String,
    pub description: String,
    pub sha256: String,
    pub root_device: u64,
    pub root_inode: u64,
    pub file_device: u64,
    pub file_inode: u64,
    /// Hash of root/file identity, ownership, mode, size and change timestamps.
    pub source_identity: String,
}
/// Deliberately neither Debug nor Serialize: callers explicitly access content.
pub struct InstructionOnlySkill {
    path: PathBuf,
    metadata: InstructionOnlySkillMetadata,
    instruction: String,
}
fn leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    name: String,
    description: String,
}
pub fn instruction_only_skill_metadata(
    directory_name: &str,
    instruction: &str,
) -> Result<(String, String)> {
    if instruction.len() > INSTRUCTION_ONLY_SKILL_MAX_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    if !leaf(directory_name) || instruction.contains("!`") || instruction.contains('\0') {
        return Err(error(ErrorCode::Conflict));
    }
    let start = instruction
        .strip_prefix("---\n")
        .or_else(|| instruction.strip_prefix("---\r\n"))
        .ok_or_else(|| error(ErrorCode::Conflict))?;
    let end = start
        .match_indices("\n---")
        .find_map(|(i, _)| {
            let trailing = &start[i + 4..];
            (trailing.is_empty() || trailing.starts_with('\n') || trailing.starts_with("\r\n"))
                .then_some(i)
        })
        .ok_or_else(|| error(ErrorCode::Conflict))?;
    // Closed scalar-only metadata additionally rejects fields the historical
    // discovery parser can preview but never authorizes for this profile.
    let header: Header =
        serde_yaml_ng::from_str(&start[..end]).map_err(|_| error(ErrorCode::Conflict))?;
    if header.name.trim().is_empty() || header.description.trim().is_empty() {
        return Err(error(ErrorCode::Conflict));
    }
    let parsed = parse_skill(directory_name, instruction.as_bytes(), SkillDialect::Codex);
    if parsed.status != ParseStatus::Parsed
        || parsed.dynamic_context_present
        || !parsed.unknown_frontmatter_names.is_empty()
        || parsed.compatibility_hint.is_some()
        || !parsed.dependency_hints.is_empty()
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok((
        parsed.name.ok_or_else(|| error(ErrorCode::Conflict))?,
        parsed
            .description
            .ok_or_else(|| error(ErrorCode::Conflict))?,
    ))
}
fn stamp(meta: &Metadata) -> (u64, u64, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.mode(),
        meta.uid(),
        meta.nlink(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}
fn selected_directory(path: &Path) -> Result<File> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || io_error(path.canonicalize())? != path
    {
        return Err(error(ErrorCode::InvalidPath));
    }
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let mut directory = File::from(io_error(rfs::open("/", flags, Mode::empty()))?);
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = File::from(io_error(rfs::openat(
                    &directory,
                    name,
                    flags,
                    Mode::empty(),
                ))?)
            }
            _ => return Err(error(ErrorCode::InvalidPath)),
        }
    }
    let meta = io_error(directory.metadata())?;
    if meta.uid() != rustix::process::geteuid().as_raw() || meta.mode() & 0o022 != 0 {
        return Err(error(ErrorCode::PermissionDenied));
    }
    Ok(directory)
}
impl InstructionOnlySkill {
    pub fn read_selected(path: &Path) -> Result<Self> {
        let directory_name = path
            .file_name()
            .and_then(|v| v.to_str())
            .filter(|v| leaf(v))
            .ok_or_else(|| error(ErrorCode::InvalidPath))?
            .to_owned();
        let directory = selected_directory(path)?;
        let root_before = io_error(directory.metadata())?;
        if directory_names(&directory, 2)? != ["SKILL.md"] {
            return Err(error(ErrorCode::Conflict));
        }
        let mut file = File::from(io_error(rfs::openat(
            &directory,
            "SKILL.md",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ))?);
        let before = io_error(file.metadata())?;
        if !before.is_file()
            || before.uid() != rustix::process::geteuid().as_raw()
            || before.nlink() != 1
            || before.mode() & 0o022 != 0
        {
            return Err(error(ErrorCode::PermissionDenied));
        }
        if before.len() > INSTRUCTION_ONLY_SKILL_MAX_BYTES as u64 {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let mut bytes = Vec::new();
        io_error(
            (&mut file)
                .take((INSTRUCTION_ONLY_SKILL_MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes),
        )?;
        if bytes.len() > INSTRUCTION_ONLY_SKILL_MAX_BYTES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let instruction = String::from_utf8(bytes).map_err(|_| error(ErrorCode::Conflict))?;
        let (name, description) = instruction_only_skill_metadata(&directory_name, &instruction)?;
        let current = selected_directory(path)?;
        if stamp(&before) != stamp(&io_error(file.metadata())?)
            || stamp(&root_before) != stamp(&io_error(current.metadata())?)
            || directory_names(&current, 2)? != ["SKILL.md"]
        {
            return Err(error(ErrorCode::Conflict));
        }
        let current_file = File::from(io_error(rfs::openat(
            &current,
            "SKILL.md",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ))?);
        if stamp(&io_error(current_file.metadata())?) != stamp(&before) {
            return Err(error(ErrorCode::Conflict));
        }
        let source_identity = sha256(
            &serde_json::to_vec(&(stamp(&root_before), stamp(&before)))
                .map_err(|_| error(ErrorCode::Conflict))?,
        );
        Ok(Self {
            path: path.into(),
            metadata: InstructionOnlySkillMetadata {
                directory_name,
                name,
                description,
                sha256: sha256(instruction.as_bytes()),
                root_device: root_before.dev(),
                root_inode: root_before.ino(),
                file_device: before.dev(),
                file_inode: before.ino(),
                source_identity,
            },
            instruction,
        })
    }
    pub fn metadata(&self) -> &InstructionOnlySkillMetadata {
        &self.metadata
    }
    pub fn instruction(&self) -> &str {
        &self.instruction
    }
    pub fn verify_unchanged(&self) -> Result<()> {
        let current = Self::read_selected(&self.path)?;
        if current.metadata != self.metadata || current.instruction != self.instruction {
            return Err(error(ErrorCode::Conflict));
        }
        Ok(())
    }
}
