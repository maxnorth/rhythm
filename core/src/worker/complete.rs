//! Work completion logic

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db;
use crate::types::ExecutionOutcome;

/// Finish work (complete, fail, or suspend) and re-queue parent if exists
///
/// This is a helper that:
/// 1. Marks the execution as completed, failed, or suspended
/// 2. Completes the work queue entry
/// 3. Re-queues the parent workflow if one exists
///
/// The transaction must be used for all operations to ensure atomicity.
///
/// Note: Workflow execution context management (upsert/delete) should be handled
/// by the caller before calling this function.
pub async fn finish_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    execution_id: &str,
    outcome: ExecutionOutcome,
) -> Result<()> {
    // Handle execution based on outcome
    let execution = match outcome {
        ExecutionOutcome::Success(output) => {
            db::executions::complete_execution(&mut **tx, execution_id, output)
                .await
                .context("Failed to complete execution")?
        }
        ExecutionOutcome::Failure(error) => {
            db::executions::fail_execution(&mut **tx, execution_id, error)
                .await
                .context("Failed to fail execution")?
        }
        ExecutionOutcome::Suspended => db::executions::suspend_execution(&mut **tx, execution_id)
            .await
            .context("Failed to suspend execution")?,
    };

    let execution =
        execution.ok_or_else(|| anyhow::anyhow!("Execution not found: {}", execution_id))?;

    // Complete the work queue entry
    db::work_queue::complete_work(&mut **tx, execution_id)
        .await
        .context("Failed to complete work queue entry")?;

    // Re-queue parent workflow if this execution has a parent
    if let Some(ref parent_id) = execution.parent_workflow_id {
        db::work_queue::enqueue_work(&mut **tx, parent_id, &execution.queue, 0)
            .await
            .context("Failed to re-queue parent workflow")?;
    }

    Ok(())
}

/// Complete work after task execution
///
/// Either result OR error should be Some, not both.
/// If result is Some, marks the task as completed.
/// If error is Some, marks the task as failed.
pub async fn complete_work(
    pool: &PgPool,
    execution_id: &str,
    result: Option<JsonValue>,
    error: Option<JsonValue>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let outcome = match (result, error) {
        (Some(output), None) => ExecutionOutcome::Success(output),
        (None, Some(error_output)) => ExecutionOutcome::Failure(error_output),
        _ => {
            return Err(anyhow::anyhow!(
                "Exactly one of result or error must be provided"
            ));
        }
    };

    finish_work(&mut tx, execution_id, outcome).await?;

    tx.commit().await?;

    Ok(())
}
