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

pub struct MrReviewExecuteResult {
    pub outcome: ExecuteOutcome,
    pub duration_sec: i64,
    pub error: Option<String>,
    pub stdout: String,
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

    let mut command = build_weekly_command(config, working_dir, &manifest_path)?;
    command.stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(Error::Io)?;
    let mut stderr = child.stderr.take();
    let wait_result = time::timeout(Duration::from_secs(timeout_sec), child.wait()).await;

    let duration_sec = started.elapsed().as_secs() as i64;
    // Kill before draining stderr so timeout is not blocked on a still-running child.
    if wait_result.is_err() {
        kill_process_tree(&mut child).await;
    }
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
        Err(_) => Ok((ExecuteOutcome::SkippedTimeout, duration_sec, None)),
    }
}

pub async fn execute_mr_review(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
    timeout_sec: u64,
    _agent: ReviewerAgent,
) -> MrReviewExecuteResult {
    let started = Instant::now();
    let mut command = match build_mr_scan_command(config, working_dir, manifest_path) {
        Ok(command) => command,
        Err(err) => {
            return MrReviewExecuteResult {
                outcome: ExecuteOutcome::Failed,
                duration_sec: 0,
                error: Some(err.to_string()),
                stdout: String::new(),
            };
        }
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return MrReviewExecuteResult {
                outcome: ExecuteOutcome::Failed,
                duration_sec: 0,
                error: Some(format!("executor spawn failed: {err}")),
                stdout: String::new(),
            };
        }
    };

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let wait_result = time::timeout(Duration::from_secs(timeout_sec), child.wait()).await;

    let duration_sec = started.elapsed().as_secs() as i64;
    // On timeout, kill before draining pipes — otherwise read_to_end blocks until the
    // child exits and the configured timeout is effectively ignored.
    if wait_result.is_err() {
        kill_process_tree(&mut child).await;
    }
    let stdout_text = read_child_stdout(&mut stdout).await;
    let stderr_text = read_child_stderr(&mut stderr).await;

    match wait_result {
        Ok(Ok(status)) if status.success() => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Success,
            duration_sec,
            error: None,
            stdout: stdout_text,
        },
        Ok(Ok(status)) => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Failed,
            duration_sec,
            error: Some(format_executor_failure(&stderr_text, Some(&status))),
            stdout: stdout_text,
        },
        Ok(Err(err)) => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Failed,
            duration_sec,
            error: Some(if stderr_text.trim().is_empty() {
                format!("executor wait failed: {err}")
            } else {
                format_executor_failure(&stderr_text, None)
            }),
            stdout: stdout_text,
        },
        Err(_) => MrReviewExecuteResult {
            outcome: ExecuteOutcome::SkippedTimeout,
            duration_sec,
            error: None,
            stdout: stdout_text,
        },
    }
}

pub async fn execute_agent_turn(
    config: &AppConfig,
    working_dir: &Path,
    session_id: &str,
    message: &str,
    agent: ReviewerAgent,
) -> Result<(String, Option<String>), Error> {
    let mut command = build_agent_turn_command(config, working_dir, session_id, message, agent)?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(Error::Io)?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let status = child.wait().await.map_err(Error::Io)?;

    let stdout_text = read_child_stdout(&mut stdout).await;
    let stderr_text = read_child_stderr(&mut stderr).await;

    if !status.success() {
        return Err(Error::AgentFailed(format_executor_failure(
            &stderr_text,
            Some(&status),
        )));
    }

    let reply = extract_agent_reply(&stdout_text, agent)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Error::AgentFailed("agent returned empty reply".into()))?;
    let new_session = parse_agent_session_id(&stdout_text, agent);
    Ok((reply, new_session))
}

