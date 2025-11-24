//! V2 Database Pool Factory
//!
//! Simple factory for creating database connection pools.
//! No caching, no static storage - just a factory function.

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;

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
