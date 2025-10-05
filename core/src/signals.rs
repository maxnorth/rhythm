use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::Row;
use uuid::Uuid;

use crate::db::get_pool;
use crate::types::WorkflowSignal;

/// Send a signal to a workflow
pub async fn send_signal(
    workflow_id: &str,
    signal_name: &str,
    payload: JsonValue,
) -> Result<String> {
    let pool = get_pool().await?;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO workflow_signals (id, workflow_id, signal_name, payload)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(&id)
    .bind(workflow_id)
    .bind(signal_name)
    .bind(payload)
    .execute(pool.as_ref())
    .await
    .context("Failed to send signal")?;

    // Resume the workflow if it's suspended
    sqlx::query(
        r#"
        UPDATE executions
        SET status = 'pending',
            worker_id = NULL,
            claimed_at = NULL
        WHERE id = $1
          AND status = 'suspended'
        "#,
    )
    .bind(workflow_id)
    .execute(pool.as_ref())
    .await
    .ok(); // Don't fail if workflow not suspended

    Ok(id)
}

/// Get unconsumed signals for a workflow
pub async fn get_signals(workflow_id: &str, signal_name: &str) -> Result<Vec<WorkflowSignal>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT * FROM workflow_signals
        WHERE workflow_id = $1
          AND signal_name = $2
          AND consumed = FALSE
        ORDER BY created_at ASC
        "#,
    )
    .bind(workflow_id)
    .bind(signal_name)
    .fetch_all(pool.as_ref())
    .await
    .context("Failed to get signals")?;

    let mut signals = Vec::new();
    for row in rows {
        signals.push(WorkflowSignal {
            id: row.get("id"),
            workflow_id: row.get("workflow_id"),
            signal_name: row.get("signal_name"),
            payload: row.get("payload"),
            created_at: row.get("created_at"),
            consumed: row.get("consumed"),
        });
    }

    Ok(signals)
}

/// Mark a signal as consumed
pub async fn consume_signal(signal_id: &str) -> Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE workflow_signals
        SET consumed = TRUE
        WHERE id = $1
        "#,
    )
    .bind(signal_id)
    .execute(pool.as_ref())
    .await
    .context("Failed to consume signal")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executions::{create_execution, CreateExecutionParams};
    use crate::types::ExecutionType;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_send_and_get_signal() {
        // Create a workflow first
        let params = CreateExecutionParams {
            exec_type: ExecutionType::Workflow,
            function_name: "test.workflow".to_string(),
            queue: "test".to_string(),
            priority: 5,
            args: serde_json::json!([]),
            kwargs: serde_json::json!({}),
            max_retries: 3,
            timeout_seconds: Some(300),
            parent_workflow_id: None,
        };
        let workflow_id = create_execution(params).await.unwrap();

        // Send a signal
        let payload = serde_json::json!({"approved": true});
        let signal_id = send_signal(&workflow_id, "approval", payload.clone())
            .await
            .unwrap();

        // Get signals
        let signals = get_signals(&workflow_id, "approval").await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].payload, payload);

        // Consume signal
        consume_signal(&signal_id).await.unwrap();

        // Verify consumed
        let signals = get_signals(&workflow_id, "approval").await.unwrap();
        assert_eq!(signals.len(), 0);
    }
}
