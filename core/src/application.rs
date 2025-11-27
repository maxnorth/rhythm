//! Stateless initialization system for Rhythm
//!
//! Provides the initialization logic that creates an Application instance
//! with all configured services. The client module is responsible for
//! storing the singleton.

use anyhow::Result;
use sqlx::PgPool;

use crate::config::Config;
use crate::services::{ExecutionService, InitializationService, WorkerService, WorkflowService};

/// The Rhythm application instance with all services
pub struct Application {
    pub config: Config,
    pub pool: PgPool,
    pub execution_service: ExecutionService,
    pub workflow_service: WorkflowService,
    pub worker_service: WorkerService,
    pub initialization_service: InitializationService,
}

impl Application {
    /// Create a new Application instance (pure instantiation, no I/O)
    pub fn new(config: Config, pool: PgPool) -> Self {
        Self {
            config,
            pool: pool.clone(),
            execution_service: ExecutionService::new(pool.clone()),
            workflow_service: WorkflowService::new(pool.clone()),
            worker_service: WorkerService::new(pool.clone()),
            initialization_service: InitializationService::new(pool),
        }
    }

    /// Get the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
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

    // Create pool
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
        .await?;

    // Instantiate
    let app = Application::new(config, pool);

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
