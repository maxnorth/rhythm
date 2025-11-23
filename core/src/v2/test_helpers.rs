//! Test helpers for V2 workflow engine tests

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::ops::Deref;

use crate::v2::client_adapter::ClientAdapter;
use crate::v2::db;

/// Test database pool that automatically cleans up on drop
pub struct TestPool {
    pool: PgPool,
}

impl Deref for TestPool {
    type Target = PgPool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

impl AsRef<PgPool> for TestPool {
    fn as_ref(&self) -> &PgPool {
        &self.pool
    }
}

impl Drop for TestPool {
    fn drop(&mut self) {
        // Clean up tables when test completes
        let pool = self.pool.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let _ = sqlx::query(
                        "TRUNCATE TABLE executions, workflow_definitions, workflow_execution_context, work_queue CASCADE"
                    )
                    .execute(&pool)
                    .await;
                });
            });
        }));
    }
}

/// Initialize test database and return a pool
///
/// The returned pool will automatically clean up when dropped.
/// Each test gets its own independent pool.
pub async fn with_test_db() -> TestPool {
    let pool = db::create_pool_with_max_connections(1)
        .await
        .expect("Failed to create test pool");

    // Clean up any leftover data from previous runs
    sqlx::query(
        "TRUNCATE TABLE executions, workflow_definitions, workflow_execution_context, work_queue CASCADE"
    )
    .execute(&pool)
    .await
    .expect("Failed to clean up test database");

    TestPool { pool }
}

/// Helper to set up a workflow test
///
/// Creates workflow, submits execution, and claims work.
/// Returns (pool, adapter, execution_id) for use in tests.
pub async fn setup_workflow_test(
    workflow_name: &str,
    workflow_source: &str,
    inputs: JsonValue,
) -> (TestPool, ClientAdapter, String) {
    setup_workflow_test_with_pool(None, workflow_name, workflow_source, inputs).await
}

/// Helper to set up a workflow test with an optional existing pool
///
/// If pool is provided, reuses it. Otherwise creates a new one.
/// Returns (pool, adapter, execution_id) for use in tests.
pub async fn setup_workflow_test_with_pool(
    pool: Option<TestPool>,
    workflow_name: &str,
    workflow_source: &str,
    inputs: JsonValue,
) -> (TestPool, ClientAdapter, String) {
    let pool = pool.unwrap_or_else(|| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                with_test_db().await
            })
        })
    });

    let adapter = ClientAdapter::new(pool.clone());

    adapter
        .create_workflow_version(workflow_name, workflow_source)
        .await
        .unwrap();

    let execution_id = adapter
        .run_workflow(workflow_name, inputs, "default")
        .await
        .unwrap();

    db::work_queue::claim_specific_execution(&pool, &execution_id)
        .await
        .unwrap();

    (pool, adapter, execution_id)
}

/// Helper to enqueue and claim work for an execution
pub async fn enqueue_and_claim_execution(
    pool: &PgPool,
    execution_id: &str,
    queue: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    db::work_queue::enqueue_work(&mut *tx, execution_id, queue, 0).await?;

    // Manually claim the work (simpler than using claim_work for testing)
    sqlx::query(
        r#"
        UPDATE work_queue
        SET claimed_until = NOW() + INTERVAL '5 minutes'
        WHERE execution_id = $1 AND claimed_until IS NULL
        "#,
    )
    .bind(execution_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Helper to get work queue entry count for an execution
pub async fn get_work_queue_count(pool: &PgPool, execution_id: &str) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue WHERE execution_id = $1",
    )
    .bind(execution_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Helper to get child task count for a workflow
pub async fn get_child_task_count(pool: &PgPool, parent_id: &str) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM executions WHERE parent_workflow_id = $1",
    )
    .bind(parent_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Helper to get all child tasks for a workflow
///
/// Returns a list of (task_id, function_name) tuples ordered by creation time.
pub async fn get_child_tasks(pool: &PgPool, parent_id: &str) -> Result<Vec<(String, String)>> {
    let tasks = sqlx::query_as(
        "SELECT id, function_name FROM executions WHERE parent_workflow_id = $1 ORDER BY created_at",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

/// Helper to get a specific child task by function name
///
/// Returns the task ID for a child task with the given function name.
pub async fn get_task_by_function_name(
    pool: &PgPool,
    parent_id: &str,
    function_name: &str,
) -> Result<String> {
    let task_id = sqlx::query_scalar(
        "SELECT id FROM executions WHERE parent_workflow_id = $1 AND function_name = $2",
    )
    .bind(parent_id)
    .bind(function_name)
    .fetch_one(pool)
    .await?;
    Ok(task_id)
}

/// Helper to get unclaimed work queue entry count for an execution
pub async fn get_unclaimed_work_count(pool: &PgPool, execution_id: &str) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_queue WHERE execution_id = $1 AND claimed_until IS NULL",
    )
    .bind(execution_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}
