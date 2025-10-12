use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::Row;
use uuid::Uuid;

use crate::db::get_pool;
use crate::types::*;

/// Create a new execution
pub async fn create_execution(params: CreateExecutionParams) -> Result<String> {
    let pool = get_pool().await?;
    let id = format!(
        "{}_{}",
        match params.exec_type {
            ExecutionType::Task => "task",
            ExecutionType::Workflow => "wor",
        },
        Uuid::new_v4()
    );

    sqlx::query(
        r#"
        INSERT INTO executions (
            id, type, function_name, queue, status, priority,
            args, kwargs, options, max_retries, timeout_seconds, parent_workflow_id
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(&id)
    .bind(&params.exec_type)
    .bind(&params.function_name)
    .bind(&params.queue)
    .bind(ExecutionStatus::Pending)
    .bind(params.priority)
    .bind(&params.args)
    .bind(&params.kwargs)
    .bind(serde_json::json!({}))
    .bind(params.max_retries)
    .bind(params.timeout_seconds)
    .bind(params.parent_workflow_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to create execution")?;

    // Send LISTEN/NOTIFY (quote channel name to handle reserved keywords like 'default')
    sqlx::query(&format!("NOTIFY \"{}\", '{}'", params.queue, id))
        .execute(pool.as_ref())
        .await
        .ok(); // Don't fail if notify fails

    Ok(id)
}

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
            checkpoint: row.get("checkpoint"),
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
pub async fn claim_executions_batch(worker_id: &str, queues: &[String], limit: i32) -> Result<Vec<Execution>> {
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
            checkpoint: row.get("checkpoint"),
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(executions)
}

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

/// Fail an execution
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

/// Get execution by ID
pub async fn get_execution(execution_id: &str) -> Result<Option<Execution>> {
    let pool = get_pool().await?;

    let result = sqlx::query(
        r#"
        SELECT * FROM executions WHERE id = $1
        "#,
    )
    .bind(execution_id)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to get execution")?;

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
            checkpoint: row.get("checkpoint"),
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

/// List executions with filters
pub async fn list_executions(filter: ExecutionListFilter) -> Result<Vec<Execution>> {
    let pool = get_pool().await?;

    let mut query = String::from("SELECT * FROM executions WHERE 1=1");

    if filter.queue.is_some() {
        query.push_str(" AND queue = $1");
    }
    if filter.status.is_some() {
        let param_num = if filter.queue.is_some() { 2 } else { 1 };
        query.push_str(&format!(" AND status = ${}", param_num));
    }

    query.push_str(" ORDER BY created_at DESC");

    if filter.limit.is_some() {
        let param_num = if filter.status.is_some() && filter.queue.is_some() {
            3
        } else if filter.status.is_some() || filter.queue.is_some() {
            2
        } else {
            1
        };
        query.push_str(&format!(" LIMIT ${}", param_num));
    }

    let mut q = sqlx::query(&query);

    if let Some(ref queue) = filter.queue {
        q = q.bind(queue);
    }
    if let Some(ref status) = filter.status {
        q = q.bind(status);
    }
    if let Some(limit) = filter.limit {
        q = q.bind(limit);
    }

    let rows = q
        .fetch_all(pool.as_ref())
        .await
        .context("Failed to list executions")?;

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
            checkpoint: row.get("checkpoint"),
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(executions)
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

/// Get child task executions for a workflow
pub async fn get_workflow_tasks(workflow_id: &str) -> Result<Vec<Execution>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT * FROM executions
        WHERE parent_workflow_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(workflow_id)
    .fetch_all(pool.as_ref())
    .await
    .context("Failed to get workflow child tasks")?;

    let mut child_tasks = Vec::new();
    for row in rows {
        child_tasks.push(Execution {
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
            checkpoint: row.get("checkpoint"),
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(child_tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_create_and_claim_execution() {
        let params = CreateExecutionParams {
            exec_type: ExecutionType::Task,
            function_name: "test.task".to_string(),
            queue: "test".to_string(),
            priority: 5,
            args: serde_json::json!([]),
            kwargs: serde_json::json!({}),
            max_retries: 3,
            timeout_seconds: Some(300),
            parent_workflow_id: None,
        };

        let id = create_execution(params).await.unwrap();
        assert!(id.starts_with("task_"));

        let execution = claim_execution("test-worker", &["test".to_string()])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(execution.id, id);
        assert_eq!(execution.status, ExecutionStatus::Running);
    }
}
