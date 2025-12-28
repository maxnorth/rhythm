//! Signal Service
//!
//! Provides signal operations for external callers to send signals to workflows.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db;

/// Service for signal operations
#[derive(Clone)]
pub struct SignalService {
    pool: PgPool,
}

impl SignalService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Send a signal to a workflow
    ///
    /// Inserts the signal and enqueues the workflow for processing.
    /// The workflow will pick up the signal on its next resumption via
    /// resolve_signal_claims.
    pub async fn send_signal(
        &self,
        workflow_id: &str,
        signal_name: &str,
        payload: JsonValue,
        queue: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Insert the signal
        db::signals::send_signal(&mut *tx, workflow_id, signal_name, &payload)
            .await
            .context("Failed to send signal")?;

        // Enqueue the workflow for processing
        db::work_queue::enqueue_work(&mut *tx, workflow_id, queue, 0)
            .await
            .context("Failed to enqueue workflow")?;

        tx.commit().await?;

        Ok(())
    }
}
