use anyhow::{Context, Result};
use sqlx::Row;

use crate::db::get_pool;
use crate::types::*;

/// Claim an execution for a worker
pub async fn claim_execution(worker_id: &str, queues: &[String]) -> Result<Option<Execution>> {
    let pool = get_pool().await?;

    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'running',
            worker_id = $1,
            claimed_at = NOW(),
            attempt = attempt + 1
        WHERE id = (
            SELECT id FROM executions
            WHERE queue = ANY($2)
              AND status = 'pending'
            ORDER BY priority DESC, created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(worker_id)
    .bind(queues)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to claim execution")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            function_name: row.get("function_name"),
            queue: row.get("queue"),
            status: row.get("status"),
            priority: row.get("priority"),
            args: row.get("args"),
            kwargs: row.get("kwargs"),
            options: row.get("options"),
            result: row.get("result"),
            error: row.get("error"),
            attempt: row.get("attempt"),
            max_retries: row.get("max_retries"),
            parent_workflow_id: row.get("parent_workflow_id"),
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        };
        return Ok(Some(exec));
    }

    Ok(None)
}

/// Claim multiple executions for a worker (batch claiming)
pub async fn claim_executions_batch(
    worker_id: &str,
    queues: &[String],
    limit: i32,
) -> Result<Vec<Execution>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'running',
            worker_id = $1,
            claimed_at = NOW(),
            attempt = attempt + 1
        WHERE id IN (
            SELECT id FROM executions
            WHERE queue = ANY($2)
              AND status = 'pending'
            ORDER BY priority DESC, created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT $3
        )
        RETURNING *
        "#,
    )
    .bind(worker_id)
    .bind(queues)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await
    .context("Failed to claim executions batch")?;

    let mut executions = Vec::new();
    for row in rows {
        executions.push(Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            function_name: row.get("function_name"),
            queue: row.get("queue"),
            status: row.get("status"),
            priority: row.get("priority"),
            args: row.get("args"),
            kwargs: row.get("kwargs"),
            options: row.get("options"),
            result: row.get("result"),
            error: row.get("error"),
            attempt: row.get("attempt"),
            max_retries: row.get("max_retries"),
            parent_workflow_id: row.get("parent_workflow_id"),
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(executions)
}
