//! Python FFI bindings for Rhythm Core
//!
//! This module provides thin PyO3 wrappers around the Client interface.
//! All functions delegate to `rhythm_core::Client` for a stable, language-agnostic API.

use ::rhythm_core::{
    Client, CreateExecutionParams, ExecutionType, ScheduleExecutionParams, WorkflowFile,
};
use pyo3::prelude::*;
use serde_json::Value as JsonValue;
use std::sync::OnceLock;

/// Global shared Tokio runtime
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or initialize the global runtime
fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

/// Initialize the Rust runtime (must be called once)
#[pyfunction]
fn init_runtime() -> PyResult<()> {
    // Just initialize the global runtime
    let _ = get_runtime();
    Ok(())
}

/* ===================== System ===================== */

/// Initialize Rhythm with configuration options
#[pyfunction]
#[pyo3(signature = (database_url=None, config_path=None, auto_migrate=true, workflows_json=None))]
fn initialize_sync(
    py: Python,
    database_url: Option<String>,
    config_path: Option<String>,
    auto_migrate: bool,
    workflows_json: Option<String>,
) -> PyResult<()> {
    let runtime = get_runtime();

    // Parse workflows if provided
    let workflows = if let Some(json) = workflows_json {
        let workflows_data: Vec<serde_json::Value> = serde_json::from_str(&json).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid workflows JSON: {}",
                e
            ))
        })?;

        let mut wf_list = Vec::new();
        for workflow_data in workflows_data {
            let name = workflow_data["name"]
                .as_str()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>("Workflow missing 'name' field")
                })?
                .to_string();
            let source = workflow_data["source"]
                .as_str()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Workflow missing 'source' field",
                    )
                })?
                .to_string();
            let file_path = workflow_data["file_path"]
                .as_str()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Workflow missing 'file_path' field",
                    )
                })?
                .to_string();

            wf_list.push(WorkflowFile {
                name,
                source,
                file_path,
            });
        }
        wf_list
    } else {
        Vec::new()
    };

    // Release GIL while doing DB initialization
    py.allow_threads(|| {
        runtime.block_on(Client::initialize(
            database_url,
            config_path,
            auto_migrate,
            workflows,
        ))
    })
    .map_err(|e| {
        let error_msg = format!("{:?}", e);
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(error_msg)
    })
}

/* ===================== Execution Lifecycle ===================== */

