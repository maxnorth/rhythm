//! Tests for execution operations

use crate::db::executions::{complete_execution, fail_execution, start_execution_unless_finished};
use crate::types::{CreateExecutionParams, ExecutionStatus, ExecutionType};
use sqlx::PgPool;

/// Helper to create test executions
async fn create_test_execution(pool: &PgPool, id: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let params = CreateExecutionParams {
        id: Some(id.to_string()),
        exec_type: ExecutionType::Task,
        target_name: "test_task".to_string(),
        queue: "default".to_string(),
        inputs: serde_json::json!({}),
        parent_workflow_id: None,
    };
    crate::db::executions::create_execution(&mut tx, params).await?;
    tx.commit().await?;
    Ok(())
}

#[sqlx::test]
async fn test_start_execution_unless_finished_starts_pending(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1").await?;

    let execution = start_execution_unless_finished(&pool, "exec1")
        .await?
        .expect("execution should exist");

    assert_eq!(execution.status, ExecutionStatus::Running);
    Ok(())
}

#[sqlx::test]
async fn test_start_execution_unless_finished_skips_completed(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1").await?;

    // Start and complete the execution
    start_execution_unless_finished(&pool, "exec1").await?;
    complete_execution(&pool, "exec1", serde_json::json!({"result": "done"})).await?;

    // Try to start again - should return the execution but not change status
    let execution = start_execution_unless_finished(&pool, "exec1")
        .await?
        .expect("execution should exist");

    assert_eq!(
        execution.status,
        ExecutionStatus::Completed,
        "should not change completed status to running"
    );
    Ok(())
}

#[sqlx::test]
async fn test_start_execution_unless_finished_skips_failed(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1").await?;

    // Start and fail the execution
    start_execution_unless_finished(&pool, "exec1").await?;
    fail_execution(&pool, "exec1", serde_json::json!({"error": "oops"})).await?;

    // Try to start again - should return the execution but not change status
    let execution = start_execution_unless_finished(&pool, "exec1")
        .await?
        .expect("execution should exist");

    assert_eq!(
        execution.status,
        ExecutionStatus::Failed,
        "should not change failed status to running"
    );
    Ok(())
}

#[sqlx::test]
async fn test_start_execution_unless_finished_returns_none_for_nonexistent(
    pool: PgPool,
) -> anyhow::Result<()> {
    let result = start_execution_unless_finished(&pool, "nonexistent").await?;
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn test_start_execution_unless_finished_starts_suspended(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1").await?;

    // Start and suspend the execution
    start_execution_unless_finished(&pool, "exec1").await?;
    crate::db::executions::suspend_execution(&pool, "exec1").await?;

    // Try to start again - suspended is NOT a terminal state, should start
    let execution = start_execution_unless_finished(&pool, "exec1")
        .await?
        .expect("execution should exist");

    assert_eq!(
        execution.status,
        ExecutionStatus::Running,
        "should change suspended status to running"
    );
    Ok(())
}
