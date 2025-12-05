//! V2 Database Pool Factory
//!
//! Simple factory for creating database connection pools.
//! No caching, no static storage - just a factory function.

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;

use crate::config::Config;

/// Create a new database connection pool
///
/// This is a simple factory - it creates a new pool instance every time.
/// The caller is responsible for managing the pool lifecycle.
///
/// Connection string is read from RHYTHM_DATABASE_URL environment variable.
pub async fn create_pool() -> Result<PgPool> {
    create_pool_with_max_connections(10).await
}

/// Create a new database connection pool with a specific max connections
///
/// This is a simple factory - it creates a new pool instance every time.
/// The caller is responsible for managing the pool lifecycle.
///
/// Connection string is read from RHYTHM_DATABASE_URL environment variable.
pub async fn create_pool_with_max_connections(max_connections: u32) -> Result<PgPool> {
    let database_url = env::var("RHYTHM_DATABASE_URL")
        .context("RHYTHM_DATABASE_URL environment variable not set")?;

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    Ok(pool)
}

/// Create a new database connection pool from a Config object
///
/// This is the recommended way to create a pool as it uses all configuration
/// settings from the Config (max_connections, timeouts, etc.)
pub async fn create_pool_from_config(config: &Config) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
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

    Ok(pool)
}
