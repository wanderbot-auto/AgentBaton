//! Request-owned, read-only Factory approval/start observations. These types are
//! not Deserialize and cannot be submitted by a Renderer, Provider or MCP token.
use super::{store::*, FactoryScopeBindings, FactoryScopeSelection};
use crate::{
    data_workspaces::{DataArtifactPins, DataInputWitness},
    storage::{db_error, decode, encode, Store},
    CoreError, Result,
};
use agentbaton_workspaces::{
    validation::{ArtifactRequirement, BoundArtifact},
    WorkspaceKind,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(crate) struct FactoryReadControl {
    cancel: Arc<AtomicBool>,
    deadline: Instant,
}
impl FactoryReadControl {
    pub(crate) fn check(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Factory read observation arrived after its original deadline",
            ));
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(CoreError::new(
                "CANCEL_REQUESTED",
                "The owned Factory read was cancelled before admission",
            ));
        }
        Ok(())
    }
}
struct QueryProof {
    sql: &'static str,
    args: Vec<String>,
    rows: Vec<String>,
}
impl QueryProof {
    fn capture(db: &Connection, sql: &'static str, args: Vec<String>) -> Result<Self> {
        Self::capture_bounded(db, sql, args, &mut (1_048_576, 100_000))
    }
    fn capture_bounded(
        db: &Connection,
        sql: &'static str,
        args: Vec<String>,
        remaining: &mut (usize, usize),
    ) -> Result<Self> {
        // Every query is defined in this module, not deserialized/user SQL. Bound
        // raw count/bytes before fetching any complete value into Rust.
        let budget=format!("SELECT count(*),COALESCE(sum(octet_length(value)),0),COALESCE(max(octet_length(value)),0) FROM ({sql})");
        let (count, bytes, largest): (i64, i64, i64) = db
            .query_row(&budget, rusqlite::params_from_iter(&args), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(db_error)?;
        if count > 100_000 || bytes > 1_048_576 || largest > 1_048_576 || count < 0 || bytes < 0 {
            return Err(limited());
        }
        let bytes = usize::try_from(bytes).map_err(|_| limited())?;
        let count = usize::try_from(count).map_err(|_| limited())?;
        if bytes > remaining.0 || count > remaining.1 {
            return Err(limited());
        }
        // Consume shared budget BEFORE materializing the next query's rows.
        remaining.0 -= bytes;
        remaining.1 -= count;
        let mut query = db.prepare(sql).map_err(db_error)?;
        let rows = query
            .query_map(rusqlite::params_from_iter(&args), |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        Ok(Self { sql, args, rows })
    }
    fn recheck(&self, db: &Connection) -> Result<()> {
        if Self::capture(db, self.sql, self.args.clone())?.rows != self.rows {
            return Err(changed());
        }
        Ok(())
    }
}
// Constructed only after the private Goal atomic-reservation phase and exact
// newly inserted intent/session have been checked against their durable origin.
struct OwnedGoalReadDelta {
    project_budget: String,
    root_budget: String,
    root_task: String,
    task: String,
    intent_row: String,
}
impl QueryProof {
    fn recheck_owned_goal(
        &self,
        db: &Connection,
        delta: &OwnedGoalReadDelta,
        remaining: &mut (usize, usize),
    ) -> Result<()> {
        let current = Self::capture_bounded(db, self.sql, self.args.clone(), remaining)?;
        let mut expected = self.rows.clone();
        if self.sql=="SELECT json_array(total_limit,parallel_limit,consumed,active) AS value FROM budgets WHERE scope=?1" && self.args.len()==1 && (self.args[0]==delta.project_budget || self.args[0]==delta.root_budget) {
            if expected.len()!=1{return Err(changed());}
            let old:Vec<u64>=serde_json::from_str(&expected[0]).map_err(|_|changed())?;
            if old.len()!=4 || old[2]>=old[0] || old[3]>=old[1] {return Err(changed());}
            expected[0]=encode(&[old[0],old[1],old[2].checked_add(1).ok_or_else(limited)?,old[3].checked_add(1).ok_or_else(limited)?])?;
        }else if self.sql=="SELECT json_array(d.id,d.state,d.document,a.binding) AS value FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE a.task_id=?1 ORDER BY d.id" && self.args==[delta.root_task.clone()] && delta.task==delta.root_task {
            let id=|row:&str|->Result<String>{let v:serde_json::Value=decode(row)?;Ok(v[0].as_str().ok_or_else(changed)?.to_owned())};
            let added=id(&delta.intent_row)?;
            if expected.iter().map(|r|id(r)).collect::<Result<Vec<_>>>()?.contains(&added){return Err(changed());}
            expected.push(delta.intent_row.clone());
            let mut keyed=expected.into_iter().map(|r|Ok((id(&r)?,r))).collect::<Result<Vec<_>>>()?;
            keyed.sort_by(|a,b|a.0.cmp(&b.0));expected=keyed.into_iter().map(|(_,r)|r).collect();
        }
        if current.rows != expected {
            return Err(changed());
        }
        Ok(())
    }
}
fn owned_goal_delta(
    db: &Connection,
    scope: &super::FactoryScope,
    p: &crate::goal_store::GoalReservationRead<'_>,
) -> Result<OwnedGoalReadDelta> {
    let r = p.run();
    let node = p.node();
    let source:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM factory_child_goal_runs f JOIN goal_runs g ON g.id=f.goal_run_id WHERE f.goal_run_id=?1 AND f.goal_scope_id=?2 AND f.factory_scope_id=?3 AND f.task_id=?4 AND f.root_task_id=?5 AND g.scope_id=?2 AND g.task_id=?4 AND g.project_id=?6 AND json_extract(g.document,'$.root_grant_id')=?7)",rusqlite::params![r.id,r.scope_id,scope.id,r.task_id,scope.root_task_id,scope.project_id,scope.root_grant_id],|row|row.get(0)).map_err(db_error)?;
    let current:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=262144 THEN document END FROM goal_run_nodes WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3 AND state='not_started' AND session_id IS NULL AND attempt_id IS NULL",rusqlite::params![r.id,node.id,i64::try_from(r.plan_revision).map_err(|_|changed())?],|row|row.get(0)).map_err(db_error)?;
    let current: serde_json::Value = decode(&current.ok_or_else(limited)?)?;
    if !source
        || current["node"] != serde_json::json!(node)
        || p.binding().workspace_id != node.workspace_id
        || p.binding().policy_revision
            != format!("{}:{}", scope.root_grant_id, scope.root_grant_revision)
    {
        return Err(changed());
    }
    let raw:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM dispatch_intents WHERE id=?1 AND attempt_id=?2 AND state='prepared' AND executor_instance IS NULL",rusqlite::params![p.intent_id(),p.attempt_id()],|row|row.get(0)).map_err(db_error)?;
    let raw = raw.ok_or_else(limited)?;
    let actual: serde_json::Value = decode(&raw)?;
    let expected = crate::execution::DispatchIntent {
        id: p.intent_id().into(),
        attempt_id: p.attempt_id().into(),
        task_id: r.task_id.clone(),
        root_task_id: scope.root_task_id.clone(),
        state: crate::execution::IntentState::Prepared,
        executor_instance: None,
        binding: p.binding().clone(),
    };
    if actual != serde_json::json!(expected) {
        return Err(changed());
    }
    let attempt:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM attempts WHERE id=?1 AND task_id=?2 AND root_task_id=?3 AND binding=?4) AND NOT EXISTS(SELECT 1 FROM factory_attempts WHERE attempt_id=?1) AND NOT EXISTS(SELECT 1 FROM goal_run_reservations WHERE attempt_id=?1)",rusqlite::params![p.attempt_id(),r.task_id,scope.root_task_id,encode(p.binding())?],|row|row.get(0)).map_err(db_error)?;
    if !attempt {
        return Err(changed());
    }
    let sessions: u32 = db
        .query_row(
            "SELECT count(*) FROM sessions WHERE json_extract(document,'$.attempt_id')=?1",
            [p.attempt_id()],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if let Some(session) = p.session() {
        if sessions != 1
            || !matches!(
                node.kind,
                crate::goal_plan::NodeKind::Work | crate::goal_plan::NodeKind::Summary
            )
            || session.task_id != r.task_id
            || session.project_id != r.project_id
            || session.grant_id != r.root_grant_id
            || session.attempt_id != p.attempt_id()
            || session.intent_id != p.intent_id()
            || session.workspace_id != node.workspace_id
            || session.connection_id != p.binding().connection_id
            || session.state != "starting"
            || session.revision != 1
            || session.turn_state.is_some()
            || session.native_session_id.is_some()
        {
            return Err(changed());
        }
        let raw:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM sessions WHERE id=?1 AND task_id=?2 AND root_grant_id=?3",rusqlite::params![session.id,r.task_id,r.root_grant_id],|row|row.get(0)).map_err(db_error)?;
        if decode::<serde_json::Value>(&raw.ok_or_else(limited)?)? != serde_json::json!(session) {
            return Err(changed());
        }
        let lease:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM workspace_leases WHERE resource=?1 AND holder=?2 AND fencing_token=?3)",rusqlite::params![node.workspace_id,session.id,i64::try_from(session.fencing_token).map_err(|_|changed())?],|row|row.get(0)).map_err(db_error)?;
        if !lease {
            return Err(changed());
        }
    } else if sessions != 0
        || node.kind != crate::goal_plan::NodeKind::Validate
        || !matches!(node.executor, crate::goal_plan::Executor::Validator { .. })
    {
        return Err(changed());
    }
    let intent_row:String=db.query_row("SELECT json_array(d.id,d.state,d.document,a.binding) FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE d.id=?1",[p.intent_id()],|row|row.get(0)).map_err(db_error)?;
    Ok(OwnedGoalReadDelta {
        project_budget: format!("project:{}", scope.project_id),
        root_budget: format!("root:{}", scope.root_task_id),
        root_task: scope.root_task_id.clone(),
        task: r.task_id.clone(),
        intent_row,
    })
}

