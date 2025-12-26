//! Work claiming logic

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use super::runner;
use crate::db;
use crate::types::{ExecutionStatus, ExecutionType};

/// Delegated action returned to the client for cooperative execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DelegatedAction {
    /// Execute a task in the host language
    ExecuteTask {
        execution_id: String,
        target_name: String,
        inputs: JsonValue,
    },
    /// Continue immediately - workflow was executed, check for more work
    Continue,
    /// Wait for the specified duration before checking for more work
    Wait { duration_ms: u64 },
    /// Shutdown requested - worker should exit gracefully
    Shutdown,
}

/// Claim and process one unit of work
///
/// This method attempts to claim work once and returns an action for the host:
/// - If it's a workflow: executes it internally and returns Continue
/// - If it's a task: returns ExecuteTask with task details
/// - If no work: returns Wait with suggested duration
///
/// The host should call this in a loop, handling each action appropriately.
/// The queue is hardcoded to "default".
pub async fn run_cooperative_worker_loop(
    pool: &PgPool,
    shutdown_token: &CancellationToken,
) -> Result<DelegatedAction> {
    let queue = "default";

    // Check for shutdown signal
    if shutdown_token.is_cancelled() {
        return Ok(DelegatedAction::Shutdown);
    }

    // Try to claim work (one attempt)
    let claimed_ids = db::work_queue::claim_work(pool, queue, 1).await?;
    if let Some(claimed_execution_id) = claimed_ids.into_iter().next() {
        let execution =
            db::executions::start_execution_unless_finished(pool, &claimed_execution_id)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("Claimed execution not found: {}", claimed_execution_id)
                })?;

        let is_finished = matches!(
            execution.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed
        );

        if is_finished {
            if execution.exec_type == ExecutionType::Task {
                tracing::error!(
                    execution_id = %claimed_execution_id,
                    status = ?execution.status,
                    "Task claimed from work queue but already in terminal state - this indicates a bug"
                );
            }
            db::work_queue::complete_work(pool, &claimed_execution_id).await?;
            return Ok(DelegatedAction::Continue);
        }

        match execution.exec_type {
            ExecutionType::Workflow => {
                // Execute the workflow internally
                runner::run_workflow(pool, execution).await?;

                // Return Continue so host can immediately check for more work
                return Ok(DelegatedAction::Continue);
            }
            ExecutionType::Task => {
                // Return task details to host for execution
                return Ok(DelegatedAction::ExecuteTask {
                    execution_id: execution.id,
                    target_name: execution.target_name,
                    inputs: execution.inputs,
                });
            }
        }
    }

    // No work available, tell host to wait before retrying
    Ok(DelegatedAction::Wait { duration_ms: 1000 })
}
