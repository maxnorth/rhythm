use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db::get_pool;

/// Complete an execution successfully
///
/// Atomically marks the execution as completed and enqueues a resume task
/// for the parent workflow (if any). Uses a CTE to perform both operations
/// in a single round-trip to the database.
pub async fn complete_execution(execution_id: &str, result: JsonValue) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        WITH completed_task AS (
            UPDATE executions
            SET status = 'completed',
                result = $1,
                completed_at = NOW()
            WHERE id = $2
            RETURNING parent_workflow_id
        )
        INSERT INTO executions (id, type, function_name, queue, status, args, kwargs, priority, max_retries)
        SELECT
            gen_random_uuid()::text,
            'task',
            'builtin.resume_workflow',
            'system',
            'pending',
            jsonb_build_array(parent_workflow_id),
            '{}'::jsonb,
            10,
            0
        FROM completed_task
        WHERE parent_workflow_id IS NOT NULL
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
        WITH completed_tasks AS (
            UPDATE executions
            SET status = 'completed',
                result = data.result,
                completed_at = NOW()
            FROM (
                SELECT UNNEST($1::text[]) as id, UNNEST($2::jsonb[]) as result
            ) data
            WHERE executions.id = data.id
            RETURNING parent_workflow_id
        )
        INSERT INTO executions (id, type, function_name, queue, status, args, kwargs, priority, max_retries)
        SELECT
            gen_random_uuid()::text,
            'task',
            'builtin.resume_workflow',
            'system',
            'pending',
            jsonb_build_array(parent_workflow_id),
            '{}'::jsonb,
            10,
            0
        FROM completed_tasks
        WHERE parent_workflow_id IS NOT NULL
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
        // Mark as permanently failed and enqueue resume task for parent workflow
        sqlx::query(
            r#"
            WITH failed_task AS (
                UPDATE executions
                SET status = 'failed',
                    error = $1,
                    completed_at = NOW()
                WHERE id = $2
                RETURNING parent_workflow_id
            )
            INSERT INTO executions (id, type, function_name, queue, status, args, kwargs, priority, max_retries)
            SELECT
                gen_random_uuid()::text,
                'task',
                'builtin.resume_workflow',
                'system',
                'pending',
                jsonb_build_array(parent_workflow_id),
                '{}'::jsonb,
                10,
                0
            FROM failed_task
            WHERE parent_workflow_id IS NOT NULL
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
