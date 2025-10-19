use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::config::Config;

/// Global database pool (initialized once, reused forever)
static POOL: OnceLock<Arc<PgPool>> = OnceLock::new();
static POOL_INIT: OnceLock<Mutex<()>> = OnceLock::new();


/// Get or create a database pool (uses global singleton)
pub async fn get_pool() -> Result<Arc<PgPool>> {
    // Fast path: pool already initialized
    if let Some(pool) = POOL.get() {
        return Ok(pool.clone());
    }

    // Slow path: need to initialize (with mutex to prevent races)
    let init_lock = POOL_INIT.get_or_init(|| Mutex::new(()));
    let _guard = init_lock.lock().await;

    // Check again in case another thread initialized while we waited
    if let Some(pool) = POOL.get() {
        return Ok(pool.clone());
    }

    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Get database URL (validated by config loading, so safe to unwrap)
    let database_url = config
        .database
        .url
        .expect("Database URL should be validated by config loading");

    // Actually initialize the pool
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
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    let pool = Arc::new(pool);
    let _ = POOL.set(pool.clone());

    Ok(pool)
}

/// Run database migrations
pub async fn migrate() -> Result<()> {
    let pool = get_pool().await?;

    sqlx::migrate!("./migrations")
        .run(pool.as_ref())
        .await
        .context("Failed to run migrations")?;

    Ok(())
}

/// Check if the database has been initialized (migrations have been run)
/// Returns Ok(()) if initialized, or an error with helpful message if not
pub async fn check_initialized() -> Result<()> {
    let pool = get_pool().await?;

    // Check if the executions table exists (primary table created by migrations)
    let result = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_name = 'executions'
        )
        "#,
    )
    .fetch_one(pool.as_ref())
    .await
    .context("Failed to check database initialization")?;

    let exists: bool = result.get(0);

    if !exists {
        anyhow::bail!(
            "Currant database has not been initialized\n\n\
            Please run migrations first using your language adapter:\n\
              Python: python -m currant migrate\n\
              Node:   npx currant migrate"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_pool_initialization() {
        let pool = get_pool().await.unwrap();
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
        assert_eq!(result.0, 1);
    }
}
