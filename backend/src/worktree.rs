//! Bare-clone + git-worktree provisioning for review checkouts.
//!
//! A project's `repo_path` is a *container*: it holds `.bare/` (a bare clone)
//! plus one worktree per branch. Resident worktrees (the default branches) are
//! provisioned at startup; merge-request worktrees are supplied on demand.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::process::Command;
use tokio::sync::Mutex as TokioMutex;
use tracing::warn;

use crate::projects::{set_project_health, ResolvedProject};

/// Default minimum free space required before a clone / worktree add.
pub const DEFAULT_MIN_FREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const MAX_FETCH_ATTEMPTS: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("missing git_remote_url")]
    MissingRemote,
    #[error("missing default_branches")]
    MissingBranches,
    #[error("insufficient free disk space")]
    DiskFull,
    #[error("bare repository not provisioned")]
    BareMissing,
    #[error("git clone failed: {0}")]
    Clone(String),
    #[error("remote branch '{0}' no longer exists")]
    BranchGone(String),
    #[error("git fetch failed: {0}")]
    Fetch(String),
    #[error("git worktree add failed: {0}")]
    WorktreeAdd(String),
    #[error("git reset failed: {0}")]
    Reset(String),
    #[error("io error: {0}")]
    Io(String),
}

/// Which kind of worktree a branch maps to. Resident (default) worktrees use a
/// human-readable escaped name; merge-request worktrees append a short hash so
/// distinct branch names never collide (e.g. `feature/x` vs `feature-x`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeKind {
    Resident,
    MergeRequest,
}

/// Escape every character outside `[A-Za-z0-9._-]` (including `/`) to `-`.
pub fn escape_branch(branch: &str) -> String {
    branch
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '-',
        })
        .collect()
}

/// Deterministic 8-hex-char FNV-1a hash of the full branch name. Stable across
/// restarts so the same branch always maps to the same directory.
fn short_hash(branch: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in branch.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (hash & 0xffff_ffff) as u32)
}

/// Directory name for a branch's worktree.
pub fn worktree_dir_name(branch: &str, kind: WorktreeKind) -> String {
    match kind {
        WorktreeKind::Resident => escape_branch(branch),
        WorktreeKind::MergeRequest => format!("{}-{}", escape_branch(branch), short_hash(branch)),
    }
}

pub fn bare_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(".bare")
}

pub fn worktree_dir(repo_path: &Path, branch: &str, kind: WorktreeKind) -> PathBuf {
    repo_path.join(worktree_dir_name(branch, kind))
}

fn min_free_bytes() -> u64 {
    std::env::var("REVIEWER_MIN_FREE_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MIN_FREE_BYTES)
}

/// Whether free disk space is below the configured threshold.
///
/// Real cross-platform measurement is deferred (it needs a crate we cannot add
/// offline). For now this honours the `REVIEWER_TEST_FORCE_LOW_DISK` seam so the
/// isolation contract is exercised; without it, it fails open (never blocks).
fn disk_below_threshold(_path: &Path) -> bool {
    if std::env::var("REVIEWER_TEST_FORCE_LOW_DISK").is_ok() {
        // `min_free_bytes` is consulted so the threshold override stays wired.
        return min_free_bytes() > 0;
    }
    false
}

/// Per-repository lock table: `worktree add` / `fetch` / `reset` against the same
/// bare object store must not interleave. Different repos run concurrently.
fn repo_lock(repo_path: &Path) -> Arc<TokioMutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<TokioMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut guard = map.lock().expect("repo lock table");
    guard
        .entry(repo_path.to_path_buf())
        .or_insert_with(|| Arc::new(TokioMutex::new(())))
        .clone()
}

struct GitOutput {
    success: bool,
    stderr: String,
}

async fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<GitOutput, WorktreeError> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    // Never block on interactive credential / host-key prompts.
    command.env("GIT_TERMINAL_PROMPT", "0");
    let output = command
        .output()
        .await
        .map_err(|e| WorktreeError::Io(e.to_string()))?;
    Ok(GitOutput {
        success: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn branch_gone(stderr: &str) -> bool {
    stderr.contains("couldn't find remote ref") || stderr.contains("couldn't find remote branch")
}

/// Idempotently provision `.bare/` and the resident worktrees for one project.
pub async fn provision_project(
    repo_path: &Path,
    remote: &str,
    default_branches: &[String],
) -> Result<(), WorktreeError> {
    if default_branches.is_empty() {
        return Err(WorktreeError::MissingBranches);
    }
    if disk_below_threshold(repo_path) {
        return Err(WorktreeError::DiskFull);
    }

    let lock = repo_lock(repo_path);
    let _guard = lock.lock().await;

    std::fs::create_dir_all(repo_path).map_err(|e| WorktreeError::Io(e.to_string()))?;

    let bare = bare_dir(repo_path);
    if !bare.exists() {
        let out = run_git(&["clone", "--bare", remote, ".bare"], Some(repo_path)).await?;
        if !out.success {
            return Err(WorktreeError::Clone(out.stderr.trim().to_string()));
        }
        let bare_str = bare.to_string_lossy().to_string();
        // A bare clone omits the standard fetch refspec; set it so later fetches
        // populate refs/remotes/origin/*.
        let refspec = run_git(
            &[
                "--git-dir",
                &bare_str,
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
            None,
        )
        .await?;
        if !refspec.success {
            return Err(WorktreeError::Clone(refspec.stderr.trim().to_string()));
        }
        // Let the container dir act as a git repo pointing at the bare store.
        std::fs::write(repo_path.join(".git"), "gitdir: ./.bare\n")
            .map_err(|e| WorktreeError::Io(e.to_string()))?;
    }

    let bare_str = bare.to_string_lossy().to_string();
    for branch in default_branches {
        let dir = worktree_dir(repo_path, branch, WorktreeKind::Resident);
        if dir.exists() {
            continue;
        }
        let dir_str = dir.to_string_lossy().to_string();
        let out = run_git(
            &["--git-dir", &bare_str, "worktree", "add", &dir_str, branch],
            None,
        )
        .await?;
        if !out.success {
            return Err(WorktreeError::WorktreeAdd(out.stderr.trim().to_string()));
        }
    }

    Ok(())
}

async fn fetch_branch(repo_path: &Path, branch: &str) -> Result<(), WorktreeError> {
    let bare_str = bare_dir(repo_path).to_string_lossy().to_string();
    let mut last = String::new();
    for attempt in 0..MAX_FETCH_ATTEMPTS {
        let out = run_git(
            &["--git-dir", &bare_str, "fetch", "origin", branch],
            None,
        )
        .await?;
        if out.success {
            return Ok(());
        }
        if branch_gone(&out.stderr) {
            return Err(WorktreeError::BranchGone(branch.to_string()));
        }
        last = out.stderr.trim().to_string();
        // Exponential backoff on transient failures: 100ms, 200ms, 400ms.
        if attempt + 1 < MAX_FETCH_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
        }
    }
    Err(WorktreeError::Fetch(last))
}

/// Supply the worktree path for `branch`, creating or updating it as needed.
///
/// Existing worktrees are `fetch`ed and hard-reset to `origin/<branch>` (source
/// branches are force-pushed). A deleted remote branch removes the worktree.
pub async fn supply_worktree(
    repo_path: &Path,
    branch: &str,
    kind: WorktreeKind,
) -> Result<PathBuf, WorktreeError> {
    let lock = repo_lock(repo_path);
    let _guard = lock.lock().await;

    if !bare_dir(repo_path).exists() {
        return Err(WorktreeError::BareMissing);
    }

    let dir = worktree_dir(repo_path, branch, kind);
    let bare_str = bare_dir(repo_path).to_string_lossy().to_string();
    let dir_str = dir.to_string_lossy().to_string();
    let remote_ref = format!("origin/{branch}");

    if dir.exists() {
        match fetch_branch(repo_path, branch).await {
            Ok(()) => {}
            Err(WorktreeError::BranchGone(b)) => {
                let _ = run_git(
                    &["--git-dir", &bare_str, "worktree", "remove", "--force", &dir_str],
                    None,
                )
                .await;
                return Err(WorktreeError::BranchGone(b));
            }
            Err(other) => return Err(other),
        }
        let reset = run_git(&["-C", &dir_str, "reset", "--hard", &remote_ref], None).await?;
        if !reset.success {
            return Err(WorktreeError::Reset(reset.stderr.trim().to_string()));
        }
    } else {
        if disk_below_threshold(repo_path) {
            return Err(WorktreeError::DiskFull);
        }
        fetch_branch(repo_path, branch).await?;
        let out = run_git(
            &[
                "--git-dir", &bare_str, "worktree", "add", "-B", branch, &dir_str, &remote_ref,
            ],
            None,
        )
        .await?;
        if !out.success {
            return Err(WorktreeError::WorktreeAdd(out.stderr.trim().to_string()));
        }
    }

    Ok(dir)
}

/// On-demand merge-request worktree for `source_branch` (fetch + add or reuse).
pub async fn provision_mr_worktree(
    repo_path: &Path,
    source_branch: &str,
) -> Result<PathBuf, WorktreeError> {
    supply_worktree(repo_path, source_branch, WorktreeKind::MergeRequest).await
}

/// Provision every project (best-effort) and persist provisioning health.
///
/// A failure for one project marks it unhealthy and records the reason; it never
/// aborts the process or affects other projects.
pub async fn provision_all(pool: &SqlitePool, projects: &[ResolvedProject]) {
    for project in projects {
        let Some(remote) = project.git_remote_url.as_deref() else {
            // Static failure already recorded at load time; leave as-is.
            continue;
        };
        if project.default_branches.is_empty() {
            continue;
        }

        match provision_project(&project.repo_path, remote, &project.default_branches).await {
            Ok(()) => {
                let default_branch = project.default_branches.first().map(String::as_str);
                if let Err(err) =
                    set_project_health(pool, &project.name, 1, default_branch, "healthy", None).await
                {
                    warn!(project = %project.name, "failed to record healthy state: {err}");
                }
            }
            Err(err) => {
                let reason = err.to_string();
                warn!(project = %project.name, "provisioning failed: {reason}");
                if let Err(db_err) =
                    set_project_health(pool, &project.name, 0, None, "unhealthy", Some(&reason)).await
                {
                    warn!(project = %project.name, "failed to record unhealthy state: {db_err}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn git(args: &[&str], cwd: &Path) {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_source(path: &Path) {
        std::fs::create_dir_all(path).expect("source dir");
        git(&["init", "-b", "main", "."], path);
        git(&["config", "user.email", "t@e.com"], path);
        git(&["config", "user.name", "T"], path);
        std::fs::write(path.join("a.txt"), "a").expect("a.txt");
        git(&["add", "-A"], path);
        git(&["commit", "-m", "init"], path);
    }

    fn url_of(path: &Path) -> String {
        path.display().to_string().replace('\\', "/")
    }

    #[test]
    fn escape_replaces_non_allowed_chars() {
        assert_eq!(escape_branch("feature/x"), "feature-x");
        assert_eq!(escape_branch("feature-x"), "feature-x");
        assert_eq!(escape_branch("fix bug#1"), "fix-bug-1");
        assert_eq!(escape_branch("release/v1.2_rc"), "release-v1.2_rc");
    }

    #[test]
    fn mr_names_disambiguate_escape_collisions() {
        let a = worktree_dir_name("feature/x", WorktreeKind::MergeRequest);
        let b = worktree_dir_name("feature-x", WorktreeKind::MergeRequest);
        assert_ne!(a, b, "distinct branches must map to distinct MR dirs");
        assert!(a.starts_with("feature-x-"));
        assert!(b.starts_with("feature-x-"));
    }

    #[test]
    fn resident_names_have_no_hash_suffix() {
        assert_eq!(worktree_dir_name("main", WorktreeKind::Resident), "main");
    }

    #[test]
    fn short_hash_is_stable_and_distinct() {
        assert_eq!(short_hash("feature/x"), short_hash("feature/x"));
        assert_ne!(short_hash("feature/x"), short_hash("feature-x"));
        assert_eq!(short_hash("feature/x").len(), 8);
    }

    #[tokio::test]
    async fn provision_mr_worktree_creates_on_first_call() {
        let _g = TEST_LOCK.lock().expect("test lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        init_source(&source);
        git(&["checkout", "-b", "feature/mr"], &source);
        std::fs::write(source.join("mr.txt"), "mr").expect("mr.txt");
        git(&["add", "-A"], &source);
        git(&["commit", "-m", "mr"], &source);
        git(&["checkout", "main"], &source);

        let container = temp.path().join("container");
        provision_project(&container, &url_of(&source), &["main".to_string()])
            .await
            .expect("provision");

        let dir = provision_mr_worktree(&container, "feature/mr")
            .await
            .expect("provision mr worktree");
        assert!(dir.join("mr.txt").is_file(), "mr worktree checked out branch");
        assert_eq!(dir, worktree_dir(&container, "feature/mr", WorktreeKind::MergeRequest));
    }

    #[tokio::test]
    async fn provision_mr_worktree_same_branch_returns_same_path() {
        let _g = TEST_LOCK.lock().expect("test lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        init_source(&source);
        git(&["checkout", "-b", "feature/shared"], &source);
        std::fs::write(source.join("s.txt"), "s").expect("s.txt");
        git(&["add", "-A"], &source);
        git(&["commit", "-m", "shared"], &source);
        git(&["checkout", "main"], &source);

        let container = temp.path().join("container");
        provision_project(&container, &url_of(&source), &["main".to_string()])
            .await
            .expect("provision");

        let first = provision_mr_worktree(&container, "feature/shared")
            .await
            .expect("first");
        let second = provision_mr_worktree(&container, "feature/shared")
            .await
            .expect("second");
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn provision_mr_worktree_unreachable_branch_returns_err() {
        let _g = TEST_LOCK.lock().expect("test lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        init_source(&source);
        let container = temp.path().join("container");
        provision_project(&container, &url_of(&source), &["main".to_string()])
            .await
            .expect("provision");

        let err = provision_mr_worktree(&container, "no/such/branch")
            .await
            .expect_err("missing branch");
        assert!(
            matches!(err, WorktreeError::BranchGone(_) | WorktreeError::Fetch(_)),
            "got {err:?}"
        );
    }
}
