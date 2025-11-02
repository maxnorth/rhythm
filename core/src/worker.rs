use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use sqlx::Row;

use crate::db::get_pool;
use crate::types::*;

/// Update worker heartbeat
pub async fn update_heartbeat(worker_id: &str, queues: &[String]) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        INSERT INTO worker_heartbeats (worker_id, last_heartbeat, queues, status, metadata)
        VALUES ($1, NOW(), $2, 'running', '{}')
        ON CONFLICT (worker_id)
        DO UPDATE SET
            last_heartbeat = NOW(),
            queues = $2,
            status = 'running'
        "#,
    )
    .bind(worker_id)
    .bind(queues)
    .execute(pool.as_ref())
    .await
    .context("Failed to update heartbeat")?;

    Ok(())
}

/// Mark worker as stopped
pub async fn stop_worker(worker_id: &str) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE worker_heartbeats
        SET status = 'stopped'
        WHERE worker_id = $1
        "#,
    )
    .bind(worker_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to stop worker")?;

    Ok(())
}

/// Detect and recover from dead workers
pub async fn recover_dead_workers(timeout_seconds: i64) -> Result<usize> {
    let pool = get_pool().await?;

    let cutoff = Utc::now() - Duration::seconds(timeout_seconds);

    // Find dead workers
    let dead_workers: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT worker_id FROM worker_heartbeats
        WHERE last_heartbeat < $1
          AND status = 'running'
        "#,
    )
    .bind(cutoff)
    .fetch_all(pool.as_ref())
    .await
    .context("Failed to find dead workers")?;

    if dead_workers.is_empty() {
        return Ok(0);
    }

    // Reset their executions to pending
    let result = sqlx::query(
        r#"
        UPDATE executions
        SET status = 'pending',
            worker_id = NULL,
            claimed_at = NULL
        WHERE worker_id = ANY($1)
          AND status IN ('running', 'suspended')
        "#,
    )
    .bind(&dead_workers)
    .execute(pool.as_ref())
    .await
    .context("Failed to reset dead worker executions")?;

    // Mark workers as stopped
    sqlx::query(
        r#"
        UPDATE worker_heartbeats
        SET status = 'stopped'
        WHERE worker_id = ANY($1)
        "#,
    )
    .bind(&dead_workers)
    .execute(pool.as_ref())
    .await
    .context("Failed to mark workers as stopped")?;

    Ok(result.rows_affected() as usize)
}

/// Get all active workers
pub async fn get_active_workers() -> Result<Vec<WorkerHeartbeat>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT * FROM worker_heartbeats
        WHERE status = 'running'
        ORDER BY last_heartbeat DESC
        "#,
    )
    .fetch_all(pool.as_ref())
    .await
    .context("Failed to get active workers")?;

    let mut workers = Vec::new();
    for row in rows {
        workers.push(WorkerHeartbeat {
            worker_id: row.get("worker_id"),
            last_heartbeat: row.get("last_heartbeat"),
            queues: row.get("queues"),
            status: row.get("status"),
            metadata: row.get("metadata"),
        });
    }

    Ok(workers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_heartbeat() {
        let _guard = with_test_db().await;

        update_heartbeat("test-worker", &["test".to_string()])
            .await
            .unwrap();

        let workers = get_active_workers().await.unwrap();
        assert!(workers.iter().any(|w| w.worker_id == "test-worker"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_recover_dead_workers() {
        let _guard = with_test_db().await;

        // Create a worker with old heartbeat
        let pool = get_pool().await.unwrap();
        sqlx::query(
            r#"
            INSERT INTO worker_heartbeats (worker_id, last_heartbeat, queues, status)
            VALUES ('dead-worker', NOW() - INTERVAL '1 hour', ARRAY['test'], 'running')
            ON CONFLICT (worker_id) DO UPDATE
            SET last_heartbeat = NOW() - INTERVAL '1 hour'
            "#,
        )
        .execute(pool.as_ref())
        .await
        .unwrap();

        let _recovered = recover_dead_workers(30).await.unwrap();
        // Test just verifies it doesn't error
    }
}
