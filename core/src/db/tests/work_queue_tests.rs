//! Tests for work queue operations
//!
//! These tests verify critical work queue behavior, especially around claim_work
//! which had a bug where it would claim multiple items despite LIMIT=1.

use crate::db::{claim_work, complete_work, enqueue_work};
use crate::types::{CreateExecutionParams, ExecutionType};
use sqlx::PgPool;

/// Helper to create test executions
async fn create_test_execution(pool: &PgPool, id: &str, queue: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let params = CreateExecutionParams {
        id: Some(id.to_string()),
        exec_type: ExecutionType::Task,
        target_name: "test_task".to_string(),
        queue: queue.to_string(),
        inputs: serde_json::json!({}),
        parent_workflow_id: None,
    };
    crate::db::executions::create_execution(&mut tx, params).await?;
    tx.commit().await?;
    Ok(())
}

/// Helper to count unclaimed work items in queue
async fn count_unclaimed(pool: &PgPool, queue: &str) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue WHERE queue = $1 AND claimed_until IS NULL",
    )
    .bind(queue)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Helper to count claimed work items in queue
async fn count_claimed(pool: &PgPool, queue: &str) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue WHERE queue = $1 AND claimed_until IS NOT NULL AND claimed_until > NOW()"
    )
    .bind(queue)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

#[sqlx::test]
async fn test_claim_work_respects_limit(pool: PgPool) -> anyhow::Result<()> {
    // This is the critical test that would have caught the bug where
    // claim_work was claiming multiple items despite LIMIT=1

    // Create 3 test executions and enqueue them
    create_test_execution(&pool, "exec1", "default").await?;
    create_test_execution(&pool, "exec2", "default").await?;
    create_test_execution(&pool, "exec3", "default").await?;

    enqueue_work(&pool, "exec1", "default", 0).await?;
    enqueue_work(&pool, "exec2", "default", 0).await?;
    enqueue_work(&pool, "exec3", "default", 0).await?;

    // Verify we have 3 unclaimed items
    assert_eq!(count_unclaimed(&pool, "default").await?, 3);

    // Claim with LIMIT 1
    let claimed = claim_work(&pool, "default", 1).await?;

    // CRITICAL: Should only claim 1 item
    assert_eq!(claimed.len(), 1, "claim_work should respect LIMIT=1");
    assert_eq!(count_claimed(&pool, "default").await?, 1);
    assert_eq!(count_unclaimed(&pool, "default").await?, 2);

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_with_multiple_limit(pool: PgPool) -> anyhow::Result<()> {
    // Create 5 test executions
    for i in 1..=5 {
        let id = format!("exec{}", i);
        create_test_execution(&pool, &id, "default").await?;
        enqueue_work(&pool, &id, "default", 0).await?;
    }

    // Claim with LIMIT 3
    let claimed = claim_work(&pool, "default", 3).await?;

    assert_eq!(claimed.len(), 3);
    assert_eq!(count_claimed(&pool, "default").await?, 3);
    assert_eq!(count_unclaimed(&pool, "default").await?, 2);

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_respects_priority(pool: PgPool) -> anyhow::Result<()> {
    // Create executions with different priorities
    create_test_execution(&pool, "low", "default").await?;
    create_test_execution(&pool, "high", "default").await?;
    create_test_execution(&pool, "medium", "default").await?;

    enqueue_work(&pool, "low", "default", 0).await?; // priority 0
    enqueue_work(&pool, "high", "default", 100).await?; // priority 100
    enqueue_work(&pool, "medium", "default", 50).await?; // priority 50

    // Claim 1 - should get highest priority
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0], "high");

    // Claim 1 more - should get medium priority
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0], "medium");

    // Claim 1 more - should get low priority
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0], "low");

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_skips_already_claimed(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1", "default").await?;
    create_test_execution(&pool, "exec2", "default").await?;

    enqueue_work(&pool, "exec1", "default", 0).await?;
    enqueue_work(&pool, "exec2", "default", 0).await?;

    // First claim
    let claimed1 = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed1.len(), 1);

    // Second claim should get the other execution
    let claimed2 = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed2.len(), 1);
    assert_ne!(claimed1[0], claimed2[0]);

    // Third claim should get nothing
    let claimed3 = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed3.len(), 0);

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_reclaims_expired(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1", "default").await?;
    enqueue_work(&pool, "exec1", "default", 0).await?;

    // Claim the work
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);

    // Manually expire the claim by setting claimed_until to the past
    sqlx::query(
        "UPDATE work_queue SET claimed_until = NOW() - INTERVAL '1 minute' WHERE execution_id = $1",
    )
    .bind("exec1")
    .execute(&pool)
    .await?;

    // Should be able to claim again
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0], "exec1");

    Ok(())
}

#[sqlx::test]
async fn test_complete_work(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1", "default").await?;
    enqueue_work(&pool, "exec1", "default", 0).await?;

    // Claim work
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(count_claimed(&pool, "default").await?, 1);

    // Complete work
    complete_work(&pool, "exec1").await?;

    // Should be removed from work queue
    assert_eq!(count_claimed(&pool, "default").await?, 0);
    assert_eq!(count_unclaimed(&pool, "default").await?, 0);

    Ok(())
}

#[sqlx::test]
async fn test_enqueue_work_is_idempotent(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1", "default").await?;

    // Enqueue same work multiple times
    enqueue_work(&pool, "exec1", "default", 0).await?;
    enqueue_work(&pool, "exec1", "default", 0).await?;
    enqueue_work(&pool, "exec1", "default", 0).await?;

    // Should only have 1 unclaimed entry
    assert_eq!(count_unclaimed(&pool, "default").await?, 1);

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_respects_queue(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "exec1", "queue1").await?;
    create_test_execution(&pool, "exec2", "queue2").await?;

    enqueue_work(&pool, "exec1", "queue1", 0).await?;
    enqueue_work(&pool, "exec2", "queue2", 0).await?;

    // Claim from queue1
    let claimed = claim_work(&pool, "queue1", 1).await?;
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0], "exec1");

    // queue2 should still have unclaimed work
    assert_eq!(count_unclaimed(&pool, "queue2").await?, 1);

    Ok(())
}

#[sqlx::test]
async fn test_claim_work_prevents_claiming_execution_with_active_claim(
    pool: PgPool,
) -> anyhow::Result<()> {
    // This tests the NOT EXISTS clause that prevents claiming an execution
    // if it already has an active claim in the work queue

    create_test_execution(&pool, "exec1", "default").await?;

    // Enqueue and claim the work
    enqueue_work(&pool, "exec1", "default", 0).await?;
    let claimed = claim_work(&pool, "default", 1).await?;
    assert_eq!(claimed.len(), 1);

    // Now enqueue the same execution again (this can happen if work is re-queued
    // while still being processed)
    enqueue_work(&pool, "exec1", "default", 0).await?;

    // We should have 1 claimed and 1 unclaimed entry for the same execution
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE execution_id = $1")
        .bind("exec1")
        .fetch_one(&pool)
        .await?;
    assert_eq!(total, 2);

    // Trying to claim more work should NOT claim the unclaimed entry
    // because the execution already has an active claim
    let claimed2 = claim_work(&pool, "default", 10).await?;
    assert_eq!(
        claimed2.len(),
        0,
        "Should not claim execution with active claim"
    );

    Ok(())
}
