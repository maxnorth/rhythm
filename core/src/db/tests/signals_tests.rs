//! Tests for signal database operations

use crate::db::signals::{
    check_signal_claimed, claim_signal, get_signal_payload, get_unclaimed_signals_by_name,
    insert_signal_request, send_signal,
};
use crate::types::{CreateExecutionParams, ExecutionType};
use crate::worker::signals::resolve_signal_claims;
use serde_json::json;
use sqlx::PgPool;

/// Helper to create a test execution (required for foreign key constraint)
async fn create_test_execution(pool: &PgPool, id: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let params = CreateExecutionParams {
        id: Some(id.to_string()),
        exec_type: ExecutionType::Workflow,
        target_name: "test_workflow".to_string(),
        queue: "default".to_string(),
        inputs: json!({}),
        parent_workflow_id: None,
    };
    crate::db::executions::create_execution(&mut tx, params).await?;
    tx.commit().await?;
    Ok(())
}

/// Helper to count signals for a workflow
async fn count_signals(pool: &PgPool, workflow_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE workflow_id = $1")
        .bind(workflow_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Helper to count signals by status
async fn count_signals_by_status(pool: &PgPool, workflow_id: &str, status: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM signals WHERE workflow_id = $1 AND status = $2")
        .bind(workflow_id)
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap()
}

/* ===================== insert_signal_request Tests ===================== */

#[sqlx::test]
async fn test_insert_signal_request_creates_requested_row(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-123").await?;

    assert_eq!(count_signals(&pool, "wf-1").await, 1);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 1);

    // Verify the claim_id is set
    let claim_id: Option<String> =
        sqlx::query_scalar("SELECT claim_id FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;
    assert_eq!(claim_id, Some("claim-123".to_string()));

    Ok(())
}

#[sqlx::test]
async fn test_insert_multiple_signal_requests(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-2").await?;
    insert_signal_request(&pool, "wf-1", "other", "claim-3").await?;

    assert_eq!(count_signals(&pool, "wf-1").await, 3);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 3);

    Ok(())
}

/* ===================== send_signal Tests ===================== */

#[sqlx::test]
async fn test_send_signal_creates_sent_row(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    send_signal(&pool, "wf-1", "approval", &json!({"approved": true})).await?;

    assert_eq!(count_signals(&pool, "wf-1").await, 1);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "sent").await, 1);

    // Verify claim_id is NULL (unclaimed)
    let claim_id: Option<String> =
        sqlx::query_scalar("SELECT claim_id FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;
    assert_eq!(claim_id, None);

    Ok(())
}

#[sqlx::test]
async fn test_send_signal_stores_payload(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    let payload = json!({"approved": true, "reviewer": "alice"});
    send_signal(&pool, "wf-1", "approval", &payload).await?;

    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT payload FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored, payload);

    Ok(())
}

/* ===================== claim_signal Tests ===================== */

