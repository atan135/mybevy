//! Isolated filesystem boundaries for the closed-loop UI runner.
//!
//! A run is allowed to create a draft staging directory or a detached Git
//! worktree. Neither mode copies the caller's uncommitted files. The module
//! deliberately has no cleanup operation for a run root: cancellation and a
//! process crash preserve evidence, while expiring lock files are reclaimed
//! one file at a time after identity checks.

use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Mutex, MutexGuard, TryLockError,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};

pub const WORKSPACE_ISOLATION_PROTOCOL_VERSION: u32 = 1;
const MAX_SNAPSHOT_FILES: usize = 16_384;
const MAX_SNAPSHOT_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SNAPSHOT_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LOCK_FILE_BYTES: u64 = 16 * 1024;
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(25);
static LEASE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static LOCAL_RECLAIM_GUARD: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolatedWorkspaceMode {
    DraftStaging,
    CodeWorktree,
}

impl IsolatedWorkspaceMode {
    fn directory_name(self) -> &'static str {
        match self {
            Self::DraftStaging => "staging",
            Self::CodeWorktree => "worktree",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceWorktreeSnapshot {
    pub source_commit: String,
    pub is_dirty: bool,
    pub porcelain_v1_sha256: String,
    pub porcelain_entry_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFileSnapshot {
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceTreeSnapshot {
    pub captured_at_unix_ms: u64,
    pub files: BTreeMap<String, WorkspaceFileSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFileChange {
    pub relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<WorkspaceFileSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<WorkspaceFileSnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFileDiff {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<WorkspaceFileChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modified: Vec<WorkspaceFileChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<WorkspaceFileChange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIterationSnapshot {
    pub before: WorkspaceTreeSnapshot,
    pub after: WorkspaceTreeSnapshot,
    pub diff: WorkspaceFileDiff,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLockRecoveryPolicy {
    pub stale_after_ms: u64,
    pub cancellation_preserves_workspace: bool,
    pub process_crash_reclaims_expired_locks: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLeaseRecord {
    pub target: String,
    pub lease_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIsolationRecord {
    pub protocol_version: u32,
    pub mode: IsolatedWorkspaceMode,
    pub source: SourceWorktreeSnapshot,
    pub workspace_relative_path: String,
    pub allowed_modification_roots: Vec<String>,
    pub lock_targets: Vec<String>,
    pub lock_leases: Vec<WorkspaceLeaseRecord>,
    pub lock_recovery: WorkspaceLockRecoveryPolicy,
    pub initial_snapshot: WorkspaceTreeSnapshot,
}

impl WorkspaceIsolationRecord {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.protocol_version != WORKSPACE_ISOLATION_PROTOCOL_VERSION
            || !is_commit_id(&self.source.source_commit)
            || !is_sha256(&self.source.porcelain_v1_sha256)
            || self.workspace_relative_path != self.mode.directory_name()
            || self.allowed_modification_roots.is_empty()
            || self.lock_targets.is_empty()
            || self.lock_targets.len() != self.lock_leases.len()
            || self.lock_recovery.stale_after_ms == 0
            || !self.lock_recovery.cancellation_preserves_workspace
            || !self.lock_recovery.process_crash_reclaims_expired_locks
        {
            return Err(workspace_failure(
                TaskFailureKind::ManifestCorrupt,
                "workspace isolation record is incomplete or incompatible",
                None,
            ));
        }
        validate_allowed_roots(&self.allowed_modification_roots)?;
        validate_lock_targets(&self.lock_targets)?;
        let lease_targets: Vec<_> = self
            .lock_leases
            .iter()
            .map(|lease| lease.target.clone())
            .collect();
        validate_lock_targets(&lease_targets)?;
        if self
            .lock_leases
            .iter()
            .zip(&self.lock_targets)
            .any(|(lease, target)| lease.target != *target || !is_sha256(&lease.lease_id))
        {
            return Err(workspace_failure(
                TaskFailureKind::ManifestCorrupt,
                "workspace isolation record has invalid lock lease identities",
                None,
            ));
        }
        validate_snapshot(&self.initial_snapshot)
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceIsolationOptions {
    pub mode: IsolatedWorkspaceMode,
    pub allowed_modification_roots: Vec<String>,
    pub lock_targets: Vec<String>,
    pub lock_timeout: Duration,
    pub stale_lock_after: Duration,
}

impl WorkspaceIsolationOptions {
    pub fn draft(allowed_modification_roots: Vec<String>, lock_targets: Vec<String>) -> Self {
        Self {
            mode: IsolatedWorkspaceMode::DraftStaging,
            allowed_modification_roots,
            lock_targets,
            lock_timeout: Duration::from_secs(30),
            stale_lock_after: Duration::from_secs(10 * 60),
        }
    }
}

/// A live isolated workspace. Dropping it releases only locks owned by this
/// run. The workspace itself is intentionally retained for recovery/audit.
#[derive(Debug)]
pub struct IsolatedWorkspace {
    repository_root: PathBuf,
    run_root: PathBuf,
    workspace_root: PathBuf,
    allowed_modification_roots: Vec<String>,
    record: WorkspaceIsolationRecord,
    locks: Vec<WorkspaceLock>,
}

impl IsolatedWorkspace {
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub fn run_root(&self) -> &Path {
        &self.run_root
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn record(&self) -> &WorkspaceIsolationRecord {
        &self.record
    }

    /// Resolves a write target without creating it. The value must stay under
    /// an explicitly allowed root and no existing component may be a link.
    pub fn resolve_output_path(&self, relative: &Path) -> Result<PathBuf, TaskFailure> {
        let relative_text = normal_relative_path(relative)?;
        if !self
            .allowed_modification_roots
            .iter()
            .any(|root| path_is_within_root(&relative_text, root))
        {
            return Err(workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace output is outside its allowed modification roots",
                Some(relative.display().to_string()),
            ));
        }
        resolve_contained_path(&self.workspace_root, relative)
    }

    pub fn capture_before_iteration(
        &self,
        captured_at_unix_ms: u64,
    ) -> Result<WorkspaceTreeSnapshot, TaskFailure> {
        capture_workspace_tree(
            &self.workspace_root,
            &self.allowed_modification_roots,
            captured_at_unix_ms,
        )
    }

    pub fn capture_after_iteration(
        &self,
        before: WorkspaceTreeSnapshot,
        captured_at_unix_ms: u64,
    ) -> Result<WorkspaceIterationSnapshot, TaskFailure> {
        let after = capture_workspace_tree(
            &self.workspace_root,
            &self.allowed_modification_roots,
            captured_at_unix_ms,
        )?;
        Ok(WorkspaceIterationSnapshot {
            diff: diff_workspace_trees(&before, &after),
            before,
            after,
        })
    }

    /// Normal cancellation releases owned locks but preserves every staging
    /// file/worktree and both snapshots for a future recovery attempt.
    pub fn release_locks(&mut self) {
        self.locks.clear();
    }

    /// Extends every currently owned lease. The runner must call this at
    /// bounded long-running external-call boundaries; a lease that has already
    /// expired is not revived, because another process may have recovered it.
    pub fn refresh_locks(&mut self, now_unix_ms: u64) -> Result<(), TaskFailure> {
        for lock in &mut self.locks {
            lock.refresh(now_unix_ms)?;
        }
        Ok(())
    }
}

/// Creates a new no-clobber staging directory or detached worktree. The Git
/// commands here are intentionally limited to read-only inspection and
/// `worktree add` targeting the new run directory; this API never invokes
/// reset, clean, checkout, restore, or worktree removal on the caller tree.
pub fn prepare_isolated_workspace(
    repository_root: &Path,
    run_root: &Path,
    run_id: &str,
    options: WorkspaceIsolationOptions,
    now_unix_ms: u64,
) -> Result<IsolatedWorkspace, TaskFailure> {
    crate::directory::RunId::parse(run_id)?;
    validate_allowed_roots(&options.allowed_modification_roots)?;
    validate_lock_targets(&options.lock_targets)?;
    if options.stale_lock_after.is_zero() {
        return Err(workspace_failure(
            TaskFailureKind::InvalidInput,
            "workspace lock stale duration must be positive",
            None,
        ));
    }

    let repository_root =
        canonical_regular_directory(repository_root, "workspace repository root")?;
    let run_root = canonical_regular_directory(run_root, "workspace run root")?;
    if !run_root.starts_with(&repository_root) {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "workspace run root escapes the repository",
            Some(run_root.display().to_string()),
        ));
    }
    let source = capture_source_worktree(&repository_root)?;
    let locks = acquire_workspace_locks(
        &repository_root,
        run_id,
        &options.lock_targets,
        options.lock_timeout,
        options.stale_lock_after,
        now_unix_ms,
    )?;

    let workspace_root = run_root.join(options.mode.directory_name());
    match options.mode {
        IsolatedWorkspaceMode::DraftStaging => {
            create_new_workspace_root(&run_root, &workspace_root)?;
            for allowed_root in &options.allowed_modification_roots {
                create_workspace_directory(&workspace_root, Path::new(allowed_root))?;
            }
        }
        IsolatedWorkspaceMode::CodeWorktree => {
            ensure_new_workspace_target(&run_root, &workspace_root)?;
            create_detached_worktree(&repository_root, &workspace_root, &source.source_commit)?;
            for allowed_root in &options.allowed_modification_roots {
                validate_existing_or_missing_path(&workspace_root, Path::new(allowed_root))?;
            }
        }
    }
    let workspace_root = canonical_regular_directory(&workspace_root, "isolated workspace root")?;
    if !workspace_root.starts_with(&run_root) {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "isolated workspace root escaped the run root",
            Some(workspace_root.display().to_string()),
        ));
    }
    let initial_snapshot = capture_workspace_tree(
        &workspace_root,
        &options.allowed_modification_roots,
        now_unix_ms,
    )?;
    let stale_after_ms = duration_to_millis(options.stale_lock_after)?;
    let record = WorkspaceIsolationRecord {
        protocol_version: WORKSPACE_ISOLATION_PROTOCOL_VERSION,
        mode: options.mode,
        source,
        workspace_relative_path: options.mode.directory_name().to_owned(),
        allowed_modification_roots: options.allowed_modification_roots.clone(),
        lock_targets: options.lock_targets.clone(),
        lock_leases: locks.iter().map(WorkspaceLock::record).collect(),
        lock_recovery: WorkspaceLockRecoveryPolicy {
            stale_after_ms,
            cancellation_preserves_workspace: true,
            process_crash_reclaims_expired_locks: true,
        },
        initial_snapshot,
    };
    record.validate()?;
    Ok(IsolatedWorkspace {
        repository_root,
        run_root,
        workspace_root,
        allowed_modification_roots: options.allowed_modification_roots,
        record,
        locks,
    })
}

pub fn capture_workspace_tree(
    workspace_root: &Path,
    allowed_modification_roots: &[String],
    captured_at_unix_ms: u64,
) -> Result<WorkspaceTreeSnapshot, TaskFailure> {
    validate_allowed_roots(allowed_modification_roots)?;
    let root = canonical_regular_directory(workspace_root, "workspace snapshot root")?;
    let mut files = BTreeMap::new();
    let mut total_bytes = 0_u64;
    for allowed in allowed_modification_roots {
        let relative = Path::new(allowed);
        match checked_path_metadata(&root, relative)? {
            None => continue,
            Some(metadata) if metadata.is_file() => {
                snapshot_file(&root, relative, &mut files, &mut total_bytes)?;
            }
            Some(metadata) if metadata.is_dir() => {
                walk_snapshot_directory(&root, relative, &mut files, &mut total_bytes)?;
            }
            Some(_) => {
                return Err(workspace_failure(
                    TaskFailureKind::WorkspaceSnapshotFailed,
                    "workspace snapshot root contains a non-file, non-directory entry",
                    Some(relative.display().to_string()),
                ));
            }
        }
    }
    let snapshot = WorkspaceTreeSnapshot {
        captured_at_unix_ms,
        files,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn diff_workspace_trees(
    before: &WorkspaceTreeSnapshot,
    after: &WorkspaceTreeSnapshot,
) -> WorkspaceFileDiff {
    let mut result = WorkspaceFileDiff::default();
    let paths: BTreeSet<_> = before
        .files
        .keys()
        .chain(after.files.keys())
        .cloned()
        .collect();
    for path in paths {
        match (before.files.get(&path), after.files.get(&path)) {
            (None, Some(after)) => result.created.push(WorkspaceFileChange {
                relative_path: path,
                before: None,
                after: Some(after.clone()),
            }),
            (Some(before), None) => result.deleted.push(WorkspaceFileChange {
                relative_path: path,
                before: Some(before.clone()),
                after: None,
            }),
            (Some(before), Some(after)) if before != after => {
                result.modified.push(WorkspaceFileChange {
                    relative_path: path,
                    before: Some(before.clone()),
                    after: Some(after.clone()),
                });
            }
            _ => {}
        }
    }
    result
}

#[derive(Debug)]
struct WorkspaceLock {
    path: PathBuf,
    owner_run_id: String,
    target: String,
    lease_id: String,
    lease_duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceLockFile {
    protocol_version: u32,
    target: String,
    owner_run_id: String,
    lease_id: String,
    acquired_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug)]
struct TargetReclaimGuard {
    _local: MutexGuard<'static, ()>,
    _file: fs::File,
}

impl WorkspaceLock {
    fn record(&self) -> WorkspaceLeaseRecord {
        WorkspaceLeaseRecord {
            target: self.target.clone(),
            lease_id: self.lease_id.clone(),
        }
    }

    fn refresh(&mut self, now_unix_ms: u64) -> Result<(), TaskFailure> {
        let Some(_guard) = try_acquire_target_reclaim_guard(&self.path)? else {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                "workspace lease refresh could not acquire its target guard",
                Some(self.target.clone()),
            ));
        };
        let record = read_lock_record(&self.path)?;
        if !lock_record_matches(&record, &self.owner_run_id, &self.target, &self.lease_id) {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                "workspace lease refresh no longer owns the target lock",
                Some(self.target.clone()),
            ));
        }
        if now_unix_ms >= record.expires_at_unix_ms {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                "workspace lease expired before it could be refreshed",
                Some(self.target.clone()),
            ));
        }
        let refreshed = WorkspaceLockFile {
            expires_at_unix_ms: now_unix_ms.saturating_add(self.lease_duration_ms),
            ..record
        };
        replace_owned_lock(&self.path, &self.lease_id, &refreshed)?;
        Ok(())
    }
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        let Ok(Some(_guard)) = try_acquire_target_reclaim_guard(&self.path) else {
            return;
        };
        let Ok(record) = read_lock_record(&self.path) else {
            return;
        };
        if lock_record_matches(&record, &self.owner_run_id, &self.target, &self.lease_id) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn acquire_workspace_locks(
    repository_root: &Path,
    run_id: &str,
    targets: &[String],
    timeout: Duration,
    stale_after: Duration,
    initial_now_unix_ms: u64,
) -> Result<Vec<WorkspaceLock>, TaskFailure> {
    let lock_root =
        create_workspace_directory(repository_root, Path::new("summary/ui-generation/.locks"))?;
    let stale_after_ms = duration_to_millis(stale_after)?;
    let started = std::time::Instant::now();
    let mut locks = Vec::with_capacity(targets.len());
    for target in targets {
        let lock_path = lock_root.join(format!("{}.lock", hash_bytes(target.as_bytes())));
        loop {
            let now_unix_ms = initial_now_unix_ms
                .saturating_add(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
            let expires_at_unix_ms = now_unix_ms.saturating_add(stale_after_ms);
            let record = WorkspaceLockFile {
                protocol_version: WORKSPACE_ISOLATION_PROTOCOL_VERSION,
                target: target.clone(),
                owner_run_id: run_id.to_owned(),
                lease_id: new_lease_id(run_id, target, now_unix_ms),
                acquired_at_unix_ms: now_unix_ms,
                expires_at_unix_ms,
            };
            match write_new_lock(&lock_path, &record) {
                Ok(()) => {
                    locks.push(WorkspaceLock {
                        path: lock_path,
                        owner_run_id: run_id.to_owned(),
                        target: target.clone(),
                        lease_id: record.lease_id,
                        lease_duration_ms: stale_after_ms,
                    });
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if reclaim_expired_lock(&lock_path, target, now_unix_ms)? {
                        continue;
                    }
                }
                Err(error) => {
                    return Err(workspace_failure(
                        TaskFailureKind::WorkspaceLockTimeout,
                        format!("could not create workspace lock: {error}"),
                        Some(lock_path.display().to_string()),
                    ));
                }
            }
            if started.elapsed() >= timeout {
                return Err(workspace_failure(
                    TaskFailureKind::WorkspaceLockTimeout,
                    "timed out waiting for an exclusive workspace target lock",
                    Some(target.clone()),
                ));
            }
            thread::sleep(LOCK_RETRY_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
        }
    }
    Ok(locks)
}

fn write_new_lock(path: &Path, record: &WorkspaceLockFile) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(record).expect("workspace lock record is serializable");
    bytes.push(b'\n');
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

fn reclaim_expired_lock(path: &Path, target: &str, now_unix_ms: u64) -> Result<bool, TaskFailure> {
    reclaim_expired_lock_with_hooks(path, target, now_unix_ms, || {}, || {})
}

fn reclaim_expired_lock_with_hooks<OnGuard, AfterRemove>(
    path: &Path,
    target: &str,
    now_unix_ms: u64,
    on_guard: OnGuard,
    after_remove: AfterRemove,
) -> Result<bool, TaskFailure>
where
    OnGuard: FnOnce(),
    AfterRemove: FnOnce(),
{
    let Some(_guard) = try_acquire_target_reclaim_guard(path)? else {
        return Ok(false);
    };
    on_guard();
    let record = read_lock_record(path)?;
    if record.protocol_version != WORKSPACE_ISOLATION_PROTOCOL_VERSION || record.target != target {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "existing workspace lock identity does not match the requested target",
            Some(path.display().to_string()),
        ));
    }
    if now_unix_ms < record.expires_at_unix_ms {
        return Ok(false);
    }
    fs::remove_file(path).map_err(|error| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            format!("expired workspace lock could not be reclaimed: {error}"),
            Some(path.display().to_string()),
        )
    })?;
    after_remove();
    Ok(true)
}

fn try_acquire_target_reclaim_guard(
    path: &Path,
) -> Result<Option<TargetReclaimGuard>, TaskFailure> {
    let local = match LOCAL_RECLAIM_GUARD.try_lock() {
        Ok(local) => local,
        Err(TryLockError::WouldBlock) => return Ok(None),
        Err(TryLockError::Poisoned(_)) => {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                "workspace target guard process mutex is poisoned",
                Some(path.display().to_string()),
            ));
        }
    };
    let guard_path = path.with_extension("reclaim");
    match fs::symlink_metadata(&guard_path) {
        Ok(metadata) if metadata_is_reparse(&metadata) || !metadata.is_file() => {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                "workspace target guard is not a regular file",
                Some(guard_path.display().to_string()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                format!("workspace target guard cannot be inspected: {error}"),
                Some(guard_path.display().to_string()),
            ));
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&guard_path)
        .map_err(|error| {
            workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                format!("workspace target guard cannot be opened: {error}"),
                Some(guard_path.display().to_string()),
            )
        })?;
    let metadata = file.metadata().map_err(|error| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            format!("workspace target guard cannot be inspected after open: {error}"),
            Some(guard_path.display().to_string()),
        )
    })?;
    let link_metadata = fs::symlink_metadata(&guard_path).map_err(|error| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            format!("workspace target guard cannot be rechecked: {error}"),
            Some(guard_path.display().to_string()),
        )
    })?;
    if !metadata.is_file() || metadata_is_reparse(&link_metadata) {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "workspace target guard changed into an unsafe filesystem entry",
            Some(guard_path.display().to_string()),
        ));
    }
    match file.try_lock() {
        Ok(()) => Ok(Some(TargetReclaimGuard {
            _local: local,
            _file: file,
        })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            format!("workspace target guard cannot be locked: {error}"),
            Some(guard_path.display().to_string()),
        )),
    }
}

