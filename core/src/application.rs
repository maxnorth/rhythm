//! Stateless initialization system for Rhythm
//!
//! Provides the initialization logic that creates an Application instance
//! with all configured services. The client module is responsible for
//! storing the singleton.

use anyhow::{bail, Result};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::services::{
    ExecutionService, InitializationService, SchedulerService, SignalService, WorkerService,
    WorkflowService,
};

/// The Rhythm application instance with all services
pub struct Application {
    pub config: Config,
    pub pool: PgPool,
    pub shutdown_token: CancellationToken,
    pub execution_service: ExecutionService,
    pub workflow_service: WorkflowService,
    pub worker_service: WorkerService,
    pub scheduler_service: SchedulerService,
    pub signal_service: SignalService,
    pub initialization_service: InitializationService,
    internal_worker_started: AtomicBool,
}

impl Application {
    /// Create a new Application instance
    pub async fn new(config: Config) -> Result<Self> {
        // Create pool from config
        let pool = crate::db::pool::create_pool_from_config(&config).await?;

        let shutdown_token = CancellationToken::new();

        let scheduler_service = SchedulerService::new(pool.clone());

        Ok(Self {
            config,
            pool: pool.clone(),
            shutdown_token: shutdown_token.clone(),
            execution_service: ExecutionService::new(pool.clone()),
            workflow_service: WorkflowService::new(pool.clone()),
            worker_service: WorkerService::new(pool.clone(), shutdown_token),
            scheduler_service,
            signal_service: SignalService::new(pool.clone()),
            initialization_service: InitializationService::new(pool),
            internal_worker_started: AtomicBool::new(false),
        })
    }

    /// Get the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Request graceful shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_token.cancel();
    }

    /// Start the internal worker (scheduler queue processor)
    ///
    /// This should be called when starting a worker process. The internal worker
    /// handles background tasks like processing the scheduled queue.
    ///
    /// Returns an error if the internal worker has already been started.
    pub fn start_internal_worker(&self) -> Result<()> {
        if self.internal_worker_started.swap(true, Ordering::SeqCst) {
            bail!("Internal worker has already been started");
        }

        let internal_worker = crate::internal_worker::InternalWorker::new(
            self.scheduler_service.clone(),
            self.shutdown_token.clone(),
        );
        tokio::spawn(internal_worker.run());
        Ok(())
    }
}

/// Workflow file for registration
#[derive(Debug, Clone)]
pub struct WorkflowFile {
    pub name: String,
    pub source: String,
    pub file_path: String,
}

/// Options for initializing Rhythm
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Database URL (overrides config file and env vars)
    pub database_url: Option<String>,

    /// Config file path (overrides default search)
    pub config_path: Option<String>,

    /// Whether to automatically run migrations if database is not initialized
    pub auto_migrate: bool,

    /// Workflow files to register during initialization
    pub workflows: Vec<WorkflowFile>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            database_url: None,
            config_path: None,
            auto_migrate: true,
            workflows: Vec::new(),
        }
    }
}

/// Builder for constructing InitOptions
pub struct InitBuilder {
    options: InitOptions,
}

impl InitBuilder {
    /// Create a new builder with default options
    pub fn new() -> Self {
        Self {
            options: InitOptions::default(),
        }
    }

    /// Set the database URL
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.options.database_url = Some(url.into());
        self
    }

    /// Set the config file path
    pub fn config_path(mut self, path: impl Into<String>) -> Self {
        self.options.config_path = Some(path.into());
        self
    }

    /// Set whether to automatically run migrations
    pub fn auto_migrate(mut self, auto: bool) -> Self {
        self.options.auto_migrate = auto;
        self
    }

    /// Add workflow files to register during initialization
    pub fn workflows(mut self, workflows: Vec<WorkflowFile>) -> Self {
        self.options.workflows = workflows;
        self
    }

    /// Initialize Rhythm with the configured options
    pub async fn init(self) -> Result<Application> {
        initialize(self.options).await
    }
}

impl Default for InitBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize Rhythm and return an Application instance
///
/// Thin wrapper for direct usage (without Client singleton).
/// Most users should use Client::initialize() instead.
pub async fn initialize(options: InitOptions) -> Result<Application> {
    // Bootstrap: Load config
    let config = crate::config::Config::builder()
        .database_url(options.database_url)
        .config_path(options.config_path.map(std::path::PathBuf::from))
        .build()?;

    // Instantiate (creates pool internally)
    let app = Application::new(config).await?;

    // Initialize
    app.initialization_service
        .initialize(options.auto_migrate, options.workflows)
        .await?;

    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_init_with_defaults() {
        let result = initialize(InitOptions {
            database_url: Some("postgresql://rhythm@localhost/rhythm".to_string()),
            ..Default::default()
        })
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_init_without_database_url() {
        // Temporarily unset DATABASE_URL for this test
        let original = std::env::var("RHYTHM_DATABASE_URL").ok();
        std::env::remove_var("RHYTHM_DATABASE_URL");

        let result = initialize(InitOptions::default()).await;
        // Should fail because no database URL configured
        assert!(result.is_err());

        // Restore original value
        if let Some(url) = original {
            std::env::set_var("RHYTHM_DATABASE_URL", url);
        }
    }
}