pub fn parse_agent_session_id(stdout: &str, agent: ReviewerAgent) -> Option<String> {
    let mut last_session = None;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
            last_session = Some(session_id.to_string());
        }
        if let Some(session_id) = value
            .pointer("/result/session_id")
            .and_then(|v| v.as_str())
        {
            last_session = Some(session_id.to_string());
        }
        match agent {
            ReviewerAgent::Cursor => {
                if value.get("type").and_then(|v| v.as_str()) == Some("system")
                    && value.get("subtype").and_then(|v| v.as_str()) == Some("init")
                {
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        last_session = Some(session_id.to_string());
                    }
                }
            }
            ReviewerAgent::Claude => {
                if value.get("type").and_then(|v| v.as_str()) == Some("result") {
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        last_session = Some(session_id.to_string());
                    }
                }
            }
        }
    }
    last_session
}

fn extract_agent_reply(stdout: &str, agent: ReviewerAgent) -> Option<String> {
    let mut chunks = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        if let Some(text) = value
            .pointer("/message/content/0/text")
            .and_then(|v| v.as_str())
        {
            chunks.push(text.to_string());
            continue;
        }
        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            chunks.push(text.to_string());
            continue;
        }
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            chunks.push(content.to_string());
        }
    }
    if !chunks.is_empty() {
        return Some(chunks.join("\n"));
    }

    if let Some(session_id) = parse_agent_session_id(stdout, agent) {
        return Some(format!("(session continued: {session_id})"));
    }
    None
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

