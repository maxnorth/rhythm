//! Scheduled Queue Database Operations
//!
//! Provides scheduling primitives for delayed work execution.

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;
use sqlx::Row;
use uuid::Uuid;

/// A scheduled item from the database
#[derive(Debug)]
pub struct ScheduledItem {
    pub id: Uuid,
    pub run_at: NaiveDateTime,
    pub params: JsonValue,
}

/// Schedule an item for later execution
pub async fn schedule_item<'e, E>(
    executor: E,
    run_at: NaiveDateTime,
    params: &JsonValue,
) -> Result<Uuid>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        INSERT INTO scheduled_queue (run_at, params)
        VALUES ($1, $2)
        RETURNING id
        "#,
    )
    .bind(run_at)
    .bind(params)
    .fetch_one(executor)
    .await
    .context("Failed to schedule item")?;

    Ok(row.get("id"))
}

/// Claim ready items from the scheduled queue
///
/// Returns items where run_at <= NOW(), locked for update.
/// Must be called within a transaction.
pub async fn claim_ready_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    limit: i32,
) -> Result<Vec<ScheduledItem>> {
    let rows = sqlx::query(
        r#"
        SELECT id, run_at, params
        FROM scheduled_queue
        WHERE run_at <= NOW()
        ORDER BY run_at ASC
        LIMIT $1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
    .context("Failed to claim ready items")?;

    Ok(rows
        .into_iter()
        .map(|row| ScheduledItem {
            id: row.get("id"),
            run_at: row.get("run_at"),
            params: row.get("params"),
        })
        .collect())
}

/// Delete items from the scheduled queue by IDs
pub async fn delete_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ids: &[Uuid],
) -> Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        r#"
        DELETE FROM scheduled_queue
        WHERE id = ANY($1)
        "#,
    )
    .bind(ids)
    .execute(&mut **tx)
    .await
    .context("Failed to delete scheduled items")?;

    Ok(result.rows_affected())
}
