//! Tests for work claiming logic
//!
//! These tests verify that the cooperative worker loop correctly handles
//! edge cases like stale timer continuations.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::super::{run_cooperative_worker_loop, DelegatedAction};
use crate::db;
use crate::test_helpers::with_test_db;
use crate::types::{CreateExecutionParams, ExecutionStatus, ExecutionType};

#[tokio::test(flavor = "multi_thread")]
async fn test_stale_workflow_continuation_is_skipped() {
    // This tests the bug fix where a stale timer continuation would cause
    // a completed workflow to restart from scratch.

    let pool = with_test_db().await;
    let shutdown_token = CancellationToken::new();

    let workflow_source = r#"
        return 42
    "#;

    // Create workflow definition
    db::workflow_definitions::create_workflow_definition(
        &pool,
        "stale_continuation_test",
        "test-hash",
        workflow_source,
    )
    .await
    .unwrap();

    // Create execution and enqueue work (but don't pre-claim like setup_workflow_test does)
    let params = CreateExecutionParams {
        id: None,
        exec_type: ExecutionType::Workflow,
        target_name: "stale_continuation_test".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        parent_workflow_id: None,
    };

    let mut tx = pool.begin().await.unwrap();
    let workflow_id = db::executions::create_execution(&mut tx, params)
        .await
        .unwrap();
    db::work_queue::enqueue_work(&mut *tx, &workflow_id, "default", 0)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Run the cooperative worker loop - it should claim and complete the workflow
    let action = run_cooperative_worker_loop(&pool, &shutdown_token)
        .await
        .unwrap();
    assert!(matches!(action, DelegatedAction::Continue));

    // Verify workflow completed
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
    let original_completed_at = execution.completed_at;

    // Simulate a stale timer continuation by enqueueing work for the completed workflow
    db::work_queue::enqueue_work(&*pool, &workflow_id, "default", 0)
        .await
        .unwrap();

    // Run the worker loop again - should skip the stale continuation
    let action = run_cooperative_worker_loop(&pool, &shutdown_token)
        .await
        .unwrap();
    assert!(
        matches!(action, DelegatedAction::Continue),
        "Should return Continue after skipping stale continuation"
    );

    // Verify workflow is still completed (not restarted)
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        execution.status,
        ExecutionStatus::Completed,
        "Workflow should still be completed, not restarted"
    );
    assert_eq!(
        execution.completed_at, original_completed_at,
        "completed_at should be unchanged"
    );
    assert_eq!(
        execution.output,
        Some(json!(42.0)),
        "Workflow output should be preserved"
    );

    // Verify work queue is now empty
    let work_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE execution_id = $1")
            .bind(&workflow_id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(work_count, 0, "Work queue should be empty after processing");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_stale_continuation_for_failed_workflow_is_skipped() {
    // Test that stale continuations are skipped for failed workflows.

    let pool = with_test_db().await;
    let shutdown_token = CancellationToken::new();

    let workflow_source = r#"
        return undefined_variable
    "#;

    // Create workflow definition
    db::workflow_definitions::create_workflow_definition(
        &pool,
        "failed_continuation_test",
        "test-hash",
        workflow_source,
    )
    .await
    .unwrap();

    // Create execution and enqueue work
    let params = CreateExecutionParams {
        id: None,
        exec_type: ExecutionType::Workflow,
        target_name: "failed_continuation_test".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        parent_workflow_id: None,
    };

    let mut tx = pool.begin().await.unwrap();
    let workflow_id = db::executions::create_execution(&mut tx, params)
        .await
        .unwrap();
    db::work_queue::enqueue_work(&mut *tx, &workflow_id, "default", 0)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Run the workflow - it should fail
    let action = run_cooperative_worker_loop(&pool, &shutdown_token)
        .await
        .unwrap();
    assert!(matches!(action, DelegatedAction::Continue));

    // Verify workflow failed
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, ExecutionStatus::Failed);
    let original_completed_at = execution.completed_at;

    // Simulate a stale timer continuation
    db::work_queue::enqueue_work(&*pool, &workflow_id, "default", 0)
        .await
        .unwrap();

    // Run the worker loop again - should skip the stale continuation
    let action = run_cooperative_worker_loop(&pool, &shutdown_token)
        .await
        .unwrap();
    assert!(matches!(action, DelegatedAction::Continue));

    // Verify workflow is still failed (not restarted)
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        execution.status,
        ExecutionStatus::Failed,
        "Workflow should still be failed"
    );
    assert_eq!(
        execution.completed_at, original_completed_at,
        "completed_at should be unchanged"
    );

    // Verify work queue is now empty
    let work_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE execution_id = $1")
            .bind(&workflow_id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(work_count, 0, "Work queue should be empty after processing");
}
