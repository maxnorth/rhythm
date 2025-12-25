//! Tests for scheduled queue operations

use crate::db::scheduled_queue::{claim_ready_items, delete_items, schedule_item};
use chrono::{NaiveDateTime, Utc};
use serde_json::json;
use sqlx::PgPool;

/// Helper to get the count of items in the scheduled queue
async fn count_scheduled_items(pool: &PgPool) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_queue")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Helper to create a NaiveDateTime offset from now
fn now_plus_seconds(seconds: i64) -> NaiveDateTime {
    (Utc::now() + chrono::Duration::seconds(seconds)).naive_utc()
}

#[sqlx::test]
async fn test_schedule_item_creates_entry(pool: PgPool) -> anyhow::Result<()> {
    let run_at = now_plus_seconds(60);
    let params = json!({"type": "workflow_continuation", "execution_id": "exec1", "queue": "default", "priority": 0});

    let id = schedule_item(&pool, run_at, &params).await?;

    assert!(!id.is_nil());
    assert_eq!(count_scheduled_items(&pool).await?, 1);

    Ok(())
}

#[sqlx::test]
async fn test_claim_ready_items_returns_past_items(pool: PgPool) -> anyhow::Result<()> {
    // Schedule an item in the past
    let run_at = now_plus_seconds(-10);
    let params = json!({"type": "workflow_continuation", "execution_id": "exec1", "queue": "default", "priority": 0});
    schedule_item(&pool, run_at, &params).await?;

    let mut tx = pool.begin().await?;
    let items = claim_ready_items(&mut tx, 10).await?;
    tx.commit().await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].params, params);

    Ok(())
}

#[sqlx::test]
async fn test_claim_ready_items_skips_future_items(pool: PgPool) -> anyhow::Result<()> {
    // Schedule an item in the future
    let run_at = now_plus_seconds(60);
    let params = json!({"type": "workflow_continuation", "execution_id": "exec1", "queue": "default", "priority": 0});
    schedule_item(&pool, run_at, &params).await?;

    let mut tx = pool.begin().await?;
    let items = claim_ready_items(&mut tx, 10).await?;
    tx.commit().await?;

    assert_eq!(items.len(), 0);

    Ok(())
}

#[sqlx::test]
async fn test_claim_ready_items_respects_limit(pool: PgPool) -> anyhow::Result<()> {
    // Schedule 5 items in the past
    for i in 0..5 {
        let run_at = now_plus_seconds(-10 - i);
        let params = json!({"type": "workflow_continuation", "execution_id": format!("exec{}", i), "queue": "default", "priority": 0});
        schedule_item(&pool, run_at, &params).await?;
    }

    let mut tx = pool.begin().await?;
    let items = claim_ready_items(&mut tx, 3).await?;
    tx.commit().await?;

    assert_eq!(items.len(), 3);

    Ok(())
}

#[sqlx::test]
async fn test_claim_ready_items_orders_by_run_at(pool: PgPool) -> anyhow::Result<()> {
    // Schedule items with different run_at times (all in past)
    let params1 = json!({"execution_id": "exec1"});
    let params2 = json!({"execution_id": "exec2"});
    let params3 = json!({"execution_id": "exec3"});

    // Schedule out of order
    schedule_item(&pool, now_plus_seconds(-5), &params2).await?;
    schedule_item(&pool, now_plus_seconds(-10), &params1).await?; // earliest
    schedule_item(&pool, now_plus_seconds(-1), &params3).await?;

    let mut tx = pool.begin().await?;
    let items = claim_ready_items(&mut tx, 10).await?;
    tx.commit().await?;

    assert_eq!(items.len(), 3);
    // Should be ordered by run_at ascending (earliest first)
    assert_eq!(items[0].params["execution_id"], "exec1");
    assert_eq!(items[1].params["execution_id"], "exec2");
    assert_eq!(items[2].params["execution_id"], "exec3");

    Ok(())
}

#[sqlx::test]
async fn test_delete_items_removes_entries(pool: PgPool) -> anyhow::Result<()> {
    let run_at = now_plus_seconds(-10);
    let params = json!({"execution_id": "exec1"});

    let id = schedule_item(&pool, run_at, &params).await?;
    assert_eq!(count_scheduled_items(&pool).await?, 1);

    let mut tx = pool.begin().await?;
    let deleted = delete_items(&mut tx, &[id]).await?;
    tx.commit().await?;

    assert_eq!(deleted, 1);
    assert_eq!(count_scheduled_items(&pool).await?, 0);

    Ok(())
}

#[sqlx::test]
async fn test_delete_items_with_empty_list(pool: PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let deleted = delete_items(&mut tx, &[]).await?;
    tx.commit().await?;

    assert_eq!(deleted, 0);

    Ok(())
}