fn read_lock_record(path: &Path) -> Result<WorkspaceLockFile, TaskFailure> {
    let bytes = stable_read_regular_file(path, MAX_LOCK_FILE_BYTES).map_err(|_| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "existing workspace lock cannot be safely inspected",
            Some(path.display().to_string()),
        )
    })?;
    let record: WorkspaceLockFile = serde_json::from_slice(&bytes).map_err(|_| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "existing workspace lock is corrupt and cannot be reclaimed",
            Some(path.display().to_string()),
        )
    })?;
    if record.protocol_version != WORKSPACE_ISOLATION_PROTOCOL_VERSION
        || !is_sha256(&record.lease_id)
        || record.owner_run_id.is_empty()
        || record.target.is_empty()
        || record.expires_at_unix_ms < record.acquired_at_unix_ms
    {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "existing workspace lock has an invalid lease record",
            Some(path.display().to_string()),
        ));
    }
    Ok(record)
}

fn replace_owned_lock(
    path: &Path,
    expected_lease_id: &str,
    next: &WorkspaceLockFile,
) -> Result<(), TaskFailure> {
    let current = read_lock_record(path)?;
    if current.lease_id != expected_lease_id
        || !lock_record_matches(
            &current,
            &next.owner_run_id,
            &next.target,
            expected_lease_id,
        )
    {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "workspace lock changed before its owned lease could be refreshed",
            Some(path.display().to_string()),
        ));
    }
    let mut bytes = serde_json::to_vec(next).map_err(|_| {
        workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "workspace lease refresh record cannot be serialized",
            Some(path.display().to_string()),
        )
    })?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| {
            workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                format!("workspace lease refresh cannot replace its record: {error}"),
                Some(path.display().to_string()),
            )
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            workspace_failure(
                TaskFailureKind::WorkspaceLockTimeout,
                format!("workspace lease refresh cannot persist its record: {error}"),
                Some(path.display().to_string()),
            )
        })?;
    let persisted = read_lock_record(path)?;
    if persisted != *next {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceLockTimeout,
            "workspace lease refresh record changed after it was persisted",
            Some(path.display().to_string()),
        ));
    }
    Ok(())
}