// This seals the complete observed work tree, not just selected inputs. A
// directory stamp detects entry creation/removal, while each nofollow leaf stat
// detects edits outside the selected set. Captured BEFORE the worker's full Git
// clean/base check; rechecked after it and at the Actor boundary. This is an
// observation, never continuous OS immutability. Ignored entries are included
// conservatively. Large/deep trees fail explicitly rather than being truncated.
const MAX_TREE_ENTRIES: usize = 8192;
const MAX_TREE_DIRECTORIES: usize = 256;
const MAX_TREE_NAME_BYTES: usize = 1_048_576;
#[derive(PartialEq, Eq)]
struct TreeStamp([i128; 11]);
fn tree_stamp(s: &rustix::fs::Stat) -> TreeStamp {
    TreeStamp([
        s.st_dev as i128,
        s.st_ino as i128,
        s.st_mode as i128,
        s.st_uid as i128,
        s.st_gid as i128,
        s.st_nlink as i128,
        s.st_size as i128,
        s.st_mtime as i128,
        s.st_mtime_nsec as i128,
        s.st_ctime as i128,
        s.st_ctime_nsec as i128,
    ])
}
struct TreeDirectory {
    fd: File,
    stamp: TreeStamp,
    depth: usize,
}
struct TreeEntry {
    directory: usize,
    name: std::ffi::OsString,
    stamp: TreeStamp,
}
struct GitRefObservation {
    parent: PathBuf,
    fd: File,
    stamp: TreeStamp,
    refs: Option<GitTreeObservation>,
}
impl GitRefObservation {
    fn capture(
        parent: &Path,
        control: &FactoryReadControl,
        budget: &mut TreeBudget,
    ) -> Result<Self> {
        if budget.directories >= MAX_TREE_DIRECTORIES {
            return Err(limited());
        }
        budget.directories += 1;
        let fd = tree_root(parent)?;
        let stamp = tree_stamp(&rustix::fs::fstat(&fd).map_err(|_| changed())?);
        let refs = match rustix::fs::statat(&fd, "refs", rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(st)
                if rustix::fs::FileType::from_raw_mode(st.st_mode)
                    == rustix::fs::FileType::Directory =>
            {
                Some(GitTreeObservation::capture(
                    &parent.join("refs"),
                    control,
                    budget,
                )?)
            }
            Err(rustix::io::Errno::NOENT) => None,
            _ => return Err(changed()),
        };
        let observed = Self {
            parent: parent.into(),
            fd,
            stamp,
            refs,
        };
        observed.recheck()?;
        Ok(observed)
    }
    fn recheck(&self) -> Result<()> {
        if tree_stamp(&rustix::fs::fstat(tree_root(&self.parent)?).map_err(|_| changed())?)
            != self.stamp
            || tree_stamp(&rustix::fs::fstat(&self.fd).map_err(|_| changed())?) != self.stamp
        {
            return Err(changed());
        }
        match &self.refs {
            Some(tree) => tree.recheck()?,
            None => {
                if !matches!(
                    rustix::fs::statat(&self.fd, "refs", rustix::fs::AtFlags::SYMLINK_NOFOLLOW),
                    Err(rustix::io::Errno::NOENT)
                ) {
                    return Err(changed());
                }
            }
        }
        Ok(())
    }
}
struct GitTreeObservation {
    root: PathBuf,
    directories: Vec<TreeDirectory>,
    entries: Vec<TreeEntry>,
}
#[derive(Default)]
struct TreeBudget {
    entries: usize,
    directories: usize,
    names: usize,
}
fn tree_root(path: &Path) -> Result<File> {
    use rustix::fs::{open, openat, Mode, OFlags};
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let mut fd: File = open("/", flags, Mode::empty())
        .map_err(|_| changed())?
        .into();
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(changed());
    }
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(name) => {
                fd = openat(&fd, name, flags, Mode::empty())
                    .map_err(|_| changed())?
                    .into();
            }
            _ => return Err(changed()),
        }
    }
    Ok(fd)
}
impl GitTreeObservation {
    fn capture(root: &Path, control: &FactoryReadControl, budget: &mut TreeBudget) -> Result<Self> {
        use rustix::fs::{fstat, openat, statat, AtFlags, Dir, FileType, Mode, OFlags};
        use std::os::unix::ffi::OsStrExt;
        control.check()?;
        if budget.directories >= MAX_TREE_DIRECTORIES {
            return Err(limited());
        }
        let fd = tree_root(root)?;
        let root_stat = fstat(&fd).map_err(|_| changed())?;
        budget.directories += 1;
        let mut observation = Self {
            root: root.into(),
            directories: vec![TreeDirectory {
                fd,
                stamp: tree_stamp(&root_stat),
                depth: 0,
            }],
            entries: vec![],
        };
        let mut i = 0;
        while i < observation.directories.len() {
            control.check()?;
            let mut stream =
                Dir::read_from(&observation.directories[i].fd).map_err(|_| changed())?;
            while let Some(entry) = stream.read() {
                control.check()?;
                let entry = entry.map_err(|_| changed())?;
                let name = entry.file_name();
                if name.to_bytes() == b"." || name.to_bytes() == b".." {
                    continue;
                }
                // .git is separately pinned by capture_pins, including its
                // worktree pointer, HEAD/index/config and common metadata.
                if i == 0 && name.to_bytes() == b".git" {
                    continue;
                }
                if budget.entries >= MAX_TREE_ENTRIES
                    || budget
                        .names
                        .checked_add(name.to_bytes().len())
                        .is_none_or(|n| n > MAX_TREE_NAME_BYTES)
                {
                    return Err(limited());
                }
                budget.entries += 1;
                budget.names += name.to_bytes().len();
                let name = std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string();
                let st = statat(
                    &observation.directories[i].fd,
                    &name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| changed())?;
                let kind = FileType::from_raw_mode(st.st_mode);
                if !matches!(
                    kind,
                    FileType::RegularFile | FileType::Directory | FileType::Symlink
                ) || (kind != FileType::Directory && st.st_nlink != 1)
                {
                    return Err(changed());
                }
                if kind == FileType::Directory {
                    if budget.directories >= MAX_TREE_DIRECTORIES
                        || observation.directories[i].depth >= 64
                    {
                        return Err(limited());
                    }
                    let fd: File = openat(
                        &observation.directories[i].fd,
                        &name,
                        OFlags::RDONLY
                            | OFlags::DIRECTORY
                            | OFlags::NOFOLLOW
                            | OFlags::NONBLOCK
                            | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| changed())?
                    .into();
                    if tree_stamp(&fstat(&fd).map_err(|_| changed())?) != tree_stamp(&st) {
                        return Err(changed());
                    }
                    budget.directories += 1;
                    let depth = observation.directories[i].depth + 1;
                    observation.directories.push(TreeDirectory {
                        fd,
                        stamp: tree_stamp(&st),
                        depth,
                    });
                }
                observation.entries.push(TreeEntry {
                    directory: i,
                    name,
                    stamp: tree_stamp(&st),
                });
            }
            i += 1;
        }
        observation.recheck()?;
        Ok(observation)
    }
    fn recheck(&self) -> Result<()> {
        use rustix::fs::{fstat, statat, AtFlags};
        let current = tree_root(&self.root)?;
        if tree_stamp(&fstat(current).map_err(|_| changed())?) != self.directories[0].stamp {
            return Err(changed());
        }
        for directory in &self.directories {
            if tree_stamp(&fstat(&directory.fd).map_err(|_| changed())?) != directory.stamp {
                return Err(changed());
            }
        }
        for entry in &self.entries {
            let st = statat(
                &self.directories[entry.directory].fd,
                &entry.name,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| changed())?;
            if tree_stamp(&st) != entry.stamp {
                return Err(changed());
            }
        }
        // Recheck parent directories after leaf observations to catch changes
        // during this bounded traversal. Kernel stalls remain non-interruptible.
        for directory in &self.directories {
            if tree_stamp(&fstat(&directory.fd).map_err(|_| changed())?) != directory.stamp {
                return Err(changed());
            }
        }
        Ok(())
    }
}
fn git_directory(root: &Path) -> Result<PathBuf> {
    use std::io::Read;
    let path = root.join(".git");
    let m = fs::symlink_metadata(&path).map_err(|_| changed())?;
    if m.is_dir() {
        return path.canonicalize().map_err(|_| changed());
    }
    if !m.is_file() || m.nlink() != 1 {
        return Err(changed());
    }
    let mut text = String::new();
    OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(&path)
        .map_err(|_| changed())?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(|_| changed())?;
    let target = text
        .strip_prefix("gitdir: ")
        .ok_or_else(changed)?
        .trim_end_matches(['\n', '\r']);
    if text.len() > 4096 || target.is_empty() || target.contains(['\0', '\n', '\r']) {
        return Err(changed());
    }
    root.join(target).canonicalize().map_err(|_| changed())
}
fn check_head_reference(root: &Path) -> Result<()> {
    use std::io::Read;
    let mut text = String::new();
    OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(root.join("HEAD"))
        .map_err(|_| changed())?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(|_| changed())?;
    let text = text.trim_end_matches(['\n', '\r']);
    if text.len() > 4096 {
        return Err(changed());
    }
    if let Some(reference) = text.strip_prefix("ref: refs/") {
        if reference.is_empty()
            || reference.contains(['\0', '\n', '\r'])
            || Path::new(reference)
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err(changed());
        }
    } else if !matches!(text.len(), 40 | 64) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(changed());
    }
    Ok(())
}
fn capture_git_trees(
    db: &Connection,
    selection: &FactoryScopeSelection,
    control: &FactoryReadControl,
) -> Result<(Vec<GitTreeObservation>, Vec<GitRefObservation>)> {
    let grant = crate::policy::checked_lineage(db, &selection.grant_id)?;
    let mut budget = TreeBudget::default();
    let mut trees = vec![];
    let mut refs = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for id in &grant.scope.workspace_ids {
        let workspace = read_workspace(db, id)?;
        if let Some(WorkspaceKind::Git { common_dir, .. }) =
            workspace.identity.as_ref().map(|i| &i.kind)
        {
            trees.push(GitTreeObservation::capture(
                &workspace.root,
                control,
                &mut budget,
            )?);
            let gitdir = git_directory(&workspace.root)?;
            check_head_reference(&gitdir)?;
            for parent in [&gitdir, common_dir] {
                if seen.insert(parent.clone()) {
                    refs.push(GitRefObservation::capture(parent, control, &mut budget)?);
                }
            }
        }
    }
    Ok((trees, refs))
}

