pub mod db;
pub mod executions;
pub mod signals;
pub mod types;
pub mod worker;

// Re-export main types
pub use types::*;

use pyo3::prelude::*;
use serde_json::Value as JsonValue;

/// Initialize the Rust runtime (must be called once)
#[pyfunction]
fn init_runtime() -> PyResult<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
    Ok(())
}

/// Create an execution
#[pyfunction]
fn create_execution_sync(
    exec_type: String,
    function_name: String,
    queue: String,
    priority: i32,
    args: String,
    kwargs: String,
    max_retries: i32,
    timeout_seconds: Option<i32>,
    parent_workflow_id: Option<String>,
) -> PyResult<String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let exec_type = match exec_type.as_str() {
        "job" => ExecutionType::Job,
        "activity" => ExecutionType::Activity,
        "workflow" => ExecutionType::Workflow,
        _ => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid execution type",
            ))
        }
    };

    let args: JsonValue = serde_json::from_str(&args)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    let kwargs: JsonValue = serde_json::from_str(&kwargs)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

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

    runtime
        .block_on(executions::create_execution(params))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Claim an execution for a worker
#[pyfunction]
fn claim_execution_sync(worker_id: String, queues: Vec<String>) -> PyResult<Option<String>> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let result = runtime
        .block_on(executions::claim_execution(&worker_id, &queues))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    if let Some(exec) = result {
        let json = serde_json::to_string(&exec)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(Some(json))
    } else {
        Ok(None)
    }
}

/// Complete an execution
#[pyfunction]
fn complete_execution_sync(execution_id: String, result: String) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::complete_execution(&execution_id, result))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Fail an execution
#[pyfunction]
fn fail_execution_sync(execution_id: String, error: String, retry: bool) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::fail_execution(&execution_id, error, retry))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Suspend a workflow
#[pyfunction]
fn suspend_workflow_sync(workflow_id: String, checkpoint: String) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let checkpoint: JsonValue = serde_json::from_str(&checkpoint)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::suspend_workflow(&workflow_id, checkpoint))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Resume a workflow
#[pyfunction]
fn resume_workflow_sync(workflow_id: String) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(executions::resume_workflow(&workflow_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get execution by ID
#[pyfunction]
fn get_execution_sync(execution_id: String) -> PyResult<Option<String>> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let result = runtime
        .block_on(executions::get_execution(&execution_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    if let Some(exec) = result {
        let json = serde_json::to_string(&exec)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(Some(json))
    } else {
        Ok(None)
    }
}

/// Get workflow activities
#[pyfunction]
fn get_workflow_activities_sync(workflow_id: String) -> PyResult<String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let activities = runtime
        .block_on(executions::get_workflow_activities(&workflow_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&activities)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Update worker heartbeat
#[pyfunction]
fn update_heartbeat_sync(worker_id: String, queues: Vec<String>) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(worker::update_heartbeat(&worker_id, &queues))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Stop worker
#[pyfunction]
fn stop_worker_sync(worker_id: String) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(worker::stop_worker(&worker_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Recover dead workers
#[pyfunction]
fn recover_dead_workers_sync(timeout_seconds: i64) -> PyResult<usize> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(worker::recover_dead_workers(timeout_seconds))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Send a signal to a workflow
#[pyfunction]
fn send_signal_sync(workflow_id: String, signal_name: String, payload: String) -> PyResult<String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let payload: JsonValue = serde_json::from_str(&payload)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(signals::send_signal(&workflow_id, &signal_name, payload))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get signals for a workflow
#[pyfunction]
fn get_signals_sync(workflow_id: String, signal_name: String) -> PyResult<String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let signals = runtime
        .block_on(signals::get_signals(&workflow_id, &signal_name))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&signals)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Consume a signal
#[pyfunction]
fn consume_signal_sync(signal_id: String) -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(signals::consume_signal(&signal_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run migrations
#[pyfunction]
fn migrate_sync() -> PyResult<()> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    runtime
        .block_on(db::migrate())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Python module
#[pymodule]
fn currant_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(create_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(claim_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(complete_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(fail_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(suspend_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(resume_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_workflow_activities_sync, m)?)?;
    m.add_function(wrap_pyfunction!(update_heartbeat_sync, m)?)?;
    m.add_function(wrap_pyfunction!(stop_worker_sync, m)?)?;
    m.add_function(wrap_pyfunction!(recover_dead_workers_sync, m)?)?;
    m.add_function(wrap_pyfunction!(send_signal_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_signals_sync, m)?)?;
    m.add_function(wrap_pyfunction!(consume_signal_sync, m)?)?;
    m.add_function(wrap_pyfunction!(migrate_sync, m)?)?;
    Ok(())
}
