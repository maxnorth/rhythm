use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::env;
use std::sync::Arc;

/// Get or create a database pool
/// Note: sqlx pools are internally reference-counted, so creating multiple
/// "instances" actually shares the same underlying connection pool
pub async fn get_pool() -> Result<Arc<PgPool>> {
    let database_url = env::var("CURRANT_DATABASE_URL")
        .context("CURRANT_DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(pool))
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
