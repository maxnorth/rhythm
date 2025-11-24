//! Database migrations
//!
//! Provides migration functions.
//! All functions take a pool parameter - no global state.

use anyhow::{Context, Result};
use sqlx::PgPool;

/// Run database migrations
///
/// This is idempotent - safe to call multiple times.
pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("Failed to run migrations")?;

    Ok(())
}
