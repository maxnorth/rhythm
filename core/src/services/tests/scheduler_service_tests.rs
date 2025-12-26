//! Tests for scheduler service operations

use crate::services::SchedulerService;
use crate::types::{ExecutionType, ScheduleExecutionParams};
use chrono::{NaiveDateTime, Utc};
use serde_json::json;
use sqlx::PgPool;

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

/// Helper to get execution status
async fn get_execution_status(pool: &PgPool, id: &str) -> anyhow::Result<String> {
    let status: String = sqlx::query_scalar("SELECT status FROM executions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(status)
}

#[sqlx::test]
async fn test_schedule_execution_creates_pending_execution(pool: PgPool) -> anyhow::Result<()> {
    let service = SchedulerService::new(pool.clone());

    let params = ScheduleExecutionParams {
        exec_type: ExecutionType::Task,
        target_name: "my_task".to_string(),
        queue: "default".to_string(),
        inputs: json!({"key": "value"}),
        run_at: now_plus_seconds(60),
    };

    let execution_id = service.schedule_execution(params).await?;

    // Execution should exist in Pending status
    let status = get_execution_status(&pool, &execution_id).await?;
    assert_eq!(status, "pending");

    // Should be in scheduled queue, not work queue
    assert_eq!(count_scheduled_items(&pool).await?, 1);
    assert_eq!(count_work_queue_items(&pool).await?, 0);

    Ok(())
}

#[sqlx::test]
async fn test_process_ready_items_moves_to_work_queue(pool: PgPool) -> anyhow::Result<()> {
    let service = SchedulerService::new(pool.clone());

    // Schedule an execution in the past (ready to run)
    let params = ScheduleExecutionParams {
        exec_type: ExecutionType::Task,
        target_name: "my_task".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        run_at: now_plus_seconds(-10), // in the past
    };

    let execution_id = service.schedule_execution(params).await?;

    // Verify it's in scheduled queue
    assert_eq!(count_scheduled_items(&pool).await?, 1);
    assert_eq!(count_work_queue_items(&pool).await?, 0);

    // Process ready items
    let processed = service.process_ready_items(10).await?;
    assert_eq!(processed, 1);

    // Should now be in work queue, not scheduled queue
    assert_eq!(count_scheduled_items(&pool).await?, 0);
    assert_eq!(count_work_queue_items(&pool).await?, 1);

    // Verify execution still in Pending status
    let status = get_execution_status(&pool, &execution_id).await?;
    assert_eq!(status, "pending");

    Ok(())
}

#[sqlx::test]
async fn test_process_ready_items_skips_future(pool: PgPool) -> anyhow::Result<()> {
    let service = SchedulerService::new(pool.clone());

    // Schedule an execution in the future
    let params = ScheduleExecutionParams {
        exec_type: ExecutionType::Task,
        target_name: "my_task".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        run_at: now_plus_seconds(60), // in the future
    };

    service.schedule_execution(params).await?;

    // Process ready items - should find nothing
    let processed = service.process_ready_items(10).await?;
    assert_eq!(processed, 0);

    // Should still be in scheduled queue
    assert_eq!(count_scheduled_items(&pool).await?, 1);
    assert_eq!(count_work_queue_items(&pool).await?, 0);

    Ok(())
}

#[sqlx::test]
async fn test_process_ready_items_respects_limit(pool: PgPool) -> anyhow::Result<()> {
    let service = SchedulerService::new(pool.clone());

    // Schedule 5 executions in the past
    for i in 0..5 {
        let params = ScheduleExecutionParams {
            exec_type: ExecutionType::Task,
            target_name: format!("task_{}", i),
            queue: "default".to_string(),
            inputs: json!({}),
            run_at: now_plus_seconds(-10 - i as i64),
        };
        service.schedule_execution(params).await?;
    }

    // Process with limit of 2
    let processed = service.process_ready_items(2).await?;
    assert_eq!(processed, 2);

    // Should have 3 in scheduled queue, 2 in work queue
    assert_eq!(count_scheduled_items(&pool).await?, 3);
    assert_eq!(count_work_queue_items(&pool).await?, 2);

    Ok(())
}

#[sqlx::test]
async fn test_process_ready_items_handles_empty_queue(pool: PgPool) -> anyhow::Result<()> {
    let service = SchedulerService::new(pool.clone());

    // Process with no items scheduled
    let processed = service.process_ready_items(10).await?;
    assert_eq!(processed, 0);

    Ok(())
}
