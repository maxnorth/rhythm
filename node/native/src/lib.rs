use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as JsonValue;

use currant_core::types::*;
use currant_core::{db, executions, signals, worker};

/// Create an execution
#[napi]
pub async fn create_execution(
    exec_type: String,
    function_name: String,
    queue: String,
    priority: i32,
    args: String,
    kwargs: String,
    max_retries: i32,
    timeout_seconds: Option<i32>,
    parent_workflow_id: Option<String>,
) -> Result<String> {
    let exec_type = match exec_type.as_str() {
        "job" => ExecutionType::Job,
        "activity" => ExecutionType::Activity,
        "workflow" => ExecutionType::Workflow,
        _ => return Err(Error::from_reason("Invalid execution type")),
    };

    let args: JsonValue = serde_json::from_str(&args)
        .map_err(|e| Error::from_reason(format!("Invalid args JSON: {}", e)))?;

    let kwargs: JsonValue = serde_json::from_str(&kwargs)
        .map_err(|e| Error::from_reason(format!("Invalid kwargs JSON: {}", e)))?;

    let params = CreateExecutionParams {
        exec_type,
        function_name,
        queue,
        priority,
        args,
        kwargs,
        max_retries,
        timeout_seconds,
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

/// Suspend a workflow
#[napi]
pub async fn suspend_workflow(workflow_id: String, checkpoint: String) -> Result<()> {
    let checkpoint: JsonValue = serde_json::from_str(&checkpoint)
        .map_err(|e| Error::from_reason(format!("Invalid checkpoint JSON: {}", e)))?;

    executions::suspend_workflow(&workflow_id, checkpoint)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Resume a workflow
#[napi]
pub async fn resume_workflow(workflow_id: String) -> Result<()> {
    executions::resume_workflow(&workflow_id)
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

/// Get workflow activities
#[napi]
pub async fn get_workflow_activities(workflow_id: String) -> Result<String> {
    let activities = executions::get_workflow_activities(&workflow_id)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    serde_json::to_string(&activities).map_err(|e| Error::from_reason(e.to_string()))
}

/// Update worker heartbeat
#[napi]
pub async fn update_heartbeat(worker_id: String, queues: Vec<String>) -> Result<()> {
    worker::update_heartbeat(&worker_id, &queues)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Stop worker
#[napi]
pub async fn stop_worker(worker_id: String) -> Result<()> {
    worker::stop_worker(&worker_id)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Recover dead workers
#[napi]
pub async fn recover_dead_workers(timeout_seconds: i64) -> Result<u32> {
    let count = worker::recover_dead_workers(timeout_seconds)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    Ok(count as u32)
}

/// Send a signal to a workflow
#[napi]
pub async fn send_signal(
    workflow_id: String,
    signal_name: String,
    payload: String,
) -> Result<String> {
    let payload: JsonValue = serde_json::from_str(&payload)
        .map_err(|e| Error::from_reason(format!("Invalid payload JSON: {}", e)))?;

    signals::send_signal(&workflow_id, &signal_name, payload)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Get signals for a workflow
#[napi]
pub async fn get_signals(workflow_id: String, signal_name: String) -> Result<String> {
    let signals_list = signals::get_signals(&workflow_id, &signal_name)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?;

    serde_json::to_string(&signals_list).map_err(|e| Error::from_reason(e.to_string()))
}

/// Consume a signal
#[napi]
pub async fn consume_signal(signal_id: String) -> Result<()> {
    signals::consume_signal(&signal_id)
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Run database migrations
#[napi]
pub async fn migrate() -> Result<()> {
    db::migrate()
        .await
        .map_err(|e| Error::from_reason(e.to_string()))
}
