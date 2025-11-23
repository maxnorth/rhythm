//! Work claiming logic

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::v2::db;
use crate::v2::types::ExecutionType;
use super::runner;

/// Task details returned to the host for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimedTask {
    pub execution_id: String,
    pub function_name: String,
    pub inputs: JsonValue,
}

/// Claim work from the queue
///
/// This method blocks/retries until work is available. When it finds work:
/// - If it's a workflow: executes it internally and loops again
/// - If it's a task: returns the task details to the host for execution
///
/// The queue parameter is hardcoded to "default" for now.
pub async fn claim_work(pool: &PgPool) -> Result<ClaimedTask> {
    let queue = "default"; // TODO: Make this configurable

    loop {
        // Try to claim work from the queue
        // TODO: heartbeat for claimed work
        let claimed_ids = db::work_queue::claim_work(pool, queue, 1).await?;

        if let Some(claimed_execution_id) = claimed_ids.into_iter().next() {
            // Fetch the execution and mark it as running in a single query
            let execution = db::executions::start_execution(pool, &claimed_execution_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Claimed execution not found: {}", claimed_execution_id))?;

            match execution.exec_type {
                ExecutionType::Workflow => {
                    // Execute the workflow internally
                    runner::run_workflow(pool, execution).await?;

                    // Loop again to find more work (workflows are handled automatically)
                    continue;
                }
                ExecutionType::Task => {
                    // Return task details to host for execution
                    return Ok(ClaimedTask {
                        execution_id: execution.id,
                        function_name: execution.function_name,
                        inputs: execution.inputs,
                    });
                }
            }
        }

        // No work available, sleep briefly and retry
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