#[sqlx::test]
async fn test_claim_signal_sets_claim_id(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Send a signal (creates unclaimed 'sent' row)
    send_signal(&pool, "wf-1", "approval", &json!({"data": "test"})).await?;

    // Get the signal ID
    let signal_id: String =
        sqlx::query_scalar("SELECT id::text FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;

    // Claim it
    claim_signal(&pool, &signal_id, "claim-abc").await?;

    // Verify claim_id is set
    let claim_id: Option<String> =
        sqlx::query_scalar("SELECT claim_id FROM signals WHERE id = $1::uuid")
            .bind(&signal_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(claim_id, Some("claim-abc".to_string()));

    Ok(())
}

/* ===================== check_signal_claimed Tests ===================== */

#[sqlx::test]
async fn test_check_signal_claimed_returns_payload_when_claimed(
    pool: PgPool,
) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    let payload = json!({"approved": true, "reviewer": "bob"});

    // Send a signal and claim it
    send_signal(&pool, "wf-1", "approval", &payload).await?;
    let signal_id: String =
        sqlx::query_scalar("SELECT id::text FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;
    claim_signal(&pool, &signal_id, "claim-xyz").await?;

    // Check - should return the payload
    let result = check_signal_claimed(&pool, "claim-xyz").await?;
    assert_eq!(result, Some(payload));

    Ok(())
}

#[sqlx::test]
async fn test_check_signal_claimed_returns_none_when_unclaimed(pool: PgPool) -> anyhow::Result<()> {
    // No signals exist
    let result = check_signal_claimed(&pool, "nonexistent-claim").await?;
    assert_eq!(result, None);

    Ok(())
}

#[sqlx::test]
async fn test_check_signal_claimed_ignores_requested_status(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Insert a 'requested' signal with this claim_id
    insert_signal_request(&pool, "wf-1", "approval", "claim-123").await?;

    // Check - should return None because it's 'requested' not 'sent'
    let result = check_signal_claimed(&pool, "claim-123").await?;
    assert_eq!(result, None);

    Ok(())
}

/* ===================== get_unclaimed_signals_by_name Tests ===================== */

#[sqlx::test]
async fn test_get_unclaimed_signals_by_name_returns_fifo_order(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Send multiple signals - they should be returned in FIFO order
    send_signal(&pool, "wf-1", "approval", &json!({"order": 1})).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    send_signal(&pool, "wf-1", "approval", &json!({"order": 2})).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    send_signal(&pool, "wf-1", "approval", &json!({"order": 3})).await?;

    let ids = get_unclaimed_signals_by_name(&pool, "wf-1", "approval", 10).await?;
    assert_eq!(ids.len(), 3);

    // Verify order by checking payloads
    for (i, signal_id) in ids.iter().enumerate() {
        let payload = get_signal_payload(&pool, signal_id).await?;
        assert_eq!(payload["order"], json!(i + 1));
    }

    Ok(())
}

#[sqlx::test]
async fn test_get_unclaimed_signals_by_name_excludes_claimed(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Send two signals
    send_signal(&pool, "wf-1", "approval", &json!({"id": 1})).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    send_signal(&pool, "wf-1", "approval", &json!({"id": 2})).await?;

    // Claim the first one
    let all_ids = get_unclaimed_signals_by_name(&pool, "wf-1", "approval", 10).await?;
    claim_signal(&pool, &all_ids[0], "some-claim").await?;

    // Now should only return the second one
    let ids = get_unclaimed_signals_by_name(&pool, "wf-1", "approval", 10).await?;
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], all_ids[1]);

    Ok(())
}

#[sqlx::test]
async fn test_get_unclaimed_signals_by_name_respects_limit(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Send 5 signals
    for i in 0..5 {
        send_signal(&pool, "wf-1", "approval", &json!({"order": i})).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    let ids = get_unclaimed_signals_by_name(&pool, "wf-1", "approval", 3).await?;
    assert_eq!(ids.len(), 3);

    Ok(())
}

#[sqlx::test]
async fn test_get_unclaimed_signals_by_name_filters_by_name(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    send_signal(&pool, "wf-1", "approval", &json!({})).await?;
    send_signal(&pool, "wf-1", "other_channel", &json!({})).await?;
    send_signal(&pool, "wf-1", "approval", &json!({})).await?;

    let approval_ids = get_unclaimed_signals_by_name(&pool, "wf-1", "approval", 10).await?;
    assert_eq!(approval_ids.len(), 2);

    let other_ids = get_unclaimed_signals_by_name(&pool, "wf-1", "other_channel", 10).await?;
    assert_eq!(other_ids.len(), 1);

    Ok(())
}

/* ===================== get_signal_payload Tests ===================== */

#[sqlx::test]
async fn test_get_signal_payload_returns_payload(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    let expected = json!({"key": "value", "nested": {"a": 1}});
    send_signal(&pool, "wf-1", "test", &expected).await?;

    let signal_id: String =
        sqlx::query_scalar("SELECT id::text FROM signals WHERE workflow_id = $1")
            .bind("wf-1")
            .fetch_one(&pool)
            .await?;

    let payload = get_signal_payload(&pool, &signal_id).await?;
    assert_eq!(payload, expected);

    Ok(())
}

/* ===================== resolve_signal_claims Tests ===================== */

#[sqlx::test]
async fn test_resolve_signal_claims_matches_request_to_signal(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Create a 'requested' signal (workflow waiting)
    insert_signal_request(&pool, "wf-1", "approval", "claim-abc").await?;

    // Create a 'sent' signal (external signal arrived)
    send_signal(&pool, "wf-1", "approval", &json!({"approved": true})).await?;

    // Before resolution: 1 requested, 1 sent (unclaimed)
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 1);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "sent").await, 1);

    // Resolve claims
    let resolved = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved, 1);

    // After resolution: 0 requested, 1 sent (claimed)
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 0);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "sent").await, 1);

    // Verify the signal is now claimed with the correct claim_id
    let result = check_signal_claimed(&pool, "claim-abc").await?;
    assert_eq!(result, Some(json!({"approved": true})));

    Ok(())
}

