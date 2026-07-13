//! Precompute MR change materials (log / stat / diff) before spawning the agent.
//!
//! Agents often burn the per-MR timeout on `git fetch` + full `git diff`. The
//! worker writes these files once so the skill only Reads them.

use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Soft cap for `change.diff`. Keep small enough that one Read covers the
/// overview; agents that page through a 2k-line diff burn the whole timeout.
pub const DEFAULT_DIFF_MAX_BYTES: usize = 48 * 1024;

const TRUNCATION_MARKER: &str = "\n\n--- TRUNCATED: full diff exceeded limit; use `git diff origin/<target_branch>...HEAD -- <path>` for remaining files ---\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeMaterialPaths {
    pub change_log_path: PathBuf,
    pub change_stat_path: PathBuf,
    pub change_diff_path: PathBuf,
    pub diff_truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ChangeMaterialsError {
    #[error("target_branch is empty")]
    EmptyTargetBranch,
    #[error("git fetch origin/{0} failed: {1}")]
    Fetch(String, String),
    #[error("git {0} failed: {1}")]
    Git(String, String),
    #[error("io error: {0}")]
    Io(String),
}

/// Run-layout directory for one MR's precomputed change files.
pub fn mr_change_materials_dir(
    data_root: &Path,
    run_id: i64,
    project_id: i64,
    mr_iid: i64,
) -> PathBuf {
    data_root
        .join("runs")
        .join(run_id.to_string())
        .join("projects")
        .join(project_id.to_string())
        .join(format!("mr-{mr_iid}"))
}

/// Stub files for `REVIEWER_EXECUTOR` / integration tests (no real git).
pub fn write_stub_change_materials(out_dir: &Path) -> Result<ChangeMaterialPaths, ChangeMaterialsError> {
    std::fs::create_dir_all(out_dir).map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    let change_log_path = out_dir.join("change_log.txt");
    let change_stat_path = out_dir.join("change_stat.txt");
    let change_diff_path = out_dir.join("change.diff");
    std::fs::write(&change_log_path, "stub commit\n")
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    std::fs::write(&change_stat_path, " stub.txt | 1 +\n 1 file changed, 1 insertion(+)\n")
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    std::fs::write(&change_diff_path, "diff --git a/stub.txt b/stub.txt\n")
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    Ok(ChangeMaterialPaths {
        change_log_path,
        change_stat_path,
        change_diff_path,
        diff_truncated: false,
    })
}

/// Fetch `origin/<target_branch>` then write log / stat / capped diff under `out_dir`.
pub async fn prepare_change_materials(
    worktree: &Path,
    target_branch: &str,
    out_dir: &Path,
    max_diff_bytes: usize,
) -> Result<ChangeMaterialPaths, ChangeMaterialsError> {
    let tb = target_branch.trim();
    if tb.is_empty() {
        return Err(ChangeMaterialsError::EmptyTargetBranch);
    }

    std::fs::create_dir_all(out_dir).map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;

    let fetch = run_git(worktree, &["fetch", "origin", tb]).await?;
    if !fetch.success {
        return Err(ChangeMaterialsError::Fetch(
            tb.to_string(),
            fetch.stderr.trim().to_string(),
        ));
    }

    let range = format!("origin/{tb}...HEAD");
    let log = run_git(worktree, &["log", "--oneline", &range]).await?;
    if !log.success {
        return Err(ChangeMaterialsError::Git(
            "log".into(),
            log.stderr.trim().to_string(),
        ));
    }
    let stat = run_git(worktree, &["diff", "--stat", &range]).await?;
    if !stat.success {
        return Err(ChangeMaterialsError::Git(
            "diff --stat".into(),
            stat.stderr.trim().to_string(),
        ));
    }
    let diff = run_git(worktree, &["diff", &range]).await?;
    if !diff.success {
        return Err(ChangeMaterialsError::Git(
            "diff".into(),
            diff.stderr.trim().to_string(),
        ));
    }

    let (diff_bytes, diff_truncated) = truncate_diff(&diff.stdout, max_diff_bytes);

    let change_log_path = out_dir.join("change_log.txt");
    let change_stat_path = out_dir.join("change_stat.txt");
    let change_diff_path = out_dir.join("change.diff");

    std::fs::write(&change_log_path, &log.stdout)
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    std::fs::write(&change_stat_path, &stat.stdout)
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    std::fs::write(&change_diff_path, &diff_bytes)
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;

    Ok(ChangeMaterialPaths {
        change_log_path,
        change_stat_path,
        change_diff_path,
        diff_truncated,
    })
}

