use ::currant_core::{benchmark, cli, db, executions, init, signals, worker, CreateExecutionParams, ExecutionType};
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

/// Initialize Currant with configuration options
#[pyfunction]
#[pyo3(signature = (database_url=None, config_path=None, auto_migrate=true, require_initialized=true))]
fn initialize_sync(
    database_url: Option<String>,
    config_path: Option<String>,
    auto_migrate: bool,
    require_initialized: bool,
) -> PyResult<()> {
    let runtime = get_runtime();

    let mut builder = init::InitBuilder::new()
        .auto_migrate(auto_migrate)
        .require_initialized(require_initialized);

    if let Some(url) = database_url {
        builder = builder.database_url(url);
    }

    if let Some(path) = config_path {
        builder = builder.config_path(path);
    }

    runtime
        .block_on(builder.init())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
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
    let runtime = get_runtime();

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

/// Claim multiple executions (batch claiming)
#[pyfunction]
fn claim_executions_batch_sync(worker_id: String, queues: Vec<String>, limit: i32) -> PyResult<Vec<String>> {
    let runtime = get_runtime();

    let result = runtime
        .block_on(executions::claim_executions_batch(&worker_id, &queues, limit))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let mut json_executions = Vec::new();
    for exec in result {
        let json = serde_json::to_string(&exec)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        json_executions.push(json);
    }

    Ok(json_executions)
}

/// Complete an execution
#[pyfunction]
fn complete_execution_sync(execution_id: String, result: String) -> PyResult<()> {
    let runtime = get_runtime();

    let result: JsonValue = serde_json::from_str(&result)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::complete_execution(&execution_id, result))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Complete multiple executions in batch
#[pyfunction]
fn complete_executions_batch_sync(completions: Vec<(String, String)>) -> PyResult<()> {
    let runtime = get_runtime();

    // Parse JSON results
    let mut parsed_completions = Vec::new();
    for (id, result_str) in completions {
        let result: JsonValue = serde_json::from_str(&result_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        parsed_completions.push((id, result));
    }

    runtime
        .block_on(executions::complete_executions_batch(parsed_completions))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Fail an execution
#[pyfunction]
fn fail_execution_sync(execution_id: String, error: String, retry: bool) -> PyResult<()> {
    let runtime = get_runtime();

    let error: JsonValue = serde_json::from_str(&error)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::fail_execution(&execution_id, error, retry))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Suspend a workflow
#[pyfunction]
fn suspend_workflow_sync(workflow_id: String, checkpoint: String) -> PyResult<()> {
    let runtime = get_runtime();

    let checkpoint: JsonValue = serde_json::from_str(&checkpoint)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(executions::suspend_workflow(&workflow_id, checkpoint))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Resume a workflow
#[pyfunction]
fn resume_workflow_sync(workflow_id: String) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(executions::resume_workflow(&workflow_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get execution by ID
#[pyfunction]
fn get_execution_sync(execution_id: String) -> PyResult<Option<String>> {
    let runtime = get_runtime();

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

/// Get workflow child tasks
#[pyfunction]
fn get_workflow_tasks_sync(workflow_id: String) -> PyResult<String> {
    let runtime = get_runtime();

    let child_tasks = runtime
        .block_on(executions::get_workflow_tasks(&workflow_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&child_tasks)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Update worker heartbeat
#[pyfunction]
fn update_heartbeat_sync(worker_id: String, queues: Vec<String>) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(worker::update_heartbeat(&worker_id, &queues))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Stop worker
#[pyfunction]
fn stop_worker_sync(worker_id: String) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(worker::stop_worker(&worker_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Recover dead workers
#[pyfunction]
fn recover_dead_workers_sync(timeout_seconds: i64) -> PyResult<usize> {
    let runtime = get_runtime();

    runtime
        .block_on(worker::recover_dead_workers(timeout_seconds))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Send a signal to a workflow
#[pyfunction]
fn send_signal_sync(workflow_id: String, signal_name: String, payload: String) -> PyResult<String> {
    let runtime = get_runtime();

    let payload: JsonValue = serde_json::from_str(&payload)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    runtime
        .block_on(signals::send_signal(&workflow_id, &signal_name, payload))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Get signals for a workflow
#[pyfunction]
fn get_signals_sync(workflow_id: String, signal_name: String) -> PyResult<String> {
    let runtime = get_runtime();

    let signals = runtime
        .block_on(signals::get_signals(&workflow_id, &signal_name))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    serde_json::to_string(&signals)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Consume a signal
#[pyfunction]
fn consume_signal_sync(signal_id: String) -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(signals::consume_signal(&signal_id))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run migrations
#[pyfunction]
fn migrate_sync() -> PyResult<()> {
    let runtime = get_runtime();

    runtime
        .block_on(db::migrate())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

/// Run the CLI
///
/// This function takes Python's sys.argv to parse commands and handle all CLI logic.
/// It's called from Python's __main__.py after any necessary module imports.
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

/// Python module
#[pymodule]
fn currant_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(initialize_sync, m)?)?;
    m.add_function(wrap_pyfunction!(create_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(claim_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(claim_executions_batch_sync, m)?)?;
    m.add_function(wrap_pyfunction!(complete_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(complete_executions_batch_sync, m)?)?;
    m.add_function(wrap_pyfunction!(fail_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(suspend_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(resume_workflow_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_execution_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_workflow_tasks_sync, m)?)?;
    m.add_function(wrap_pyfunction!(update_heartbeat_sync, m)?)?;
    m.add_function(wrap_pyfunction!(stop_worker_sync, m)?)?;
    m.add_function(wrap_pyfunction!(recover_dead_workers_sync, m)?)?;
    m.add_function(wrap_pyfunction!(send_signal_sync, m)?)?;
    m.add_function(wrap_pyfunction!(get_signals_sync, m)?)?;
    m.add_function(wrap_pyfunction!(consume_signal_sync, m)?)?;
    m.add_function(wrap_pyfunction!(migrate_sync, m)?)?;
    m.add_function(wrap_pyfunction!(run_cli_sync, m)?)?;
    m.add_function(wrap_pyfunction!(run_benchmark_sync, m)?)?;
    Ok(())
}
