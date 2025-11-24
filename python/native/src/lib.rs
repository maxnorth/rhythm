//! Python FFI bindings for Rhythm Core
//!
//! This module provides thin PyO3 wrappers around the core adapter interface.
//! All functions delegate to `rhythm_core::adapter` for a stable, language-agnostic API.

use ::rhythm_core::{adapter, workflows, ExecutionType};
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
#[pyo3(signature = (database_url=None, config_path=None, auto_migrate=true, require_initialized=true, workflows_json=None))]
fn initialize_sync(
    py: Python,
    database_url: Option<String>,
    config_path: Option<String>,
    auto_migrate: bool,
    require_initialized: bool,
    workflows_json: Option<String>,
) -> PyResult<()> {
    let runtime = get_runtime();

    // Parse workflows if provided
    let workflows = if let Some(json) = workflows_json {
        let workflows_data: Vec<serde_json::Value> = serde_json::from_str(&json)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid workflows JSON: {}", e)))?;

        let mut wf_list = Vec::new();
        for workflow_data in workflows_data {
            let name = workflow_data["name"]
                .as_str()
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Workflow missing 'name' field"))?
                .to_string();
            let source = workflow_data["source"]
                .as_str()
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Workflow missing 'source' field"))?
                .to_string();
            let file_path = workflow_data["file_path"]
                .as_str()
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Workflow missing 'file_path' field"))?
                .to_string();

            wf_list.push(workflows::WorkflowFile { name, source, file_path });
        }
        Some(wf_list)
    } else {
        None
    };

    // Release GIL while doing DB initialization
    py.allow_threads(|| {
        runtime.block_on(adapter::initialize(database_url, config_path, auto_migrate, require_initialized, workflows))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run migrations
#[pyfunction]
fn migrate_sync(py: Python) -> PyResult<()> {
    let runtime = get_runtime();

    // Release GIL while running migrations
    py.allow_threads(|| {
        runtime.block_on(adapter::migrate())
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Execution Lifecycle ===================== */

/// Create an execution
#[pyfunction]
#[pyo3(signature = (exec_type, function_name, queue, inputs, parent_workflow_id=None, id=None))]
fn create_execution_sync(
    py: Python,
    exec_type: String,
    function_name: String,
    queue: String,
    inputs: String,
    parent_workflow_id: Option<String>,
    id: Option<String>,
) -> PyResult<String> {
    let runtime = get_runtime();

    let exec_type = match exec_type.as_str() {
        "task" => ExecutionType::Task,
        "workflow" => ExecutionType::Workflow,
        _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid execution type")),
    };

    let inputs: JsonValue = serde_json::from_str(&inputs)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    // Release GIL while doing DB write
    py.allow_threads(|| {
        runtime.block_on(adapter::create_execution(
            exec_type,
            function_name,
            queue,
            inputs,
            parent_workflow_id,
            id,
        ))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Claim an execution for a worker
#[pyfunction]
fn claim_execution_sync(py: Python, worker_id: String, queues: Vec<String>) -> PyResult<Option<String>> {
    let runtime = get_runtime();

    // Release GIL while doing DB query
    let result = py.allow_threads(|| {
        runtime.block_on(adapter::claim_execution(worker_id, queues))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.map(|json| json.to_string()))
}

/// Complete an execution
#[pyfunction]
fn complete_execution_sync(py: Python, execution_id: String, result: String) -> PyResult<()> {
    let runtime = get_runtime();

    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    // Release GIL while doing DB write
    py.allow_threads(|| {
        runtime.block_on(adapter::complete_execution(execution_id, result))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Fail an execution
#[pyfunction]
fn fail_execution_sync(py: Python, execution_id: String, error: String, retry: bool) -> PyResult<()> {
    let runtime = get_runtime();

    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    // Release GIL while doing DB write
    py.allow_threads(|| {
        runtime.block_on(adapter::fail_execution(execution_id, error, retry))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get execution by ID
#[pyfunction]
fn get_execution_sync(py: Python, execution_id: String) -> PyResult<Option<String>> {
    let runtime = get_runtime();

    // Release GIL while doing DB query
    let result = py.allow_threads(|| {
        runtime.block_on(adapter::get_execution(execution_id))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.map(|json| json.to_string()))
}

/* ===================== Workflow Operations ===================== */

/// Start a workflow execution
#[pyfunction]
fn start_workflow_sync(py: Python, workflow_name: String, inputs_json: String) -> PyResult<String> {
    let runtime = get_runtime();

    let inputs: serde_json::Value = serde_json::from_str(&inputs_json)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid inputs JSON: {}", e)))?;

    // Release GIL while doing DB write
    py.allow_threads(|| {
        runtime.block_on(adapter::start_workflow(workflow_name, inputs))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get workflow child tasks
#[pyfunction]
fn get_workflow_tasks_sync(py: Python, workflow_id: String) -> PyResult<String> {
    let runtime = get_runtime();

    // Release GIL while doing DB query
    let tasks = py.allow_threads(|| {
        runtime.block_on(adapter::get_workflow_tasks(workflow_id))
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&tasks)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Python Module ===================== */

/// Python module definition
#[pymodule]
fn rhythm_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // System
    m.add_function(wrap_pyfunction!(init_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(initialize_sync, m)?)?;
    m.add_function(wrap_pyfunction!(migrate_sync, m)?)?;

    // Execution lifecycle
    m.add_function(wrap_pyfunction!(create_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(claim_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(complete_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(fail_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_execution_sync, m)?)?;

    // Workflow operations
    m.add_function(wrap_pyfunction!(start_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_workflow_tasks_sync, m)?)?;

    Ok(())
}
