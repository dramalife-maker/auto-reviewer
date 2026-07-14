use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, ChildStdout, Command};
use tokio::task::JoinHandle;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::config::{AppConfig, ReviewerAgent};
use crate::runs::{ProjectRow, write_weekly_manifest, SHUTDOWN_INTERRUPTED_ERROR};
use crate::Error;

/// The result of racing a child process wait against timeout and shutdown
/// cancellation.
enum WaitOutcome {
    Exited(std::io::Result<std::process::ExitStatus>),
    TimedOut,
    Cancelled,
}

/// Race a child's `wait()` against a timeout and a shutdown cancellation
/// token. Cancellation and timeout are mutually exclusive outcomes — if the
/// token fires first, the result is `Cancelled` even if the timeout would
/// also have elapsed around the same moment.
async fn wait_with_cancel(
    child: &mut tokio::process::Child,
    timeout_sec: u64,
    cancel: &CancellationToken,
) -> WaitOutcome {
    tokio::select! {
        _ = cancel.cancelled() => WaitOutcome::Cancelled,
        result = time::timeout(Duration::from_secs(timeout_sec), child.wait()) => {
            match result {
                Ok(status) => WaitOutcome::Exited(status),
                Err(_) => WaitOutcome::TimedOut,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOutcome {
    Success,
    SkippedTimeout,
    Failed,
}

pub struct MrReviewExecuteResult {
    pub outcome: ExecuteOutcome,
    pub duration_sec: i64,
    /// Wall time spent waiting for the child (until exit or timeout).
    pub wait_ms: u64,
    /// Time spent killing (on timeout) + draining stdout/stderr pipes.
    pub drain_ms: u64,
    pub error: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

pub async fn execute_weekly_batch(
    pool: &SqlitePool,
    config: &AppConfig,
    run_id: i64,
    project: &ProjectRow,
    working_dir: &Path,
    timeout_sec: u64,
    cancel: CancellationToken,
) -> Result<(ExecuteOutcome, i64, Option<String>), Error> {
    let working_dir_str = working_dir.display().to_string();
    let manifest_path =
        write_weekly_manifest(pool, config.data_dir(), run_id, project, &working_dir_str).await?;
    let started = Instant::now();

    let mut command = build_weekly_command(config, working_dir, &manifest_path)?;
    command.stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(Error::Io)?;
    let stderr_task = spawn_stderr_drain(child.stderr.take());
    let wait_outcome = wait_with_cancel(&mut child, timeout_sec, &cancel).await;

    let duration_sec = started.elapsed().as_secs() as i64;
    // Kill before awaiting drain so timeout/cancel is not blocked on a still-running child.
    if !matches!(wait_outcome, WaitOutcome::Exited(_)) {
        kill_process_tree(&mut child).await;
    }
    let stderr_text = join_drain(stderr_task).await;

    match wait_outcome {
        WaitOutcome::Exited(Ok(status)) if status.success() => {
            Ok((ExecuteOutcome::Success, duration_sec, None))
        }
        WaitOutcome::Exited(Ok(status)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(format_executor_failure(&stderr_text, Some(&status))),
        )),
        WaitOutcome::Exited(Err(err)) => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(if stderr_text.trim().is_empty() {
                format!("executor wait failed: {err}")
            } else {
                format_executor_failure(&stderr_text, None)
            }),
        )),
        WaitOutcome::TimedOut => Ok((ExecuteOutcome::SkippedTimeout, duration_sec, None)),
        WaitOutcome::Cancelled => Ok((
            ExecuteOutcome::Failed,
            duration_sec,
            Some(SHUTDOWN_INTERRUPTED_ERROR.to_string()),
        )),
    }
}

pub async fn execute_mr_review(
    config: &AppConfig,
    working_dir: &Path,
    manifest_path: &Path,
    timeout_sec: u64,
    _agent: ReviewerAgent,
    cancel: CancellationToken,
) -> MrReviewExecuteResult {
    let started = Instant::now();
    let mut command = match build_mr_scan_command(config, working_dir, manifest_path) {
        Ok(command) => command,
        Err(err) => {
            return MrReviewExecuteResult {
                outcome: ExecuteOutcome::Failed,
                duration_sec: 0,
                wait_ms: 0,
                drain_ms: 0,
                error: Some(err.to_string()),
                stdout: String::new(),
                stderr: String::new(),
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
                wait_ms: 0,
                drain_ms: 0,
                error: Some(format!("executor spawn failed: {err}")),
                stdout: String::new(),
                stderr: String::new(),
            };
        }
    };

    // Drain pipes while waiting. If we only read after wait(), a chatty
    // stream-json agent fills the ~64KiB pipe buffer and blocks forever
    // (classic deadlock); wall timeout then looks like "agent too slow".
    let stdout_task = spawn_stdout_drain(child.stdout.take());
    let stderr_task = spawn_stderr_drain(child.stderr.take());
    let wait_started = Instant::now();
    let wait_outcome = wait_with_cancel(&mut child, timeout_sec, &cancel).await;
    let wait_ms = wait_started.elapsed().as_millis() as u64;

    let drain_started = Instant::now();
    if !matches!(wait_outcome, WaitOutcome::Exited(_)) {
        kill_process_tree(&mut child).await;
    }
    let stdout_text = join_drain(stdout_task).await;
    let stderr_text = join_drain(stderr_task).await;
    let drain_ms = drain_started.elapsed().as_millis() as u64;

    let duration_sec = started.elapsed().as_secs() as i64;

    match wait_outcome {
        WaitOutcome::Exited(Ok(status)) if status.success() => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Success,
            duration_sec,
            wait_ms,
            drain_ms,
            error: None,
            stdout: stdout_text,
            stderr: stderr_text,
        },
        WaitOutcome::Exited(Ok(status)) => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Failed,
            duration_sec,
            wait_ms,
            drain_ms,
            error: Some(format_executor_failure(&stderr_text, Some(&status))),
            stdout: stdout_text,
            stderr: stderr_text,
        },
        WaitOutcome::Exited(Err(err)) => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Failed,
            duration_sec,
            wait_ms,
            drain_ms,
            error: Some(if stderr_text.trim().is_empty() {
                format!("executor wait failed: {err}")
            } else {
                format_executor_failure(&stderr_text, None)
            }),
            stdout: stdout_text,
            stderr: stderr_text,
        },
        WaitOutcome::TimedOut => MrReviewExecuteResult {
            outcome: ExecuteOutcome::SkippedTimeout,
            duration_sec,
            wait_ms,
            drain_ms,
            error: None,
            stdout: stdout_text,
            stderr: stderr_text,
        },
        WaitOutcome::Cancelled => MrReviewExecuteResult {
            outcome: ExecuteOutcome::Failed,
            duration_sec,
            wait_ms,
            drain_ms,
            error: Some(SHUTDOWN_INTERRUPTED_ERROR.to_string()),
            stdout: stdout_text,
            stderr: stderr_text,
        },
    }
}