async fn read_child_stdout(stdout: &mut Option<tokio::process::ChildStdout>) -> String {
    let Some(stdout) = stdout else {
        return String::new();
    };
    let mut buf = Vec::new();
    if stdout.read_to_end(&mut buf).await.is_err() {
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

fn build_weekly_command(
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

    let (workflow, contract) = weekly_skill_paths(config);
    match config.reviewer_agent() {
        ReviewerAgent::Claude => build_claude_command(
            config,
            working_dir,
            manifest_path,
            &[&workflow, &contract],
            true,
        ),
        ReviewerAgent::Cursor => build_cursor_command(
            config,
            working_dir,
            manifest_path,
            &[
                ("WORKFLOW", workflow.as_path()),
                ("OUTPUT CONTRACT", contract.as_path()),
            ],
        ),
    }
}

fn build_mr_scan_command(
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

    let (workflow, contract, observation) = mr_scan_skill_paths(config);
    match config.reviewer_agent() {
        ReviewerAgent::Claude => build_claude_command(
            config,
            working_dir,
            manifest_path,
            &[&workflow, &contract, &observation],
            false,
        ),
        ReviewerAgent::Cursor => build_cursor_command(
            config,
            working_dir,
            manifest_path,
            &[
                ("WORKFLOW", workflow.as_path()),
                ("OUTPUT CONTRACT (draft / inbox)", contract.as_path()),
                (
                    "OBSERVATION GUIDELINES (pending / manager)",
                    observation.as_path(),
                ),
            ],
        ),
    }
}

fn build_agent_turn_command(
    config: &AppConfig,
    working_dir: &Path,
    session_id: &str,
    message: &str,
    agent: ReviewerAgent,
) -> Result<Command, Error> {
    if let Some(program) = config.reviewer_executor() {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(program);
            return Ok(command);
        }
        return Ok(Command::new(program));
    }

    match agent {
        ReviewerAgent::Claude => {
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
                .arg("-p")
                .arg(message)
                .arg("--resume")
                .arg(session_id)
                .arg("--output-format")
                .arg("stream-json");
            append_model_arg(&mut command, config);
            Ok(command)
        }
        ReviewerAgent::Cursor => {
            let prompt = prepare_prompt_for_cli(message);
            let mut command = reviewer_command("cursor-agent");
            command
                .current_dir(working_dir)
                .arg("--print")
                .arg("--output-format")
                .arg("stream-json")
                .arg("--trust")
                .arg("--force")
                .arg("--resume")
                .arg(session_id);
            append_model_arg(&mut command, config);
            command.arg(prompt);
            Ok(command)
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

fn weekly_skill_paths(config: &AppConfig) -> (PathBuf, PathBuf) {
    let workflow_dir = config
        .app_root()
        .join("skills")
        .join("reviewer-batch");
    (
        workflow_dir.join("WORKFLOW.md"),
        workflow_dir.join("output-contract.md"),
    )
}

fn mr_scan_skill_paths(config: &AppConfig) -> (PathBuf, PathBuf, PathBuf) {
    let workflow_dir = config
        .app_root()
        .join("skills")
        .join("scan-mrs-headless");
    (
        workflow_dir.join("WORKFLOW.md"),
        workflow_dir.join("output-contract.md"),
        workflow_dir.join("observation-guidelines.md"),
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
    prompt_files: &[&Path],
    disable_session_persistence: bool,
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
        .arg(working_dir);
    for path in prompt_files {
        command.arg("--append-system-prompt-file").arg(path);
    }
    append_model_arg(&mut command, config);
    command
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json");
    if disable_session_persistence {
        command.arg("--no-session-persistence");
    }

    Ok(command)
}

fn build_cursor_command(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
    prompt_sections: &[(&str, &Path)],
) -> Result<Command, Error> {
    let mut prompt = base_reviewer_prompt(manifest_path);
    for (label, path) in prompt_sections {
        let text = std::fs::read_to_string(path).map_err(Error::Io)?;
        prompt.push_str("\n\n--- ");
        prompt.push_str(label);
        prompt.push_str(" ---\n");
        prompt.push_str(&text);
    }
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

    #[test]
    fn parse_agent_session_id_reads_claude_result() {
        let stdout = r#"{"type":"result","session_id":"claude-sess-1"}
{"type":"assistant","message":{"content":[{"text":"hi"}]}}"#;
        assert_eq!(
            parse_agent_session_id(stdout, ReviewerAgent::Claude).as_deref(),
            Some("claude-sess-1")
        );
    }

    #[test]
    fn parse_agent_session_id_reads_cursor_init() {
        let stdout = r#"{"type":"system","subtype":"init","session_id":"cursor-sess-9"}
{"type":"assistant","text":"done"}"#;
        assert_eq!(
            parse_agent_session_id(stdout, ReviewerAgent::Cursor).as_deref(),
            Some("cursor-sess-9")
        );
    }

    #[test]
    fn parse_agent_session_id_returns_none_without_session() {
        let stdout = r#"{"type":"assistant","text":"only reply"}"#;
        assert!(parse_agent_session_id(stdout, ReviewerAgent::Cursor).is_none());
    }

    #[test]
    fn weekly_claude_command_includes_no_session_persistence() {
        let config = test_config();
        let manifest = config.app_root.join("manifest.json");
        let command = build_weekly_command(&config, config.app_root(), &manifest).expect("cmd");
        let args = command_args(&command);
        assert!(args.iter().any(|arg| arg == "--no-session-persistence"));
    }

    #[test]
    fn mr_scan_claude_command_omits_no_session_persistence() {
        let config = test_config();
        let manifest = config.app_root.join("manifest.json");
        let command = build_mr_scan_command(&config, config.app_root(), &manifest).expect("cmd");
        let args = command_args(&command);
        assert!(!args.iter().any(|arg| arg == "--no-session-persistence"));
    }

    #[test]
    fn mr_scan_claude_command_appends_observation_guidelines() {
        let config = test_config();
        let manifest = config.app_root.join("manifest.json");
        let command = build_mr_scan_command(&config, config.app_root(), &manifest).expect("cmd");
        let args = command_args(&command);
        let joined = args.join(" ");
        assert!(
            joined.contains("observation-guidelines.md"),
            "expected observation guidelines in append files, got: {joined}"
        );
        assert!(
            joined.contains("output-contract.md"),
            "expected draft contract in append files, got: {joined}"
        );
    }

    fn test_config() -> AppConfig {
        AppConfig {
            data_dir: PathBuf::from("/data"),
            port: 8080,
            projects_config_path: PathBuf::from("projects.yaml"),
            app_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."),
            cors_allow_origins: Vec::new(),
            reviewer_agent: ReviewerAgent::Claude,
            reviewer_model: None,
            reviewer_executor: None,
        }
    }

    fn command_args(command: &Command) -> Vec<String> {
        command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }
}
