use anyhow::{Context, Result};
use serde_json::Value as JsonValue;

use crate::db;
use crate::executions;
use crate::types::{ExecutionStatus, ExecutionType, CreateExecutionParams};

/// Result of executing a workflow step
#[derive(Debug)]
pub enum StepResult {
    /// Workflow is suspended, waiting for something
    Suspended,
    /// Workflow completed successfully
    Completed,
    /// Continue to next step immediately
    Continue,
}

/// Execute a single workflow step
///
/// This is the core of the workflow execution engine. It:
/// 1. Loads the workflow context (which statement we're at)
/// 2. Loads and parses the workflow definition
/// 3. Executes the current statement
/// 4. Updates state based on what happened
pub async fn execute_workflow_step(execution_id: &str) -> Result<StepResult> {
    let pool = db::get_pool().await?;

    // 1. Load workflow execution context
    let context: Option<(i32, i32, JsonValue, Option<String>)> = sqlx::query_as(
        r#"
        SELECT workflow_definition_id, statement_index, locals, awaiting_task_id
        FROM workflow_execution_context
        WHERE execution_id = $1
        "#,
    )
    .bind(execution_id)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to load workflow context")?;

    let (workflow_def_id, statement_index, _locals, awaiting_task_id) = context
        .ok_or_else(|| anyhow::anyhow!("Workflow context not found for execution {}", execution_id))?;

    // If we're awaiting a task, check if it's done
    if let Some(task_id) = awaiting_task_id {
        let task_status: Option<(ExecutionStatus,)> = sqlx::query_as(
            "SELECT status FROM executions WHERE id = $1"
        )
        .bind(&task_id)
        .fetch_optional(pool.as_ref())
        .await
        .context("Failed to check task status")?;

        match task_status {
            Some((ExecutionStatus::Completed,)) => {
                // Task is done, move to next statement
                sqlx::query(
                    r#"
                    UPDATE workflow_execution_context
                    SET statement_index = statement_index + 1, awaiting_task_id = NULL
                    WHERE execution_id = $1
                    "#,
                )
                .bind(execution_id)
                .execute(pool.as_ref())
                .await
                .context("Failed to advance to next statement")?;

                // Continue executing from next statement
                return Box::pin(execute_workflow_step(execution_id)).await;
            }
            Some((ExecutionStatus::Failed,)) => {
                // Task failed, fail the workflow
                sqlx::query("UPDATE executions SET status = $1 WHERE id = $2")
                    .bind(&ExecutionStatus::Failed)
                    .bind(execution_id)
                    .execute(pool.as_ref())
                    .await
                    .context("Failed to mark workflow as failed")?;

                return Ok(StepResult::Completed); // Workflow is done (failed)
            }
            _ => {
                // Task still running/pending, stay suspended
                return Ok(StepResult::Suspended);
            }
        }
    }

    // 2. Load workflow definition (with cached parsed steps)
    let workflow_def: Option<(JsonValue,)> = sqlx::query_as(
        "SELECT parsed_steps FROM workflow_definitions WHERE id = $1"
    )
    .bind(workflow_def_id)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to load workflow definition")?;

    let (parsed_steps_value,) = workflow_def
        .ok_or_else(|| anyhow::anyhow!("Workflow definition {} not found", workflow_def_id))?;

    let statements = parsed_steps_value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Parsed steps is not an array"))?
        .clone();

    // Check if we've reached the end
    if statement_index >= statements.len() as i32 {
        // Workflow is complete
        sqlx::query("UPDATE executions SET status = $1, completed_at = NOW() WHERE id = $2")
            .bind(&ExecutionStatus::Completed)
            .bind(execution_id)
            .execute(pool.as_ref())
            .await
            .context("Failed to mark workflow as completed")?;

        return Ok(StepResult::Completed);
    }

    // 3. Execute current statement
    let statement = &statements[statement_index as usize];
    let statement_type = statement["type"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Statement missing 'type' field"))?;

    match statement_type {
        "task" => {
            // Create child task execution
            let task_name = statement["task"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Task statement missing 'task' field"))?;
            let inputs = statement["inputs"].clone();

            let task_id = executions::create_execution(CreateExecutionParams {
                id: None,
                exec_type: ExecutionType::Task,
                function_name: task_name.to_string(),
                queue: "default".to_string(),
                priority: 5,
                args: serde_json::json!([]),
                kwargs: inputs,
                max_retries: 3,
                timeout_seconds: None,
                parent_workflow_id: Some(execution_id.to_string()),
            })
            .await
            .context("Failed to create task execution")?;

            // Suspend workflow, wait for task
            sqlx::query(
                r#"
                UPDATE workflow_execution_context
                SET awaiting_task_id = $1
                WHERE execution_id = $2
                "#,
            )
            .bind(&task_id)
            .bind(execution_id)
            .execute(pool.as_ref())
            .await
            .context("Failed to update workflow context")?;

            sqlx::query("UPDATE executions SET status = $1 WHERE id = $2")
                .bind(&ExecutionStatus::Suspended)
                .bind(execution_id)
                .execute(pool.as_ref())
                .await
                .context("Failed to suspend workflow")?;

            Ok(StepResult::Suspended)
        }
        "sleep" => {
            let duration = statement["duration"].as_i64()
                .ok_or_else(|| anyhow::anyhow!("Sleep statement missing 'duration' field"))?;

            // For now, just move to next statement immediately
            // TODO: Implement actual sleep scheduling
            sqlx::query(
                r#"
                UPDATE workflow_execution_context
                SET statement_index = statement_index + 1
                WHERE execution_id = $1
                "#,
            )
            .bind(execution_id)
            .execute(pool.as_ref())
            .await
            .context("Failed to advance past sleep")?;

            println!("Sleep({}) - skipping for now", duration);

            // Continue to next step
            Ok(StepResult::Continue)
        }
        _ => {
            anyhow::bail!("Unknown statement type: {}", statement_type)
        }
    }
}