#[sqlx::test]
async fn test_resolve_signal_claims_respects_fifo_order(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Create two requests
    insert_signal_request(&pool, "wf-1", "approval", "claim-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-2").await?;

    // Send two signals (oldest should go to first request)
    send_signal(&pool, "wf-1", "approval", &json!({"order": "first"})).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    send_signal(&pool, "wf-1", "approval", &json!({"order": "second"})).await?;

    // Resolve
    let resolved = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved, 2);

    // Verify FIFO: first signal goes to first request
    let result1 = check_signal_claimed(&pool, "claim-1").await?;
    assert_eq!(result1, Some(json!({"order": "first"})));

    let result2 = check_signal_claimed(&pool, "claim-2").await?;
    assert_eq!(result2, Some(json!({"order": "second"})));

    Ok(())
}

#[sqlx::test]
async fn test_resolve_signal_claims_is_idempotent(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-123").await?;
    send_signal(&pool, "wf-1", "approval", &json!({"data": "test"})).await?;

    // First resolution
    let resolved1 = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved1, 1);

    // Second resolution - should resolve 0 (already done)
    let resolved2 = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved2, 0);

    // Verify still correct
    let result = check_signal_claimed(&pool, "claim-123").await?;
    assert!(result.is_some());

    Ok(())
}

#[sqlx::test]
async fn test_resolve_signal_claims_handles_no_matches(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Only a request, no signal
    insert_signal_request(&pool, "wf-1", "approval", "claim-123").await?;

    let resolved = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved, 0);

    // Request should still exist
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 1);

    Ok(())
}

#[sqlx::test]
async fn test_resolve_signal_claims_different_channels_no_match(
    pool: PgPool,
) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Request on one channel, signal on another
    insert_signal_request(&pool, "wf-1", "approval", "claim-123").await?;
    send_signal(&pool, "wf-1", "rejection", &json!({})).await?;

    let resolved = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved, 0);

    // Both should still exist unmatched
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 1);
    assert_eq!(count_signals_by_status(&pool, "wf-1", "sent").await, 1);

    Ok(())
}

#[sqlx::test]
async fn test_resolve_signal_claims_partial_match(pool: PgPool) -> anyhow::Result<()> {
    create_test_execution(&pool, "wf-1").await?;
    // Two requests, one signal
    insert_signal_request(&pool, "wf-1", "approval", "claim-1").await?;
    insert_signal_request(&pool, "wf-1", "approval", "claim-2").await?;
    send_signal(&pool, "wf-1", "approval", &json!({})).await?;

    let resolved = resolve_signal_claims(&pool, "wf-1").await?;
    assert_eq!(resolved, 1);

    // One request matched, one still pending
    assert_eq!(count_signals_by_status(&pool, "wf-1", "requested").await, 1);

    Ok(())
}