pub async fn execute_agent_turn(
    config: &AppConfig,
    working_dir: &Path,
    session_id: &str,
    message: &str,
    notes_dir: &str,
    agent: ReviewerAgent,
    cancel: CancellationToken,
) -> Result<(String, Option<String>), Error> {
    let mut command =
        build_agent_turn_command(config, working_dir, session_id, message, notes_dir, agent)?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(Error::Io)?;
    let stdout_task = spawn_stdout_drain(child.stdout.take());
    let stderr_task = spawn_stderr_drain(child.stderr.take());

    let status = tokio::select! {
        _ = cancel.cancelled() => {
            kill_process_tree(&mut child).await;
            let _ = join_drain(stdout_task).await;
            let _ = join_drain(stderr_task).await;
            return Err(Error::AgentFailed(SHUTDOWN_INTERRUPTED_ERROR.to_string()));
        }
        status = child.wait() => status.map_err(Error::Io)?,
    };

    let stdout_text = join_drain(stdout_task).await;
    let stderr_text = join_drain(stderr_task).await;

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

fn spawn_stdout_drain(stdout: Option<ChildStdout>) -> JoinHandle<String> {
    tokio::spawn(async move {
        let Some(mut stdout) = stdout else {
            return String::new();
        };
        let mut buf = Vec::new();
        if stdout.read_to_end(&mut buf).await.is_err() {
            return String::new();
        }
        String::from_utf8_lossy(&buf).into_owned()
    })
}

fn spawn_stderr_drain(stderr: Option<ChildStderr>) -> JoinHandle<String> {
    tokio::spawn(async move {
        let Some(mut stderr) = stderr else {
            return String::new();
        };
        let mut buf = Vec::new();
        if stderr.read_to_end(&mut buf).await.is_err() {
            return String::new();
        }
        String::from_utf8_lossy(&buf).into_owned()
    })
}

async fn join_drain(handle: JoinHandle<String>) -> String {
    handle.await.unwrap_or_default()
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

/// Compact view of agent stream-json stdout + stderr for diagnostics.
#[derive(Debug, Clone)]
pub struct AgentOutputSummary {
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub stdout_lines: usize,
    pub event_types: String,
    pub last_event_type: Option<String>,
    pub last_assistant_snippet: Option<String>,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

const AGENT_LOG_TAIL_CHARS: usize = 1200;
const ASSISTANT_SNIPPET_CHARS: usize = 240;

/// Summarize agent pipes without dumping the full stream-json transcript.
pub fn summarize_agent_output(stdout: &str, stderr: &str) -> AgentOutputSummary {
    let mut type_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut last_event_type = None;
    let mut last_assistant = None;
    let mut stdout_lines = 0usize;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        stdout_lines += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let event_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        *type_counts.entry(event_type.clone()).or_default() += 1;
        last_event_type = Some(event_type.clone());

        if event_type == "assistant" {
            if let Some(text) = value
                .pointer("/message/content/0/text")
                .and_then(|v| v.as_str())
                .or_else(|| value.get("text").and_then(|v| v.as_str()))
                .or_else(|| value.get("content").and_then(|v| v.as_str()))
            {
                last_assistant = Some(truncate_chars(text.trim(), ASSISTANT_SNIPPET_CHARS));
            }
        }
    }

    let event_types = type_counts
        .iter()
        .map(|(name, count)| format!("{name}={count}"))
        .collect::<Vec<_>>()
        .join(",");

    AgentOutputSummary {
        stdout_bytes: stdout.len(),
        stderr_bytes: stderr.len(),
        stdout_lines,
        event_types,
        last_event_type,
        last_assistant_snippet: last_assistant,
        stdout_tail: tail_chars(stdout, AGENT_LOG_TAIL_CHARS),
        stderr_tail: tail_chars(stderr, AGENT_LOG_TAIL_CHARS),
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn tail_chars(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let count = trimmed.chars().count();
    if count <= max_chars {
        return trimmed.to_string();
    }
    let skip = count - max_chars;
    let mut out = String::from("…");
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= skip {
            out.push(ch);
        }
    }
    out
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
    let prompt = base_reviewer_prompt(manifest_path);
    match config.reviewer_agent() {
        ReviewerAgent::Claude => build_claude_command(
            config,
            working_dir,
            &prompt,
            &[&workflow, &contract],
            true,
        ),
        ReviewerAgent::Cursor => build_cursor_command(
            config,
            working_dir,
            &prompt,
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
    let prompt = mr_scan_reviewer_prompt(manifest_path);
    match config.reviewer_agent() {
        ReviewerAgent::Claude => build_claude_command(
            config,
            working_dir,
            &prompt,
            &[&workflow, &contract, &observation],
            false,
        ),
        ReviewerAgent::Cursor => build_cursor_command(
            config,
            working_dir,
            &prompt,
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
    notes_dir: &str,
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

    let prompt = agent_turn_prompt(notes_dir, message);
    let skill_path = adr_notes_skill_path(config);

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
                .arg("--append-system-prompt-file")
                .arg(&skill_path)
                .arg("-p")
                .arg(prompt)
                .arg("--resume")
                .arg(session_id)
                .arg("--output-format")
                .arg("stream-json");
            append_model_arg(&mut command, config);
            Ok(command)
        }
        ReviewerAgent::Cursor => {
            let skill_text = std::fs::read_to_string(&skill_path).map_err(Error::Io)?;
            let mut full = prompt;
            full.push_str("\n\n--- PROJECT ADR NOTES SKILL ---\n");
            full.push_str(&skill_text);
            let full = prepare_prompt_for_cli(&full);
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
            command.arg(full);
            Ok(command)
        }
    }
}

fn agent_turn_prompt(notes_dir: &str, message: &str) -> String {
    format!("[project-adr] notes_dir={notes_dir}\n\n{message}")
}

fn adr_notes_skill_path(config: &AppConfig) -> PathBuf {
    config
        .app_root()
        .join("skills")
        .join("project-adr-notes")
        .join("SKILL.md")
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

fn mr_scan_reviewer_prompt(manifest_path: &Path) -> String {
    let manifest_str = manifest_path.display().to_string();
    format!(
        "Headless MR review. First Read manifest: {manifest_str}. Follow appended workflow. \
Time-boxed: Read change_stat+change_log first; change.diff at most once (no paging); \
then Read ≤8 key source files from the worktree; write draft_dir draft BEFORE observation; exit. \
Non-interactive; do not ask questions; do not Glob reports/**."
    )
}

fn build_claude_command(
    config: &AppConfig,
    working_dir: &Path,
    prompt: &str,
    prompt_files: &[&Path],
    disable_session_persistence: bool,
) -> Result<Command, Error> {
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
    prompt: &str,
    prompt_sections: &[(&str, &Path)],
) -> Result<Command, Error> {
    let mut full_prompt = prompt.to_string();
    for (label, path) in prompt_sections {
        let text = std::fs::read_to_string(path).map_err(Error::Io)?;
        full_prompt.push_str("\n\n--- ");
        full_prompt.push_str(label);
        full_prompt.push_str(" ---\n");
        full_prompt.push_str(&text);
    }
    let full_prompt = prepare_prompt_for_cli(&full_prompt);

    let mut command = reviewer_command("cursor-agent");
    command
        .current_dir(working_dir)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--trust")
        .arg("--force");
    append_model_arg(&mut command, config);
    command.arg(full_prompt);

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
    fn summarize_agent_output_counts_events_and_tails() {
        let stdout = r#"{"type":"system","subtype":"init","session_id":"s1"}
{"type":"assistant","text":"looking at the diff"}
{"type":"assistant","message":{"content":[{"text":"final note"}]}}
"#;
        let stderr = "warn: slow tool\n";
        let summary = summarize_agent_output(stdout, stderr);
        assert_eq!(summary.stdout_lines, 3);
        assert!(summary.event_types.contains("assistant=2"));
        assert!(summary.event_types.contains("system=1"));
        assert_eq!(summary.last_event_type.as_deref(), Some("assistant"));
        assert_eq!(summary.last_assistant_snippet.as_deref(), Some("final note"));
        assert!(summary.stderr_tail.contains("warn: slow tool"));
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

    #[test]
    fn mr_scan_prompt_is_time_boxed() {
        let prompt = mr_scan_reviewer_prompt(Path::new("/data/manifest.json"));
        assert!(prompt.contains("change.diff at most once"));
        assert!(prompt.contains("≤8 key source files"));
        assert!(prompt.contains("draft BEFORE observation"));
    }

    #[test]
    fn agent_turn_claude_command_appends_adr_skill_and_notes_dir() {
        let config = test_config();
        let command = build_agent_turn_command(
            &config,
            config.app_root(),
            "sess-1",
            "record as ADR please",
            "/data/reports/alpha/.notes",
            ReviewerAgent::Claude,
        )
        .expect("cmd");
        let args = command_args(&command);
        let joined = args.join(" ");
        assert!(
            joined.contains("project-adr-notes") && joined.contains("SKILL.md"),
            "expected ADR skill append, got: {joined}"
        );
        assert!(
            joined.contains("[project-adr] notes_dir=/data/reports/alpha/.notes"),
            "expected notes_dir in prompt, got: {joined}"
        );
    }

    #[test]
    fn agent_turn_prompt_prefixes_notes_dir() {
        let prompt = agent_turn_prompt("/data/reports/alpha/.notes", "記成 ADR");
        assert_eq!(
            prompt,
            "[project-adr] notes_dir=/data/reports/alpha/.notes\n\n記成 ADR"
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
