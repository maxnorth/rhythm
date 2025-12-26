//! Internal Worker
//!
//! Background worker that handles internal maintenance tasks like
//! promoting scheduled work to the ready queue.

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::services::SchedulerService;

#[cfg(test)]
mod tests;

const POLL_INTERVAL: Duration = Duration::from_millis(1000);
const BATCH_SIZE: i32 = 100;

/// Internal worker that handles background maintenance tasks.
pub struct InternalWorker {
    scheduler_service: SchedulerService,
    shutdown_token: CancellationToken,
}

impl InternalWorker {
    /// Create a new internal worker.
    pub fn new(scheduler_service: SchedulerService, shutdown_token: CancellationToken) -> Self {
        Self {
            scheduler_service,
            shutdown_token,
        }
    }

    /// Run the internal worker loop.
    ///
    /// This loop runs continuously until the shutdown token is cancelled.
    /// It handles internal maintenance tasks like promoting scheduled work.
    pub async fn run(self) {
        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    debug!("Internal worker received shutdown signal");
                    break;
                }
                _ = tokio::time::sleep(POLL_INTERVAL) => {
                    if let Err(e) = self.process_scheduled_work().await {
                        error!("Error processing scheduled work: {}", e);
                    }
                }
            }
        }

        debug!("Internal worker stopped");
    }

    /// Process ready items from the scheduled queue.
    async fn process_scheduled_work(&self) -> anyhow::Result<()> {
        let count = self
            .scheduler_service
            .process_ready_items(BATCH_SIZE)
            .await?;

        if count > 0 {
            debug!("Promoted {} scheduled items to work queue", count);
        }

        Ok(())
    }
}
