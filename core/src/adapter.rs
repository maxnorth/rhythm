//! Language Adapter Interface
//!
//! This module defines the stable API surface for language adapters (Python, TypeScript, etc.).
//! Language-specific FFI layers should ONLY call functions from this module.
//!
//! ## Design Principles
//!
//! 1. **Stable Interface**: These function signatures are the contract with language adapters
//! 2. **Thin Wrapper**: Each function is a simple delegation to core modules
//! 3. **Single Source of Truth**: If a language needs it, it's defined here
//! 4. **Version Safe**: Changes here require coordination with all language adapters

use anyhow::Result;
use serde_json::Value as JsonValue;

use crate::{
    Application, application, db, worker,
    types::{CreateExecutionParams, ExecutionType as CoreExecutionType},
};

// Re-export types for FFI layers
pub use crate::types::ExecutionType;

/// Represents a workflow file from a language adapter
#[derive(Debug, Clone)]
pub struct WorkflowFile {
    pub name: String,
    pub source: String,
    pub file_path: String,
}

/* ===================== System ===================== */

/// Initialize Rhythm with configuration options
pub async fn initialize(
    database_url: Option<String>,
    config_path: Option<String>,
    auto_migrate: bool,
    require_initialized: bool,
    workflows: Option<Vec<WorkflowFile>>,
) -> Result<()> {
    let options = application::InitOptions {
        database_url,
        config_path,
        auto_migrate,
        require_initialized,
        workflows: workflows.unwrap_or_default(),
    };

    // Initialize core (db pool, config, migrations)
    application::initialize(options).await?;

    // Workflow registration is now handled in application::initialize
    Ok(())
}

/// Run database migrations
pub async fn migrate() -> Result<()> {
    let app = Application::get();
    db::migrate(app.pool()).await
}

/* ===================== Execution Lifecycle ===================== */

/// Create a new execution
pub async fn create_execution(
    exec_type: ExecutionType,
    function_name: String,
    queue: String,
    inputs: JsonValue,
    parent_workflow_id: Option<String>,
    id: Option<String>,
) -> Result<String> {
    let app = Application::get();
    let mut tx = app.pool().begin().await?;

    // Convert adapter ExecutionType to core ExecutionType
    let core_exec_type = match exec_type {
        ExecutionType::Task => CoreExecutionType::Task,
        ExecutionType::Workflow => CoreExecutionType::Workflow,
    };

    let params = CreateExecutionParams {
        id,
        exec_type: core_exec_type,
        function_name,
        queue: queue.clone(),
        inputs,
        parent_workflow_id,
    };

    let execution_id = db::executions::create_execution(&mut tx, params).await?;

    // Enqueue work for processing
    db::work_queue::enqueue_work(&mut *tx, &execution_id, &queue, 0).await?;

    tx.commit().await?;

    Ok(execution_id)
}

/// Claim an execution for a worker
pub async fn claim_execution(_worker_id: String, _queues: Vec<String>) -> Result<Option<JsonValue>> {
    let app = Application::get();

    // claim_work handles workflows internally and only returns tasks
    // TODO: support worker_id and queues parameters (currently hardcoded to "default")
    let claimed_task = worker::claim_work(app.pool()).await?;

    // Return the claimed task as JSON (includes execution_id, function_name, inputs)
    Ok(Some(serde_json::to_value(claimed_task)?))
}

/// Complete an execution with a result
pub async fn complete_execution(execution_id: String, result: JsonValue) -> Result<()> {
    let app = Application::get();
    worker::complete_work(app.pool(), &execution_id, Some(result), None).await
}

/// Fail an execution with an error
pub async fn fail_execution(execution_id: String, error: JsonValue, _retry: bool) -> Result<()> {
    let app = Application::get();
    // No separate retry flag - just report the error
    worker::complete_work(app.pool(), &execution_id, None, Some(error)).await
}

/// Get execution by ID
pub async fn get_execution(execution_id: String) -> Result<Option<JsonValue>> {
    let app = Application::get();
    let execution = db::executions::get_execution(app.pool(), &execution_id).await?;
    Ok(execution.map(|e| serde_json::to_value(e).unwrap()))
}

/* ===================== Workflow Operations ===================== */

/// Start a workflow execution
pub async fn start_workflow(workflow_name: String, inputs: JsonValue) -> Result<String> {
    let app = Application::get();
    let mut tx = app.pool().begin().await?;

    // Create execution record
    let execution_id = db::executions::create_execution(
        &mut tx,
        CreateExecutionParams {
            id: None,
            exec_type: CoreExecutionType::Workflow,
            function_name: workflow_name.clone(),
            queue: "default".to_string(),
            inputs,
            parent_workflow_id: None,
        },
    )
    .await?;

    // Enqueue work
    db::work_queue::enqueue_work(&mut *tx, &execution_id, "default", 0).await?;

    tx.commit().await?;

    Ok(execution_id)
}

/// Get all child task executions for a workflow
pub async fn get_workflow_tasks(workflow_id: String) -> Result<Vec<JsonValue>> {
    let app = Application::get();

    // Query all child executions with this parent_workflow_id
    let tasks = db::executions::query_executions(
        app.pool(),
        crate::types::ExecutionFilters {
            parent_workflow_id: Some(workflow_id),
            ..Default::default()
        },
    )
    .await?;

    Ok(tasks
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect())
}
