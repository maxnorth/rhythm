use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::sync::Arc;

#[cfg(not(test))]
use std::sync::OnceLock;

#[cfg(test)]
use std::sync::RwLock;

use crate::config::Config;

/// Global database pool
#[cfg(not(test))]
static POOL: OnceLock<Arc<PgPool>> = OnceLock::new();

#[cfg(test)]
static POOL: RwLock<Option<Arc<PgPool>>> = RwLock::new(None);

/// Get the database pool (must be initialized first)
pub async fn get_pool() -> Result<Arc<PgPool>> {
    #[cfg(not(test))]
    {
        POOL.get()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Database pool not initialized"))
    }

    #[cfg(test)]
    {
        POOL.read()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Database pool not initialized - use with_test_db() helper in tests"))
    }
}

/// Initialize the database pool (production use)
pub async fn initialize_pool() -> Result<Arc<PgPool>> {
    #[cfg(not(test))]
    {
        if let Some(pool) = POOL.get() {
            return Ok(pool.clone());
        }
    }

    #[cfg(test)]
    {
        if let Some(pool) = POOL.read().unwrap().as_ref() {
            return Ok(pool.clone());
        }
    }

    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Get database URL
    let database_url = config
        .database
        .url
        .expect("Database URL should be validated by config loading");

    // Create the pool
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

    #[cfg(not(test))]
    {
        let _ = POOL.set(pool.clone());
    }

    #[cfg(test)]
    {
        *POOL.write().unwrap() = Some(pool.clone());
    }

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
pub async fn check_initialized() -> Result<()> {
    let pool = get_pool().await?;

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
            "Rhythm database has not been initialized\n\n\
            Please run migrations first using your language adapter:\n\
              Python: python -m rhythm migrate\n\
              Node:   npx rhythm migrate"
        );
    }

    Ok(())
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use std::sync::{Mutex as StdMutex, OnceLock};

    /// Global mutex to ensure only one test uses the database at a time
    static TEST_MUTEX: OnceLock<StdMutex<()>> = OnceLock::new();

    /// Guard that ensures exclusive database access and cleanup
    pub struct TestDbGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for TestDbGuard {
        fn drop(&mut self) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        // Truncate all tables before closing
                        if let Ok(pool) = get_pool().await {
                            let _ = sqlx::query("TRUNCATE TABLE worker_heartbeats, executions, workflow_definitions, workflow_execution_context, workflow_signals, dead_letter_queue CASCADE")
                                .execute(pool.as_ref())
                                .await;
                        }

                        // Close and reset the pool
                        if let Some(pool) = POOL.write().unwrap().take() {
                            pool.close().await;
                        }
                    });
                });
            }));
        }
    }

    /// Initialize the database for testing
    /// Returns a guard that will clean up when dropped
    pub async fn with_test_db() -> TestDbGuard {
        let mutex = TEST_MUTEX.get_or_init(|| StdMutex::new(()));
        let lock = match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        // Initialize the pool
        initialize_pool().await.expect("Failed to initialize test database");

        TestDbGuard { _lock: lock }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_initialization() {
        let _guard = with_test_db().await;

        let pool = get_pool().await.unwrap();
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
        assert_eq!(result.0, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_fails_without_initialization() {
        // Don't call with_test_db()
        let result = get_pool().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }
}
