use anyhow::{Context, Result};
use sqlx::Row;

use crate::db::get_pool;
use crate::types::*;

/// Claim an execution for a worker
pub async fn claim_execution(_worker_id: &str, queues: &[String]) -> Result<Option<Execution>> {
    let pool = get_pool().await?;

    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'running',
            attempt = attempt + 1
        WHERE id = (
            SELECT id FROM executions
            WHERE queue = ANY($1)
              AND status = 'pending'
            ORDER BY created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING *
        "#,
    )
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
            inputs: row.get("inputs"),
            output: row.get("output"),
            attempt: row.get("attempt"),
            parent_workflow_id: row.get("parent_workflow_id"),
            created_at: row.get("created_at"),
            completed_at: row.get("completed_at"),
        };
        return Ok(Some(exec));
    }

    Ok(None)
}
