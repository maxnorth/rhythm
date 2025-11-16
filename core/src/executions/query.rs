use anyhow::{Context, Result};
use sqlx::Row;

use crate::db::get_pool;
use crate::types::*;

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
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(executions)
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
            created_at: row.get("created_at"),
            claimed_at: row.get("claimed_at"),
            completed_at: row.get("completed_at"),
            timeout_seconds: row.get("timeout_seconds"),
            worker_id: row.get("worker_id"),
        });
    }

    Ok(child_tasks)
}
