//! Client - FFI boundary for Rhythm
//!
//! This is the ONLY stateful module in the core library. It holds the global
//! Application singleton and provides static methods that delegate to services.
//!
//! Language adapters (Python, Node.js, etc.) should ONLY call Client methods.

use anyhow::{anyhow, Context, Result};
use serde_json::Value as JsonValue;
use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::application::{Application, WorkflowFile};
use crate::types::{CreateExecutionParams, ScheduleExecutionParams};

/// Global application instance (ONLY place with static state)
static APP: OnceLock<Application> = OnceLock::new();

/// Lock to prevent concurrent initialization
static INIT_LOCK: Mutex<()> = Mutex::const_new(());

/// Client provides the FFI boundary for all Rhythm operations
pub struct Client;

impl Client {
    /* ===================== System ===================== */

    /// Initialize Rhythm (call once at startup)
    ///
    /// Handles bootstrap → instantiation → initialization → storage
    /// Thread-safe: uses mutex to prevent concurrent initialization
    pub async fn initialize(
        database_url: Option<String>,
        config_path: Option<String>,
        auto_migrate: bool,
        workflows: Vec<WorkflowFile>,
    ) -> Result<()> {
        // Acquire lock to prevent concurrent initialization
        let _guard = INIT_LOCK.lock().await;

        // Check if already initialized
        if APP.get().is_some() {
            return Ok(());
        }

        // Delegate to application::initialize for all the complex work
        let app = crate::application::initialize(crate::application::InitOptions {
            database_url,
            config_path,
            auto_migrate,
            workflows,
        })
        .await
        .context("Failed to initialize application")?;

        // Store the singleton
        APP.set(app)
            .map_err(|_| anyhow!("Application already initialized"))?;

        Ok(())
    }

    /// Check if the client has been initialized
    pub fn is_initialized() -> bool {
        APP.get().is_some()
    }

    /* ===================== Execution Lifecycle ===================== */

    /// Create a new execution and enqueue it for processing
    pub async fn create_execution(params: CreateExecutionParams) -> Result<String> {
        let app = Self::get_app()?;
        app.execution_service.create_execution(params).await
    }

    /// Get execution by ID
    pub async fn get_execution(execution_id: String) -> Result<Option<JsonValue>> {
        let app = Self::get_app()?;
        let execution = app.execution_service.get_execution(&execution_id).await?;
        Ok(execution.map(|e| serde_json::to_value(e).unwrap()))
    }

    /// Complete an execution with a result
    pub async fn complete_execution(execution_id: String, result: JsonValue) -> Result<()> {
        let app = Self::get_app()?;
        app.worker_service
            .complete_work(&execution_id, Some(result), None)
            .await
    }

    /// Fail an execution with an error
    pub async fn fail_execution(execution_id: String, error: JsonValue) -> Result<()> {
        let app = Self::get_app()?;
        app.worker_service
            .complete_work(&execution_id, None, Some(error))
            .await
    }

    /* ===================== Worker Operations ===================== */

    /// Run cooperative worker loop - blocks until task needs host execution
    ///
    /// This blocks/retries until work is available. When it finds work:
    /// - If it's a workflow: executes it internally and loops again
    /// - If it's a task: returns the task details to the host for execution
    ///
    /// Only returns when it has a task that needs to be executed by the host.
    /// Queue is hardcoded to "default".
    pub async fn run_cooperative_worker_loop() -> Result<JsonValue> {
        let app = Self::get_app()?;
        let action = app.worker_service.run_cooperative_worker_loop().await?;
        Ok(serde_json::to_value(action)?)
    }

    /// Request graceful shutdown of worker loops
    ///
    /// Triggers the shutdown token, causing all active worker loops to
    /// exit gracefully on their next iteration (~100ms latency).
    pub fn request_shutdown() -> Result<()> {
        let app = Self::get_app()?;
        app.request_shutdown();
        Ok(())
    }

    /* ===================== Workflow Operations ===================== */

    /// Start a workflow execution
    pub async fn start_workflow(
        workflow_name: String,
        inputs: JsonValue,
        queue: Option<String>,
    ) -> Result<String> {
        let app = Self::get_app()?;
        let queue = queue.as_deref().unwrap_or("default");
        app.workflow_service
            .start_workflow(&workflow_name, inputs, queue)
            .await
    }

    /// Schedule an execution (workflow or task) to start at a future time
    ///
    /// Creates the execution immediately in Pending status, then schedules
    /// it to be enqueued at the specified time.
    pub async fn schedule_execution(params: ScheduleExecutionParams) -> Result<String> {
        let app = Self::get_app()?;
        app.scheduler_service.schedule_execution(params).await
    }

    /// Register a workflow definition
    pub async fn register_workflow(name: String, source: String) -> Result<i32> {
        let app = Self::get_app()?;
        app.workflow_service.register_workflow(&name, &source).await
    }

    /// Get all child task executions for a workflow
    pub async fn get_workflow_tasks(workflow_id: String) -> Result<Vec<JsonValue>> {
        let app = Self::get_app()?;
        let tasks = app
            .workflow_service
            .get_workflow_tasks(&workflow_id)
            .await?;
        Ok(tasks
            .into_iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect())
    }

    /* ===================== Signal Operations ===================== */

    /// Send a signal to a workflow
    ///
    /// The workflow will be enqueued for processing and will pick up the
    /// signal on its next resumption.
    pub async fn send_signal(
        workflow_id: String,
        signal_name: String,
        payload: JsonValue,
        queue: Option<String>,
    ) -> Result<()> {
        let app = Self::get_app()?;
        let queue = queue.as_deref().unwrap_or("default");
        app.signal_service
            .send_signal(&workflow_id, &signal_name, payload, queue)
            .await
    }

    /* ===================== Internal Operations ===================== */

    /// Start the internal worker (scheduler queue processor)
    ///
    /// This should be called by the language adapter when starting a worker.
    /// Not intended for public API use.
    ///
    /// Returns an error if the internal worker has already been started.
    pub fn start_internal_worker() -> Result<()> {
        let app = Self::get_app()?;
        app.start_internal_worker()
    }

    /* ===================== Internal Helpers ===================== */

    /// Get the application instance or return an error
    fn get_app() -> Result<&'static Application> {
        APP.get()
            .ok_or_else(|| anyhow!("Application not initialized - call Client::initialize() first"))
    }
}
