//! Workflow Execution Context Database Operations

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};

/// Workflow execution context from database
#[derive(Debug)]
pub struct WorkflowExecutionContext {
    pub workflow_definition_id: i32,
    pub vm_state: JsonValue,
}

/// Get workflow execution context for a given execution ID
///
/// Returns None if no context exists (first run), or Some with the VM state (resume).
pub async fn get_context(
    pool: &PgPool,
    execution_id: &str,
) -> Result<Option<WorkflowExecutionContext>> {
    let maybe_row = sqlx::query(
        r#"
        SELECT workflow_definition_id, locals as vm_state
        FROM workflow_execution_context
        WHERE execution_id = $1
        "#,
    )
    .bind(execution_id)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch workflow execution context")?;

    Ok(maybe_row.map(|row| WorkflowExecutionContext {
        workflow_definition_id: row.get("workflow_definition_id"),
        vm_state: row.get("vm_state"),
    }))
}

/// Upsert workflow execution context
///
/// Creates a new record if it doesn't exist, updates if it does.
pub async fn upsert_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    execution_id: &str,
    workflow_definition_id: i32,
    vm_state: &JsonValue,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO workflow_execution_context (execution_id, workflow_definition_id, locals)
        VALUES ($1, $2, $3)
        ON CONFLICT (execution_id)
        DO UPDATE SET
            locals = EXCLUDED.locals,
            updated_at = NOW()
        "#,
    )
    .bind(execution_id)
    .bind(workflow_definition_id)
    .bind(vm_state)
    .execute(&mut **tx)
    .await
    .context("Failed to upsert workflow execution context")?;

    Ok(())
}

/// Delete workflow execution context
///
/// Called when workflow completes or fails.
pub async fn delete_context<'e, E>(executor: E, execution_id: &str) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        DELETE FROM workflow_execution_context
        WHERE execution_id = $1
        "#,
    )
    .bind(execution_id)
    .execute(executor)
    .await
    .context("Failed to delete workflow execution context")?;

    Ok(())
}