struct GitCapture {
    success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

async fn run_git(cwd: &Path, args: &[&str]) -> Result<GitCapture, ChangeMaterialsError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .await
        .map_err(|e| ChangeMaterialsError::Io(e.to_string()))?;
    Ok(GitCapture {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn truncate_diff(bytes: &[u8], max: usize) -> (Vec<u8>, bool) {
    if bytes.len() <= max {
        return (bytes.to_vec(), false);
    }
    let mut out = bytes[..max].to_vec();
    out.extend_from_slice(TRUNCATION_MARKER.as_bytes());
    (out, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

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

    /// Local repo with `origin/main` remotely reachable via a bare clone.
    fn fixture_feature_worktree(root: &Path) -> PathBuf {
        let remote = root.join("remote.git");
        let work = root.join("work");
        std::fs::create_dir_all(&work).expect("work");
        git(&["init", "-b", "main", "."], &work);
        git(&["config", "user.email", "t@e.com"], &work);
        git(&["config", "user.name", "T"], &work);
        std::fs::write(work.join("base.txt"), "base\n").expect("base");
        git(&["add", "-A"], &work);
        git(&["commit", "-m", "base"], &work);

        // Bare remote from current main.
        let work_url = work.display().to_string().replace('\\', "/");
        let remote_url = remote.display().to_string().replace('\\', "/");
        git(&["clone", "--bare", &work_url, &remote_url], root);
        git(&["remote", "add", "origin", &remote_url], &work);
        git(&["fetch", "origin", "main"], &work);

        git(&["checkout", "-b", "feature"], &work);
        std::fs::write(work.join("feat.txt"), "feat\n").expect("feat");
        git(&["add", "-A"], &work);
        git(&["commit", "-m", "feat"], &work);
        work
    }

    #[test]
    fn truncate_diff_marks_overflow() {
        let (out, truncated) = truncate_diff(b"abcdefghij", 4);
        assert!(truncated);
        assert!(out.starts_with(b"abcd"));
        assert!(String::from_utf8_lossy(&out).contains("TRUNCATED"));
    }

    #[test]
    fn truncate_diff_keeps_small_payload() {
        let (out, truncated) = truncate_diff(b"abc", 10);
        assert!(!truncated);
        assert_eq!(out, b"abc");
    }

    #[test]
    fn stub_materials_write_three_files() {
        let temp = tempfile::tempdir().expect("temp");
        let paths = write_stub_change_materials(temp.path()).expect("stub");
        assert!(paths.change_log_path.is_file());
        assert!(paths.change_stat_path.is_file());
        assert!(paths.change_diff_path.is_file());
        assert!(!paths.diff_truncated);
    }

    #[tokio::test]
    async fn prepare_writes_log_stat_diff() {
        let temp = tempfile::tempdir().expect("temp");
        let work = fixture_feature_worktree(temp.path());
        let out = temp.path().join("materials");
        let paths = prepare_change_materials(&work, "main", &out, DEFAULT_DIFF_MAX_BYTES)
            .await
            .expect("prepare");

        let log = std::fs::read_to_string(&paths.change_log_path).expect("log");
        let stat = std::fs::read_to_string(&paths.change_stat_path).expect("stat");
        let diff = std::fs::read_to_string(&paths.change_diff_path).expect("diff");
        assert!(log.contains("feat"), "log={log}");
        assert!(stat.contains("feat.txt"), "stat={stat}");
        assert!(diff.contains("feat.txt"), "diff={diff}");
        assert!(!paths.diff_truncated);
    }

    #[tokio::test]
    async fn prepare_truncates_large_diff() {
        let temp = tempfile::tempdir().expect("temp");
        let work = fixture_feature_worktree(temp.path());
        // Inflate the feature tip so the three-dot diff exceeds a tiny cap.
        let big = "x".repeat(8_000);
        std::fs::write(work.join("big.txt"), &big).expect("big");
        git(&["add", "-A"], &work);
        git(&["commit", "-m", "big"], &work);

        let out = temp.path().join("materials");
        let paths = prepare_change_materials(&work, "main", &out, 512)
            .await
            .expect("prepare");
        assert!(paths.diff_truncated);
        let diff = std::fs::read_to_string(&paths.change_diff_path).expect("diff");
        assert!(diff.contains("TRUNCATED"));
        assert!(diff.len() < 8_000);
    }

    #[tokio::test]
    async fn prepare_rejects_empty_target() {
        let temp = tempfile::tempdir().expect("temp");
        let err = prepare_change_materials(temp.path(), "  ", temp.path(), 100)
            .await
            .expect_err("empty");
        assert!(matches!(err, ChangeMaterialsError::EmptyTargetBranch));
    }
}
