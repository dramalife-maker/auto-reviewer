use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::process::Command;
use tokio::time;

use crate::config::AppConfig;
use crate::runs::{ProjectRow, write_weekly_manifest};
use crate::Error;

const REVIEWER_AGENT_ENV: &str = "REVIEWER_AGENT";
const REVIEWER_CURSOR_MODEL_ENV: &str = "REVIEWER_CURSOR_MODEL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOutcome {
    Success,
    SkippedTimeout,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentProvider {
    Claude,
    Cursor,
}

pub async fn execute_weekly_batch(
    pool: &SqlitePool,
    config: &AppConfig,
    run_id: i64,
    project: &ProjectRow,
    working_dir: &Path,
    timeout_sec: u64,
) -> Result<(ExecuteOutcome, i64, Option<String>), Error> {
    let working_dir_str = working_dir.display().to_string();
    let manifest_path =
        write_weekly_manifest(pool, config.data_dir(), run_id, project, &working_dir_str).await?;
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

    let (workflow, contract) = reviewer_skill_paths(config);
    match agent_provider_from_env() {
        AgentProvider::Claude => build_claude_command(config, working_dir, manifest_path, &workflow, &contract),
        AgentProvider::Cursor => {
            build_cursor_command(config, working_dir, manifest_path, &workflow, &contract)
        }
    }
}

fn agent_provider_from_env() -> AgentProvider {
    match std::env::var(REVIEWER_AGENT_ENV)
        .unwrap_or_else(|_| "cursor".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "claude" => AgentProvider::Claude,
        _ => AgentProvider::Cursor,
    }
}

fn reviewer_skill_paths(config: &AppConfig) -> (PathBuf, PathBuf) {
    let workflow_dir = config
        .app_root()
        .join("skills")
        .join("reviewer-batch");
    (
        workflow_dir.join("WORKFLOW.md"),
        workflow_dir.join("output-contract.md"),
    )
}

fn base_reviewer_prompt(manifest_path: &Path) -> String {
    let manifest_str = manifest_path.display().to_string();
    format!(
        "Headless run. First Read manifest: {manifest_str}. Follow appended workflow. Non-interactive; do not ask questions. Write only under paths in manifest."
    )
}

fn build_claude_command(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
    workflow: &Path,
    contract: &Path,
) -> Result<Command, Error> {
    let prompt = base_reviewer_prompt(manifest_path);

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
        .arg(workflow)
        .arg("--append-system-prompt-file")
        .arg(contract)
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--no-session-persistence");

    Ok(command)
}

fn build_cursor_command(
    _config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
    workflow: &Path,
    contract: &Path,
) -> Result<Command, Error> {
    let workflow_text = std::fs::read_to_string(workflow).map_err(Error::Io)?;
    let contract_text = std::fs::read_to_string(contract).map_err(Error::Io)?;
    let prompt = format!(
        "{}\n\n--- WORKFLOW ---\n{workflow_text}\n\n--- OUTPUT CONTRACT ---\n{contract_text}",
        base_reviewer_prompt(manifest_path)
    );
    let prompt = prepare_prompt_for_cli(&prompt);

    let mut command = Command::new("cursor-agent");
    command
        .current_dir(working_dir)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--trust")
        .arg("--force");

    if let Ok(model) = std::env::var(REVIEWER_CURSOR_MODEL_ENV) {
        let model = model.trim();
        if !model.is_empty() {
            command.arg("--model").arg(model);
        }
    }

    command.arg(prompt);

    Ok(command)
}

/// Windows `cmd.exe` truncates arguments at LF; cursor-agent has no stdin prompt mode.
fn prepare_prompt_for_cli(prompt: &str) -> String {
    if cfg!(windows) && prompt.contains('\n') {
        prompt.replace('\n', r"\n")
    } else {
        prompt.to_string()
    }
}

fn executor_program() -> PathBuf {
    std::env::var("REVIEWER_EXECUTOR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("claude"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(name: &str, previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }

    #[test]
    fn agent_provider_defaults_to_cursor() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::remove_var(REVIEWER_AGENT_ENV);
        assert_eq!(agent_provider_from_env(), AgentProvider::Cursor);
        restore_env(REVIEWER_AGENT_ENV, previous);
    }

    #[test]
    fn agent_provider_reads_claude() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::set_var(REVIEWER_AGENT_ENV, "claude");
        assert_eq!(agent_provider_from_env(), AgentProvider::Claude);
        restore_env(REVIEWER_AGENT_ENV, previous);
    }

    #[test]
    fn agent_provider_reads_cursor() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::set_var(REVIEWER_AGENT_ENV, "cursor");
        assert_eq!(agent_provider_from_env(), AgentProvider::Cursor);
        restore_env(REVIEWER_AGENT_ENV, previous);
    }

    #[test]
    fn prepare_prompt_for_cli_replaces_lf_on_windows() {
        let input = "line one\nline two";
        let output = prepare_prompt_for_cli(input);
        if cfg!(windows) {
            assert_eq!(output, r"line one\nline two");
        } else {
            assert_eq!(output, input);
        }
    }
}
