use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db;
use crate::types::{CreateExecutionParams, Execution, ExecutionFilters};

/// Service for managing execution lifecycle
#[derive(Clone)]
pub struct ExecutionService {
    pool: PgPool,
}

impl ExecutionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new execution and enqueue it for processing
    pub async fn create_execution(&self, params: CreateExecutionParams) -> Result<String> {
        let mut tx = self.pool.begin().await?;

        let execution_id = db::executions::create_execution(&mut tx, params.clone()).await?;

        // Enqueue work for processing
        db::work_queue::enqueue_work(&mut *tx, &execution_id, &params.queue, 0).await?;

        tx.commit().await?;

        Ok(execution_id)
    }

    /// Get execution by ID
    pub async fn get_execution(&self, execution_id: &str) -> Result<Option<Execution>> {
        db::executions::get_execution(&self.pool, execution_id).await
    }

    /// Query executions with filters
    pub async fn query_executions(&self, filters: ExecutionFilters) -> Result<Vec<Execution>> {
        db::executions::query_executions(&self.pool, filters).await
    }

    /// Mark execution as failed
    pub async fn fail_execution(&self, execution_id: &str, error: JsonValue) -> Result<()> {
        crate::worker::complete_work(&self.pool, execution_id, None, Some(error)).await
    }
}
