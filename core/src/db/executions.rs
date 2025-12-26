//! Execution Database Operations for V2

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::types::{CreateExecutionParams, Execution, ExecutionFilters, ExecutionStatus};

pub async fn get_execution(pool: &PgPool, execution_id: &str) -> Result<Option<Execution>> {
    let result = sqlx::query(
        r#"
        SELECT * FROM executions WHERE id = $1
        "#,
    )
    .bind(execution_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get execution")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
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

pub async fn create_execution(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    params: CreateExecutionParams,
) -> Result<String> {
    let mut current_params = params;

    loop {
        let id = current_params
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let result: Option<(String, bool)> = sqlx::query_as(
            r#"
            INSERT INTO executions (
                id, type, target_name, queue, status,
                inputs, parent_workflow_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO NOTHING
            RETURNING id, (xmax = 0) AS inserted
            "#,
        )
        .bind(&id)
        .bind(&current_params.exec_type)
        .bind(&current_params.target_name)
        .bind(&current_params.queue)
        .bind(ExecutionStatus::Pending)
        .bind(&current_params.inputs)
        .bind(&current_params.parent_workflow_id)
        .fetch_optional(&mut **tx)
        .await
        .context("Failed to create execution")?;

        match result {
            Some((id, true)) => return Ok(id),
            Some((_, false)) | None => {
                let existing: Option<(ExecutionStatus,)> =
                    sqlx::query_as("SELECT status FROM executions WHERE id = $1")
                        .bind(&id)
                        .fetch_optional(&mut **tx)
                        .await
                        .context("Failed to check existing execution status")?;

                match existing {
                    Some((ExecutionStatus::Failed,)) => {
                        sqlx::query("DELETE FROM executions WHERE id = $1")
                            .bind(&id)
                            .execute(&mut **tx)
                            .await
                            .context("Failed to delete failed execution")?;

                        current_params.id = Some(id);
                        continue;
                    }
                    Some((status,)) => {
                        return Err(anyhow::anyhow!(
                            "Execution with id '{}' already exists with status {:?}",
                            id,
                            status
                        ));
                    }
                    None => {
                        current_params.id = Some(id);
                        continue;
                    }
                }
            }
        }
    }
}

pub async fn start_execution_unless_finished<'e, E>(
    executor: E,
    execution_id: &str,
) -> Result<Option<Execution>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let result = sqlx::query(
        r#"
        WITH updated AS (
            UPDATE executions
            SET status = 'running'
            WHERE id = $1
              AND status NOT IN ('completed', 'failed')
            RETURNING *
        )
        SELECT * FROM updated
        UNION ALL
        SELECT * FROM executions WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM updated)
        "#,
    )
    .bind(execution_id)
    .fetch_optional(executor)
    .await
    .context("Failed to start execution")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
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

pub async fn complete_execution<'e, E>(
    executor: E,
    execution_id: &str,
    output: JsonValue,
) -> Result<Option<Execution>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'completed',
            output = $1,
            completed_at = NOW()
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(output)
    .bind(execution_id)
    .fetch_optional(executor)
    .await
    .context("Failed to complete execution")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
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

pub async fn fail_execution<'e, E>(
    executor: E,
    execution_id: &str,
    output: JsonValue,
) -> Result<Option<Execution>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'failed',
            output = $1,
            completed_at = NOW()
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(&output)
    .bind(execution_id)
    .fetch_optional(executor)
    .await
    .context("Failed to mark execution as failed")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
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

pub async fn suspend_execution<'e, E>(executor: E, execution_id: &str) -> Result<Option<Execution>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'suspended',
            completed_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(execution_id)
    .fetch_optional(executor)
    .await
    .context("Failed to suspend execution")?;

    if let Some(row) = result {
        let exec = Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
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

/// Query executions with filters
///
/// Returns a list of executions matching the provided filters.
pub async fn query_executions(pool: &PgPool, filters: ExecutionFilters) -> Result<Vec<Execution>> {
    let mut query = String::from("SELECT * FROM executions WHERE 1=1");
    let mut bind_count = 0;

    // Build WHERE clause based on filters
    if filters.parent_workflow_id.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND parent_workflow_id = ${}", bind_count));
    }

    if filters.status.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND status = ${}", bind_count));
    }

    if filters.target_name.is_some() {
        bind_count += 1;
        query.push_str(&format!(" AND target_name = ${}", bind_count));
    }

    query.push_str(" ORDER BY created_at DESC");

    if filters.limit.is_some() {
        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));
    }

    if filters.offset.is_some() {
        bind_count += 1;
        query.push_str(&format!(" OFFSET ${}", bind_count));
    }

    // Build query with bindings
    let mut sql_query = sqlx::query(&query);

    if let Some(ref parent_id) = filters.parent_workflow_id {
        sql_query = sql_query.bind(parent_id);
    }

    if let Some(ref status) = filters.status {
        sql_query = sql_query.bind(status);
    }

    if let Some(ref target_name) = filters.target_name {
        sql_query = sql_query.bind(target_name);
    }

    if let Some(limit) = filters.limit {
        sql_query = sql_query.bind(limit);
    }

    if let Some(offset) = filters.offset {
        sql_query = sql_query.bind(offset);
    }

    let rows = sql_query
        .fetch_all(pool)
        .await
        .context("Failed to query executions")?;

    let executions = rows
        .into_iter()
        .map(|row| Execution {
            id: row.get("id"),
            exec_type: row.get("type"),
            target_name: row.get("target_name"),
            queue: row.get("queue"),
            status: row.get("status"),
            inputs: row.get("inputs"),
            output: row.get("output"),
            attempt: row.get("attempt"),
            parent_workflow_id: row.get("parent_workflow_id"),
            created_at: row.get("created_at"),
            completed_at: row.get("completed_at"),
        })
        .collect();

    Ok(executions)
}
