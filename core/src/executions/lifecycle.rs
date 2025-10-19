use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db::get_pool;

/// Complete an execution successfully
pub async fn complete_execution(execution_id: &str, result: JsonValue) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'completed',
            result = $1,
            completed_at = NOW()
        WHERE id = $2
        "#,
    )
    .bind(result)
    .bind(execution_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to complete execution")?;

    Ok(())
}

/// Complete multiple executions in a single batch operation
pub async fn complete_executions_batch(completions: Vec<(String, JsonValue)>) -> Result<()> {
    if completions.is_empty() {
        return Ok(());
    }

    let pool = get_pool().await?;

    // Build dynamic query with UNNEST for batch update
    let ids: Vec<String> = completions.iter().map(|(id, _)| id.clone()).collect();
    let results: Vec<JsonValue> = completions.iter().map(|(_, result)| result.clone()).collect();

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'completed',
            result = data.result,
            completed_at = NOW()
        FROM (
            SELECT UNNEST($1::text[]) as id, UNNEST($2::jsonb[]) as result
        ) data
        WHERE executions.id = data.id
        "#,
    )
    .bind(&ids)
    .bind(&results)
    .execute(pool.as_ref())
    .await
    .context("Failed to batch complete executions")?;

    Ok(())
}

/// Fail an execution with JSON error
pub async fn fail_execution(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    let pool = get_pool().await?;

    if retry {
        // Reset to pending for retry
        sqlx::query(
            r#"
            UPDATE executions
            SET status = 'pending',
                error = $1,
                worker_id = NULL,
                claimed_at = NULL
            WHERE id = $2
            "#,
        )
        .bind(&error)
        .bind(execution_id)
        .execute(pool.as_ref())
        .await
        .context("Failed to update execution for retry")?;
    } else {
        // Mark as permanently failed
        sqlx::query(
            r#"
            UPDATE executions
            SET status = 'failed',
                error = $1,
                completed_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&error)
        .bind(execution_id)
        .execute(pool.as_ref())
        .await
        .context("Failed to mark execution as failed")?;

        // Insert into dead letter queue
        sqlx::query(
            r#"
            INSERT INTO dead_letter_queue (id, execution_id, execution_data, failure_reason)
            SELECT $1, id, row_to_json(executions.*)::jsonb, $2
            FROM executions
            WHERE id = $3
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(error.to_string())
        .bind(execution_id)
        .execute(pool.as_ref())
        .await
        .ok(); // Don't fail if DLQ insert fails
    }

    Ok(())
}

/// Suspend a workflow (for task execution)
pub async fn suspend_workflow(workflow_id: &str, checkpoint: JsonValue) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'suspended',
            checkpoint = $1
        WHERE id = $2
        "#,
    )
    .bind(checkpoint)
    .bind(workflow_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to suspend workflow")?;

    Ok(())
}

/// Resume a workflow (re-queue it)
pub async fn resume_workflow(workflow_id: &str) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'pending',
            worker_id = NULL,
            claimed_at = NULL
        WHERE id = $1
        "#,
    )
    .bind(workflow_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to resume workflow")?;

    Ok(())
}

/// Cancel an execution
pub async fn cancel_execution(execution_id: &str) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'failed',
            error = '{"error": "Cancelled by user"}',
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