fn lock_record_matches(
    record: &WorkspaceLockFile,
    owner_run_id: &str,
    target: &str,
    lease_id: &str,
) -> bool {
    record.owner_run_id == owner_run_id && record.target == target && record.lease_id == lease_id
}

fn new_lease_id(run_id: &str, target: &str, now_unix_ms: u64) -> String {
    let sequence = LEASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(b"ui-generation-workspace-lease-v1");
    hasher.update(run_id.as_bytes());
    hasher.update([0]);
    hasher.update(target.as_bytes());
    hasher.update(now_unix_ms.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(sequence.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

fn capture_source_worktree(repository_root: &Path) -> Result<SourceWorktreeSnapshot, TaskFailure> {
    let source_commit = git_output(repository_root, ["rev-parse", "--verify", "HEAD^{commit}"])?;
    let source_commit = String::from_utf8(source_commit)
        .map_err(|_| {
            workspace_failure(
                TaskFailureKind::WorkspaceIsolationFailed,
                "Git returned a non-UTF-8 source commit",
                None,
            )
        })?
        .trim()
        .to_owned();
    if !is_commit_id(&source_commit) {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceIsolationFailed,
            "Git returned an invalid source commit",
            None,
        ));
    }
    let porcelain = git_output(
        repository_root,
        ["status", "--porcelain=v1", "--untracked-files=all", "-z"],
    )?;
    let entry_count = porcelain
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .count();
    Ok(SourceWorktreeSnapshot {
        source_commit,
        is_dirty: !porcelain.is_empty(),
        porcelain_v1_sha256: hash_bytes(&porcelain),
        porcelain_entry_count: u32::try_from(entry_count).unwrap_or(u32::MAX),
    })
}

fn create_detached_worktree(
    repository_root: &Path,
    workspace_root: &Path,
    source_commit: &str,
) -> Result<(), TaskFailure> {
    let output = Command::new("git")
        .arg("-C")
        .arg(git_command_path(repository_root))
        .args(["worktree", "add", "--detach"])
        .arg(git_command_path(workspace_root))
        .arg(source_commit)
        .output()
        .map_err(|error| {
            workspace_failure(
                TaskFailureKind::WorkspaceIsolationFailed,
                format!("could not launch Git worktree add: {error}"),
                Some(workspace_root.display().to_string()),
            )
        })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceIsolationFailed,
            format!("Git could not create the detached isolated worktree: {detail}"),
            Some(workspace_root.display().to_string()),
        ));
    }
    let actual = git_output(workspace_root, ["rev-parse", "--verify", "HEAD^{commit}"])?;
    if actual.as_slice().strip_suffix(b"\n").unwrap_or(&actual) != source_commit.as_bytes() {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceIsolationFailed,
            "detached worktree commit differs from the recorded source commit",
            Some(workspace_root.display().to_string()),
        ));
    }
    Ok(())
}

