//! Standard library functions for Rhythm workflows
//!
//! This module contains all built-in functions available to workflows.
//! Functions are organized in namespaces (e.g., Task.run).

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
        inputs,
        parent_workflow_id: Some(execution_id.to_string()),
    })
    .await
    .context("Failed to create task execution")?;

    Ok(JsonValue::String(task_id))
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
            _ => anyhow::bail!("Unknown function: {}", full_name),
        }
    }
}
