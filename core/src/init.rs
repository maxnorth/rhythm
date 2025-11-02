//! Initialization system for Rhythm
//!
//! Provides a centralized initialization API that language adapters should call
//! before using any other Rhythm functionality. This ensures proper configuration
//! and database setup.
//!
//! # Example
//!
//! ```rust
//! use rhythm_core::init::{InitOptions, InitBuilder};
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

use crate::config::Config;
use crate::db;
use crate::workflows::WorkflowFile;

/// Global initialization state
static INIT_STATE: OnceLock<InitState> = OnceLock::new();

/// Initialization state
#[derive(Debug)]
struct InitState {
    config: Config,
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
    // If already initialized, this is a no-op
    if INIT_STATE.get().is_some() {
        return Ok(());
    }

    // Apply options to environment variables so they're used by config loading
    if let Some(url) = &options.database_url {
        std::env::set_var("RHYTHM_DATABASE_URL", url);
    }

    if let Some(path) = &options.config_path {
        std::env::set_var("RHYTHM_CONFIG_PATH", path);
    }

    // Load configuration (now with env vars set)
    let config = Config::load().context("Failed to load configuration")?;

    // Initialize the database pool
    db::initialize_pool().await.context("Failed to initialize database pool")?;

    // Check if database is initialized
    let is_initialized = match db::check_initialized().await {
        Ok(()) => true,
        Err(_) => false,
    };

    // Handle uninitialized database based on options
    if !is_initialized {
        if options.auto_migrate {
            // Automatically run migrations
            db::migrate()
                .await
                .context("Failed to run automatic migrations")?;
        } else if options.require_initialized {
            // Fail if database is not initialized
            anyhow::bail!(
                "Database has not been initialized\n\n\
                Please run migrations first using your language adapter:\n\
                  Python: python -m rhythm migrate\n\
                  Node:   npx rhythm migrate"
            );
        }
        // If neither auto_migrate nor require_initialized, allow uninitialized database
    }

    // Register workflows after migrations (if any provided)
    if !options.workflows.is_empty() {
        crate::workflows::register_workflows(options.workflows)
            .await
            .context("Failed to register workflows")?;
    }

    // Store initialization state
    let state = InitState { config };

    INIT_STATE
        .set(state)
        .map_err(|_| anyhow!("Initialization already completed"))?;

    Ok(())
}

/// Check if Rhythm has been initialized
pub fn is_initialized() -> bool {
    INIT_STATE.get().is_some()
}

/// Get the current configuration (panics if not initialized)
pub fn get_config() -> &'static Config {
    &INIT_STATE
        .get()
        .expect("Rhythm not initialized - call init() first")
        .config
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