fn git_output<const N: usize>(
    repository_root: &Path,
    args: [&str; N],
) -> Result<Vec<u8>, TaskFailure> {
    let output = Command::new("git")
        .arg("-C")
        .arg(git_command_path(repository_root))
        .args(args)
        .output()
        .map_err(|error| {
            workspace_failure(
                TaskFailureKind::WorkspaceIsolationFailed,
                format!("could not launch Git: {error}"),
                Some(repository_root.display().to_string()),
            )
        })?;
    if !output.status.success()
        || output.stdout.len()
            > usize::try_from(MAX_LOCK_FILE_BYTES * 64).expect("static budget fits usize")
    {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceIsolationFailed,
            "Git source snapshot command failed or exceeded its output budget",
            Some(repository_root.display().to_string()),
        ));
    }
    Ok(output.stdout)
}

fn git_command_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.as_os_str().to_string_lossy();
        if let Some(without_verbatim_prefix) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(without_verbatim_prefix);
        }
    }
    path.to_path_buf()
}

fn create_new_workspace_root(run_root: &Path, workspace_root: &Path) -> Result<(), TaskFailure> {
    ensure_new_workspace_target(run_root, workspace_root)?;
    fs::create_dir(workspace_root).map_err(|error| {
        workspace_failure(
            TaskFailureKind::WorkspaceIsolationFailed,
            format!("could not create isolated workspace root: {error}"),
            Some(workspace_root.display().to_string()),
        )
    })
}

