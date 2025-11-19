use anyhow::{Context, Result};
use uuid::Uuid;

use crate::db::get_pool;
use crate::types::*;

/// Create a new execution with idempotency support
pub async fn create_execution(params: CreateExecutionParams) -> Result<String> {
    let pool = get_pool().await?;

    // Use user-provided id or generate UUID
    let id = params.id.as_ref().map(|s| s.clone()).unwrap_or_else(|| Uuid::new_v4().to_string());

    // Attempt atomic insert with ON CONFLICT for idempotency
    // This is a single database roundtrip with no race conditions
    let result: Option<(String, bool)> = sqlx::query_as(
        r#"
        INSERT INTO executions (
            id, type, function_name, queue, status,
            inputs, parent_workflow_id
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (id) DO NOTHING
        RETURNING id, (xmax = 0) AS inserted
        "#,
    )
    .bind(&id)
    .bind(&params.exec_type)
    .bind(&params.function_name)
    .bind(&params.queue)
    .bind(ExecutionStatus::Pending)
    .bind(&params.inputs)
    .bind(&params.parent_workflow_id)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to create execution")?;

    match result {
        Some((id, true)) => {
            // Successfully inserted
            Ok(id)
        }
        Some((_, false)) | None => {
            // Conflict occurred - check if we can retry (execution failed)
            let existing: Option<(ExecutionStatus,)> = sqlx::query_as(
                "SELECT status FROM executions WHERE id = $1",
            )
            .bind(&id)
            .fetch_optional(pool.as_ref())
            .await
            .context("Failed to check existing execution status")?;

            match existing {
                Some((ExecutionStatus::Failed,)) => {
                    // Delete failed execution and retry insert
                    sqlx::query("DELETE FROM executions WHERE id = $1")
                        .bind(&id)
                        .execute(pool.as_ref())
                        .await
                        .context("Failed to delete failed execution")?;

                    // Retry insert (recursion - will only happen once since row is deleted)
                    // Clone params and preserve the ID we already computed
                    let mut retry_params = params.clone();
                    retry_params.id = Some(id);
                    Box::pin(create_execution(retry_params)).await
                }
                Some((status,)) => {
                    // Execution exists in non-retryable state
                    Err(anyhow::anyhow!(
                        "Execution with id '{}' already exists with status {:?}",
                        id,
                        status
                    ))
                }
                None => {
                    // Race condition: row was deleted between INSERT and SELECT
                    // Retry the entire operation
                    let mut retry_params = params.clone();
                    retry_params.id = Some(id);
                    Box::pin(create_execution(retry_params)).await
                }
            }
        }
    }
}
