//! Scheduler Service
//!
//! Handles scheduling and processing of delayed work items.

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;

/// Parameters for scheduled items, tagged by type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduledParams {
    /// Resume a suspended workflow
    WorkflowContinuation {
        execution_id: String,
        queue: String,
        priority: i32,
    },
    /// Start a scheduled execution (workflow or task)
    ScheduledExecution {
        execution_id: String,
        queue: String,
        priority: i32,
    },
}

/// Service for scheduler operations
#[derive(Clone)]
pub struct SchedulerService {
    pool: PgPool,
}

impl SchedulerService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Schedule a workflow continuation for later execution
    pub async fn schedule_workflow_continuation(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        execution_id: &str,
        queue: &str,
        priority: i32,
        run_at: NaiveDateTime,
    ) -> Result<()> {
        let params = ScheduledParams::WorkflowContinuation {
            execution_id: execution_id.to_string(),
            queue: queue.to_string(),
            priority,
        };

        let params_json =
            serde_json::to_value(&params).context("Failed to serialize scheduled params")?;

        db::scheduled_queue::schedule_item(&mut **tx, run_at, &params_json).await?;

        Ok(())
    }

    /// Schedule a new execution (workflow or task) to start at a future time.
    ///
    /// Creates the execution immediately in Pending status, then schedules
    /// it to be enqueued in the work queue at the specified time.
    pub async fn schedule_execution(
        &self,
        params: crate::types::ScheduleExecutionParams,
    ) -> Result<String> {
        let mut tx = self.pool.begin().await?;

        // Create the execution immediately in Pending status
        let create_params = crate::types::CreateExecutionParams {
            id: None,
            exec_type: params.exec_type,
            target_name: params.target_name,
            queue: params.queue.clone(),
            inputs: params.inputs,
            parent_workflow_id: None,
        };
        let execution_id = db::executions::create_execution(&mut tx, create_params).await?;

        // Schedule it to be enqueued later
        let scheduled_params = ScheduledParams::ScheduledExecution {
            execution_id: execution_id.clone(),
            queue: params.queue,
            priority: 0,
        };

        let params_json = serde_json::to_value(&scheduled_params)
            .context("Failed to serialize scheduled params")?;

        db::scheduled_queue::schedule_item(&mut *tx, params.run_at, &params_json).await?;

        tx.commit().await?;

        Ok(execution_id)
    }

    /// Process ready items from the scheduled queue
    ///
    /// Claims items that are ready to run, enqueues them in the work queue,
    /// and removes them from the scheduled queue. All within a single transaction.
    ///
    /// Returns the number of items processed.
    pub async fn process_ready_items(&self, limit: i32) -> Result<u32> {
        let mut tx = self.pool.begin().await?;

        // 1. Claim ready items (SELECT FOR UPDATE SKIP LOCKED)
        let items = db::scheduled_queue::claim_ready_items(&mut tx, limit).await?;

        if items.is_empty() {
            return Ok(0);
        }

        let mut item_ids = Vec::with_capacity(items.len());

        // 2. Process each item based on type
        for item in &items {
            item_ids.push(item.id);

            let params: ScheduledParams = serde_json::from_value(item.params.clone())
                .context("Failed to deserialize scheduled params")?;

            match params {
                ScheduledParams::WorkflowContinuation {
                    execution_id,
                    queue,
                    priority,
                } => {
                    db::work_queue::enqueue_work(&mut *tx, &execution_id, &queue, priority).await?;
                }
                ScheduledParams::ScheduledExecution {
                    execution_id,
                    queue,
                    priority,
                } => {
                    db::work_queue::enqueue_work(&mut *tx, &execution_id, &queue, priority).await?;
                }
            }
        }

        // 3. Delete processed items
        db::scheduled_queue::delete_items(&mut tx, &item_ids).await?;

        let count = items.len() as u32;

        tx.commit().await?;

        Ok(count)
    }
}
