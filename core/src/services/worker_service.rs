use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::worker::{self, ClaimedTask};

/// Service for worker operations (claiming and completing work)
#[derive(Clone)]
pub struct WorkerService {
    pool: PgPool,
}

impl WorkerService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Claim work from the queue
    ///
    /// This method blocks/retries until work is available. When it finds work:
    /// - If it's a workflow: executes it internally and loops again
    /// - If it's a task: returns the task details to the host for execution
    pub async fn claim_work(&self) -> Result<ClaimedTask> {
        worker::claim_work(&self.pool).await
    }

    /// Complete work after task execution
    ///
    /// Either result OR error should be Some, not both.
    /// If result is Some, marks the task as completed.
    /// If error is Some, marks the task as failed.
    pub async fn complete_work(
        &self,
        execution_id: &str,
        result: Option<JsonValue>,
        error: Option<JsonValue>,
    ) -> Result<()> {
        worker::complete_work(&self.pool, execution_id, result, error).await
    }
}
