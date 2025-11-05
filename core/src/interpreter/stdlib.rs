//! Standard library functions for Rhythm workflows
//!
//! This module contains all built-in functions available to workflows.
//! Functions are organized in namespaces (e.g., Task.run, Task.delay).

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

use crate::executions;
use crate::types::{ExecutionType, CreateExecutionParams};

/// Execute a Task.run() function call
///
/// Creates a child task execution and returns the task ID.
/// Arguments: (task_name: string, inputs?: object)
pub async fn task_run(
    args: &[JsonValue],
    pool: &PgPool,
    execution_id: &str,
) -> Result<JsonValue> {
    if args.is_empty() {
        anyhow::bail!("Task.run() requires at least a task name");
    }

    let task_name = args[0]
        .as_str()
        .ok_or_else(|| anyhow!("Task.run() first argument must be a string (task name)"))?;

    let inputs = if args.len() > 1 {
        args[1].clone()
    } else {
        json!({})
    };

    let task_id = executions::create_execution(CreateExecutionParams {
        id: None,
        exec_type: ExecutionType::Task,
        function_name: task_name.to_string(),
        queue: "default".to_string(),
        priority: 5,
        args: json!([inputs]),  // Pass inputs as args[0], not as kwargs
        kwargs: json!({}),
        max_retries: 3,
        timeout_seconds: None,
        parent_workflow_id: Some(execution_id.to_string()),
    })
    .await
    .context("Failed to create task execution")?;

    Ok(JsonValue::String(task_id))
}

/// Execute a Task.delay() function call
///
/// Suspends workflow execution for a specified duration.
/// Arguments: (duration_ms: number)
pub async fn task_delay(
    args: &[JsonValue],
    pool: &PgPool,
    execution_id: &str,
) -> Result<JsonValue> {
    if args.is_empty() {
        anyhow::bail!("Task.delay() requires a duration in milliseconds");
    }

    let duration = args[0]
        .as_i64()
        .ok_or_else(|| anyhow!("Task.delay() argument must be a number (milliseconds)"))?;

    if duration < 0 {
        anyhow::bail!("Task.delay() duration must be non-negative");
    }

    // Calculate wake time
    let wake_time = chrono::Utc::now() + chrono::Duration::milliseconds(duration);

    // Update execution to sleep
    sqlx::query(
        r#"
        UPDATE workflow_execution_context
        SET wake_time = $1
        WHERE execution_id = $2
        "#,
    )
    .bind(wake_time)
    .bind(execution_id)
    .execute(pool)
    .await
    .context("Failed to set wake time")?;

    // Return null (delay doesn't return a value)
    Ok(JsonValue::Null)
}

/// Registry of all standard library functions
///
/// Maps function names to their implementations.
pub struct StdlibRegistry;

impl StdlibRegistry {
    /// Get the function implementation for a given name path
    ///
    /// Example: ["Task", "run"] -> task_run function
    pub async fn call(
        name: &[String],
        args: &[JsonValue],
        pool: &PgPool,
        execution_id: &str,
    ) -> Result<JsonValue> {
        let full_name = name.join(".");

        match full_name.as_str() {
            "Task.run" => task_run(args, pool, execution_id).await,
            "Task.delay" => task_delay(args, pool, execution_id).await,
            _ => anyhow::bail!("Unknown function: {}", full_name),
        }
    }
}
