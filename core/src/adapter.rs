//! Language Adapter Interface
//!
//! This module defines the stable API surface for language adapters (Python, TypeScript, etc.).
//! Language-specific FFI layers should ONLY call functions from this module.
//!
//! ## Design Principles
//!
//! 1. **Stable Interface**: These function signatures are the contract with language adapters
//! 2. **Thin Wrapper**: Each function is a simple delegation to internal core modules
//! 3. **Single Source of Truth**: If a language needs it, it's defined here
//! 4. **Version Safe**: Changes here require coordination with all language adapters

use anyhow::Result;
use serde_json::Value as JsonValue;

use crate::{
    db, executions, init, workflows, CreateExecutionParams, ExecutionType,
};

/* ===================== System ===================== */

/// Initialize Rhythm with configuration options
pub async fn initialize(
    database_url: Option<String>,
    config_path: Option<String>,
    auto_migrate: bool,
    require_initialized: bool,
    workflows: Option<Vec<workflows::WorkflowFile>>,
) -> Result<()> {
    let options = init::InitOptions {
        database_url,
        config_path,
        auto_migrate,
        require_initialized,
        workflows: workflows.unwrap_or_default(),
    };
    init::initialize(options).await
}

/// Run database migrations
pub async fn migrate() -> Result<()> {
    db::migrate().await
}

/* ===================== Execution Lifecycle ===================== */

/// Create a new execution
pub async fn create_execution(
    exec_type: ExecutionType,
    function_name: String,
    queue: String,
    args: JsonValue,
    kwargs: JsonValue,
    max_retries: i32,
    parent_workflow_id: Option<String>,
    id: Option<String>,
) -> Result<String> {
    let params = CreateExecutionParams {
        id,
        exec_type,
        function_name,
        queue,
        args,
        kwargs,
        max_retries,
        parent_workflow_id,
    };
    executions::create_execution(params).await
}

/// Claim an execution for a worker
pub async fn claim_execution(worker_id: String, queues: Vec<String>) -> Result<Option<JsonValue>> {
    let execution = executions::claim_execution(&worker_id, &queues).await?;
    Ok(execution.map(|e| serde_json::to_value(e).unwrap()))
}

/// Complete an execution with a result
pub async fn complete_execution(execution_id: String, result: JsonValue) -> Result<()> {
    executions::complete_execution(&execution_id, result).await
}

/// Fail an execution with an error
pub async fn fail_execution(execution_id: String, error: JsonValue, retry: bool) -> Result<()> {
    executions::fail_execution(&execution_id, error, retry).await
}

/// Get execution by ID
pub async fn get_execution(execution_id: String) -> Result<Option<JsonValue>> {
    let execution = executions::get_execution(&execution_id).await?;
    Ok(execution.map(|e| serde_json::to_value(e).unwrap()))
}

/* ===================== Workflow Operations ===================== */

/// Start a workflow execution
pub async fn start_workflow(workflow_name: String, inputs: JsonValue) -> Result<String> {
    workflows::start_workflow(&workflow_name, inputs).await
}

/// Execute one step of a workflow
///
/// This is the main workflow execution entry point. Returns:
/// - "Suspended" if workflow is waiting for a task
/// - "Completed" if workflow finished successfully
/// - Error if workflow failed
pub async fn execute_workflow_step(execution_id: String) -> Result<String> {
    let result = workflows::execute_workflow_step(&execution_id).await?;
    Ok(format!("{:?}", result))
}

/// Get all child task executions for a workflow
pub async fn get_workflow_tasks(workflow_id: String) -> Result<Vec<JsonValue>> {
    let tasks = executions::get_workflow_tasks(&workflow_id).await?;
    Ok(tasks
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect())
}


