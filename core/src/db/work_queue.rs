//! Work Queue Database Operations
//!
//! Provides scheduling primitives for the V2 workflow engine.

use anyhow::{Context, Result};
use sqlx::Row;

/// Enqueue work for an execution
///
/// Creates an unclaimed work queue entry. If an unclaimed entry already exists,
/// this operation does nothing (idempotent).
pub async fn enqueue_work<'e, E>(
    executor: E,
    execution_id: &str,
    queue: &str,
    priority: i32,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO work_queue (execution_id, queue, priority)
        VALUES ($1, $2, $3)
        ON CONFLICT (execution_id, (claimed_until IS NULL))
        DO NOTHING
        "#,
    )
    .bind(execution_id)
    .bind(queue)
    .bind(priority)
    .execute(executor)
    .await
    .context("Failed to enqueue work")?;

    Ok(())
}

/// Claim work from the queue
///
/// Returns a list of execution IDs that were successfully claimed.
/// Uses lease-based claiming with a 1-minute timeout.
pub async fn claim_work<'e, E>(executor: E, queue: &str, limit: i32) -> Result<Vec<String>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let rows = sqlx::query(
        r#"
        WITH to_claim AS (
            SELECT id
            FROM work_queue
            WHERE queue = $1
              AND (claimed_until IS NULL OR claimed_until < NOW())
              AND NOT EXISTS (
                  SELECT 1 FROM work_queue wq2
                  WHERE wq2.execution_id = work_queue.execution_id
                    AND wq2.claimed_until IS NOT NULL
                    AND wq2.claimed_until > NOW()
              )
            ORDER BY priority DESC, created_at ASC
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        UPDATE work_queue
        SET claimed_until = NOW() + INTERVAL '1 minute'
        WHERE id IN (SELECT id FROM to_claim)
        RETURNING execution_id
        "#,
    )
    .bind(queue)
    .bind(limit)
    .fetch_all(executor)
    .await
    .context("Failed to claim work")?;

    Ok(rows
        .into_iter()
        .map(|row| row.get("execution_id"))
        .collect())
}

/// Claim work for a specific execution
///
/// Claims the unclaimed work queue entry for a specific execution.
/// Useful for testing or manual work claiming.
/// Uses lease-based claiming with a 1-minute timeout.
pub async fn claim_specific_execution(pool: &sqlx::PgPool, execution_id: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE work_queue
        SET claimed_until = NOW() + INTERVAL '1 minute'
        WHERE execution_id = $1 AND claimed_until IS NULL
        "#,
    )
    .bind(execution_id)
    .execute(pool)
    .await
    .context("Failed to claim specific execution")?;

    Ok(())
}

/// Complete work for an execution
///
/// Deletes the claimed work queue entry. Preserves any unclaimed entry that
/// was queued while this work was in progress.
pub async fn complete_work<'e, E>(executor: E, execution_id: &str) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        DELETE FROM work_queue
        WHERE execution_id = $1
          AND claimed_until IS NOT NULL
        "#,
    )
    .bind(execution_id)
    .execute(executor)
    .await
    .context("Failed to complete work")?;

    Ok(())
}
