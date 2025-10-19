use super::*;
use crate::types::*;

/// Helper function to reset the test database by truncating all tables
async fn reset_db() {
    let pool = crate::db::get_pool().await.unwrap();

    // Single query with multiple truncates - uses only one connection
    sqlx::query("TRUNCATE TABLE executions CASCADE")
        .execute(pool.as_ref())
        .await
        .unwrap();
}


#[tokio::test]
async fn test_db_pool_limit() {
    reset_db().await;
    for _ in 0..50 {
        let _ = test_create_and_claim_execution().await;
    }
}

// This is a known limitation of the global pool singleton architecture.
// #[tokio::test]
async fn test_create_and_claim_execution() {
    reset_db().await;

    let params = CreateExecutionParams {
        id: None,
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
    };

    let id = create_execution(params).await.unwrap();

    // Verify ID is a clean UUID (no prefix)
    assert_eq!(id.len(), 36); // UUID with hyphens
    assert!(id.contains('-'));

    let execution = claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(execution.id, id);
    assert_eq!(execution.status, ExecutionStatus::Running);
}

#[tokio::test]
async fn test_create_execution_with_user_provided_id() {
    reset_db().await;

    let params = CreateExecutionParams {
        id: Some("payment-order-123".to_string()),
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
    };

    let id = create_execution(params).await.unwrap();

    // Verify execution was created with user-provided ID
    assert_eq!(id, "payment-order-123");
    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.id, "payment-order-123");
    assert_eq!(execution.status, ExecutionStatus::Pending);
}

#[tokio::test]
async fn test_create_execution_without_id_generates_uuid() {
    reset_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: None,
    };

    let id = create_execution(params).await.unwrap();

    // Verify execution has auto-generated id (UUID)
    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.id, id);
    // UUIDs are 36 characters with hyphens
    assert_eq!(execution.id.len(), 36);
    assert!(execution.id.contains('-'));
}

#[tokio::test]
async fn test_duplicate_id_pending_fails() {
    reset_db().await;

    let params1 = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("payment-order-456".to_string()),
    };

    // Create first execution
    let id1 = create_execution(params1.clone()).await.unwrap();

    // Try to create duplicate (should fail with error)
    let result = create_execution(params1).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("already exists") || error_msg.contains("duplicate"));

    // Verify only one execution exists and it's still pending
    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Pending);
}

#[tokio::test]
async fn test_duplicate_id_running_fails() {
    reset_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("payment-order-789".to_string()),
    };

    // Create and claim execution (status = running)
    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();

    // Try to create duplicate (should fail with error)
    let result = create_execution(params).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("already exists") || error_msg.contains("duplicate"));

    // Verify execution is still running
    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Running);
}

#[tokio::test]
async fn test_duplicate_id_completed_fails() {
    reset_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("payment-order-999".to_string()),
    };

    // Create, claim, and complete execution
    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();
    complete_execution(&id1, serde_json::json!({"status": "success"}))
        .await
        .unwrap();

    // Try to create duplicate (should fail with error)
    let result = create_execution(params).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("already exists")
            || error_msg.contains("duplicate")
            || error_msg.contains("completed")
    );

    // Verify execution is still completed with result preserved
    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert!(execution.result.is_some());
}

#[tokio::test]
async fn test_duplicate_id_failed_allows_retry() {
    reset_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("payment-order-failed".to_string()),
    };

    // Create, claim, and fail execution
    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();
    fail_execution(&id1, serde_json::json!({"error": "Network error"}), false)
        .await
        .unwrap();

    // Verify first execution is failed
    let execution1 = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution1.status, ExecutionStatus::Failed);

    // Try to create duplicate with same ID after failure
    // In simplified design: this should still succeed because failed executions can be retried
    let id2 = create_execution(params).await.unwrap();

    // Should be same ID
    assert_eq!(id1, id2);
    assert_eq!(id2, "payment-order-failed");
}

#[tokio::test]
async fn test_different_ids_can_coexist() {
    reset_db().await;

    let params1 = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("order-aaa".to_string()),
    };

    let params2 = CreateExecutionParams {
        id: Some("order-bbb".to_string()),
        ..params1.clone()
    };

    // Create both executions
    let id1 = create_execution(params1).await.unwrap();
    let id2 = create_execution(params2).await.unwrap();

    // Should be different
    assert_ne!(id1, id2);

    // Both should exist
    let execution1 = get_execution(&id1).await.unwrap().unwrap();
    let execution2 = get_execution(&id2).await.unwrap().unwrap();

    assert_eq!(execution1.id, "order-aaa");
    assert_eq!(execution2.id, "order-bbb");
}

#[tokio::test]
async fn test_result_is_stored_on_completion() {
    reset_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
        priority: 5,
        args: serde_json::json!([]),
        kwargs: serde_json::json!({}),
        max_retries: 3,
        timeout_seconds: Some(300),
        parent_workflow_id: None,
        id: Some("result-test".to_string()),
    };

    let id = create_execution(params).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();

    let result = serde_json::json!({
        "payment_id": "pay_123",
        "amount": 99.99,
        "status": "success"
    });

    complete_execution(&id, result.clone()).await.unwrap();

    // Verify result is stored
    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.result, Some(result));
    assert_eq!(execution.status, ExecutionStatus::Completed);
}
