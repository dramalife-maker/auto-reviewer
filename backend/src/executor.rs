use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, Command};
use tokio::time;

use crate::config::{AppConfig, ReviewerAgent};
use crate::runs::{ProjectRow, write_weekly_manifest};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOutcome {
    Success,
    SkippedTimeout,
    Failed,
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
    command.stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(Error::Io)?;
    let mut stderr = child.stderr.take();
    let wait_result = time::timeout(Duration::from_secs(timeout_sec), child.wait()).await;

    let duration_sec = started.elapsed().as_secs() as i64;
    let stderr_text = read_child_stderr(&mut stderr).await;

    match wait_result {
        Ok(Ok(status)) if status.success() => Ok((ExecuteOutcome::Success, duration_sec, None)),
        Ok(Ok(status)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(format_executor_failure(&stderr_text, Some(&status))),
        )),
        Ok(Err(err)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(if stderr_text.trim().is_empty() {
                format!("executor wait failed: {err}")
            } else {
                format_executor_failure(&stderr_text, None)
            }),
        )),
        Err(_) => {
            kill_process_tree(&mut child).await;
            Ok((ExecuteOutcome::SkippedTimeout, duration_sec, None))
        }
    }
}

/// Kill the reviewer subprocess and all of its descendants.
///
/// `Child::kill` only terminates the direct child. On Windows the reviewer is
/// launched through a `cmd.exe` shim (`cursor-agent.cmd` -> node), so a bare
/// kill orphans the node process; `taskkill /F /T` tears down the whole tree.
async fn kill_process_tree(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    if let Some(pid) = child.id() {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await;
    }
    let _ = child.kill().await;
}

async fn read_child_stderr(stderr: &mut Option<ChildStderr>) -> String {
    let Some(stderr) = stderr else {
        return String::new();
    };
    let mut buf = Vec::new();
    if stderr.read_to_end(&mut buf).await.is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

const AUTH_FAILURE_MESSAGE: &str =
    "cursor-agent 認證失敗：請在本機執行 cursor-agent login（須與啟動 reviewer-server 的使用者相同）";

fn is_auth_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("authentication required")
        || lower.contains("agent login")
        || lower.contains("not logged in")
        || lower.contains("unauthorized")
}

fn format_executor_failure(stderr: &str, status: Option<&std::process::ExitStatus>) -> String {
    if is_auth_failure(stderr) {
        return AUTH_FAILURE_MESSAGE.to_string();
    }
    let trimmed = stderr.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Some(status) = status {
        format!("executor exited with {status}")
    } else {
        "executor failed".to_string()
    }
}

fn build_command(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
) -> Result<Command, Error> {
    if let Some(program) = config.reviewer_executor() {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(program);
            return Ok(command);
        }
        return Ok(Command::new(program));
    }

    let (workflow, contract) = reviewer_skill_paths(config);
    match config.reviewer_agent() {
        ReviewerAgent::Claude => {
            build_claude_command(config, working_dir, manifest_path, &workflow, &contract)
        }
        ReviewerAgent::Cursor => {
            build_cursor_command(config, working_dir, manifest_path, &workflow, &contract)
        }
    }
}

/// Build a `Command` for a reviewer CLI, resolving Windows shims robustly.
///
/// On Windows `cursor-agent` ships as a `cursor-agent.cmd` shim whose body is
/// `powershell -File cursor-agent.ps1 %*`. Spawning the `.cmd` via `Command`
/// routes it through `cmd.exe`, and the shim's `%*` then re-parses the args a
/// SECOND time in PowerShell. Our reviewer prompt embeds the whole workflow +
/// contract (full of `"`, backticks, `{}`), and that double parse shreds it —
/// PowerShell ends up treating a fragment as a parameter name and fails with
/// "the value of argument name is not valid". Invoking the sibling `.ps1`
/// directly through `powershell.exe -File` keeps a single parse layer, which
/// handles arbitrary prompt content correctly. A real `.exe` (e.g. `claude`)
/// needs none of this and is spawned directly.
fn reviewer_command(program: &str) -> Command {
    let resolved = which::which(program).unwrap_or_else(|_| PathBuf::from(program));

    #[cfg(windows)]
    {
        let is_shim = resolved
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if is_shim {
            let ps1 = resolved.with_extension("ps1");
            if ps1.exists() {
                let mut command = Command::new("powershell.exe");
                command
                    .arg("-NoProfile")
                    .arg("-ExecutionPolicy")
                    .arg("Bypass")
                    .arg("-File")
                    .arg(ps1);
                return command;
            }
        }
    }

    Command::new(resolved)
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

    let mut command = reviewer_command("claude");
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
        .arg(contract);
    append_model_arg(&mut command, config);
    command
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--no-session-persistence");

    Ok(command)
}

fn build_cursor_command(
    config: &AppConfig,
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

    let mut command = reviewer_command("cursor-agent");
    command
        .current_dir(working_dir)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--trust")
        .arg("--force");
    append_model_arg(&mut command, config);
    command.arg(prompt);

    Ok(command)
}

/// Append `--model` only when `REVIEWER_MODEL` is set.
fn append_model_arg(command: &mut Command, config: &AppConfig) {
    if let Some(model) = config.reviewer_model() {
        command.arg("--model").arg(model);
    }
}

/// Windows `cmd.exe` truncates arguments at LF; cursor-agent has no stdin prompt mode.
fn prepare_prompt_for_cli(prompt: &str) -> String {
    if cfg!(windows) && prompt.contains('\n') {
        prompt.replace('\n', r"\n")
    } else {
        prompt.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn format_executor_failure_detects_auth_error() {
        let stderr = "Error: Authentication required. Please run 'agent login' first";
        assert_eq!(format_executor_failure(stderr, None), AUTH_FAILURE_MESSAGE);
    }

    #[test]
    fn format_executor_failure_uses_stderr_when_present() {
        assert_eq!(
            format_executor_failure("something went wrong", None),
            "something went wrong"
        );
    }
}