/// Create an execution
#[pyfunction]
#[pyo3(signature = (exec_type, target_name, queue, inputs, parent_workflow_id=None, id=None))]
fn create_execution_sync(
    py: Python,
    exec_type: String,
    target_name: String,
    queue: String,
    inputs: String,
    parent_workflow_id: Option<String>,
    id: Option<String>,
) -> PyResult<String> {
    let runtime = get_runtime();

    let exec_type = match exec_type.as_str() {
        "task" => ExecutionType::Task,
        "workflow" => ExecutionType::Workflow,
        _ => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid execution type",
            ))
        }
    };

    let inputs: JsonValue = serde_json::from_str(&inputs)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    let params = CreateExecutionParams {
        id,
        exec_type,
        target_name,
        queue,
        inputs,
        parent_workflow_id,
    };

    // Release GIL while doing DB write
    py.allow_threads(|| runtime.block_on(Client::create_execution(params)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run cooperative worker loop - blocks until task needs host execution
///
/// Workflows are executed internally, only returns when task needs Python execution.
/// Queue is hardcoded to "default".
#[pyfunction]
fn run_cooperative_worker_loop(py: Python) -> PyResult<String> {
    let runtime = get_runtime();

    // Release GIL while running the worker loop
    let result = py
        .allow_threads(|| runtime.block_on(Client::run_cooperative_worker_loop()))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.to_string())
}

/// Request graceful shutdown of worker loops
#[pyfunction]
fn request_shutdown() -> PyResult<()> {
    Client::request_shutdown()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Start the internal worker (scheduler queue processor)
///
/// This should be called when starting a worker process. Not intended for public API use.
#[pyfunction]
fn start_internal_worker() -> PyResult<()> {
    let runtime = get_runtime();
    let _guard = runtime.enter();
    Client::start_internal_worker()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Complete an execution
#[pyfunction]
fn complete_execution_sync(py: Python, execution_id: String, result: String) -> PyResult<()> {
    let runtime = get_runtime();

    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    // Release GIL while doing DB write
    py.allow_threads(|| runtime.block_on(Client::complete_execution(execution_id, result)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Fail an execution
#[pyfunction]
fn fail_execution_sync(
    py: Python,
    execution_id: String,
    error: String,
    _retry: bool,
) -> PyResult<()> {
    let runtime = get_runtime();

    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    // Release GIL while doing DB write
    py.allow_threads(|| runtime.block_on(Client::fail_execution(execution_id, error)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get execution by ID
#[pyfunction]
fn get_execution_sync(py: Python, execution_id: String) -> PyResult<Option<String>> {
    let runtime = get_runtime();

    // Release GIL while doing DB query
    let result = py
        .allow_threads(|| runtime.block_on(Client::get_execution(execution_id)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.map(|json| json.to_string()))
}

/* ===================== Workflow Operations ===================== */

/// Start a workflow execution
#[pyfunction]
fn start_workflow_sync(py: Python, workflow_name: String, inputs_json: String) -> PyResult<String> {
    let runtime = get_runtime();

    let inputs: serde_json::Value = serde_json::from_str(&inputs_json).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid inputs JSON: {}", e))
    })?;

    // Release GIL while doing DB write
    py.allow_threads(|| runtime.block_on(Client::start_workflow(workflow_name, inputs, None)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get workflow child tasks
#[pyfunction]
fn get_workflow_tasks_sync(py: Python, workflow_id: String) -> PyResult<String> {
    let runtime = get_runtime();

    // Release GIL while doing DB query
    let tasks = py
        .allow_threads(|| runtime.block_on(Client::get_workflow_tasks(workflow_id)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&tasks)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Signal Operations ===================== */

/// Send a signal to a workflow
#[pyfunction]
#[pyo3(signature = (workflow_id, signal_name, payload_json, queue=None))]
fn send_signal_sync(
    py: Python,
    workflow_id: String,
    signal_name: String,
    payload_json: String,
    queue: Option<String>,
) -> PyResult<()> {
    let runtime = get_runtime();

    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid payload JSON: {}", e))
    })?;

    // Release GIL while doing DB write
    py.allow_threads(|| {
        runtime.block_on(Client::send_signal(workflow_id, signal_name, payload, queue))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Scheduling Operations ===================== */

/// Schedule an execution (workflow or task) to start at a future time
#[pyfunction]
#[pyo3(signature = (exec_type, target_name, inputs_json, run_at_iso, queue))]
fn schedule_execution_sync(
    py: Python,
    exec_type: String,
    target_name: String,
    inputs_json: String,
    run_at_iso: String,
    queue: String,
) -> PyResult<String> {
    let runtime = get_runtime();

    let exec_type = match exec_type.as_str() {
        "task" => ExecutionType::Task,
        "workflow" => ExecutionType::Workflow,
        _ => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid execution type",
            ))
        }
    };

    let inputs: serde_json::Value = serde_json::from_str(&inputs_json).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid inputs JSON: {}", e))
    })?;

    let run_at = chrono::NaiveDateTime::parse_from_str(&run_at_iso, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&run_at_iso, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid run_at datetime: {}",
                e
            ))
        })?;

    let params = ScheduleExecutionParams {
        exec_type,
        target_name,
        queue,
        inputs,
        run_at,
    };

    // Release GIL while doing DB write
    py.allow_threads(|| runtime.block_on(Client::schedule_execution(params)))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Python Module ===================== */

/// Python module definition
#[pymodule]
fn rhythm_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // System
    m.add_function(wrap_pyfunction!(init_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(initialize_sync, m)?)?;

    // Execution lifecycle
    m.add_function(wrap_pyfunction!(create_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(run_cooperative_worker_loop, m)?)?;
    m.add_function(wrap_pyfunction!(request_shutdown, m)?)?;
    m.add_function(wrap_pyfunction!(start_internal_worker, m)?)?;
    m.add_function(wrap_pyfunction!(complete_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(fail_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_execution_sync, m)?)?;

    // Workflow operations
    m.add_function(wrap_pyfunction!(start_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_workflow_tasks_sync, m)?)?;

    // Signal operations
    m.add_function(wrap_pyfunction!(send_signal_sync, m)?)?;

    // Scheduling operations
    m.add_function(wrap_pyfunction!(schedule_execution_sync, m)?)?;

    Ok(())
}
