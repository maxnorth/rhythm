//! Initialization system for Rhythm
//!
//! Provides a centralized initialization API that language adapters should call
//! before using any other Rhythm functionality. This ensures proper configuration
//! and database setup.
//!
//! # Example
//!
//! ```rust
//! use rhythm_core::application::{InitOptions, InitBuilder};
//!
//! // Simple initialization (auto-migrate)
//! InitBuilder::new().init().await?;
//!
//! // Custom configuration
//! InitBuilder::new()
//!     .database_url("postgresql://localhost/rhythm")
//!     .auto_migrate(false)
//!     .init()
//!     .await?;
//! ```

use anyhow::{Context, Result, anyhow};
use std::sync::OnceLock;
use sqlx::PgPool;

use crate::adapter::WorkflowFile;
use crate::config::Config;
use crate::db;

/// Global application instance
static APPLICATION: OnceLock<Application> = OnceLock::new();

/// The Rhythm application instance
///
/// This represents the running Rhythm system with its configuration and database pool.
/// There is one global instance per process, accessible via `Application::get()`.
#[derive(Debug, Clone)]
pub struct Application {
    config: Config,
    pool: PgPool,
}

impl Application {
    /// Get the global application instance
    ///
    /// Panics if the application has not been initialized.
    pub fn get() -> &'static Self {
        APPLICATION
            .get()
            .expect("Application not initialized - call rhythm_core::initialize() first")
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

/// Options for initializing Rhythm
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Database URL (overrides config file and env vars)
    pub database_url: Option<String>,

    /// Config file path (overrides default search)
    pub config_path: Option<String>,

    /// Whether to automatically run migrations if database is not initialized
    pub auto_migrate: bool,

    /// Whether to fail if database is not initialized (when auto_migrate is false)
    pub require_initialized: bool,

    /// Workflow files to register during initialization
    pub workflows: Vec<WorkflowFile>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            database_url: None,
            config_path: None,
            auto_migrate: true,
            require_initialized: true,
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

    /// Set whether to require database to be initialized
    pub fn require_initialized(mut self, require: bool) -> Self {
        self.options.require_initialized = require;
        self
    }

    /// Add workflow files to register during initialization
    pub fn workflows(mut self, workflows: Vec<WorkflowFile>) -> Self {
        self.options.workflows = workflows;
        self
    }

    /// Initialize Rhythm with the configured options
    pub async fn init(self) -> Result<()> {
        initialize(self.options).await
    }
}

impl Default for InitBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize Rhythm with the given options
///
/// This function should be called once at the start of your application,
/// before using any other Rhythm functionality. It:
/// - Applies configuration overrides to environment variables
/// - Loads and validates configuration
/// - Checks database initialization
/// - Optionally runs migrations
/// - Sets up global state
///
/// Calling this function multiple times is safe - subsequent calls are no-ops.
pub async fn initialize(options: InitOptions) -> Result<()> {
    // Apply options to environment variables so they're used by config loading
    if let Some(url) = &options.database_url {
        std::env::set_var("RHYTHM_DATABASE_URL", url);
    }

    if let Some(path) = &options.config_path {
        std::env::set_var("RHYTHM_CONFIG_PATH", path);
    }

    // If already initialized in this process, skip the rest but still run migrations if requested
    if let Some(app) = APPLICATION.get() {
        // Handle migrations based on options
        if options.auto_migrate {
            // Always run migrations when auto_migrate is true (sqlx migrate is idempotent)
            db::migrate(app.pool())
                .await
                .context("Failed to run automatic migrations")?;
        }

        return Ok(());
    }

    // First time initialization - load config and create pool
    let config = Config::load().context("Failed to load configuration")?;

    // Create database pool using v2 factory with config settings
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
                    .expect("Database URL should be validated by config loading"),
            )
            .await
            .context("Failed to connect to database")?;

    // Handle migrations based on options
    if options.auto_migrate {
        // Always run migrations when auto_migrate is true (sqlx migrate is idempotent)
        db::migrate(&pool)
            .await
            .context("Failed to run automatic migrations")?;
    }

    // Register workflows after migrations (if any provided)
    if !options.workflows.is_empty() {
        for workflow in options.workflows {
            // Parse and validate the workflow source using v2 parser
            let _ast = crate::parser::parse(&workflow.source)
                .map_err(|e| anyhow!("Failed to parse workflow '{}' from {}: {:?}", workflow.name, workflow.file_path, e))?;

            // Register the workflow definition (stores raw source)
            crate::db::workflow_definitions::create_workflow_definition(
                &pool,
                &workflow.name,
                &workflow.source,
            )
            .await
            .with_context(|| format!("Failed to register workflow '{}'", workflow.name))?;
        }
    }

    // Store application instance
    let app = Application { config, pool };

    APPLICATION
        .set(app)
        .map_err(|_| anyhow!("Application already initialized"))?;

    Ok(())
}

/// Check if the application has been initialized
pub fn is_initialized() -> bool {
    APPLICATION.get().is_some()
}

/// Get the current configuration (convenience wrapper)
///
/// Panics if not initialized. Same as `Application::get().config()`.
pub fn get_config() -> &'static Config {
    Application::get().config()
}

/// Get the database pool (convenience wrapper)
///
/// Returns error if not initialized. Same as `Application::get().pool()` but with Result.
pub fn get_pool() -> Result<PgPool> {
    Ok(APPLICATION
        .get()
        .ok_or_else(|| anyhow!("Application not initialized - call rhythm_core::initialize() first"))?
        .pool.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_init_with_defaults() {
        let result = InitBuilder::new()
            .database_url("postgresql://rhythm@localhost/rhythm")
            .init()
            .await;

        assert!(result.is_ok());
        assert!(is_initialized());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore] // Cannot test with shared runtime - pool is already initialized
    async fn test_init_without_database_url() {
        // Temporarily unset DATABASE_URL for this test
        let original = std::env::var("RHYTHM_DATABASE_URL").ok();
        std::env::remove_var("RHYTHM_DATABASE_URL");

        let result = InitBuilder::new().init().await;
        // Should fail because no database URL configured
        assert!(result.is_err());

        // Restore original value
        if let Some(url) = original {
            std::env::set_var("RHYTHM_DATABASE_URL", url);
        }
    }
}