fn ensure_new_workspace_target(run_root: &Path, workspace_root: &Path) -> Result<(), TaskFailure> {
    let name = workspace_root.file_name().ok_or_else(|| {
        workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "workspace root has no final path component",
            Some(workspace_root.display().to_string()),
        )
    })?;
    if workspace_root.parent() != Some(run_root) || !is_safe_component(name) {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "workspace root must be a direct safe child of the run root",
            Some(workspace_root.display().to_string()),
        ));
    }
    match fs::symlink_metadata(workspace_root) {
        Ok(_) => Err(workspace_failure(
            TaskFailureKind::OutputDirectoryConflict,
            "isolated workspace target already exists and will not be replaced",
            Some(workspace_root.display().to_string()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            format!("could not inspect isolated workspace target: {error}"),
            Some(workspace_root.display().to_string()),
        )),
    }
}

fn create_workspace_directory(root: &Path, relative: &Path) -> Result<PathBuf, TaskFailure> {
    normal_relative_path(relative)?;
    let root = canonical_regular_directory(root, "workspace directory root")?;
    let mut current = root.clone();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("normal_relative_path checks components")
        };
        let child = current.join(component);
        match fs::symlink_metadata(&child) {
            Ok(metadata) => {
                if metadata_is_reparse(&metadata) || !metadata.is_dir() {
                    return Err(workspace_failure(
                        TaskFailureKind::UnsafeOutputPath,
                        "workspace directory component is not a regular directory",
                        Some(child.display().to_string()),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&child).map_err(|error| {
                    workspace_failure(
                        TaskFailureKind::WorkspaceIsolationFailed,
                        format!("could not create workspace directory: {error}"),
                        Some(child.display().to_string()),
                    )
                })?;
            }
            Err(error) => {
                return Err(workspace_failure(
                    TaskFailureKind::UnsafeOutputPath,
                    format!("could not inspect workspace directory: {error}"),
                    Some(child.display().to_string()),
                ));
            }
        }
        current = canonical_regular_directory(&child, "workspace directory component")?;
        if !current.starts_with(&root) {
            return Err(workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace directory component escapes its approved root",
                Some(current.display().to_string()),
            ));
        }
    }
    Ok(current)
}

fn resolve_contained_path(root: &Path, relative: &Path) -> Result<PathBuf, TaskFailure> {
    normal_relative_path(relative)?;
    let root = canonical_regular_directory(root, "workspace output root")?;
    validate_existing_or_missing_path(&root, relative)?;
    let candidate = root.join(relative);
    let mut existing = candidate.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace output has no existing parent",
                Some(candidate.display().to_string()),
            )
        })?;
    }
    let canonical_parent = if existing.is_dir() {
        canonical_regular_directory(existing, "workspace output parent")?
    } else {
        let parent = existing.parent().ok_or_else(|| {
            workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace output existing file has no parent",
                Some(existing.display().to_string()),
            )
        })?;
        canonical_regular_directory(parent, "workspace output parent")?
    };
    if !canonical_parent.starts_with(&root) {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "workspace output resolves outside the approved root",
            Some(candidate.display().to_string()),
        ));
    }
    Ok(candidate)
}

fn validate_existing_or_missing_path(root: &Path, relative: &Path) -> Result<(), TaskFailure> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace path has a non-normal component",
                Some(relative.display().to_string()),
            ));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata_is_reparse(&metadata) => {
                return Err(workspace_failure(
                    TaskFailureKind::UnsafeOutputPath,
                    "workspace path cannot traverse a symlink or reparse point",
                    Some(current.display().to_string()),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(workspace_failure(
                    TaskFailureKind::UnsafeOutputPath,
                    format!("workspace path cannot be inspected: {error}"),
                    Some(current.display().to_string()),
                ));
            }
        }
    }
    Ok(())
}

fn checked_path_metadata(
    root: &Path,
    relative: &Path,
) -> Result<Option<fs::Metadata>, TaskFailure> {
    validate_existing_or_missing_path(root, relative)?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata_is_reparse(&metadata) => Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot cannot traverse a symlink or reparse point",
            Some(path.display().to_string()),
        )),
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            format!("workspace snapshot path cannot be inspected: {error}"),
            Some(path.display().to_string()),
        )),
    }
}

fn walk_snapshot_directory(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<String, WorkspaceFileSnapshot>,
    total_bytes: &mut u64,
) -> Result<(), TaskFailure> {
    let directory = root.join(relative);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| workspace_snapshot_failure(&directory, "read directory", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| workspace_snapshot_failure(&directory, "enumerate directory", error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        if !is_safe_component(&name) {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceSnapshotFailed,
                "workspace snapshot found an unsafe file name",
                Some(entry.path().display().to_string()),
            ));
        }
        let child_relative = relative.join(name);
        let metadata = checked_path_metadata(root, &child_relative)?.ok_or_else(|| {
            workspace_failure(
                TaskFailureKind::WorkspaceSnapshotFailed,
                "workspace entry disappeared while it was being snapshotted",
                Some(child_relative.display().to_string()),
            )
        })?;
        if metadata.is_dir() {
            walk_snapshot_directory(root, &child_relative, files, total_bytes)?;
        } else if metadata.is_file() {
            snapshot_file(root, &child_relative, files, total_bytes)?;
        } else {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceSnapshotFailed,
                "workspace snapshot found an unsupported filesystem entry",
                Some(child_relative.display().to_string()),
            ));
        }
    }
    Ok(())
}

