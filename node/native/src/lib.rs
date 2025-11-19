use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as JsonValue;

use rhythm_core::types::*;
use rhythm_core::{db, executions};

/// Create an execution
#[napi]
pub async fn create_execution(
    exec_type: String,
    function_name: String,
    queue: String,
    inputs: String,
    parent_workflow_id: Option<String>,
) -> Result<String> {
    let exec_type = match exec_type.as_str() {
        "task" => ExecutionType::Task,
        "workflow" => ExecutionType::Workflow,
        _ => return Err(Error::from_reason("Invalid execution type")),
    };

    let inputs: JsonValue = serde_json::from_str(&inputs)
        .map_err(|e| Error::from_reason(format!("Invalid inputs JSON: {}", e)))?;

    let params = CreateExecutionParams {
        id: None,
        exec_type,
        function_name,
        queue,
        inputs,
        parent_workflow_id,
    };

    executions::create_execution(params)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Claim an execution for a worker
#[napi]
pub async fn claim_execution(worker_id: String, queues: Vec<String>) -> Result<Option<String>> {
    let result = executions::claim_execution(&worker_id, &queues)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    if let Some(exec) = result {
        let json = serde_json::to_string(&exec).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Some(json))
    } else {
        Ok(None)
    }
}

/// Complete an execution
#[napi]
pub async fn complete_execution(execution_id: String, result: String) -> Result<()> {
    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| Error::from_reason(format!("Invalid result JSON: {}", e)))?;

    executions::complete_execution(&execution_id, result)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Fail an execution
#[napi]
pub async fn fail_execution(execution_id: String, error: String, retry: bool) -> Result<()> {
    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| Error::from_reason(format!("Invalid error JSON: {}", e)))?;

    executions::fail_execution(&execution_id, error, retry)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Get execution by ID
#[napi]
pub async fn get_execution(execution_id: String) -> Result<Option<String>> {
    let result = executions::get_execution(&execution_id)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    if let Some(exec) = result {
        let json = serde_json::to_string(&exec).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Some(json))
    } else {
        Ok(None)
    }
}

/// Get workflow child tasks
#[napi]
pub async fn get_workflow_tasks(workflow_id: String) -> Result<String> {
    let child_tasks = executions::get_workflow_tasks(&workflow_id)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    serde_json::to_string(&child_tasks).map_err(|e| Error::from_reason(e.to_string()))
}

/// Run database migrations
#[napi]
pub async fn migrate() -> Result<()> {
    db::migrate()
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}
