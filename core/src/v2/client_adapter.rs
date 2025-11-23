//! Client Adapter for V2 Workflow Engine
//!
//! This module provides the high-level API for interacting with the V2 workflow engine.
//! It is designed to be used by FFI layers (Python, Node.js, etc.) and provides a clean
//! interface without requiring direct database pool management in each call.

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use super::db;
use super::types::{CreateExecutionParams, Execution, ExecutionFilters, ExecutionType};

/// High-level client adapter for workflow operations
///
/// This adapter encapsulates database dependencies and provides a clean API
/// for workflow and execution management.
pub struct ClientAdapter {
    pool: PgPool,
}

impl ClientAdapter {
    /// Create a new ClientAdapter with the given database pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Submit a workflow for execution
    ///
    /// Creates an execution record and enqueues it for processing.
    /// Does not immediately execute the workflow - that happens when a worker
    /// claims the work from the queue.
    ///
    /// Returns the execution ID.
    pub async fn run_workflow(
        &self,
        name: &str,
        inputs: JsonValue,
        queue: &str,
    ) -> Result<String> {
        let mut tx = self.pool.begin().await?;

        // Create execution record
        let execution_id = db::executions::create_execution(
            &mut tx,
            CreateExecutionParams {
                id: None,
                exec_type: ExecutionType::Workflow,
                function_name: name.to_string(),
                queue: queue.to_string(),
                inputs,
                parent_workflow_id: None,
            },
        )
        .await?;

        // Enqueue work
        db::work_queue::enqueue_work(&mut *tx, &execution_id, queue, 0).await?;

        tx.commit().await?;

        Ok(execution_id)
    }

    /// Get execution details by ID
    ///
    /// Returns the full execution record including status, output, timestamps, etc.
    pub async fn get_execution(&self, execution_id: &str) -> Result<Option<Execution>> {
        db::executions::get_execution(&self.pool, execution_id).await
    }

    /// Query executions with optional filters
    ///
    /// Returns a list of executions matching the provided filters.
    pub async fn query_executions(&self, filters: ExecutionFilters) -> Result<Vec<Execution>> {
        db::executions::query_executions(&self.pool, filters).await
    }

    /// Create a new workflow version
    ///
    /// Registers a workflow definition with the given name and source code.
    /// Returns the workflow definition ID.
    pub async fn create_workflow_version(&self, name: &str, source: &str) -> Result<i32> {
        db::workflow_definitions::create_workflow_definition(&self.pool, name, source).await
    }
}
