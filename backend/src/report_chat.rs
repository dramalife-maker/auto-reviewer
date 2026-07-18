use serde::Serialize;
use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::config::{AppConfig, ReviewerAgent};
use crate::error::{Error, Result};
use crate::executor::execute_report_chat_turn;
use crate::mr_reviews::ChatMessage;
use crate::summary::reingest_person_summaries;

#[derive(Debug, Clone, Serialize)]
pub struct ReportChatResponse {
    pub agent_session_id: Option<String>,
    pub reviewer_agent: String,
    pub chat_messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportChatAgentTurnResponse {
    pub reply: String,
    pub agent_session_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ingest_warnings: Vec<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChatRow {
    agent_session_id: Option<String>,
    reviewer_agent: String,
}

pub async fn get_report_chat(
    pool: &SqlitePool,
    config: &AppConfig,
    person_id: i64,
) -> Result<ReportChatResponse> {
    ensure_person_exists(pool, person_id).await?;

    let row = sqlx::query_as::<_, ChatRow>(
        "SELECT agent_session_id, reviewer_agent FROM person_report_chats WHERE person_id = ?",
    )
    .bind(person_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    let (agent_session_id, reviewer_agent) = match row {
        Some(row) => (row.agent_session_id, row.reviewer_agent),
        None => (None, config.reviewer_agent().as_str().to_string()),
    };

    let chat_messages = load_chat_messages(pool, person_id).await?;
    Ok(ReportChatResponse {
        agent_session_id,
        reviewer_agent,
        chat_messages,
    })
}

pub async fn agent_turn(
    pool: &SqlitePool,
    config: &AppConfig,
    person_id: i64,
    message: &str,
    cancel: CancellationToken,
) -> Result<ReportChatAgentTurnResponse> {
    let display_name = load_display_name(pool, person_id).await?;

    let existing = sqlx::query_as::<_, ChatRow>(
        "SELECT agent_session_id, reviewer_agent FROM person_report_chats WHERE person_id = ?",
    )
    .bind(person_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    let prior_session = existing
        .as_ref()
        .and_then(|row| row.agent_session_id.as_deref())
        .filter(|value| !value.is_empty());
    let agent = config.reviewer_agent();

    let (reply, new_session) = execute_report_chat_turn(
        config,
        config.data_dir(),
        prior_session,
        &display_name,
        message,
        agent,
        cancel,
    )
    .await?;

    let next_session = new_session
        .filter(|value| !value.is_empty())
        .or_else(|| prior_session.map(str::to_string));
    if next_session.is_none() {
        warn!(
            person_id,
            "report chat turn succeeded without a resolvable agent_session_id"
        );
    }

    upsert_chat_session(pool, person_id, next_session.as_deref(), agent).await?;
    insert_chat_turn(pool, person_id, message, &reply).await?;

    let ingest_warnings =
        reingest_person_summaries(pool, config.data_dir(), person_id, &display_name).await;
    for warning in &ingest_warnings {
        warn!(person_id, %warning, "report chat summary reingest warning");
    }

    Ok(ReportChatAgentTurnResponse {
        reply,
        agent_session_id: next_session,
        ingest_warnings,
    })
}

async fn ensure_person_exists(pool: &SqlitePool, person_id: i64) -> Result<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if exists.is_none() {
        return Err(Error::NotFound);
    }
    Ok(())
}

async fn load_display_name(pool: &SqlitePool, person_id: i64) -> Result<String> {
    sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?
        .ok_or(Error::NotFound)
}

async fn load_chat_messages(pool: &SqlitePool, person_id: i64) -> Result<Vec<ChatMessage>> {
    sqlx::query_as::<_, ChatMessage>(
        r#"
        SELECT id, role, content, created_at
        FROM person_report_chat_messages
        WHERE person_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(person_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

async fn upsert_chat_session(
    pool: &SqlitePool,
    person_id: i64,
    agent_session_id: Option<&str>,
    agent: ReviewerAgent,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO person_report_chats (person_id, agent_session_id, reviewer_agent, updated_at)
        VALUES (?, ?, ?, datetime('now'))
        ON CONFLICT(person_id) DO UPDATE SET
            agent_session_id = excluded.agent_session_id,
            reviewer_agent = excluded.reviewer_agent,
            updated_at = datetime('now')
        "#,
    )
    .bind(person_id)
    .bind(agent_session_id)
    .bind(agent.as_str())
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

async fn insert_chat_turn(
    pool: &SqlitePool,
    person_id: i64,
    user_content: &str,
    assistant_content: &str,
) -> Result<()> {
    let mut tx = pool.begin().await.map_err(Error::Database)?;
    sqlx::query(
        "INSERT INTO person_report_chat_messages (person_id, role, content) VALUES (?, 'user', ?)",
    )
    .bind(person_id)
    .bind(user_content)
    .execute(&mut *tx)
    .await
    .map_err(Error::Database)?;
    sqlx::query(
        "INSERT INTO person_report_chat_messages (person_id, role, content) VALUES (?, 'assistant', ?)",
    )
    .bind(person_id)
    .bind(assistant_content)
    .execute(&mut *tx)
    .await
    .map_err(Error::Database)?;
    tx.commit().await.map_err(Error::Database)?;
    Ok(())
}