fn snapshot_file(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<String, WorkspaceFileSnapshot>,
    total_bytes: &mut u64,
) -> Result<(), TaskFailure> {
    if files.len() >= MAX_SNAPSHOT_FILES {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot exceeds its file count budget",
            None,
        ));
    }
    let path = root.join(relative);
    let snapshot = stable_hash_file(&path)?;
    *total_bytes = total_bytes
        .checked_add(snapshot.byte_length)
        .ok_or_else(|| {
            workspace_failure(
                TaskFailureKind::WorkspaceSnapshotFailed,
                "workspace snapshot byte count overflowed",
                None,
            )
        })?;
    if *total_bytes > MAX_SNAPSHOT_TOTAL_BYTES {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot exceeds its byte budget",
            None,
        ));
    }
    let relative_text = normal_relative_path(relative)?;
    if files.insert(relative_text.clone(), snapshot).is_some() {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot found duplicate case-insensitive paths",
            Some(relative_text),
        ));
    }
    Ok(())
}

fn stable_hash_file(path: &Path) -> Result<WorkspaceFileSnapshot, TaskFailure> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|error| workspace_snapshot_failure(path, "inspect file", error))?;
    if metadata_is_reparse(&link_metadata) || !link_metadata.is_file() {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot file is not a regular file",
            Some(path.display().to_string()),
        ));
    }
    let mut file = fs::File::open(path)
        .map_err(|error| workspace_snapshot_failure(path, "open file", error))?;
    let before = FileMetadata::capture(
        &file
            .metadata()
            .map_err(|error| workspace_snapshot_failure(path, "read file metadata", error))?,
    );
    if before.byte_length > MAX_SNAPSHOT_FILE_BYTES {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace snapshot file exceeds its byte budget",
            Some(path.display().to_string()),
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| workspace_snapshot_failure(path, "read file", error))?;
        if count == 0 {
            break;
        }
        read = read
            .checked_add(u64::try_from(count).expect("buffer length fits u64"))
            .ok_or_else(|| {
                workspace_failure(
                    TaskFailureKind::WorkspaceSnapshotFailed,
                    "workspace file size overflowed",
                    Some(path.display().to_string()),
                )
            })?;
        if read > MAX_SNAPSHOT_FILE_BYTES {
            return Err(workspace_failure(
                TaskFailureKind::WorkspaceSnapshotFailed,
                "workspace snapshot file exceeds its byte budget",
                Some(path.display().to_string()),
            ));
        }
        hasher.update(&buffer[..count]);
    }
    let after_handle = FileMetadata::capture(
        &file
            .metadata()
            .map_err(|error| workspace_snapshot_failure(path, "re-read file metadata", error))?,
    );
    let after_path =
        FileMetadata::capture(&fs::metadata(path).map_err(|error| {
            workspace_snapshot_failure(path, "re-read file path metadata", error)
        })?);
    let after_link = fs::symlink_metadata(path)
        .map_err(|error| workspace_snapshot_failure(path, "re-read file link metadata", error))?;
    if before != after_handle
        || before != after_path
        || metadata_is_reparse(&after_link)
        || read != before.byte_length
    {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace file changed while it was being snapshotted",
            Some(path.display().to_string()),
        ));
    }
    Ok(WorkspaceFileSnapshot {
        sha256: format!("{:x}", hasher.finalize()),
        byte_length: read,
    })
}

fn stable_read_regular_file(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, TaskFailure> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|error| workspace_snapshot_failure(path, "inspect file", error))?;
    if metadata_is_reparse(&link_metadata)
        || !link_metadata.is_file()
        || link_metadata.len() > maximum_bytes
    {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace lock file is not a bounded regular file",
            Some(path.display().to_string()),
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| workspace_snapshot_failure(path, "read file", error))?;
    if bytes.len() as u64 != link_metadata.len() || bytes.len() as u64 > maximum_bytes {
        return Err(workspace_failure(
            TaskFailureKind::WorkspaceSnapshotFailed,
            "workspace lock file changed while it was being read",
            Some(path.display().to_string()),
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileMetadata {
    byte_length: u64,
    modified: Option<SystemTime>,
    readonly: bool,
    regular_file: bool,
}

impl FileMetadata {
    fn capture(metadata: &fs::Metadata) -> Self {
        Self {
            byte_length: metadata.len(),
            modified: metadata.modified().ok(),
            readonly: metadata.permissions().readonly(),
            regular_file: metadata.is_file(),
        }
    }
}

fn validate_snapshot(snapshot: &WorkspaceTreeSnapshot) -> Result<(), TaskFailure> {
    if snapshot.files.len() > MAX_SNAPSHOT_FILES {
        return Err(workspace_failure(
            TaskFailureKind::ManifestCorrupt,
            "workspace snapshot exceeds its file count budget",
            None,
        ));
    }
    let mut total = 0_u64;
    let mut lowered = BTreeSet::new();
    for (path, file) in &snapshot.files {
        normal_relative_path(Path::new(path))?;
        if !is_sha256(&file.sha256) || !lowered.insert(path.to_ascii_lowercase()) {
            return Err(workspace_failure(
                TaskFailureKind::ManifestCorrupt,
                "workspace snapshot has an invalid file identity",
                Some(path.clone()),
            ));
        }
        total = total.checked_add(file.byte_length).ok_or_else(|| {
            workspace_failure(
                TaskFailureKind::ManifestCorrupt,
                "workspace snapshot byte count overflowed",
                None,
            )
        })?;
    }
    if total > MAX_SNAPSHOT_TOTAL_BYTES {
        return Err(workspace_failure(
            TaskFailureKind::ManifestCorrupt,
            "workspace snapshot exceeds its byte budget",
            None,
        ));
    }
    Ok(())
}

fn validate_allowed_roots(roots: &[String]) -> Result<(), TaskFailure> {
    if roots.is_empty() || roots.len() > 32 {
        return Err(workspace_failure(
            TaskFailureKind::InvalidInput,
            "workspace requires 1..=32 allowed modification roots",
            None,
        ));
    }
    let mut seen = BTreeSet::new();
    for root in roots {
        let normalized = normal_relative_path(Path::new(root))?;
        if !seen.insert(normalized.to_ascii_lowercase()) {
            return Err(workspace_failure(
                TaskFailureKind::InvalidInput,
                "workspace allowed modification roots must be unique",
                Some(root.clone()),
            ));
        }
    }
    Ok(())
}

fn validate_lock_targets(targets: &[String]) -> Result<(), TaskFailure> {
    if targets.is_empty() || targets.len() > 32 {
        return Err(workspace_failure(
            TaskFailureKind::InvalidInput,
            "workspace requires 1..=32 lock targets",
            None,
        ));
    }
    let mut seen = BTreeSet::new();
    for target in targets {
        let safe = !target.is_empty()
            && target.len() <= 160
            && target.is_ascii()
            && target.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b':' | b'_' | b'-' | b'/')
            })
            && !target.contains("..")
            && !target.starts_with('/')
            && !target.ends_with('/');
        if !safe || !seen.insert(target.to_ascii_lowercase()) {
            return Err(workspace_failure(
                TaskFailureKind::InvalidInput,
                "workspace lock target is unsafe or duplicated",
                Some(target.clone()),
            ));
        }
    }
    Ok(())
}

