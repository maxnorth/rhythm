use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db;
use crate::types::{CreateExecutionParams, Execution, ExecutionFilters, ExecutionType};

/// Service for workflow operations
#[derive(Clone)]
pub struct WorkflowService {
    pool: PgPool,
}

impl WorkflowService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a workflow execution
    pub async fn start_workflow(
        &self,
        workflow_name: &str,
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
                target_name: workflow_name.to_string(),
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

    /// Register a workflow definition
    pub async fn register_workflow(&self, name: &str, source: &str) -> Result<i32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Parse and validate the workflow source
        let _ast = crate::parser::parse(source)
            .map_err(|e| anyhow::anyhow!("Failed to parse workflow '{}': {:?}", name, e))?;

        // Generate version hash
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let version_hash = format!("{:x}", hasher.finish());

        // Register the workflow definition (stores raw source)
        db::workflow_definitions::create_workflow_definition(
            &self.pool,
            name,
            &version_hash,
            source,
        )
        .await
    }

    /// Get all child task executions for a workflow
    pub async fn get_workflow_tasks(&self, workflow_id: &str) -> Result<Vec<Execution>> {
        db::executions::query_executions(
            &self.pool,
            ExecutionFilters {
                parent_workflow_id: Some(workflow_id.to_string()),
                ..Default::default()
            },
        )
        .await
    }

    /// Get workflow definition by name
    pub async fn get_workflow_definition(&self, name: &str) -> Result<Option<String>> {
        match db::workflow_definitions::get_workflow_by_name(&self.pool, name).await {
            Ok((_id, source)) => Ok(Some(source)),
            Err(_) => Ok(None),
        }
    }
}
