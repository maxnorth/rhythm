//! Python FFI bindings for Rhythm Core
//!
//! This module provides thin PyO3 wrappers around the core adapter interface.
//! All functions delegate to `rhythm_core::adapter` for a stable, language-agnostic API.

use ::rhythm_core::{adapter, benchmark, cli, workflows, ExecutionType};
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

    runtime
        .block_on(adapter::initialize(database_url, config_path, auto_migrate, require_initialized, workflows))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run migrations
#[pyfunction]
fn migrate_sync() -> PyResult<()> {
    let runtime = get_runtime();
    runtime
        .block_on(adapter::migrate())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Execution Lifecycle ===================== */

/// Create an execution
#[pyfunction]
#[pyo3(signature = (exec_type, function_name, queue, priority, args, kwargs, max_retries, timeout_seconds=None, parent_workflow_id=None, id=None))]
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
    id: Option<String>,
) -> PyResult<String> {
    let runtime = get_runtime();

    let exec_type = match exec_type.as_str() {
        "task" => ExecutionType::Task,
        "workflow" => ExecutionType::Workflow,
        _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid execution type")),
    };

    let args: JsonValue = serde_json::from_str(&args)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    let kwargs: JsonValue = serde_json::from_str(&kwargs)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(adapter::create_execution(
            exec_type,
            function_name,
            queue,
            priority,
            args,
            kwargs,
            max_retries,
            timeout_seconds,
            parent_workflow_id,
            id,
        ))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Claim an execution for a worker
#[pyfunction]
fn claim_execution_sync(worker_id: String, queues: Vec<String>) -> PyResult<Option<String>> {
    let runtime = get_runtime();

    let result = runtime
        .block_on(adapter::claim_execution(worker_id, queues))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.map(|json| json.to_string()))
}

/// Complete an execution
#[pyfunction]
fn complete_execution_sync(execution_id: String, result: String) -> PyResult<()> {
    let runtime = get_runtime();

    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(adapter::complete_execution(execution_id, result))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Fail an execution
#[pyfunction]
fn fail_execution_sync(execution_id: String, error: String, retry: bool) -> PyResult<()> {
    let runtime = get_runtime();

    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(adapter::fail_execution(execution_id, error, retry))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get execution by ID
#[pyfunction]
fn get_execution_sync(execution_id: String) -> PyResult<Option<String>> {
    let runtime = get_runtime();

    let result = runtime
        .block_on(adapter::get_execution(execution_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(result.map(|json| json.to_string()))
}

/* ===================== Workflow Operations ===================== */

/// Start a workflow execution
#[pyfunction]
fn start_workflow_sync(workflow_name: String, inputs_json: String) -> PyResult<String> {
    let runtime = get_runtime();

    let inputs: serde_json::Value = serde_json::from_str(&inputs_json)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid inputs JSON: {}", e)))?;

    runtime
        .block_on(adapter::start_workflow(workflow_name, inputs))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Execute a single workflow step
#[pyfunction]
fn execute_workflow_step_sync(execution_id: String) -> PyResult<String> {
    let runtime = get_runtime();

    runtime
        .block_on(adapter::execute_workflow_step(execution_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get workflow child tasks
#[pyfunction]
fn get_workflow_tasks_sync(workflow_id: String) -> PyResult<String> {
    let runtime = get_runtime();

    let tasks = runtime
        .block_on(adapter::get_workflow_tasks(workflow_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&tasks)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Worker Management ===================== */

/// Update worker heartbeat
#[pyfunction]
fn update_heartbeat_sync(worker_id: String, queues: Vec<String>) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(adapter::update_heartbeat(worker_id, queues))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Stop worker
#[pyfunction]
fn stop_worker_sync(worker_id: String) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(adapter::stop_worker(worker_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Recover dead workers
#[pyfunction]
fn recover_dead_workers_sync(timeout_seconds: i32) -> PyResult<i32> {
    let runtime = get_runtime();

    runtime
        .block_on(adapter::recover_dead_workers(timeout_seconds))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/* ===================== Utilities ===================== */

/// Run the CLI
#[pyfunction]
fn run_cli_sync(args: Vec<String>) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(cli::run_cli_from_args(args))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run a benchmark
#[pyfunction]
#[pyo3(signature = (worker_command, workers, tasks, workflows, task_type, payload_size, tasks_per_workflow, queues, compute_iterations, warmup_percent, queue_distribution=None, duration=None, rate=None))]
#[allow(clippy::too_many_arguments)]
fn run_benchmark_sync(
    worker_command: Vec<String>,
    workers: usize,
    tasks: usize,
    workflows: usize,
    task_type: String,
    payload_size: usize,
    tasks_per_workflow: usize,
    queues: String,
    compute_iterations: usize,
    warmup_percent: f64,
    queue_distribution: Option<String>,
    duration: Option<String>,
    rate: Option<f64>,
) -> PyResult<()> {
    let runtime = get_runtime();

    let params = benchmark::BenchmarkParams {
        mode: benchmark::WorkerMode::External {
            command: worker_command,
            workers,
        },
        tasks,
        workflows,
        task_type,
        payload_size,
        tasks_per_workflow,
        queues,
        queue_distribution,
        duration,
        rate,
        compute_iterations,
        warmup_percent,
    };

    runtime
        .block_on(benchmark::run_benchmark(params))
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
    m.add_function(wrap_pyfunction!(execute_workflow_step_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_workflow_tasks_sync, m)?)?;

    // Worker management
    m.add_function(wrap_pyfunction!(update_heartbeat_sync, m)?)?;
    m.add_function(wrap_pyfunction!(stop_worker_sync, m)?)?;
    m.add_function(wrap_pyfunction!(recover_dead_workers_sync, m)?)?;

    // Utilities
    m.add_function(wrap_pyfunction!(run_cli_sync, m)?)?;
    m.add_function(wrap_pyfunction!(run_benchmark_sync, m)?)?;

    Ok(())
}
