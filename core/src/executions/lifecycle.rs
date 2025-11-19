use anyhow::{Context, Result};
use serde_json::Value as JsonValue;

use crate::db::get_pool;

/// Complete an execution successfully
///
/// Atomically marks the execution as completed and enqueues a resume task
/// for the parent workflow (if any). Uses a CTE to perform both operations
/// in a single round-trip to the database.
pub async fn complete_execution(execution_id: &str, output: JsonValue) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        WITH completed_task AS (
            UPDATE executions
            SET status = 'completed',
                output = $1,
                completed_at = NOW()
            WHERE id = $2
            RETURNING parent_workflow_id
        )
        INSERT INTO executions (id, type, function_name, queue, status, inputs)
        SELECT
            gen_random_uuid()::text,
            'task',
            'builtin.resume_workflow',
            'system',
            'pending',
            jsonb_build_object('workflow_id', parent_workflow_id)
        FROM completed_task
        WHERE parent_workflow_id IS NOT NULL
        "#,
    )
    .bind(output)
    .bind(execution_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to complete execution")?;

    Ok(())
}

/// Fail an execution with JSON error
pub async fn fail_execution(execution_id: &str, output: JsonValue, retry: bool) -> Result<()> {
    let pool = get_pool().await?;

    if retry {
        // Reset to pending for retry
        sqlx::query(
            r#"
            UPDATE executions
            SET status = 'pending',
                output = $1
            WHERE id = $2
            "#,
        )
        .bind(&output)
        .bind(execution_id)
        .execute(pool.as_ref())
        .await
        .context("Failed to update execution for retry")?;
    } else {
        // Mark as permanently failed and enqueue resume task for parent workflow
        sqlx::query(
            r#"
            WITH failed_task AS (
                UPDATE executions
                SET status = 'failed',
                    output = $1,
                    completed_at = NOW()
                WHERE id = $2
                RETURNING parent_workflow_id
            )
            INSERT INTO executions (id, type, function_name, queue, status, inputs)
            SELECT
                gen_random_uuid()::text,
                'task',
                'builtin.resume_workflow',
                'system',
                'pending',
                jsonb_build_object('workflow_id', parent_workflow_id)
            FROM failed_task
            WHERE parent_workflow_id IS NOT NULL
            "#,
        )
        .bind(&output)
        .bind(execution_id)
        .execute(pool.as_ref())
        .await
        .context("Failed to mark execution as failed")?;
    }

    Ok(())
}

/// Cancel an execution
pub async fn cancel_execution(execution_id: &str) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'failed',
            output = '{"error": "Cancelled by user"}',
            completed_at = NOW()
        WHERE id = $1
          AND status IN ('pending', 'running', 'suspended')
        "#,
    )
    .bind(execution_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to cancel execution")?;

    Ok(())
}
