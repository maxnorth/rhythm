//! Signals Database Operations
//!
//! Provides signal storage and retrieval for workflow human-in-the-loop patterns.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::Row;
use uuid::Uuid;

use crate::types::Signal;

/// Create a new signal for a workflow
pub async fn create_signal<'e, E>(
    executor: E,
    workflow_id: &str,
    signal_name: &str,
    payload: &JsonValue,
) -> Result<String>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        INSERT INTO signals (workflow_id, signal_name, payload)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(workflow_id)
    .bind(signal_name)
    .bind(payload)
    .fetch_one(executor)
    .await
    .context("Failed to create signal")?;

    let id: Uuid = row.get("id");
    Ok(id.to_string())
}

/// Get the latest signal for a workflow/channel after a given cursor
///
/// If `after_id` is None, returns the latest signal overall.
/// If `after_id` is Some, returns the latest signal created after that signal's created_at.
///
/// This enables cursor-based consumption where each call to Signal.next()
/// returns the next unconsumed signal.
pub async fn get_latest_signal_after(
    pool: &sqlx::PgPool,
    workflow_id: &str,
    signal_name: &str,
    after_id: Option<&str>,
) -> Result<Option<Signal>> {
    let cursor_uuid = after_id
        .map(|id| Uuid::parse_str(id))
        .transpose()
        .context("Invalid signal cursor ID")?;

    let row = if let Some(cursor_id) = cursor_uuid {
        // Find signals created after the cursor signal's created_at
        sqlx::query(
            r#"
            SELECT s.id, s.workflow_id, s.signal_name, s.payload, s.created_at
            FROM signals s
            WHERE s.workflow_id = $1
              AND s.signal_name = $2
              AND s.created_at > (
                  SELECT created_at FROM signals WHERE id = $3
              )
            ORDER BY s.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .bind(signal_name)
        .bind(cursor_id)
        .fetch_optional(pool)
        .await
        .context("Failed to get signal after cursor")?
    } else {
        // No cursor - get the latest signal
        sqlx::query(
            r#"
            SELECT id, workflow_id, signal_name, payload, created_at
            FROM signals
            WHERE workflow_id = $1
              AND signal_name = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .bind(signal_name)
        .fetch_optional(pool)
        .await
        .context("Failed to get latest signal")?
    };

    Ok(row.map(|r| {
        let id: Uuid = r.get("id");
        Signal {
            id: id.to_string(),
            workflow_id: r.get("workflow_id"),
            signal_name: r.get("signal_name"),
            payload: r.get("payload"),
            created_at: r.get("created_at"),
        }
    }))
}
