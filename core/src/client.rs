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
use crate::types::CreateExecutionParams;

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

        // 1. Bootstrap: Load config and create pool
        let config = crate::config::Config::builder()
            .database_url(database_url)
            .config_path(config_path.map(std::path::PathBuf::from))
            .build()
            .context("Failed to load configuration")?;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .min_connections(config.database.min_connections)
            .acquire_timeout(std::time::Duration::from_secs(
                config.database.acquire_timeout_secs,
            ))
            .idle_timeout(std::time::Duration::from_secs(
                config.database.idle_timeout_secs,
            ))
            .max_lifetime(std::time::Duration::from_secs(
                config.database.max_lifetime_secs,
            ))
            .connect(
                &config
                    .database
                    .url
                    .clone()
                    .expect("Database URL validated by config loading"),
            )
            .await
            .context("Failed to connect to database")?;

        // 2. Instantiate: Construct application with services
        let app = Application::new(config, pool);

        // 3. Initialize: Run migrations and register workflows
        app.initialization_service
            .initialize(auto_migrate, workflows)
            .await
            .context("Failed to initialize application")?;

        // 4. Store: Save singleton
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

    /// Claim work from the queue
    ///
    /// This blocks/retries until work is available. Workflows are executed
    /// internally, only tasks are returned to the caller.
    pub async fn claim_work(_worker_id: String, _queues: Vec<String>) -> Result<JsonValue> {
        let app = Self::get_app()?;
        let task = app.worker_service.claim_work().await?;
        Ok(serde_json::to_value(task)?)
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

    /* ===================== Internal Helpers ===================== */

    /// Get the application instance or return an error
    fn get_app() -> Result<&'static Application> {
        APP.get()
            .ok_or_else(|| anyhow!("Application not initialized - call Client::initialize() first"))
    }
}
