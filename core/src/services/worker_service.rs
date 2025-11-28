use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::worker::{self, DelegatedAction};

/// Service for worker operations (claiming and completing work)
#[derive(Clone)]
pub struct WorkerService {
    pool: PgPool,
    shutdown_token: CancellationToken,
}

impl WorkerService {
    pub fn new(pool: PgPool, shutdown_token: CancellationToken) -> Self {
        Self {
            pool,
            shutdown_token,
        }
    }

    /// Run cooperative worker loop - blocks until task needs host execution
    ///
    /// This method blocks/retries until work is available. When it finds work:
    /// - If it's a workflow: executes it internally and loops again
    /// - If it's a task: returns the task details to the host for execution
    ///
    /// Only returns when it has a task that needs to be executed by the host.
    pub async fn run_cooperative_worker_loop(&self) -> Result<DelegatedAction> {
        worker::run_cooperative_worker_loop(&self.pool, &self.shutdown_token).await
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