struct DatabasePin {
    path: PathBuf,
    file: File,
    parent: File,
    device: u64,
    inode: u64,
    parent_device: u64,
    parent_inode: u64,
}
impl DatabasePin {
    fn capture(db: &Connection) -> Result<Self> {
        let path = PathBuf::from(db.path().ok_or_else(changed)?);
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.canonicalize().ok().as_deref() != Some(path.as_path())
        {
            return Err(changed());
        }
        let flags = (rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC)
            .bits() as i32;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(flags | rustix::fs::OFlags::DIRECTORY.bits() as i32)
            .open(path.parent().ok_or_else(changed)?)
            .map_err(|_| changed())?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&path)
            .map_err(|_| changed())?;
        let p = parent.metadata().map_err(|_| changed())?;
        let m = file.metadata().map_err(|_| changed())?;
        if !p.is_dir()
            || p.mode() & 0o7777 != 0o700
            || !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o7777 != 0o600
            || m.uid() != rustix::process::geteuid().as_raw()
            || p.uid() != m.uid()
        {
            return Err(denied());
        }
        Ok(Self {
            path,
            file,
            parent,
            device: m.dev(),
            inode: m.ino(),
            parent_device: p.dev(),
            parent_inode: p.ino(),
        })
    }
    fn open_readonly(&self, deadline: Instant) -> Result<Connection> {
        self.recheck()?;
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(db_error)?;
        connection
            .busy_timeout(
                Duration::from_secs(3).min(deadline.saturating_duration_since(Instant::now())),
            )
            .map_err(db_error)?;
        connection
            .execute_batch("PRAGMA query_only=ON; BEGIN;")
            .map_err(db_error)?;
        Ok(connection)
    }
    fn recheck(&self) -> Result<()> {
        let held = self.file.metadata().map_err(|_| changed())?;
        let p = self.parent.metadata().map_err(|_| changed())?;
        let current = fs::symlink_metadata(&self.path).map_err(|_| changed())?;
        let current_parent =
            fs::symlink_metadata(self.path.parent().ok_or_else(changed)?).map_err(|_| changed())?;
        if !current.is_file()
            || current.nlink() != 1
            || held.nlink() != 1
            || current.mode() & 0o7777 != 0o600
            || !current_parent.is_dir()
            || current_parent.mode() & 0o7777 != 0o700
            || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
            || [
                (&held, self.device, self.inode),
                (&current, self.device, self.inode),
                (&p, self.parent_device, self.parent_inode),
                (&current_parent, self.parent_device, self.parent_inode),
            ]
            .iter()
            .any(|(m, d, i)| {
                m.dev() != *d || m.ino() != *i || m.uid() != rustix::process::geteuid().as_raw()
            })
        {
            return Err(changed());
        }
        Ok(())
    }
}
/// Stop metadata, never an approval/dispatch capability. Cloning it cannot
/// grant reads or allow any method; the request-owned proof is separate.
#[derive(Clone)]
pub(crate) struct FactoryReadWatch {
    task_id: String,
    task_revision: u64,
    project_id: String,
    project_revision: u64,
    grant_id: String,
    grant_revision: u64,
    approval: Option<(String, u64, bool)>,
    child_planning: bool,
}
impl FactoryReadWatch {
    pub(crate) fn current(&self, store: &Store) -> bool {
        let current = (|| -> Result<bool> {
            let task = store.get(&self.task_id)?;
            let project = store.get_project(&self.project_id)?;
            let grant = crate::policy::checked_lineage(&store.conn, &self.grant_id)?;
            if task.revision != self.task_revision
                || (!self.child_planning && task.state != crate::storage::TaskState::Draft)
                || (self.child_planning
                    && matches!(
                        task.state,
                        crate::storage::TaskState::Accepted
                            | crate::storage::TaskState::CancelRequested
                            | crate::storage::TaskState::Cancelled
                    ))
                || project.revision != self.project_revision
                || project.archived
                || grant.revision != self.grant_revision
            {
                return Ok(false);
            }
            if let Some((id, revision, is_scope)) = &self.approval {
                if *is_scope {
                    let scope = scope(&store.conn, id)?;
                    return Ok(!scope.revoked
                        && scope.revision == *revision
                        && scope.expires_at_ms > ms()?
                        && (!self.child_planning
                            || store.check_factory_child_plan_scope(&scope).is_ok()));
                }
                let raw:Option<(Option<String>,bool)>=store.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,result IS NOT NULL FROM approval_proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
                let Some((Some(raw), false)) = raw else {
                    return Ok(false);
                };
                let proposal: crate::sessions::ApprovalProposal = decode(&raw)?;
                return Ok(proposal.kind == "factory"
                    && !proposal.consumed
                    && proposal.revision == *revision
                    && proposal.expires_at_ms > ms()?);
            }
            Ok(true)
        })();
        current.unwrap_or(false)
    }
    pub(crate) fn scope_id(&self) -> Option<&str> {
        self.approval
            .as_ref()
            .filter(|(_, _, is_scope)| *is_scope)
            .map(|(id, _, _)| id.as_str())
    }
}
pub(crate) struct FactoryReadPlan {
    watch: FactoryReadWatch,
    child_planning: Option<super::FactoryScope>,
    selection: FactoryScopeSelection,
    companion: PathBuf,
    database: DatabasePin,
    proofs: Vec<QueryProof>,
    request_hash: String,
}
struct CommittedGoalRead {
    fingerprint: serde_json::Value,
    queries: Vec<QueryProof>,
    consumed: bool,
}
impl CommittedGoalRead {
    fn verify_queries(
        &self,
        db: &Connection,
        originals: &[QueryProof],
        fingerprint: &serde_json::Value,
    ) -> Result<()> {
        if self.consumed
            || self.fingerprint != *fingerprint
            || self.queries.len() != originals.len()
        {
            return Err(denied());
        }
        let mut remaining = (4_194_304, 100_000);
        for (captured, original) in self.queries.iter().zip(originals) {
            if captured.sql != original.sql || captured.args != original.args {
                return Err(changed());
            }
            let current = QueryProof::capture_bounded(
                db,
                captured.sql,
                captured.args.clone(),
                &mut remaining,
            )?;
            if current.rows != captured.rows {
                return Err(changed());
            }
        }
        Ok(())
    }
}
pub(crate) struct FactoryReadWitness {
    committed: std::cell::RefCell<Option<CommittedGoalRead>>,
    child_planning: bool,
    selection: FactoryScopeSelection,
    companion: PathBuf,
    database: DatabasePin,
    proofs: Vec<QueryProof>,
    bindings: FactoryScopeBindings,
    pins: Vec<DataArtifactPins>,
    git_trees: Vec<GitTreeObservation>,
    request_hash: String,
    deadline: Instant,
    git_refs: Vec<GitRefObservation>,
}
fn projection(
    db: &Connection,
    selection: &FactoryScopeSelection,
    approval: Option<&str>,
) -> Result<Vec<QueryProof>> {
    let grant = crate::policy::checked_lineage(db, &selection.grant_id)?;
    limits(selection, &grant)?;
    let task = Store::factory_read_task(db, &selection.root_task_id)?;
    let mut proofs = Vec::new();
    let mut remaining = (4_194_304, 100_000);
    let mut add = |sql, args| -> Result<()> {
        proofs.push(QueryProof::capture_bounded(db, sql, args, &mut remaining)?);
        Ok(())
    };
    add(
        "SELECT document AS value FROM tasks WHERE id=?1",
        vec![task.id.clone()],
    )?;
    add(
        "SELECT document AS value FROM projects WHERE id=?1",
        vec![task.project_id.clone()],
    )?;
    add(
        "SELECT document AS value FROM execution_grants WHERE id=?1",
        vec![grant.id.clone()],
    )?;
    for id in &grant.scope.connection_ids {
        add(
            "SELECT document AS value FROM connections WHERE id=?1",
            vec![id.clone()],
        )?;
    }
    for id in &grant.scope.workspace_ids {
        add(
            "SELECT document AS value FROM managed_workspaces WHERE id=?1",
            vec![id.clone()],
        )?;
    }
    if let Some(id) = approval {
        add(
            "SELECT document AS value FROM approval_proposals WHERE id=?1",
            vec![id.into()],
        )?;
        add(
            "SELECT COALESCE(result,'null') AS value FROM approval_proposals WHERE id=?1",
            vec![id.into()],
        )?;
        add(
            "SELECT document AS value FROM factory_scopes WHERE id=?1",
            vec![id.into()],
        )?;
        add("SELECT json_array(revision,revoked,owner_sequence) AS value FROM factory_scopes WHERE id=?1",vec![id.into()])?;
    }
    for scope in [
        format!("project:{}", task.project_id),
        format!("root:{}", task.id),
    ] {
        add("SELECT json_array(total_limit,parallel_limit,consumed,active) AS value FROM budgets WHERE scope=?1",vec![scope])?;
    }
    for scope in [
        format!("project:{}", task.project_id),
        format!("root-grant:{}", grant.root_grant_id),
    ] {
        add(
            "SELECT document AS value FROM goal_ledgers WHERE scope=?1",
            vec![scope],
        )?;
    }
    add(
        "SELECT document AS value FROM factory_scopes WHERE root_task_id=?1 ORDER BY id",
        vec![task.id.clone()],
    )?;
    add("SELECT json_array(id,revision,revoked,owner_sequence) AS value FROM factory_scopes WHERE root_task_id=?1 ORDER BY id",vec![task.id.clone()])?;
    add(
        "SELECT document AS value FROM factory_runs WHERE root_task_id=?1 ORDER BY id",
        vec![task.id.clone()],
    )?;
    add(
        "SELECT document AS value FROM factory_task_lineage WHERE task_id=?1",
        vec![task.id.clone()],
    )?;
    add("SELECT json_array(d.id,d.state,d.document,a.binding) AS value FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE a.task_id=?1 ORDER BY d.id",vec![task.id])?;
    add("SELECT document AS value FROM factory_budget_reservations WHERE project_id=?1 OR json_extract(document,'$.root_grant_id')=?2 ORDER BY attempt_id",vec![task.project_id.clone(),grant.root_grant_id.clone()])?;
    add("SELECT x.document AS value FROM goal_run_reservations x JOIN goal_runs r ON r.id=x.run_id WHERE r.project_id=?1 OR json_extract(r.document,'$.root_grant_id')=?2 ORDER BY x.attempt_id",vec![task.project_id,grant.root_grant_id])?;
    if let Some(policy) = &selection.child_planning_policy {
        for output in &policy.outputs {
            for reference in &output.validators {
                let (sql,approval,receipt) = match reference.kind {
                    crate::goal_store::GoalValidatorKind::Integrity => (
                        "SELECT document AS value FROM validation_policies WHERE id=?1",
                        "SELECT json_array(document,COALESCE(result,'null')) AS value FROM approval_proposals WHERE id=(SELECT json_extract(document,'$.approval_id') FROM validation_policies WHERE id=?1)",
                        "SELECT json_array(sequence,document) AS value FROM events WHERE json_extract(document,'$.aggregate_id')=(SELECT json_extract(document,'$.approval_id') FROM validation_policies WHERE id=?1) ORDER BY sequence"),
                    crate::goal_store::GoalValidatorKind::Data => (
                        "SELECT document AS value FROM data_validation_policies WHERE id=?1",
                        "SELECT json_array(document,COALESCE(result,'null')) AS value FROM approval_proposals WHERE id=(SELECT json_extract(document,'$.approval_id') FROM data_validation_policies WHERE id=?1)",
                        "SELECT json_array(sequence,document) AS value FROM events WHERE json_extract(document,'$.aggregate_id')=(SELECT json_extract(document,'$.approval_id') FROM data_validation_policies WHERE id=?1) ORDER BY sequence"),
                    crate::goal_store::GoalValidatorKind::Command => (
                        "SELECT document AS value FROM command_validation_policies WHERE id=?1",
                        "SELECT json_array(document,COALESCE(result,'null')) AS value FROM approval_proposals WHERE id=(SELECT json_extract(document,'$.approval_id') FROM command_validation_policies WHERE id=?1)",
                        "SELECT json_array(sequence,document) AS value FROM events WHERE json_extract(document,'$.aggregate_id')=(SELECT json_extract(document,'$.approval_id') FROM command_validation_policies WHERE id=?1) ORDER BY sequence"),
                };
                for query in [sql, approval, receipt] {
                    add(query, vec![reference.policy_id.clone()])?;
                }
                if reference.kind == crate::goal_store::GoalValidatorKind::Command {
                    add("SELECT json_array(id,state,document) AS value FROM command_validation_runs WHERE policy_id=?1 ORDER BY id",vec![reference.policy_id.clone()])?;
                }
            }
        }
    }
    Ok(proofs)
}
impl Store {
    pub(crate) fn factory_proposal_replay(
        &self,
        selection: &FactoryScopeSelection,
        key_id: &str,
    ) -> Result<bool> {
        Ok(
            replay::<serde_json::Value>(
                &self.conn,
                "factory:propose",
                key_id,
                &encode(selection)?,
            )?
            .is_some(),
        )
    }
    fn factory_read_task(db: &Connection, id: &str) -> Result<crate::storage::Task> {
        let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM tasks WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        decode(&row.flatten().ok_or_else(changed)?)
    }
    pub(crate) fn prepare_factory_owner_read(
        &self,
        method: &str,
        params: &serde_json::Value,
        revision: Option<u64>,
        key_id: Option<&str>,
        raw: &str,
        companion: Option<PathBuf>,
    ) -> Result<Option<FactoryReadPlan>> {
        if matches!(
            method,
            "factory_child_plans.context" | "factory_child_plans.submit"
        ) {
            let id = params
                .get("scope_id")
                .and_then(|v| v.as_str())
                .ok_or_else(invalid)?;
            let target = params
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(invalid)?;
            let scope = scope(&self.conn, id)?;
            if method == "factory_child_plans.submit" {
                let update: super::FactoryChildPlanUpdate =
                    serde_json::from_value(params.clone()).map_err(|_| invalid())?;
                if self.factory_child_plan_owner_replay(
                    &update,
                    revision.ok_or_else(invalid)?,
                    key_id.ok_or_else(invalid)?,
                )? {
                    return Ok(None);
                }
            }
            let run = self.get_factory_run(id)?;
            if !run.tasks.iter().any(|t| t.task_id == target) {
                return Err(denied());
            }
            // Historical contexts project only stored metadata. They grant no
            // new write and never revalidate an expired scoped credential.
            if method == "factory_child_plans.context"
                && (scope.revoked
                    || scope.expires_at_ms <= ms()?
                    || matches!(
                        run.run.state.as_str(),
                        "cancelled" | "blocked" | "outcome_unknown" | "completed"
                    ))
            {
                return Ok(None);
            }
            return self.prepare_factory_child_plan_read(&scope, raw).map(Some);
        }
        let mut pinned_companion = None;
        let (selection, approval) = match method {
            "factory_scopes.propose" => {
                let selection: FactoryScopeSelection =
                    serde_json::from_value(params.clone()).map_err(|_| invalid())?;
                let key_id = key_id.ok_or_else(invalid)?;
                if replay::<serde_json::Value>(
                    &self.conn,
                    "factory:propose",
                    key_id,
                    &encode(&selection)?,
                )?
                .is_some()
                {
                    return Ok(None);
                }
                (selection, None)
            }
            "factory_runs.start" => {
                let id = params
                    .get("scope_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(invalid)?;
                if self
                    .factory_start_replay(
                        id,
                        revision.ok_or_else(invalid)?,
                        key_id.ok_or_else(invalid)?,
                    )?
                    .is_some()
                {
                    return Ok(None);
                }
                let scope = scope(&self.conn, id)?;
                // Exact-key history above stays read-only even after authority
                // changes. A NEW stale request must fail before projection, FD
                // lookup, observation allocation or occupying the I/O worker.
                if scope.revision != revision.ok_or_else(invalid)? {
                    return Err(CoreError::new(
                        "STALE_REVISION",
                        "Factory scope changed before start",
                    ));
                }
                // Start uses the exact native-approved immutable installation,
                // never silently substitutes this daemon's adjacent executable.
                pinned_companion = Some(scope.bindings.companion.executable.clone());
                (scope.selection, Some(scope.id))
            }
            "approvals.finalize" => {
                let id = params
                    .get("proposal_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(invalid)?;
                let row:Option<(Option<String>,bool)>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,result IS NOT NULL FROM approval_proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
                let Some((document, done)) = row else {
                    return Ok(None);
                };
                if done {
                    return Ok(None);
                }
                let proposal: crate::sessions::ApprovalProposal =
                    decode(&document.ok_or_else(limited)?)?;
                if proposal.kind != "factory" {
                    return Ok(None);
                }
                if proposal.revision != revision.ok_or_else(invalid)?
                    || proposal.consumed
                    || proposal.expires_at_ms <= ms()?
                {
                    return Err(denied());
                }
                (
                    serde_json::from_value(proposal.snapshot["selection"].clone())
                        .map_err(|_| invalid())?,
                    Some(proposal.id),
                )
            }
            _ => return Ok(None),
        };
        let proofs = projection(&self.conn, &selection, approval.as_deref())?;
        let companion = match pinned_companion.or(companion) {
            Some(path) => path,
            None => crate::factory::companion_path()?,
        };
        let database = DatabasePin::capture(&self.conn)?;
        let task = self.get(&selection.root_task_id)?;
        let project = self.get_project(&task.project_id)?;
        let watch = FactoryReadWatch {
            task_id: task.id,
            task_revision: task.revision,
            project_id: project.id,
            project_revision: project.revision,
            grant_id: selection.grant_id.clone(),
            grant_revision: selection.expected_grant_revision,
            child_planning: false,
            approval: approval
                .map(|id| (id, revision.unwrap_or(0), method == "factory_runs.start")),
        };
        Ok(Some(FactoryReadPlan {
            watch,
            child_planning: None,
            selection,
            companion,
            database,
            proofs,
            request_hash: agentbaton_workspaces::validation::sha256(raw.as_bytes()),
        }))
    }
}
impl Store {
    pub(crate) fn prepare_factory_child_plan_read(
        &self,
        scope: &super::FactoryScope,
        raw: &str,
    ) -> Result<FactoryReadPlan> {
        self.check_factory_child_plan_scope(scope)?;
        let selection = scope.selection.clone();
        let task = self.get(&scope.root_task_id)?;
        let project = self.get_project(&task.project_id)?;
        Ok(FactoryReadPlan {
            watch: FactoryReadWatch {
                task_id: task.id,
                task_revision: task.revision,
                project_id: project.id,
                project_revision: project.revision,
                grant_id: scope.root_grant_id.clone(),
                grant_revision: scope.root_grant_revision,
                approval: Some((scope.id.clone(), scope.revision, true)),
                child_planning: true,
            },
            child_planning: Some(scope.clone()),
            selection: selection.clone(),
            companion: scope.bindings.companion.executable.clone(),
            database: DatabasePin::capture(&self.conn)?,
            proofs: projection(&self.conn, &selection, Some(&scope.id))?,
            request_hash: agentbaton_workspaces::validation::sha256(raw.as_bytes()),
        })
    }
    fn observe_factory_child_plan_inputs(
        &self,
        scope: &super::FactoryScope,
    ) -> Result<FactoryScopeBindings> {
        self.check_factory_child_plan_scope(scope)?;
        let control = self.factory_read_control.as_ref().ok_or_else(denied)?;
        for input in &scope.bindings.inputs {
            control.check()?;
            let bytes = agentbaton_workspaces::read_artifact(
                &scope.bindings.workspaces[0].root,
                &input.contract.requirement.relative_path,
                input.contract.requirement.max_bytes,
            )
            .map_err(|_| changed())?;
            if bytes.len() as u64 != input.bytes
                || agentbaton_workspaces::validation::sha256(&bytes) != input.sha256
            {
                return Err(changed());
            }
        }
        if companion_controlled(&scope.bindings.companion.executable, Some(control))?
            != scope.bindings.companion
        {
            return Err(changed());
        }
        self.check_factory_child_plan_bindings(&scope.selection, &scope.bindings)?;
        control.check()?;
        Ok(scope.bindings.clone())
    }
}

impl FactoryReadPlan {
    pub(crate) fn is_child_planning(&self) -> bool {
        self.child_planning.is_some()
    }
    pub(crate) fn watch(&self) -> FactoryReadWatch {
        self.watch.clone()
    }
    pub(crate) fn execute(
        self,
        witness: DataInputWitness,
        cancel: Arc<AtomicBool>,
        deadline: Instant,
    ) -> Result<DataInputWitness> {
        let control = FactoryReadControl { cancel, deadline };
        control.check()?;
        self.database.recheck()?;
        let connection = self.database.open_readonly(deadline)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version != 15 {
            return Err(changed());
        }
        // Match the live Actor's exact approved business rows BEFORE filesystem
        // reads, preventing an independently replaced readonly DB granting reads.
        #[cfg(debug_assertions)]
        let diagnostic_query = crate::ipc::goal_diagnostics::hashing_enabled().then(Instant::now);
        for proof in &self.proofs {
            control.check()?;
            proof.recheck(&connection)?;
        }
        #[cfg(debug_assertions)]
        if let Some(clock) = diagnostic_query {
            crate::ipc::goal_diagnostics::emit(
                "factory_query_check",
                Some(clock.elapsed()),
                [false; 6],
                self.proofs.len(),
                None,
            );
        }
        let mut reader = Store {
            conn: connection,
            data_input: Some(witness),
            graph_input: None,
            graph_capture_clocks: Default::default(),
            factory_read_control: Some(control),
        };
        let workspace = read_workspace(&reader.conn, &self.selection.workspace_ids[0])?;
        let (pins, required) =
            capture_pins(&reader.conn, &self.selection, &workspace, &self.companion)?;
        #[cfg(debug_assertions)]
        let diagnostic_git = crate::ipc::goal_diagnostics::hashing_enabled().then(Instant::now);
        let (git_trees, git_refs) = capture_git_trees(
            &reader.conn,
            &self.selection,
            reader.factory_read_control.as_ref().unwrap(),
        )?;
        #[cfg(debug_assertions)]
        if let Some(clock) = diagnostic_git {
            crate::ipc::goal_diagnostics::emit(
                "factory_git_snapshot",
                Some(clock.elapsed()),
                [false; 6],
                git_trees.len() + git_refs.len(),
                None,
            );
        }
        let bindings = if let Some(scope) = &self.child_planning {
            reader.observe_factory_child_plan_inputs(scope)?
        } else {
            reader.resolve_factory_bindings(&self.selection, &self.companion)?
        };
        for pin in &pins {
            pin.verify(false)?;
        }
        required.verify(true)?;
        for tree in &git_trees {
            tree.recheck()?;
        }
        for refs in &git_refs {
            refs.recheck()?;
        }
        let mut pins = pins;
        pins.push(required);
        reader.factory_read_control.as_ref().unwrap().check()?;
        self.database.recheck()?;
        reader.conn.execute_batch("ROLLBACK;").map_err(db_error)?;
        let mut witness = reader.data_input.take().ok_or_else(changed)?;
        witness.factory = Some(Box::new(FactoryReadWitness {
            committed: std::cell::RefCell::new(None),
            child_planning: self.child_planning.is_some(),
            selection: self.selection,
            companion: self.companion,
            database: self.database,
            proofs: self.proofs,
            bindings,
            pins,
            git_trees,
            git_refs,
            request_hash: self.request_hash,
            deadline,
        }));
        Ok(witness)
    }
}
impl FactoryReadWitness {
    /// Derived Graph admission consumes the same request-owned, already joined
    /// Factory I/O proof. Only bounded business rows and FD/stat/Git refs are
    /// rechecked on the Actor; no input bytes or executable are rehashed here.
    pub(crate) fn recheck_child_goal(
        &self,
        db: &rusqlite::Connection,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
    ) -> Result<()> {
        self.recheck_child_goal_phase(db, witness, scope, None)
    }
    pub(crate) fn recheck_child_goal_reservation(
        &self,
        db: &Connection,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
        phase: &crate::goal_store::GoalReservationRead<'_>,
    ) -> Result<()> {
        let delta = owned_goal_delta(db, scope, phase)?;
        self.recheck_child_goal_phase(db, witness, scope, Some(&delta))
    }
    fn recheck_child_goal_phase(
        &self,
        db: &Connection,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
        delta: Option<&OwnedGoalReadDelta>,
    ) -> Result<()> {
        self.check_child_goal_header(witness, scope)?;
        let mut remaining = (4_194_304, 100_000);
        for proof in &self.proofs {
            if let Some(delta) = delta {
                proof.recheck_owned_goal(db, delta, &mut remaining)?;
            } else {
                proof.recheck(db)?;
            }
        }
        self.check_child_goal_inputs(db, scope)
    }
    fn check_child_goal_header(
        &self,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
    ) -> Result<()> {
        if !self.child_planning
            || Instant::now() >= self.deadline
            || self.selection != scope.selection
            || self.companion != scope.bindings.companion.executable
            || !witness.for_hash(&self.request_hash)
            || serde_json::to_value(&self.bindings).map_err(|_| changed())?
                != serde_json::to_value(&scope.bindings).map_err(|_| changed())?
        {
            return Err(changed());
        }
        self.database.recheck()?;
        Ok(())
    }
    fn check_child_goal_inputs(&self, db: &Connection, scope: &super::FactoryScope) -> Result<()> {
        for pin in &self.pins {
            pin.verify(false)?;
        }
        for tree in &self.git_trees {
            tree.recheck()?;
        }
        for refs in &self.git_refs {
            refs.recheck()?;
        }
        crate::policy::observe_grant_bindings(db, &scope.bindings.approved_grant)?;
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "The original joined Factory input observation expired before child Goal admission",
            ));
        }
        Ok(())
    }
    pub(crate) fn request_hash(&self) -> &str {
        &self.request_hash
    }
    pub(crate) fn seal_child_goal_commit(
        &self,
        db: &Connection,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
        proof: &crate::goal_store::GoalCommittedRead,
    ) -> Result<()> {
        // Only the legitimate admission transaction's final exit can capture;
        // later readonly/public paths cannot rebase a changed business snapshot.
        if db.is_autocommit() {
            return Err(denied());
        }
        self.check_child_goal_header(witness, scope)?;
        let mut slot = self.committed.try_borrow_mut().map_err(|_| changed())?;
        if slot.is_some() {
            return Err(denied());
        }
        let mut remaining = (4_194_304, 100_000);
        let mut queries = Vec::with_capacity(self.proofs.len());
        for original in &self.proofs {
            queries.push(QueryProof::capture_bounded(
                db,
                original.sql,
                original.args.clone(),
                &mut remaining,
            )?);
        }
        self.check_child_goal_inputs(db, scope)?;
        *slot = Some(CommittedGoalRead {
            fingerprint: proof.fingerprint().clone(),
            queries,
            consumed: false,
        });
        Ok(())
    }
    pub(crate) fn consume_child_goal_commit(
        &self,
        db: &Connection,
        witness: &crate::data_workspaces::DataInputWitness,
        scope: &super::FactoryScope,
        proof: &crate::goal_store::GoalCommittedRead,
    ) -> Result<()> {
        self.check_child_goal_header(witness, scope)?;
        let mut slot = self.committed.try_borrow_mut().map_err(|_| changed())?;
        let post = slot.as_mut().ok_or_else(denied)?;
        post.verify_queries(db, &self.proofs, proof.fingerprint())?;
        self.check_child_goal_inputs(db, scope)?;
        post.consumed = true;
        Ok(())
    }
    pub(crate) fn is_child_planning(&self) -> bool {
        self.child_planning
    }
    pub(super) fn recheck(
        &self,
        store: &Store,
        selection: &FactoryScopeSelection,
        companion: &Path,
    ) -> Result<FactoryScopeBindings> {
        if Instant::now() >= self.deadline
            || self.selection != *selection
            || self.companion != companion
            || !store
                .data_input
                .as_ref()
                .is_some_and(|w| w.for_hash(&self.request_hash))
        {
            return Err(changed());
        }
        self.database.recheck()?;
        for proof in &self.proofs {
            proof.recheck(&store.conn)?;
        }
        for pins in &self.pins {
            pins.verify(false)?;
        }
        for tree in &self.git_trees {
            tree.recheck()?;
        }
        for refs in &self.git_refs {
            refs.recheck()?;
        }
        if self.child_planning {
            store.check_factory_child_plan_bindings(selection, &self.bindings)?;
        } else {
            check_admission_db(&store.conn, &self.bindings)?;
        }
        crate::policy::observe_grant_bindings(&store.conn, &self.bindings.approved_grant)?;
        for id in &self.bindings.approved_grant.scope.workspace_ids {
            let w = read_workspace(&store.conn, id)?;
            if matches!(
                w.identity.as_ref().map(|i| &i.kind),
                Some(WorkspaceKind::Directory)
            ) {
                store.require_data_input(&w)?;
            }
        }
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Original Factory observation expired during bounded metadata admission",
            ));
        }
        Ok(self.bindings.clone())
    }
}
fn read_workspace(db: &Connection, id: &str) -> Result<crate::registry::ManagedWorkspace> {
    let raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM managed_workspaces WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
    decode(&raw.flatten().ok_or_else(changed)?)
}
fn capture_pins(
    db: &Connection,
    selection: &FactoryScopeSelection,
    workspace: &crate::registry::ManagedWorkspace,
    path: &Path,
) -> Result<(Vec<DataArtifactPins>, DataArtifactPins)> {
    use std::io::Read;
    let mut required = selection
        .inputs
        .iter()
        .map(|input| {
            BoundArtifact::from_approved_root(&workspace.root, input.clone())
                .map(|b| b.contract_view())
                .map_err(|_| changed())
        })
        .collect::<Result<Vec<_>>>()?;
    // This is a metadata-only pin for the separately bounded 128MiB executable,
    // not an artifact read or a widening of BoundArtifact's 64MiB file limit.
    let image_root = path.parent().ok_or_else(changed)?;
    if image_root.canonicalize().ok().as_deref() != Some(image_root) {
        return Err(changed());
    }
    let root_metadata = fs::metadata(image_root).map_err(|_| changed())?;
    required.push(agentbaton_workspaces::validation::ArtifactContractView {
        canonical_root: image_root.into(),
        root_device: root_metadata.dev(),
        root_inode: root_metadata.ino(),
        requirement: ArtifactRequirement {
            artifact_id: "factory-main-image".into(),
            relative_path: path.file_name().ok_or_else(changed)?.into(),
            max_bytes: 128 * 1024 * 1024,
            expected_sha256: None,
        },
    });
    let required = DataArtifactPins::capture(&required)?;
    let grant = crate::policy::checked_lineage(db, &selection.grant_id)?;
    let mut state = Vec::new();
    let mut add = |root: &Path, relative: &Path| -> Result<()> {
        match fs::symlink_metadata(root.join(relative)) {
            Ok(m) if !m.is_file() || m.nlink() != 1 => return Err(changed()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(changed()),
        }
        let requirement = ArtifactRequirement {
            artifact_id: format!("factory-git-state-{}", state.len()),
            relative_path: relative.into(),
            max_bytes: 16 * 1024 * 1024,
            expected_sha256: None,
        };
        state.push(
            BoundArtifact::from_approved_root(root, requirement)
                .map_err(|_| changed())?
                .contract_view(),
        );
        Ok(())
    };
    for id in &grant.scope.workspace_ids {
        let workspace = read_workspace(db, id)?;
        let Some(identity) = workspace.identity else {
            return Err(changed());
        };
        let WorkspaceKind::Git { common_dir, .. } = identity.kind else {
            continue;
        };
        let dotgit = workspace.root.join(".git");
        let metadata = fs::symlink_metadata(&dotgit).map_err(|_| changed())?;
        let gitdir = if metadata.is_dir() {
            dotgit.canonicalize().map_err(|_| changed())?
        } else {
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(changed());
            }
            add(&workspace.root, Path::new(".git"))?;
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(
                    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
                )
                .open(&dotgit)
                .map_err(|_| changed())?;
            let mut text = String::new();
            file.take(4097)
                .read_to_string(&mut text)
                .map_err(|_| changed())?;
            if text.len() > 4096 {
                return Err(changed());
            }
            let target = text
                .strip_prefix("gitdir: ")
                .ok_or_else(changed)?
                .trim_end_matches(['\n', '\r']);
            if target.is_empty() || target.contains(['\0', '\n', '\r']) {
                return Err(changed());
            }
            workspace
                .root
                .join(target)
                .canonicalize()
                .map_err(|_| changed())?
        };
        for root in [&gitdir, &common_dir] {
            for relative in [
                "HEAD",
                "index",
                "config",
                "config.worktree",
                "commondir",
                "gitdir",
                "packed-refs",
                "info/sparse-checkout",
                "info/attributes",
                "objects/info/alternates",
            ] {
                add(root, Path::new(relative))?;
            }
        }
    }
    if state.len() > 512 {
        return Err(limited());
    }
    let pins = state
        .chunks(64)
        .map(DataArtifactPins::capture)
        .collect::<Result<Vec<_>>>()?;
    Ok((pins, required))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{factory::FactoryCapabilities, goal_plan::BudgetLimits};
    use agentbaton_config::runtime::{RuntimePlan, RuntimeProvider};
    use std::{os::unix::fs::PermissionsExt, process::Command};
    struct Fixture {
        _temp: tempfile::TempDir,
        store: Store,
        selection: FactoryScopeSelection,
        companion: PathBuf,
        raw: String,
        input: PathBuf,
    }
    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "user.name=Generated fixture",
                "-c",
                "user.email=fixture@invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-C",
            ])
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::Builder::new()
                .permissions(fs::Permissions::from_mode(0o700))
                .tempdir()
                .unwrap();
            let base = temp.path().canonicalize().unwrap();
            let database = base.join("baton.sqlite3");
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&database)
                    .unwrap(),
            );
            let mut store = Store::open(&database).unwrap();
            let source = base.join("source");
            fs::create_dir(&source).unwrap();
            git(&source, &["init", "--initial-branch=main"]);
            fs::write(source.join("input"), b"generated fixed data").unwrap();
            fs::write(source.join("unselected"), b"approved other bytes").unwrap();
            git(&source, &["add", "input", "unselected"]);
            git(&source, &["commit", "-m", "generated"]);
            let head = git(&source, &["rev-parse", "HEAD"]);
            let project = store
                .import_project(&source, "Generated readonly Factory fixture", "project")
                .unwrap();
            let managed = base.join("managed");
            fs::create_dir(&managed).unwrap();
            fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
            let workspace = store
                .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
                .unwrap();
            let companion = base.join("generated-not-executed-companion");
            fs::write(
                &companion,
                b"Only hashed; no child process in this unit fixture\n",
            )
            .unwrap();
            fs::set_permissions(&companion, fs::Permissions::from_mode(0o700)).unwrap();
            // Explicit synthetic ready metadata. This is not a Provider probe or
            // native OS dialog; real generated ACP/MCP tests are separate.
            let connection = serde_json::json!({"id":"fixture-connection","name":"Synthetic not executed","provider":"custom_acp","protocol_route":"custom_acp","executable":companion,"arguments":[],"executable_sha256":agentbaton_workspaces::validation::sha256(&fs::read(&companion).unwrap()),"environment_id":"fixture-environment","identity_ref":"unknown","host_id":"local","config_revision":"fixture-config","runtime_plan":RuntimePlan::empty(RuntimeProvider::CustomAcp),"probe_state":"ready","revision":1,"approval_id":"synthetic-no-provider","capability_snapshot_id":"fixture-capability","identity_status":"unknown","config_status":"baton_managed","created_at":"2026-10-28T00:00:00Z","version_evidence":null});
            store
                .conn
                .execute(
                    "INSERT INTO connections(id,document) VALUES('fixture-connection',?1)",
                    [connection.to_string()],
                )
                .unwrap();
            let task = store
                .create_registered(
                    &project.id,
                    "Read only the fixed generated artifact",
                    "task",
                )
                .unwrap();
            let grant = store
                .create_grant(
                    &project.id,
                    &workspace.id,
                    "fixture-connection",
                    4,
                    1,
                    30_000,
                    true,
                    "trusted-host-fixture-grant",
                )
                .unwrap();
            let budget = BudgetLimits {
                attempts: 4,
                parallel: 1,
                runtime_ms: 30_000,
                output_bytes: 4096,
                children: 1,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            };
            let selection = FactoryScopeSelection {
                child_planning_policy: None,
                child_goal_execution_policy: None,
                root_task_id: task.id,
                expected_task_revision: task.revision,
                grant_id: grant.id,
                expected_grant_revision: grant.revision,
                connection_ids: vec!["fixture-connection".into()],
                workspace_ids: vec![workspace.id],
                inputs: vec![ArtifactRequirement {
                    artifact_id: "input".into(),
                    relative_path: "input".into(),
                    max_bytes: 1024,
                    expected_sha256: Some(agentbaton_workspaces::validation::sha256(
                        b"generated fixed data",
                    )),
                }],
                outputs: vec![ArtifactRequirement {
                    artifact_id: "output".into(),
                    relative_path: "output".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                }],
                recipients: vec![],
                max_depth: 1,
                max_children: 1,
                max_total_nodes: 2,
                root_budget: budget.clone(),
                project_budget: budget,
                ttl_ms: 30_000,
                capabilities: FactoryCapabilities {
                    delegate_task: true,
                    resume_parent_as_new_attempt: true,
                    child_planning: false,
                    child_goal_execution: false,
                    create_workspace: false,
                },
            };
            Self {
                _temp: temp,
                store,
                selection,
                companion,
                raw: "generated exact Owner request nonce A".into(),
                input: workspace.root.join("input"),
            }
        }
        fn plan(&self) -> FactoryReadPlan {
            self.store
                .prepare_factory_owner_read(
                    "factory_scopes.propose",
                    &serde_json::to_value(&self.selection).unwrap(),
                    None,
                    Some("fixture-read"),
                    &self.raw,
                    Some(self.companion.clone()),
                )
                .unwrap()
                .unwrap()
        }
        fn observe(&self, plan: FactoryReadPlan) -> DataInputWitness {
            let deadline = Instant::now() + Duration::from_secs(30);
            let witness = DataInputWitness::restore(
                vec![],
                agentbaton_workspaces::validation::sha256(self.raw.as_bytes()),
                deadline,
            )
            .unwrap();
            plan.execute(witness, Arc::new(AtomicBool::new(false)), deadline)
                .unwrap()
        }
    }
    #[test]
    fn readonly_worker_preserves_rows_until_actor_commits_the_exact_proposal() {
        let mut f = Fixture::new();
        let seq = f.store.sequence().unwrap();
        let witness = f.observe(f.plan());
        assert!(witness.for_request(&f.raw));
        assert!(!witness.for_request("different raw nonce"));
        assert_eq!(f.store.sequence().unwrap(), seq);
        f.store.data_input = Some(witness);
        let p = f
            .store
            .propose_factory_scope(&f.selection, "fixture-read", &f.companion)
            .unwrap();
        f.store.data_input = None;
        assert_eq!(p.kind, "factory");
        assert_eq!(
            f.store
                .factory_record_counts(&f.selection.root_task_id)
                .unwrap()
                .scopes,
            0
        );
        fs::remove_file(&f.companion).unwrap();
        assert!(
            f.store
                .prepare_factory_owner_read(
                    "factory_scopes.propose",
                    &serde_json::to_value(&f.selection).unwrap(),
                    None,
                    Some("fixture-read"),
                    "new nonce historical same key",
                    Some(f.companion.clone())
                )
                .unwrap()
                .is_none(),
            "historical reply must require neither image nor new hash"
        );
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "fixture-read", &f.companion)
                .unwrap()
                .id,
            p.id
        );
    }
    #[test]
    fn wrong_owner_start_revision_never_prepares_an_observation_or_reads_the_program() {
        let mut f = Fixture::new();
        let p = f
            .store
            .propose_factory_scope(&f.selection, "cas-scope", &f.companion)
            .unwrap();
        let scope: crate::factory::FactoryScope =
            serde_json::from_value(f.store.finalize_proposal(&p.id, p.revision).unwrap()).unwrap();
        fs::remove_file(&f.companion).unwrap();
        let seq = f.store.sequence().unwrap();
        let error = f
            .store
            .prepare_factory_owner_read(
                "factory_runs.start",
                &serde_json::json!({"scope_id":scope.id}),
                Some(scope.revision + 1),
                Some("must-not-prepare-reader"),
                "wrong-cas-original-raw",
                None,
            )
            .err()
            .expect("wrong CAS must return an error, not a read plan");
        assert_eq!(error.code, "STALE_REVISION");
        assert_eq!(error.side_effect_state, "not_started");
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(
            f.store
                .factory_record_counts(&scope.root_task_id)
                .unwrap()
                .runs,
            0
        );
    }
    #[test]
    fn readonly_query_only_connection_refuses_real_writes_without_mutating_schema_or_journal() {
        let f = Fixture::new();
        let plan = f.plan();
        let sequence = f.store.sequence().unwrap();
        let readonly = plan
            .database
            .open_readonly(Instant::now() + Duration::from_secs(30))
            .unwrap();
        assert!(readonly.is_readonly(rusqlite::MAIN_DB).unwrap());
        assert_eq!(
            readonly
                .pragma_query_value(None, "query_only", |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(readonly
            .execute("INSERT INTO events(document) VALUES('{}')", [])
            .is_err());
        assert!(readonly.execute_batch("PRAGMA user_version=15;").is_err());
        assert!(readonly
            .execute_batch("CREATE TABLE cannot_create(value TEXT);")
            .is_err());
        assert_eq!(f.store.sequence().unwrap(), sequence);
        assert_eq!(
            f.store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            14
        );
    }
    #[test]
    fn actor_business_rows_are_matched_before_any_file_read_on_the_readonly_connection() {
        let f = Fixture::new();
        let plan = f.plan();
        f.store.conn.execute("UPDATE tasks SET document=json_set(document,'$.goal','changed before worker') WHERE id=?1",[&f.selection.root_task_id]).unwrap();
        fs::remove_file(&f.companion).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let witness = DataInputWitness::restore(
            vec![],
            agentbaton_workspaces::validation::sha256(f.raw.as_bytes()),
            deadline,
        )
        .unwrap();
        assert_eq!(
            plan.execute(witness, Arc::new(AtomicBool::new(false)), deadline)
                .err()
                .unwrap()
                .code,
            "INPUT_CHANGED",
            "changed rows must refuse before missing companion lookup"
        );
    }
    #[test]
    fn actor_rechecks_connection_config_after_worker_observation_without_hashing_again() {
        let mut f = Fixture::new();
        let witness = f.observe(f.plan());
        f.store.conn.execute("UPDATE connections SET document=json_set(document,'$.config_revision','unapproved') WHERE id='fixture-connection'",[]).unwrap();
        f.store.data_input = Some(witness);
        let seq = f.store.sequence().unwrap();
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "deny-current-config", &f.companion)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    #[test]
    fn actor_rejects_unselected_tracked_change_after_clean_worker_observation() {
        let mut f = Fixture::new();
        let witness = f.observe(f.plan());
        fs::write(
            f.input.parent().unwrap().join("unselected"),
            b"different other bytes",
        )
        .unwrap();
        let seq = f.store.sequence().unwrap();
        f.store.data_input = Some(witness);
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "deny-unselected", &f.companion)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    #[test]
    fn actor_rejects_new_untracked_after_clean_worker_observation() {
        let mut f = Fixture::new();
        let witness = f.observe(f.plan());
        fs::write(
            f.input.parent().unwrap().join("unapproved"),
            b"generated untracked bytes",
        )
        .unwrap();
        let seq = f.store.sequence().unwrap();
        f.store.data_input = Some(witness);
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "deny-untracked", &f.companion)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    #[test]
    fn actor_rejects_same_bytes_replaced_input_after_worker_observation() {
        let mut f = Fixture::new();
        let witness = f.observe(f.plan());
        let bytes = fs::read(&f.input).unwrap();
        fs::remove_file(&f.input).unwrap();
        fs::write(&f.input, bytes).unwrap();
        f.store.data_input = Some(witness);
        let seq = f.store.sequence().unwrap();
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "deny-replaced-input", &f.companion)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    #[test]
    fn actor_rejects_loose_head_reference_drift_after_clean_worker_observation() {
        let mut f = Fixture::new();
        let root = f.input.parent().unwrap();
        let head = git(root, &["rev-parse", "HEAD"]);
        git(
            root,
            &["symbolic-ref", "HEAD", "refs/heads/generated-factory-ref"],
        );
        git(
            root,
            &["update-ref", "refs/heads/generated-factory-ref", &head],
        );
        let witness = f.observe(f.plan());
        // A second raw commit with identical tree does not touch working-tree
        // files or index. Only the loose HEAD target changes after observation.
        let tree = git(root, &["rev-parse", "HEAD^{tree}"]);
        let replacement = git(
            root,
            &[
                "commit-tree",
                &tree,
                "-p",
                &head,
                "-m",
                "generated alternate",
            ],
        );
        git(
            root,
            &[
                "update-ref",
                "refs/heads/generated-factory-ref",
                &replacement,
            ],
        );
        let seq = f.store.sequence().unwrap();
        f.store.data_input = Some(witness);
        assert_eq!(
            f.store
                .propose_factory_scope(&f.selection, "deny-ref-drift", &f.companion)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    #[test]
    fn clean_observation_covers_nested_entries_symlink_replacement_and_ignored_files() {
        for mode in ["nested", "symlink", "ignored"] {
            let mut f = Fixture::new();
            let root = f.input.parent().unwrap();
            fs::create_dir(root.join("nested")).unwrap();
            fs::write(root.join("nested/tracked"), b"approved nested").unwrap();
            fs::write(root.join(".gitignore"), b"ignored-fixture\n").unwrap();
            git(root, &["add", "nested/tracked", ".gitignore"]);
            git(root, &["commit", "-m", "generated nested fixture"]);
            // Refresh the synthetic fixture's explicitly approved base so the
            // actual production full Git admission still runs before mutation.
            let head = git(root, &["rev-parse", "HEAD"]);
            f.store.conn.execute("UPDATE managed_workspaces SET document=json_set(document,'$.base_revision',?1,'$.identity.kind.head',?1) WHERE id=?2",rusqlite::params![head,&f.selection.workspace_ids[0]]).unwrap();
            f.store
                .conn
                .execute(
                    "UPDATE execution_grants SET document=json_set(document,?1,?2) WHERE id=?3",
                    rusqlite::params![
                        format!(
                            "$.workspace_bindings.{}.kind.head",
                            f.selection.workspace_ids[0]
                        ),
                        head,
                        &f.selection.grant_id
                    ],
                )
                .unwrap();
            let witness = f.observe(f.plan());
            match mode {
                "nested" => {
                    fs::write(root.join("nested/new"), b"new nested untracked").unwrap();
                }
                "symlink" => {
                    fs::remove_file(root.join("unselected")).unwrap();
                    std::os::unix::fs::symlink("input", root.join("unselected")).unwrap();
                }
                _ => {
                    fs::write(
                        root.join("ignored-fixture"),
                        b"conservatively observed even if ignored",
                    )
                    .unwrap();
                }
            }
            let seq = f.store.sequence().unwrap();
            f.store.data_input = Some(witness);
            assert_eq!(
                f.store
                    .propose_factory_scope(&f.selection, "deny-tree-drift", &f.companion)
                    .unwrap_err()
                    .code,
                "INPUT_CHANGED",
                "{mode}"
            );
            assert_eq!(f.store.sequence().unwrap(), seq);
        }
    }
    #[test]
    fn tree_and_aggregate_query_budgets_fail_explicitly_before_extending_observations() {
        let f = Fixture::new();
        let control = FactoryReadControl {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let mut budget = TreeBudget {
            entries: MAX_TREE_ENTRIES,
            directories: 0,
            names: 0,
        };
        assert_eq!(
            GitTreeObservation::capture(f.input.parent().unwrap(), &control, &mut budget)
                .err()
                .unwrap()
                .code,
            "RESOURCE_LIMIT"
        );
        let mut remaining = (4_194_304, 100_000);
        let sql = "SELECT hex(zeroblob(262144)) AS value";
        for _ in 0..8 {
            QueryProof::capture_bounded(&f.store.conn, sql, vec![], &mut remaining).unwrap();
        }
        assert_eq!(remaining.0, 0);
        assert_eq!(
            QueryProof::capture_bounded(&f.store.conn, sql, vec![], &mut remaining)
                .err()
                .unwrap()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(remaining.0, 0);
    }
    #[test]
    fn cancellation_and_original_deadline_refuse_before_readonly_database_or_image_open() {
        for cancel in [false, true] {
            let f = Fixture::new();
            let plan = f.plan();
            fs::remove_file(&f.companion).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let witness = DataInputWitness::restore(
                vec![],
                agentbaton_workspaces::validation::sha256(f.raw.as_bytes()),
                deadline,
            )
            .unwrap();
            let execution_deadline = if cancel {
                deadline
            } else {
                Instant::now() - Duration::from_millis(1)
            };
            assert_eq!(
                plan.execute(
                    witness,
                    Arc::new(AtomicBool::new(cancel)),
                    execution_deadline
                )
                .err()
                .unwrap()
                .code,
                if cancel {
                    "CANCEL_REQUESTED"
                } else {
                    "DEADLINE_EXCEEDED"
                }
            );
        }
    }
    #[test]
    fn private_database_replacement_is_rejected_before_readonly_connection() {
        let f = Fixture::new();
        let plan = f.plan();
        let old = PathBuf::from(f.store.conn.path().unwrap());
        fs::rename(&old, old.with_extension("retained-original")).unwrap();
        drop(
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&old)
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        let witness = DataInputWitness::restore(
            vec![],
            agentbaton_workspaces::validation::sha256(f.raw.as_bytes()),
            deadline,
        )
        .unwrap();
        assert_eq!(
            plan.execute(witness, Arc::new(AtomicBool::new(false)), deadline)
                .err()
                .unwrap()
                .code,
            "INPUT_CHANGED"
        );
    }
    mod child_planning_tests {
        include!("factory_child_plan_tests.rs");
    }
}

#[cfg(test)]
mod child_goal_admission_tests {
    use super::*;
    const COUNTERS:&str="SELECT json_array(total_limit,parallel_limit,consumed,active) AS value FROM budgets WHERE scope=?1";
    const INTENTS:&str="SELECT json_array(d.id,d.state,d.document,a.binding) AS value FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE a.task_id=?1 ORDER BY d.id";
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE budgets(scope TEXT PRIMARY KEY,total_limit INTEGER,parallel_limit INTEGER,consumed INTEGER,active INTEGER);INSERT INTO budgets VALUES('project:p',8,4,1,0),('root:r',8,4,1,0),('root:foreign',8,4,0,0);CREATE TABLE attempts(id TEXT PRIMARY KEY,task_id TEXT,binding TEXT);CREATE TABLE dispatch_intents(id TEXT PRIMARY KEY,attempt_id TEXT,state TEXT,document TEXT);").unwrap();
        db
    }
    fn delta() -> OwnedGoalReadDelta {
        OwnedGoalReadDelta {
            project_budget: "project:p".into(),
            root_budget: "root:r".into(),
            root_task: "r".into(),
            task: "r".into(),
            intent_row: "[\"own\",\"prepared\",\"fixed intent bytes\",\"fixed binding bytes\"]"
                .into(),
        }
    }
    fn check(p: &QueryProof, db: &Connection, d: &OwnedGoalReadDelta) -> Result<()> {
        p.recheck_owned_goal(db, d, &mut (4_194_304, 100_000))
    }
    #[test]
    fn exact_own_counter_delta_retains_limits_and_rejects_extra_missing_or_double_charge() {
        // This is a real SQLite readset projection unit, not an authority token
        // or owned process test. Production phase construction has separate
        // exact Goal/node/intent/session/lease checks before this helper.
        let db = db();
        let d = delta();
        for key in [&d.project_budget, &d.root_budget] {
            let p = QueryProof::capture(&db, COUNTERS, vec![key.clone()]).unwrap();
            assert!(
                check(&p, &db, &d).is_err(),
                "missing own increment must refuse"
            );
            db.execute(
                "UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1",
                [key],
            )
            .unwrap();
            check(&p, &db, &d).unwrap();
            db.execute("SAVEPOINT corrupt", []).unwrap();
            db.execute(
                "UPDATE budgets SET consumed=consumed+1 WHERE scope=?1",
                [key],
            )
            .unwrap();
            assert!(
                check(&p, &db, &d).is_err(),
                "extra/double charge must refuse"
            );
            db.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt;")
                .unwrap();
            db.execute("SAVEPOINT corrupt", []).unwrap();
            db.execute(
                "UPDATE budgets SET total_limit=total_limit+1 WHERE scope=?1",
                [key],
            )
            .unwrap();
            assert!(
                check(&p, &db, &d).is_err(),
                "unreviewed ceiling must refuse"
            );
            db.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt;")
                .unwrap();
            check(&p, &db, &d).unwrap();
        }
    }
    #[test]
    fn an_unrelated_root_never_gets_the_owned_counter_exception() {
        let db = db();
        let p = QueryProof::capture(&db, COUNTERS, vec!["root:foreign".into()]).unwrap();
        check(&p, &db, &delta()).unwrap();
        db.execute(
            "UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope='root:foreign'",
            [],
        )
        .unwrap();
        assert!(check(&p, &db, &delta()).is_err());
    }
    #[test]
    fn exact_pending_intent_and_root_target_dedup_reject_foreign_binding_missing_and_repeat() {
        let db = db();
        let p = QueryProof::capture(&db, INTENTS, vec!["r".into()]).unwrap();
        let d = delta();
        assert!(check(&p, &db, &d).is_err(), "missing own pending intent");
        db.execute(
            "INSERT INTO attempts VALUES('a','r','fixed binding bytes')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO dispatch_intents VALUES('own','a','prepared','fixed intent bytes')",
            [],
        )
        .unwrap();
        check(&p, &db, &d).unwrap();
        db.execute("SAVEPOINT corrupt", []).unwrap();
        db.execute("UPDATE attempts SET binding='foreign binding bytes'", [])
            .unwrap();
        assert!(check(&p, &db, &d).is_err());
        db.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt;")
            .unwrap();
        db.execute("SAVEPOINT corrupt", []).unwrap();
        db.execute(
            "INSERT INTO attempts VALUES('b','r','fixed binding bytes')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO dispatch_intents VALUES('foreign','b','prepared','fixed intent bytes')",
            [],
        )
        .unwrap();
        assert!(check(&p, &db, &d).is_err());
        db.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt;")
            .unwrap();
        let captured_twice = QueryProof::capture(&db, INTENTS, vec!["r".into()]).unwrap();
        assert!(
            check(&captured_twice, &db, &d).is_err(),
            "already observed intent cannot be added twice"
        );
        let mut descendant = d;
        descendant.task = "child".into();
        assert!(
            check(&p, &db, &descendant).is_err(),
            "child admission cannot change root's intent history"
        );
    }

    #[test]
    fn committed_snapshot_preserves_all_queries_and_rejects_later_budget_or_metadata_changes() {
        // Real SQLite snapshot/projection unit only; opaque Goal token creation
        // and original FD/input/expiry checks are separate production gates.
        let db = db();
        db.execute_batch("CREATE TABLE metadata(id TEXT PRIMARY KEY,document TEXT);INSERT INTO metadata VALUES('scope','{\"revoked\":false,\"task_revision\":1}');").unwrap();
        let meta = "SELECT document AS value FROM metadata WHERE id=?1";
        let originals = vec![
            QueryProof::capture(&db, COUNTERS, vec!["project:p".into()]).unwrap(),
            QueryProof::capture(&db, COUNTERS, vec!["root:r".into()]).unwrap(),
            QueryProof::capture(&db, meta, vec!["scope".into()]).unwrap(),
        ];
        db.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope IN('project:p','root:r')",[]).unwrap();
        let capture = || {
            originals
                .iter()
                .map(|q| QueryProof::capture(&db, q.sql, q.args.clone()).unwrap())
                .collect::<Vec<_>>()
        };
        let fingerprint = serde_json::json!({"session":"own","attempt":"a","intent":"i","request":"fixed-sha","fence":2,"sequence":9});
        let post = CommittedGoalRead {
            fingerprint: fingerprint.clone(),
            queries: capture(),
            consumed: false,
        };
        post.verify_queries(&db, &originals, &fingerprint).unwrap();
        assert_ne!(
            post.queries[0].rows, originals[0].rows,
            "the captured exact own committed budget is distinct from the pre-admission snapshot"
        );
        for sql in [
            "UPDATE budgets SET consumed=consumed+1 WHERE scope='project:p'",
            "UPDATE budgets SET active=active+1 WHERE scope='root:r'",
            "UPDATE metadata SET document='{\"revoked\":true,\"task_revision\":1}'",
            "UPDATE metadata SET document='{\"revoked\":false,\"task_revision\":2}'",
        ] {
            db.execute_batch("SAVEPOINT corrupt").unwrap();
            db.execute(sql, []).unwrap();
            assert!(
                post.verify_queries(&db, &originals, &fingerprint).is_err(),
                "{sql}"
            );
            db.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt;")
                .unwrap();
        }
        post.verify_queries(&db, &originals, &fingerprint).unwrap();
        let partial = CommittedGoalRead {
            fingerprint: fingerprint.clone(),
            queries: capture().into_iter().take(2).collect(),
            consumed: false,
        };
        assert!(
            partial
                .verify_queries(&db, &originals, &fingerprint)
                .is_err(),
            "no query may be silently dropped"
        );
    }
    #[test]
    fn committed_snapshot_cannot_be_rebound_reconsumed_or_substitute_other_query_arguments() {
        let db = db();
        let original = QueryProof::capture(&db, COUNTERS, vec!["root:r".into()]).unwrap();
        let fingerprint = serde_json::json!({"session":"own","attempt":"a","intent":"i","request":"fixed-sha","grant":"g","run":"r","node":"n","fence":2,"sequence":9});
        let mut post = CommittedGoalRead {
            fingerprint: fingerprint.clone(),
            queries: vec![QueryProof::capture(&db, COUNTERS, vec!["root:r".into()]).unwrap()],
            consumed: false,
        };
        let originals = [original];
        post.verify_queries(&db, &originals, &fingerprint).unwrap();
        for field in [
            "session", "attempt", "intent", "request", "grant", "run", "node", "fence", "sequence",
        ] {
            let mut changed = fingerprint.clone();
            changed[field] = serde_json::json!("foreign");
            assert!(
                post.verify_queries(&db, &originals, &changed).is_err(),
                "{field}"
            );
        }
        post.queries[0].args[0] = "root:foreign".into();
        assert!(post.verify_queries(&db, &originals, &fingerprint).is_err());
        post.queries[0].args[0] = "root:r".into();
        post.consumed = true;
        assert!(
            post.verify_queries(&db, &originals, &fingerprint).is_err(),
            "a committed proof is consumed only once"
        );
    }
}
