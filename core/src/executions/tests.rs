use super::*;
use crate::db::test_helpers::*;
use crate::types::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_db_pool_limit() {
    let _guard = with_test_db().await;

    for _ in 0..50 {
        let params = CreateExecutionParams {
            id: None,
            exec_type: ExecutionType::Task,
            function_name: "test.task".to_string(),
            queue: "test".to_string(),
            inputs: serde_json::json!({}),
            max_retries: 3,
            parent_workflow_id: None,
        };

        let _ = create_execution(params).await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_execution_with_user_provided_id() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        id: Some("payment-order-123".to_string()),
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
    };

    let id = create_execution(params).await.unwrap();

    assert_eq!(id, "payment-order-123");
    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.id, "payment-order-123");
    assert_eq!(execution.status, ExecutionStatus::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_execution_without_id_generates_uuid() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: None,
    };

    let id = create_execution(params).await.unwrap();

    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.id, id);
    assert_eq!(execution.id.len(), 36);
    assert!(execution.id.contains('-'));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_id_pending_fails() {
    let _guard = with_test_db().await;

    let params1 = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: Some("payment-order-456".to_string()),
    };

    let id1 = create_execution(params1.clone()).await.unwrap();

    let result = create_execution(params1).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("already exists") || error_msg.contains("duplicate"));

    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_id_running_fails() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: Some("payment-order-789".to_string()),
    };

    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();

    let result = create_execution(params).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("already exists") || error_msg.contains("duplicate"));

    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_id_completed_fails() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: Some("payment-order-999".to_string()),
    };

    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();
    complete_execution(&id1, serde_json::json!({"status": "success"}))
        .await
        .unwrap();

    let result = create_execution(params).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("already exists")
            || error_msg.contains("duplicate")
            || error_msg.contains("completed")
    );

    let execution = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert!(execution.output.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_id_failed_allows_retry() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: Some("payment-order-failed".to_string()),
    };

    let id1 = create_execution(params.clone()).await.unwrap();
    claim_execution("test-worker", &["test".to_string()])
        .await
        .unwrap();
    fail_execution(&id1, serde_json::json!({"error": "Network error"}), false)
        .await
        .unwrap();

    let execution1 = get_execution(&id1).await.unwrap().unwrap();
    assert_eq!(execution1.status, ExecutionStatus::Failed);

    let id2 = create_execution(params).await.unwrap();

    assert_eq!(id1, id2);
    assert_eq!(id2, "payment-order-failed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_different_ids_can_coexist() {
    let _guard = with_test_db().await;

    let params1 = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
        parent_workflow_id: None,
        id: Some("order-aaa".to_string()),
    };

    let params2 = CreateExecutionParams {
        id: Some("order-bbb".to_string()),
        ..params1.clone()
    };

    let id1 = create_execution(params1).await.unwrap();
    let id2 = create_execution(params2).await.unwrap();

    assert_ne!(id1, id2);

    let execution1 = get_execution(&id1).await.unwrap().unwrap();
    let execution2 = get_execution(&id2).await.unwrap().unwrap();

    assert_eq!(execution1.id, "order-aaa");
    assert_eq!(execution2.id, "order-bbb");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_result_is_stored_on_completion() {
    let _guard = with_test_db().await;

    let params = CreateExecutionParams {
        exec_type: ExecutionType::Task,
        function_name: "test.task".to_string(),
        queue: "test".to_string(),
            inputs: serde_json::json!({}),
        max_retries: 3,
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

    let execution = get_execution(&id).await.unwrap().unwrap();
    assert_eq!(execution.output, Some(result));
    assert_eq!(execution.status, ExecutionStatus::Completed);
}