fn normal_relative_path(path: &Path) -> Result<String, TaskFailure> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            "workspace path must be a non-empty relative path",
            Some(path.display().to_string()),
        ));
    }
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace path must contain only normal components",
                Some(path.display().to_string()),
            ));
        };
        if !is_safe_component(component) {
            return Err(workspace_failure(
                TaskFailureKind::UnsafeOutputPath,
                "workspace path contains an unsafe component",
                Some(path.display().to_string()),
            ));
        }
        components.push(component.to_string_lossy().into_owned());
    }
    Ok(components.join("/"))
}

fn is_safe_component(component: &std::ffi::OsStr) -> bool {
    let text = component.to_string_lossy();
    !text.is_empty()
        && text != "."
        && text != ".."
        && !text.contains(['/', '\\', '\0'])
        && text.len() <= 160
}

fn path_is_within_root(path: &str, allowed_root: &str) -> bool {
    path == allowed_root
        || path
            .strip_prefix(allowed_root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let link_metadata = fs::symlink_metadata(path).map_err(|_| {
        workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} cannot be resolved"),
            Some(path.display().to_string()),
        )
    })?;
    if metadata_is_reparse(&link_metadata) || !link_metadata.is_dir() {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} must be a regular directory without reparse points"),
            Some(path.display().to_string()),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|_| {
        workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} cannot be canonicalized"),
            Some(path.display().to_string()),
        )
    })?;
    if !fs::metadata(&canonical).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(workspace_failure(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} is not a regular directory"),
            Some(path.display().to_string()),
        ));
    }
    Ok(canonical)
}

fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_commit_id(value: &str) -> bool {
    (40..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn duration_to_millis(value: Duration) -> Result<u64, TaskFailure> {
    u64::try_from(value.as_millis()).map_err(|_| {
        workspace_failure(
            TaskFailureKind::InvalidInput,
            "workspace duration exceeds the supported millisecond range",
            None,
        )
    })
}

fn workspace_snapshot_failure(path: &Path, action: &str, error: std::io::Error) -> TaskFailure {
    workspace_failure(
        TaskFailureKind::WorkspaceSnapshotFailed,
        format!("could not {action} while capturing workspace snapshot: {error}"),
        Some(path.display().to_string()),
    )
}

fn workspace_failure(
    kind: TaskFailureKind,
    message: impl Into<String>,
    subject: Option<String>,
) -> TaskFailure {
    TaskFailure::new(kind, message, subject)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(repository: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn git_repository() -> tempfile::TempDir {
        let repository = tempfile::tempdir().unwrap();
        git(repository.path(), &["init", "--quiet"]);
        git(
            repository.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(repository.path(), &["config", "user.name", "Fixture"]);
        fs::create_dir_all(repository.path().join("project/src")).unwrap();
        fs::write(
            repository.path().join("project/src/lib.rs"),
            b"pub const VALUE: u8 = 1;\n",
        )
        .unwrap();
        git(repository.path(), &["add", "project/src/lib.rs"]);
        git(repository.path(), &["commit", "--quiet", "-m", "fixture"]);
        fs::create_dir_all(repository.path().join("summary/ui-generation/run-a")).unwrap();
        repository
    }

    fn options(mode: IsolatedWorkspaceMode, roots: &[&str]) -> WorkspaceIsolationOptions {
        WorkspaceIsolationOptions {
            mode,
            allowed_modification_roots: roots.iter().map(|value| (*value).to_owned()).collect(),
            lock_targets: vec!["page:fixture".to_owned()],
            lock_timeout: Duration::ZERO,
            stale_lock_after: Duration::from_secs(30),
        }
    }

    #[test]
    fn draft_staging_records_dirty_source_without_copying_it() {
        let repository = git_repository();
        let source = repository.path().join("project/src/lib.rs");
        fs::write(&source, b"pub const VALUE: u8 = 2;\n").unwrap();
        let workspace = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-a"),
            "run-a",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft", "assets"]),
            100,
        )
        .unwrap();
        assert!(workspace.record().source.is_dirty);
        assert_eq!(workspace.record().source.porcelain_entry_count, 1);
        assert!(
            !workspace
                .workspace_root()
                .join("project/src/lib.rs")
                .exists()
        );
        assert!(workspace.workspace_root().join("draft").is_dir());
        assert!(workspace.workspace_root().join("assets").is_dir());
        assert_eq!(fs::read(source).unwrap(), b"pub const VALUE: u8 = 2;\n");
    }

    #[test]
    fn code_worktree_uses_recorded_commit_not_dirty_caller_contents() {
        let repository = git_repository();
        let source = repository.path().join("project/src/lib.rs");
        fs::write(&source, b"pub const VALUE: u8 = 9;\n").unwrap();
        let workspace = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-a"),
            "run-a",
            options(IsolatedWorkspaceMode::CodeWorktree, &["project/src"]),
            100,
        )
        .unwrap();
        assert!(
            String::from_utf8(
                fs::read(workspace.workspace_root().join("project/src/lib.rs")).unwrap()
            )
            .unwrap()
            .contains("VALUE: u8 = 1;")
        );
        assert_eq!(fs::read(source).unwrap(), b"pub const VALUE: u8 = 9;\n");
        assert!(workspace.workspace_root().join(".git").is_file());
    }

    #[test]
    fn snapshots_classify_created_modified_and_deleted_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("draft")).unwrap();
        fs::write(root.path().join("draft/changed.txt"), b"before").unwrap();
        fs::write(root.path().join("draft/deleted.txt"), b"gone").unwrap();
        let roots = vec!["draft".to_owned()];
        let before = capture_workspace_tree(root.path(), &roots, 1).unwrap();
        fs::write(root.path().join("draft/changed.txt"), b"after").unwrap();
        fs::remove_file(root.path().join("draft/deleted.txt")).unwrap();
        fs::write(root.path().join("draft/created.txt"), b"new").unwrap();
        let after = capture_workspace_tree(root.path(), &roots, 2).unwrap();
        let diff = diff_workspace_trees(&before, &after);
        assert_eq!(diff.created[0].relative_path, "draft/created.txt");
        assert_eq!(diff.modified[0].relative_path, "draft/changed.txt");
        assert_eq!(diff.deleted[0].relative_path, "draft/deleted.txt");
    }

    #[test]
    fn output_resolution_rejects_escape_and_symlink_paths() {
        let repository = git_repository();
        let mut workspace = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-a"),
            "run-a",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            100,
        )
        .unwrap();
        assert!(
            workspace
                .resolve_output_path(Path::new("../outside"))
                .is_err()
        );
        assert!(
            workspace
                .resolve_output_path(Path::new("assets/nope"))
                .is_err()
        );
        let outside = tempfile::tempdir().unwrap();
        let link = workspace.workspace_root().join("draft/link");
        if create_directory_symlink(outside.path(), &link) {
            assert!(
                workspace
                    .resolve_output_path(Path::new("draft/link/escape"))
                    .is_err()
            );
            assert!(
                capture_workspace_tree(workspace.workspace_root(), &["draft".to_owned()], 101,)
                    .is_err()
            );
        }
        let retained = workspace.workspace_root().to_path_buf();
        workspace.release_locks();
        assert!(retained.is_dir());
    }

    #[test]
    fn concurrent_target_lock_times_out_and_expired_lock_is_recovered() {
        let repository = git_repository();
        let run_root = repository.path().join("summary/ui-generation/run-a");
        let held = prepare_isolated_workspace(
            repository.path(),
            &run_root,
            "run-a",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            100,
        )
        .unwrap();
        fs::create_dir_all(repository.path().join("summary/ui-generation/run-b")).unwrap();
        let failure = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-b"),
            "run-b",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            100,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::WorkspaceLockTimeout);
        drop(held);

        let lock_root = repository.path().join("summary/ui-generation/.locks");
        let target = "page:fixture";
        let path = lock_root.join(format!("{}.lock", hash_bytes(target.as_bytes())));
        let stale = WorkspaceLockFile {
            protocol_version: WORKSPACE_ISOLATION_PROTOCOL_VERSION,
            target: target.to_owned(),
            owner_run_id: "crashed-run".to_owned(),
            lease_id: "c".repeat(64),
            acquired_at_unix_ms: 1,
            expires_at_unix_ms: 2,
        };
        write_new_lock(&path, &stale).unwrap();
        let recovered = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-b"),
            "run-b",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            100,
        )
        .unwrap();
        assert!(recovered.workspace_root().is_dir());
    }

    #[test]
    fn refreshed_active_lease_blocks_a_competing_run_past_its_original_expiry() {
        let repository = git_repository();
        let mut held = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-a"),
            "run-a",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            100,
        )
        .unwrap();
        held.refresh_locks(20_000).unwrap();
        fs::create_dir_all(repository.path().join("summary/ui-generation/run-b")).unwrap();
        let failure = prepare_isolated_workspace(
            repository.path(),
            &repository.path().join("summary/ui-generation/run-b"),
            "run-b",
            options(IsolatedWorkspaceMode::DraftStaging, &["draft"]),
            40_000,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::WorkspaceLockTimeout);
    }

    #[test]
    fn expired_old_lease_drop_cannot_remove_new_lease_for_the_same_run_id() {
        let repository = git_repository();
        let targets = vec!["page:fixture".to_owned()];
        let old = acquire_workspace_locks(
            repository.path(),
            "same-run",
            &targets,
            Duration::ZERO,
            Duration::from_millis(10),
            100,
        )
        .unwrap();
        let old_lease = old[0].lease_id.clone();
        let replacement = acquire_workspace_locks(
            repository.path(),
            "same-run",
            &targets,
            Duration::ZERO,
            Duration::from_millis(10),
            111,
        )
        .unwrap();
        let replacement_lease = replacement[0].lease_id.clone();
        assert_ne!(old_lease, replacement_lease);
        let path = replacement[0].path.clone();
        drop(old);
        assert_eq!(read_lock_record(&path).unwrap().lease_id, replacement_lease);
        drop(replacement);
    }

    #[test]
    fn concurrent_stale_reclaimer_cannot_delete_a_newly_acquired_lease() {
        use std::{sync::mpsc, thread};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.lock");
        let stale = WorkspaceLockFile {
            protocol_version: WORKSPACE_ISOLATION_PROTOCOL_VERSION,
            target: "page:fixture".to_owned(),
            owner_run_id: "crashed-run".to_owned(),
            lease_id: "c".repeat(64),
            acquired_at_unix_ms: 1,
            expires_at_unix_ms: 2,
        };
        write_new_lock(&path, &stale).unwrap();

        let (guard_entered_tx, guard_entered_rx) = mpsc::channel();
        let (allow_remove_tx, allow_remove_rx) = mpsc::channel();
        let (removed_tx, removed_rx) = mpsc::channel();
        let (allow_guard_release_tx, allow_guard_release_rx) = mpsc::channel();
        let first_path = path.clone();
        let first = thread::spawn(move || {
            reclaim_expired_lock_with_hooks(
                &first_path,
                "page:fixture",
                100,
                || {
                    guard_entered_tx.send(()).unwrap();
                    allow_remove_rx.recv().unwrap();
                },
                || {
                    removed_tx.send(()).unwrap();
                    allow_guard_release_rx.recv().unwrap();
                },
            )
        });
        guard_entered_rx.recv().unwrap();

        let second_path = path.clone();
        let second = thread::spawn(move || reclaim_expired_lock(&second_path, "page:fixture", 100));
        assert!(!second.join().unwrap().unwrap());

        allow_remove_tx.send(()).unwrap();
        removed_rx.recv().unwrap();
        let new_lease = WorkspaceLockFile {
            protocol_version: WORKSPACE_ISOLATION_PROTOCOL_VERSION,
            target: "page:fixture".to_owned(),
            owner_run_id: "recovered-run".to_owned(),
            lease_id: "d".repeat(64),
            acquired_at_unix_ms: 100,
            expires_at_unix_ms: 200,
        };
        write_new_lock(&path, &new_lease).unwrap();
        allow_guard_release_tx.send(()).unwrap();
        assert!(first.join().unwrap().unwrap());
        assert_eq!(read_lock_record(&path).unwrap(), new_lease);
    }

    #[cfg(unix)]
    fn create_directory_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_directory_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
}
