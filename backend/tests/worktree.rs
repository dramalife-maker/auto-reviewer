use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use reviewer_server::worktree::{
    provision_project, supply_worktree, worktree_dir, WorktreeError, WorktreeKind,
};

// The disk-full seam toggles a process-wide env var; serialize the module so it
// cannot leak into a concurrent test.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Non-bare source repo on `main` with one committed file, usable as a clone URL.
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

#[tokio::test]
async fn provision_creates_bare_and_resident_and_is_idempotent() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    let container = temp.path().join("container");
    let branches = vec!["main".to_string()];

    provision_project(&container, &url_of(&source), &branches)
        .await
        .expect("provision");

    assert!(container.join(".bare").is_dir(), ".bare created");
    assert!(container.join(".git").is_file(), "container .git file");
    assert!(container.join("main").join("a.txt").is_file(), "resident worktree checked out");

    // Refspec must be configured so fetches populate refs/remotes/origin/*.
    let refspec = Command::new("git")
        .args(["--git-dir", &container.join(".bare").display().to_string(),
               "config", "--get", "remote.origin.fetch"])
        .output()
        .expect("config get");
    assert_eq!(
        String::from_utf8_lossy(&refspec.stdout).trim(),
        "+refs/heads/*:refs/remotes/origin/*"
    );

    // Second provision is a no-op (idempotent), not an error.
    provision_project(&container, &url_of(&source), &branches)
        .await
        .expect("re-provision idempotent");
}

#[tokio::test]
async fn supply_resident_force_aligns_to_remote() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    let container = temp.path().join("container");
    provision_project(&container, &url_of(&source), &["main".to_string()])
        .await
        .expect("provision");

    // Advance remote main with a new file.
    std::fs::write(source.join("b.txt"), "b").expect("b.txt");
    git(&["add", "-A"], &source);
    git(&["commit", "-m", "second"], &source);

    let dir = supply_worktree(&container, "main", WorktreeKind::Resident)
        .await
        .expect("supply resident");
    assert!(dir.join("b.txt").is_file(), "worktree reset to latest remote commit");
}

#[tokio::test]
async fn supply_mr_branch_uses_stable_dir() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    // Add a feature branch with its own file.
    git(&["checkout", "-b", "feature/x"], &source);
    std::fs::write(source.join("f.txt"), "f").expect("f.txt");
    git(&["add", "-A"], &source);
    git(&["commit", "-m", "feature"], &source);
    git(&["checkout", "main"], &source);

    let container = temp.path().join("container");
    provision_project(&container, &url_of(&source), &["main".to_string()])
        .await
        .expect("provision");

    let first = supply_worktree(&container, "feature/x", WorktreeKind::MergeRequest)
        .await
        .expect("supply mr");
    assert!(first.join("f.txt").is_file(), "mr worktree checked out feature branch");
    assert_eq!(first, worktree_dir(&container, "feature/x", WorktreeKind::MergeRequest));

    // Same source branch → same worktree dir (natural dedup).
    let second = supply_worktree(&container, "feature/x", WorktreeKind::MergeRequest)
        .await
        .expect("supply mr again");
    assert_eq!(first, second);
}

#[tokio::test]
async fn deleted_remote_branch_removes_worktree() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    git(&["branch", "feature"], &source);

    let container = temp.path().join("container");
    provision_project(&container, &url_of(&source), &["main".to_string()])
        .await
        .expect("provision");
    let dir = supply_worktree(&container, "feature", WorktreeKind::MergeRequest)
        .await
        .expect("supply feature");
    assert!(dir.is_dir());

    git(&["branch", "-D", "feature"], &source);

    let err = supply_worktree(&container, "feature", WorktreeKind::MergeRequest)
        .await
        .expect_err("branch gone");
    assert!(matches!(err, WorktreeError::BranchGone(_)), "got {err:?}");
    assert!(!dir.exists(), "worktree removed when remote branch is gone");
}

#[tokio::test]
async fn transient_fetch_failure_leaves_worktree_untouched() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    let container = temp.path().join("container");
    provision_project(&container, &url_of(&source), &["main".to_string()])
        .await
        .expect("provision");
    let dir = supply_worktree(&container, "main", WorktreeKind::Resident)
        .await
        .expect("supply");
    assert!(dir.join("a.txt").is_file());

    // Remove the whole source: fetch now fails transiently (not "ref gone").
    std::fs::remove_dir_all(&source).expect("rm source");
    let err = supply_worktree(&container, "main", WorktreeKind::Resident)
        .await
        .expect_err("transient fetch failure");
    assert!(matches!(err, WorktreeError::Fetch(_)), "got {err:?}");
    assert!(dir.join("a.txt").is_file(), "worktree content unchanged after failed fetch");
}

#[tokio::test]
async fn low_disk_refuses_provision_without_panic() {
    let _g = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source(&source);
    let container = temp.path().join("container");

    std::env::set_var("REVIEWER_TEST_FORCE_LOW_DISK", "1");
    let result = provision_project(&container, &url_of(&source), &["main".to_string()]).await;
    std::env::remove_var("REVIEWER_TEST_FORCE_LOW_DISK");

    assert!(matches!(result, Err(WorktreeError::DiskFull)), "got {result:?}");
    assert!(!container.join(".bare").exists(), "nothing provisioned under low disk");
}
