use anyhow::{Context, Result};
use serde_json::{Value as JsonValue, Map};

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

/// Resolve variable references in a JSON value
///
/// Replaces strings like "$varname" with actual values from locals.
/// Works recursively through objects and arrays.
fn resolve_variables(value: &JsonValue, locals: &JsonValue) -> JsonValue {
    match value {
        JsonValue::String(s) => {
            // Check if this is a variable reference (starts with $)
            if let Some(var_name) = s.strip_prefix('$') {
                // Look up the variable in locals
                if let Some(var_value) = locals.get(var_name) {
                    var_value.clone()
                } else {
                    // Variable not found, keep the original string
                    value.clone()
                }
            } else {
                value.clone()
            }
        }
        JsonValue::Array(arr) => {
            JsonValue::Array(
                arr.iter()
                    .map(|v| resolve_variables(v, locals))
                    .collect()
            )
        }
        JsonValue::Object(obj) => {
            let mut resolved = Map::new();
            for (k, v) in obj.iter() {
                resolved.insert(k.clone(), resolve_variables(v, locals));
            }
            JsonValue::Object(resolved)
        }
        _ => value.clone(),
    }
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

    let (workflow_def_id, statement_index, mut locals, awaiting_task_id) = context
        .ok_or_else(|| anyhow::anyhow!("Workflow context not found for execution {}", execution_id))?;

    // If we're awaiting a task, check if it's done
    if let Some(task_id) = awaiting_task_id {
        let task_info: Option<(ExecutionStatus, Option<JsonValue>)> = sqlx::query_as(
            "SELECT status, result FROM executions WHERE id = $1"
        )
        .bind(&task_id)
        .fetch_optional(pool.as_ref())
        .await
        .context("Failed to check task status")?;

        match task_info {
            Some((ExecutionStatus::Completed, task_result)) => {
                // Check if we need to assign the result to a variable
                // We need to get the statement that spawned this task to check for assign_to
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
                    .ok_or_else(|| anyhow::anyhow!("Parsed steps is not an array"))?;

                // Get the current statement (the one that created the task we were awaiting)
                if let Some(statement) = statements.get(statement_index as usize) {
                    if let Some(var_name) = statement.get("assign_to").and_then(|v| v.as_str()) {
                        // Store the task result in locals
                        if let Some(obj) = locals.as_object_mut() {
                            obj.insert(var_name.to_string(), task_result.unwrap_or(JsonValue::Null));
                        }
                    }
                }

                // Task is done, move to next statement
                sqlx::query(
                    r#"
                    UPDATE workflow_execution_context
                    SET statement_index = statement_index + 1, awaiting_task_id = NULL, locals = $2
                    WHERE execution_id = $1
                    "#,
                )
                .bind(execution_id)
                .bind(&locals)
                .execute(pool.as_ref())
                .await
                .context("Failed to advance to next statement")?;

                // Continue executing from next statement
                return Box::pin(execute_workflow_step(execution_id)).await;
            }
            Some((ExecutionStatus::Failed, _)) => {
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
            let inputs_template = statement["inputs"].clone();
            let should_await = statement["await"].as_bool().unwrap_or(true); // Default to await for backwards compatibility

            // Resolve variable references in inputs
            let inputs = resolve_variables(&inputs_template, &locals);

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

            if should_await {
                // Suspend workflow and wait for task to complete
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
            } else {
                // Fire-and-forget: move to next statement immediately
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
                .context("Failed to advance workflow")?;

                Ok(StepResult::Continue)
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_variables_string() {
        let locals = json!({
            "order_id": "12345",
            "user_name": "Alice"
        });

        // Simple variable reference
        let input = json!("$order_id");
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("12345"));

        // Regular string (not a variable)
        let input = json!("hello");
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("hello"));

        // Variable not found
        let input = json!("$missing");
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("$missing")); // Keeps original
    }

    #[test]
    fn test_resolve_variables_object() {
        let locals = json!({
            "order_id": "12345",
            "amount": 100
        });

        let input = json!({
            "order": "$order_id",
            "total": "$amount",
            "static": "value"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "order": "12345",
            "total": 100,
            "static": "value"
        }));
    }

    #[test]
    fn test_resolve_variables_nested() {
        let locals = json!({
            "user_id": "user123",
            "order_id": "order456"
        });

        let input = json!({
            "user": {
                "id": "$user_id",
                "orders": ["$order_id", "other"]
            },
            "count": 5
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "user": {
                "id": "user123",
                "orders": ["order456", "other"]
            },
            "count": 5
        }));
    }

    #[test]
    fn test_resolve_variables_array() {
        let locals = json!({
            "id1": "first",
            "id2": "second"
        });

        let input = json!(["$id1", "$id2", "static"]);
        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!(["first", "second", "static"]));
    }

    #[test]
    fn test_resolve_variables_complex_types() {
        let locals = json!({
            "config": {
                "timeout": 30,
                "retries": 3
            }
        });

        // Variable that resolves to an object
        let input = json!({
            "settings": "$config",
            "enabled": true
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "settings": {
                "timeout": 30,
                "retries": 3
            },
            "enabled": true
        }));
    }

    #[test]
    fn test_resolve_variables_empty_locals() {
        let locals = json!({});

        // Variable references should remain unchanged if not found
        let input = json!({
            "user_id": "$missing_var",
            "value": 123
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "user_id": "$missing_var",
            "value": 123
        }));
    }

    #[test]
    fn test_resolve_variables_mixed_found_and_missing() {
        let locals = json!({
            "found": "value1"
        });

        let input = json!({
            "a": "$found",
            "b": "$missing",
            "c": "normal"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "a": "value1",
            "b": "$missing",
            "c": "normal"
        }));
    }

    #[test]
    fn test_resolve_variables_deeply_nested() {
        let locals = json!({
            "user_id": "user123",
            "config": {
                "setting1": "value1"
            }
        });

        let input = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "user": "$user_id",
                        "data": "$config"
                    }
                }
            }
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["level1"]["level2"]["level3"]["user"], "user123");
        assert_eq!(result["level1"]["level2"]["level3"]["data"], json!({
            "setting1": "value1"
        }));
    }

    #[test]
    fn test_resolve_variables_preserve_non_variable_dollars() {
        let locals = json!({
            "amount": 100
        });

        // Test that literal $ strings are preserved (no conflict with variable format)
        let input = json!({
            "price": "$amount",
            "currency": "USD$",
            "note": "Cost is $amount dollars"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["price"], 100);
        assert_eq!(result["currency"], "USD$");
        // Literal strings with $ are preserved - no conflict!
        assert_eq!(result["note"], "Cost is $amount dollars");
    }

    #[test]
    fn test_resolve_variables_numbers_and_primitives() {
        let locals = json!({
            "count": 42,
            "ratio": 3.14,
            "enabled": true,
            "disabled": false,
            "empty": null
        });

        let input = json!({
            "n": "$count",
            "r": "$ratio",
            "e": "$enabled",
            "d": "$disabled",
            "z": "$empty"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["n"], 42);
        assert_eq!(result["r"], 3.14);
        assert_eq!(result["e"], true);
        assert_eq!(result["d"], false);
        assert_eq!(result["z"], JsonValue::Null);
    }

    #[test]
    fn test_resolve_variables_in_array_of_objects() {
        let locals = json!({
            "id1": "first",
            "id2": "second"
        });

        let input = json!([
            { "id": "$id1", "name": "Item 1" },
            { "id": "$id2", "name": "Item 2" }
        ]);

        let result = resolve_variables(&input, &locals);

        assert_eq!(result[0]["id"], "first");
        assert_eq!(result[0]["name"], "Item 1");
        assert_eq!(result[1]["id"], "second");
        assert_eq!(result[1]["name"], "Item 2");
    }
}
