use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::env;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

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

    // Actually initialize the pool
    let database_url =
        env::var("CURRANT_DATABASE_URL").context("CURRANT_DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(50)  // Increased for better concurrency
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database to be running
    async fn test_pool_initialization() {
        let pool = get_pool().await.unwrap();
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
        assert_eq!(result.0, 1);
    }
}
