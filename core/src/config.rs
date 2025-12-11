//! Configuration management for Rhythm
//!
//! Configuration is loaded with the following priority (highest to lowest):
//! 1. CLI flags (--database-url, etc.)
//! 2. Environment variables (RHYTHM_DATABASE_URL, etc.)
//! 3. Config file (rhythm.toml in project root or ~/.config/rhythm/config.toml)
//! 4. Built-in defaults
//!
//! # Example Config File (rhythm.toml)
//!
//! ```toml
//! [database]
//! url = "postgresql://localhost/rhythm"
//! max_connections = 50
//! min_connections = 5
//! acquire_timeout_secs = 10
//! idle_timeout_secs = 600
//! max_lifetime_secs = 1800
//! ```
//!
//! # Environment Variables
//!
//! All config values can be set via environment variables with the RHYTHM_ prefix:
//! - RHYTHM_DATABASE_URL
//! - RHYTHM_DATABASE_MAX_CONNECTIONS
//! - RHYTHM_DATABASE_MIN_CONNECTIONS
//! - etc.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
}

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL (required)
    pub url: Option<String>,

    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of connections in the pool
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection acquire timeout in seconds
    #[serde(default = "default_acquire_timeout_secs")]
    pub acquire_timeout_secs: u64,

    /// Idle connection timeout in seconds
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,

    /// Maximum connection lifetime in seconds
    #[serde(default = "default_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
}

// Default value functions for serde
fn default_max_connections() -> u32 {
    50
}
fn default_min_connections() -> u32 {
    5
}
fn default_acquire_timeout_secs() -> u64 {
    10
}
fn default_idle_timeout_secs() -> u64 {
    600
}
fn default_max_lifetime_secs() -> u64 {
    1800
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: None,
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            acquire_timeout_secs: default_acquire_timeout_secs(),
            idle_timeout_secs: default_idle_timeout_secs(),
            max_lifetime_secs: default_max_lifetime_secs(),
        }
    }
}

impl Config {
    /// Load configuration with full priority chain:
    /// CLI flags → env vars → config file → defaults
    pub fn load() -> Result<Self> {
        Self::builder().build()
    }

    /// Load configuration from a specific file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Config = toml::from_str(&config_str)
            .with_context(|| format!("Failed to parse config file: {:?}", path.as_ref()))?;

        Ok(config)
    }

    /// Create a builder for constructing config with overrides
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// Builder for constructing Config with optional overrides
#[derive(Default)]
pub struct ConfigBuilder {
    config_path: Option<PathBuf>,
    database_url: Option<String>,
    max_connections: Option<u32>,
    min_connections: Option<u32>,
    acquire_timeout_secs: Option<u64>,
    idle_timeout_secs: Option<u64>,
    max_lifetime_secs: Option<u64>,
}

impl ConfigBuilder {
    /// Override the config file path
    pub fn config_path(mut self, path: Option<PathBuf>) -> Self {
        self.config_path = path;
        self
    }

    /// Override the database URL
    pub fn database_url(mut self, url: Option<String>) -> Self {
        self.database_url = url;
        self
    }

    /// Override max connections
    pub fn max_connections(mut self, max: Option<u32>) -> Self {
        self.max_connections = max;
        self
    }

    /// Override min connections
    pub fn min_connections(mut self, min: Option<u32>) -> Self {
        self.min_connections = min;
        self
    }

    /// Override acquire timeout
    pub fn acquire_timeout_secs(mut self, timeout: Option<u64>) -> Self {
        self.acquire_timeout_secs = timeout;
        self
    }

    /// Override idle timeout
    pub fn idle_timeout_secs(mut self, timeout: Option<u64>) -> Self {
        self.idle_timeout_secs = timeout;
        self
    }

    /// Override max lifetime
    pub fn max_lifetime_secs(mut self, lifetime: Option<u64>) -> Self {
        self.max_lifetime_secs = lifetime;
        self
    }

    /// Build the final config by applying priority chain
    pub fn build(self) -> Result<Config> {
        // Load .env file if present (do this first, so env vars can override it)
        let _ = dotenvy::dotenv(); // Ignore errors if .env doesn't exist

        // Step 1: Start with defaults
        let mut config = Config {
            database: DatabaseConfig::default(),
        };

        // Step 2: Try to load from config file
        if let Some(file_config) = self.load_from_file()? {
            config = file_config;
        }

        // Step 3: Overlay environment variables
        self.apply_env_vars(&mut config);

        // Step 4: Apply CLI overrides (highest priority)
        self.apply_overrides(&mut config);

        // Step 5: Validate required fields
        if config.database.url.is_none() {
            anyhow::bail!(
                "Database URL not configured\n\n\
                Please set the database URL using one of:\n\
                  1. Config file: Add 'url = \"postgresql://...\"' to [database] section in rhythm.toml\n\
                  2. Environment variable: RHYTHM_DATABASE_URL=postgresql://...\n\
                  3. CLI flag: --database-url postgresql://..."
            );
        }

        Ok(config)
    }

