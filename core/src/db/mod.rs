//! V2 Database Layer
//!
//! This module provides database operations for the V2 workflow engine.
//! All SQL queries are isolated here to keep the business logic clean.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub mod executions;
pub mod migration;
pub mod pool;
pub mod scheduled_queue;
pub mod signals;
pub mod work_queue;
pub mod workflow_definitions;
pub mod workflow_execution_context;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use executions::*;
pub use migration::*;
pub use pool::*;
pub use scheduled_queue::*;
pub use signals::*;
pub use work_queue::*;
pub use workflow_definitions::*;
pub use workflow_execution_context::*;

/// Fetch current time from the database
pub async fn get_db_time(pool: &PgPool) -> Result<DateTime<Utc>> {
    let row: (DateTime<Utc>,) = sqlx::query_as("SELECT NOW()")
        .fetch_one(pool)
        .await
        .context("Failed to fetch database time")?;
    Ok(row.0)
}
