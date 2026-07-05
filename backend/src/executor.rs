use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::time;

use crate::config::AppConfig;
use crate::runs::{ProjectRow, write_weekly_manifest};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOutcome {
    Success,
    SkippedTimeout,
    Failed,
}

pub async fn execute_weekly_batch(
    config: &AppConfig,
    run_id: i64,
    project: &ProjectRow,
    working_dir: &Path,
    timeout_sec: u64,
) -> Result<(ExecuteOutcome, i64, Option<String>), Error> {
    let working_dir_str = working_dir.display().to_string();
    let manifest_path =
        write_weekly_manifest(config.data_dir(), run_id, project, &working_dir_str).await?;
    let started = Instant::now();

    let mut command = build_command(config, working_dir, &manifest_path)?;
    command.stdout(Stdio::null()).stderr(Stdio::null());

    let mut child = command.spawn().map_err(Error::Io)?;
    let wait_result = time::timeout(Duration::from_secs(timeout_sec), child.wait()).await;

    let duration_sec = started.elapsed().as_secs() as i64;

    match wait_result {
        Ok(Ok(status)) if status.success() => Ok((ExecuteOutcome::Success, duration_sec, None)),
        Ok(Ok(status)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(format!("executor exited with {status}")),
        )),
        Ok(Err(err)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(format!("executor wait failed: {err}")),
        )),
        Err(_) => {
            let _ = child.kill().await;
            Ok((ExecuteOutcome::SkippedTimeout, duration_sec, None))
        }
    }
}

fn build_command(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
) -> Result<Command, Error> {
    if std::env::var("REVIEWER_EXECUTOR").is_ok() {
        let program = executor_program();
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(&program);
            return Ok(command);
        }
        return Ok(Command::new(program));
    }

    let app_root = config.app_root();
    let workflow_dir = app_root.join("skills").join("reviewer-batch");
    let workflow = workflow_dir.join("WORKFLOW.md");
    let contract = workflow_dir.join("output-contract.md");
    let manifest_str = manifest_path.display().to_string();
    let prompt = format!(
        "Headless run. First Read manifest: {manifest_str}. Follow appended workflow. Non-interactive; do not ask questions. Write only under paths in manifest."
    );

    let mut command = Command::new("claude");
    command
        .current_dir(working_dir)
        .arg("--bare")
        .arg("--permission-mode")
        .arg("dontAsk")
        .arg("--allowedTools")
        .arg("Bash,Read,Glob,Grep,Write")
        .arg("--add-dir")
        .arg(config.data_dir())
        .arg("--add-dir")
        .arg(working_dir)
        .arg("--append-system-prompt-file")
        .arg(&workflow)
        .arg("--append-system-prompt-file")
        .arg(&contract)
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--no-session-persistence");

    Ok(command)
}

fn executor_program() -> PathBuf {
    std::env::var("REVIEWER_EXECUTOR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("claude"))
}