    /// Try to load config from file (searches default locations if no path specified)
    fn load_from_file(&self) -> Result<Option<Config>> {
        let config_path = if let Some(path) = &self.config_path {
            // Explicit path provided via builder
            if !path.exists() {
                anyhow::bail!("Config file not found: {:?}", path);
            }
            Some(path.clone())
        } else if let Ok(path_str) = env::var("RHYTHM_CONFIG_PATH") {
            // Path from environment variable
            let path = PathBuf::from(path_str);
            if !path.exists() {
                anyhow::bail!("Config file not found: {:?}", path);
            }
            Some(path)
        } else {
            // Search default locations
            self.find_config_file()
        };

        if let Some(path) = config_path {
            let config = Config::from_file(&path)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    /// Search for config file in default locations
    fn find_config_file(&self) -> Option<PathBuf> {
        // 1. Project root: ./rhythm.toml
        let project_config = PathBuf::from("rhythm.toml");
        if project_config.exists() {
            return Some(project_config);
        }

        // 2. User config: ~/.config/rhythm/config.toml
        if let Some(home) = env::var_os("HOME") {
            let user_config = PathBuf::from(home)
                .join(".config")
                .join("rhythm")
                .join("config.toml");
            if user_config.exists() {
                return Some(user_config);
            }
        }

        None
    }

    /// Apply environment variables to config
    fn apply_env_vars(&self, config: &mut Config) {
        // Database URL
        if let Ok(url) = env::var("RHYTHM_DATABASE_URL") {
            config.database.url = Some(url);
        }

        // Database pool settings
        if let Ok(max) = env::var("RHYTHM_DATABASE_MAX_CONNECTIONS") {
            if let Ok(max) = max.parse() {
                config.database.max_connections = max;
            }
        }

        if let Ok(min) = env::var("RHYTHM_DATABASE_MIN_CONNECTIONS") {
            if let Ok(min) = min.parse() {
                config.database.min_connections = min;
            }
        }

        if let Ok(timeout) = env::var("RHYTHM_DATABASE_ACQUIRE_TIMEOUT_SECS") {
            if let Ok(timeout) = timeout.parse() {
                config.database.acquire_timeout_secs = timeout;
            }
        }

        if let Ok(timeout) = env::var("RHYTHM_DATABASE_IDLE_TIMEOUT_SECS") {
            if let Ok(timeout) = timeout.parse() {
                config.database.idle_timeout_secs = timeout;
            }
        }

        if let Ok(lifetime) = env::var("RHYTHM_DATABASE_MAX_LIFETIME_SECS") {
            if let Ok(lifetime) = lifetime.parse() {
                config.database.max_lifetime_secs = lifetime;
            }
        }
    }

    /// Apply CLI overrides (highest priority)
    fn apply_overrides(&self, config: &mut Config) {
        if let Some(url) = &self.database_url {
            config.database.url = Some(url.clone());
        }

        if let Some(max) = self.max_connections {
            config.database.max_connections = max;
        }

        if let Some(min) = self.min_connections {
            config.database.min_connections = min;
        }

        if let Some(timeout) = self.acquire_timeout_secs {
            config.database.acquire_timeout_secs = timeout;
        }

        if let Some(timeout) = self.idle_timeout_secs {
            config.database.idle_timeout_secs = timeout;
        }

        if let Some(lifetime) = self.max_lifetime_secs {
            config.database.max_lifetime_secs = lifetime;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config {
            database: DatabaseConfig::default(),
        };

        assert_eq!(config.database.url, None);
        assert_eq!(config.database.max_connections, 50);
        assert_eq!(config.database.min_connections, 5);
    }

    #[test]
    fn test_parse_toml() {
        let toml_str = r#"
            [database]
            url = "postgresql://test/db"
            max_connections = 100
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.database.url,
            Some("postgresql://test/db".to_string())
        );
        assert_eq!(config.database.max_connections, 100);
        assert_eq!(config.database.min_connections, 5); // Default
    }

    #[test]
    fn test_builder_with_overrides() {
        let config = Config::builder()
            .database_url(Some("postgresql://override/db".to_string()))
            .max_connections(Some(200))
            .build()
            .unwrap();

        assert_eq!(
            config.database.url,
            Some("postgresql://override/db".to_string())
        );
        assert_eq!(config.database.max_connections, 200);
    }

    #[test]
    fn test_missing_database_url_error() {
        // Temporarily unset DATABASE_URL for this test
        let original = std::env::var("RHYTHM_DATABASE_URL").ok();
        std::env::remove_var("RHYTHM_DATABASE_URL");

        let result = Config::builder().build();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Database URL not configured"));

        // Restore original value
        if let Some(url) = original {
            std::env::set_var("RHYTHM_DATABASE_URL", url);
        }
    }
}
