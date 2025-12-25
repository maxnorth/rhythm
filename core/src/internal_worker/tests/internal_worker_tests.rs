//! Tests for internal worker

use crate::internal_worker::InternalWorker;
use crate::services::SchedulerService;
use crate::types::{ExecutionType, ScheduleExecutionParams};
use chrono::{NaiveDateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Helper to create a NaiveDateTime offset from now
fn now_plus_seconds(seconds: i64) -> NaiveDateTime {
    (Utc::now() + chrono::Duration::seconds(seconds)).naive_utc()
}

/// Helper to count items in work queue
async fn count_work_queue_items(pool: &PgPool) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Helper to count items in scheduled queue
async fn count_scheduled_items(pool: &PgPool) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_queue")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

#[sqlx::test]
async fn test_internal_worker_processes_ready_items(pool: PgPool) -> anyhow::Result<()> {
    let scheduler_service = SchedulerService::new(pool.clone());
    let shutdown_token = CancellationToken::new();

    // Schedule an execution in the past (ready to run)
    let params = ScheduleExecutionParams {
        exec_type: ExecutionType::Task,
        target_name: "my_task".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        run_at: now_plus_seconds(-10),
    };
    scheduler_service.schedule_execution(params).await?;

    // Verify initial state
    assert_eq!(count_scheduled_items(&pool).await?, 1);
    assert_eq!(count_work_queue_items(&pool).await?, 0);

    // Start internal worker
    let worker = InternalWorker::new(scheduler_service, shutdown_token.clone());
    let worker_handle = tokio::spawn(worker.run());

    // Wait for worker to process (poll interval is 1s, give it 2s)
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Trigger shutdown
    shutdown_token.cancel();
    worker_handle.await?;

    // Verify item was moved to work queue
    assert_eq!(count_scheduled_items(&pool).await?, 0);
    assert_eq!(count_work_queue_items(&pool).await?, 1);

    Ok(())
}

#[sqlx::test]
async fn test_internal_worker_respects_shutdown(pool: PgPool) -> anyhow::Result<()> {
    let scheduler_service = SchedulerService::new(pool.clone());
    let shutdown_token = CancellationToken::new();

    let worker = InternalWorker::new(scheduler_service, shutdown_token.clone());
    let worker_handle = tokio::spawn(worker.run());

    // Immediately trigger shutdown
    shutdown_token.cancel();

    // Worker should exit promptly (within 100ms)
    let result = tokio::time::timeout(Duration::from_millis(500), worker_handle).await;
    assert!(result.is_ok(), "Worker should have shut down promptly");

    Ok(())
}

#[sqlx::test]
async fn test_internal_worker_skips_future_items(pool: PgPool) -> anyhow::Result<()> {
    let scheduler_service = SchedulerService::new(pool.clone());
    let shutdown_token = CancellationToken::new();

    // Schedule an execution in the future
    let params = ScheduleExecutionParams {
        exec_type: ExecutionType::Task,
        target_name: "my_task".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        run_at: now_plus_seconds(60), // 1 minute in future
    };
    scheduler_service.schedule_execution(params).await?;

    // Start internal worker
    let worker = InternalWorker::new(scheduler_service, shutdown_token.clone());
    let worker_handle = tokio::spawn(worker.run());

    // Wait for one poll cycle
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Trigger shutdown
    shutdown_token.cancel();
    worker_handle.await?;

    // Item should still be in scheduled queue (not ready yet)
    assert_eq!(count_scheduled_items(&pool).await?, 1);
    assert_eq!(count_work_queue_items(&pool).await?, 0);

    Ok(())
}
