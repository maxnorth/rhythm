use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue, Map};

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

/// Evaluate a condition expression
///
/// Returns true if the condition is met, false otherwise.
/// Supports:
/// - Comparison operators: ==, !=, <, >, <=, >=
/// - Logical operators: &&, ||
/// - Variable and member access resolution
fn evaluate_condition(condition: &JsonValue, locals: &JsonValue) -> Result<bool> {
    let condition_type = condition.get("type")
        .and_then(|v| v.as_str());

    match condition_type {
        Some("comparison") => {
            let operator = condition["operator"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Comparison missing operator"))?;

            let left = resolve_variables(&condition["left"], locals);
            let right = resolve_variables(&condition["right"], locals);

            match operator {
                "==" => Ok(left == right),
                "!=" => Ok(left != right),
                "<" => compare_values(&left, &right, |l, r| l < r),
                ">" => compare_values(&left, &right, |l, r| l > r),
                "<=" => compare_values(&left, &right, |l, r| l <= r),
                ">=" => compare_values(&left, &right, |l, r| l >= r),
                _ => Err(anyhow::anyhow!("Unknown comparison operator: {}", operator)),
            }
        }
        Some("and") => {
            // All operands must be true
            let operands = condition["operands"].as_array()
                .ok_or_else(|| anyhow::anyhow!("AND expression missing operands"))?;

            for operand in operands {
                if !evaluate_condition(operand, locals)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Some("or") => {
            // Any operand must be true
            let operands = condition["operands"].as_array()
                .ok_or_else(|| anyhow::anyhow!("OR expression missing operands"))?;

            for operand in operands {
                if evaluate_condition(operand, locals)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => {
            // Direct value (for boolean expressions)
            let resolved = resolve_variables(condition, locals);
            Ok(resolved.as_bool().unwrap_or(false))
        }
    }
}

/// Compare two JSON values numerically
fn compare_values<F>(left: &JsonValue, right: &JsonValue, op: F) -> Result<bool>
where
    F: Fn(f64, f64) -> bool,
{
    let left_num = left.as_f64()
        .ok_or_else(|| anyhow::anyhow!("Cannot compare non-numeric value: {:?}", left))?;
    let right_num = right.as_f64()
        .ok_or_else(|| anyhow::anyhow!("Cannot compare non-numeric value: {:?}", right))?;

    Ok(op(left_num, right_num))
}

/// Resolve variable references in a JSON value
///
/// Variables are annotated with scope depth: {"var": "name", "depth": 0}
/// This enables O(1) lookup directly to the correct scope.
///
/// Also handles member access like "inputs.userId" or "ctx.workflowId"
fn resolve_variables(value: &JsonValue, locals: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(obj) => {
            // Check if this is a variable reference with scope depth annotation
            if let (Some(JsonValue::String(var_name)), Some(JsonValue::Number(depth_num))) =
                (obj.get("var"), obj.get("depth"))
            {
                if let Some(depth) = depth_num.as_u64() {
                    // Scoped variable reference - O(1) lookup
                    return lookup_scoped_variable(var_name, depth as usize, locals);
                }
            }

            // Not a variable reference - resolve all values recursively
            let mut resolved = Map::new();
            for (k, v) in obj.iter() {
                resolved.insert(k.clone(), resolve_variables(v, locals));
            }
            JsonValue::Object(resolved)
        }
        JsonValue::String(s) => {
            // Check for member access (e.g., "inputs.userId", "ctx.workflowId")
            if s.contains('.') {
                return resolve_member_access(s, locals);
            }

            // Plain string value
            value.clone()
        }
        JsonValue::Array(arr) => {
            JsonValue::Array(
                arr.iter()
                    .map(|v| resolve_variables(v, locals))
                    .collect()
            )
        }
        _ => value.clone(),
    }
}

/// Look up a variable with known scope depth - O(1) operation
fn lookup_scoped_variable(var_name: &str, depth: usize, locals: &JsonValue) -> JsonValue {
    // Access scope_stack - this MUST exist (initialized by ensure_scope_stack)
    let scope_stack = locals.get("scope_stack")
        .and_then(|v| v.as_array())
        .expect("scope_stack must exist in locals");

    // Direct access to the scope at the specified depth
    if let Some(scope) = scope_stack.get(depth) {
        if let Some(variables) = scope.get("variables") {
            if let Some(value) = variables.get(var_name) {
                return value.clone();
            }
        }
    }

    // Variable not found - return annotated reference for error visibility
    json!({
        "var": var_name,
        "depth": depth
    })
}

/// Resolve a for loop iterable specification to get the actual collection
///
/// Handles three formats:
/// 1. {"type": "array", "value": [...]} - Inline array
/// 2. {"type": "variable", "value": {"var": "name", "depth": 0}} - Variable reference
/// 3. {"type": "member_access", "value": "inputs.items"} - Member access
fn resolve_iterable(iterable_spec: &JsonValue, locals: &JsonValue) -> Result<JsonValue> {
    let iterable_type = iterable_spec.get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Iterable missing 'type' field"))?;

    match iterable_type {
        "array" => {
            // Inline array - already resolved
            Ok(iterable_spec["value"].clone())
        }
        "variable" => {
            // Variable reference - resolve it
            let var_ref = &iterable_spec["value"];
            Ok(resolve_variables(var_ref, locals))
        }
        "member_access" => {
            // Member access like inputs.items
            let path = iterable_spec["value"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Member access missing value string"))?;
            Ok(resolve_member_access(path, locals))
        }
        _ => Err(anyhow::anyhow!("Unknown iterable type: {}", iterable_type))
    }
}

/// Resolve member access like "inputs.userId" or "ctx.workflowId"
fn resolve_member_access(path: &str, locals: &JsonValue) -> JsonValue {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return JsonValue::Null;
    }

    // Start with the root object
    let mut current = locals.get(parts[0]);

    // Navigate through the path
    for part in &parts[1..] {
        if let Some(obj) = current {
            current = obj.get(part);
        } else {
            return JsonValue::Null;
        }
    }

    current.cloned().unwrap_or(JsonValue::Null)
}

/// Assign a variable to the correct scope in the scope_stack
fn assign_variable(locals: &mut JsonValue, var_name: &str, value: JsonValue, depth: Option<usize>) {
    let scope_stack = locals.get_mut("scope_stack")
        .and_then(|v| v.as_array_mut())
        .expect("scope_stack must exist in locals");

    let target_depth = depth.unwrap_or(0);

    // Access scope at target_depth
    let scope = scope_stack.get_mut(target_depth)
        .expect(&format!("scope at depth {} must exist", target_depth));

    let variables = scope.get_mut("variables")
        .and_then(|v| v.as_object_mut())
        .expect("scope must have variables object");

    variables.insert(var_name.to_string(), value);
}

/// Initialize scope_stack structure in locals if it doesn't exist
///
/// Creates a single global scope (depth 0) and migrates any existing flat variables into it.
fn ensure_scope_stack(locals: &mut JsonValue) {
    // Check if scope_stack already exists
    if locals.get("scope_stack").is_some() {
        return;
    }

    // Create new scope_stack with global scope
    let mut global_variables = Map::new();

    // Migrate existing flat variables to global scope
    if let Some(obj) = locals.as_object() {
        for (key, value) in obj.iter() {
            // Don't migrate special fields like "inputs", "ctx"
            if key != "inputs" && key != "ctx" && key != "scope_stack" {
                global_variables.insert(key.clone(), value.clone());
            }
        }
    }

    // Create global scope
    let global_scope = serde_json::json!({
        "depth": 0,
        "scope_type": "global",
        "variables": global_variables
    });

    // Update locals to use scope_stack
    if let Some(obj) = locals.as_object_mut() {
        // Remove migrated variables
        let keys_to_remove: Vec<String> = obj.keys()
            .filter(|k| *k != "inputs" && *k != "ctx")
            .cloned()
            .collect();

        for key in keys_to_remove {
            obj.remove(&key);
        }

        // Add scope_stack
        obj.insert("scope_stack".to_string(), serde_json::json!([global_scope]));
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

    // Ensure scope_stack exists (migrate old workflows if needed)
    ensure_scope_stack(&mut locals);

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
                        // Get the scope depth for this variable (if available)
                        let depth = statement.get("assign_to_depth")
                            .and_then(|v| v.as_u64())
                            .map(|d| d as usize);

                        // Store the task result in the correct scope
                        assign_variable(&mut locals, var_name, task_result.unwrap_or(JsonValue::Null), depth);
                    }
                }

                // Check if we're inside a for loop
                let scope_stack = locals.get("scope_stack")
                    .and_then(|v| v.as_array());
                let in_for_loop = scope_stack
                    .and_then(|stack| stack.last())
                    .and_then(|scope| scope.get("scope_type"))
                    .and_then(|t| t.as_str()) == Some("for_loop");

                if in_for_loop {
                    // We're inside a for loop - don't advance statement_index
                    // The loop manages its own iteration. Just clear awaiting_task_id
                    sqlx::query(
                        r#"
                        UPDATE workflow_execution_context
                        SET awaiting_task_id = NULL, locals = $2
                        WHERE execution_id = $1
                        "#,
                    )
                    .bind(execution_id)
                    .bind(&locals)
                    .execute(pool.as_ref())
                    .await
                    .context("Failed to clear awaiting_task_id")?;
                } else {
                    // Regular task - advance to next statement
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
                }

                // Continue executing from next statement/iteration
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
        "if" => {
            // Evaluate the condition
            let condition = &statement["condition"];
            let is_true = evaluate_condition(condition, &locals)
                .context("Failed to evaluate if condition")?;

            // Get the appropriate branch
            let branch_statements = if is_true {
                statement["then_statements"].as_array()
            } else {
                statement.get("else_statements")
                    .and_then(|v| v.as_array())
            };

            // If we have statements to execute, flatten them into the workflow
            // For now, we'll execute them inline and advance past the if statement
            if let Some(stmts) = branch_statements {
                // Execute each statement in the branch
                for branch_stmt in stmts {
                    let stmt_type = branch_stmt["type"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Branch statement missing 'type' field"))?;

                    match stmt_type {
                        "task" => {
                            let task_name = branch_stmt["task"].as_str()
                                .ok_or_else(|| anyhow::anyhow!("Task statement missing 'task' field"))?;
                            let inputs_template = branch_stmt["inputs"].clone();
                            let should_await = branch_stmt["await"].as_bool().unwrap_or(true);

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
                                // NOTE: This is a simplified implementation - it only supports
                                // awaiting the last task in the branch. For multiple awaited tasks
                                // in a branch, we'd need more sophisticated state tracking.
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

                                return Ok(StepResult::Suspended);
                            }
                        }
                        _ => {
                            // For now, only tasks are supported in if branches
                            anyhow::bail!("Unsupported statement type in if branch: {}", stmt_type);
                        }
                    }
                }
            }

            // Move to next statement after the if
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
            .context("Failed to advance past if statement")?;

            // Continue to next step
            Ok(StepResult::Continue)
        }
        "for" => {
            // For loop execution with scope management and await support
            let loop_variable = statement["loop_variable"].as_str()
                .ok_or_else(|| anyhow::anyhow!("For loop missing loop_variable"))?;

            let iterable_spec = &statement["iterable"];
            let body_statements = statement["body_statements"].as_array()
                .ok_or_else(|| anyhow::anyhow!("For loop missing body_statements"))?;

            // Resolve the iterable to get the actual collection
            let collection = resolve_iterable(iterable_spec, &locals)?;
            let items = collection.as_array()
                .ok_or_else(|| anyhow::anyhow!("For loop iterable must be an array"))?;

            ensure_scope_stack(&mut locals);
            let scope_stack = locals.get_mut("scope_stack")
                .and_then(|v| v.as_array_mut())
                .expect("scope_stack must exist");

            let loop_depth = scope_stack.len();

            // Check if we're resuming an existing loop or starting a new one
            let is_resuming = scope_stack.len() > loop_depth
                && scope_stack.get(loop_depth)
                    .and_then(|s| s.get("scope_type"))
                    .and_then(|t| t.as_str()) == Some("for_loop");

            if !is_resuming {
                // Starting a new loop - create loop scope
                let loop_scope = json!({
                    "depth": loop_depth,
                    "scope_type": "for_loop",
                    "variables": {
                        loop_variable: items.get(0).cloned().unwrap_or(JsonValue::Null)
                    },
                    "metadata": {
                        "loop_variable": loop_variable,
                        "collection": items.clone(),
                        "current_index": 0,
                        "body_statement_index": 0
                    }
                });
                scope_stack.push(loop_scope);
            }

            // Get current loop state
            let loop_scope = scope_stack.get_mut(loop_depth)
                .ok_or_else(|| anyhow::anyhow!("Loop scope not found"))?;

            let current_index = loop_scope["metadata"]["current_index"].as_u64()
                .ok_or_else(|| anyhow::anyhow!("current_index missing"))? as usize;
            let body_stmt_index = loop_scope["metadata"]["body_statement_index"].as_u64()
                .ok_or_else(|| anyhow::anyhow!("body_statement_index missing"))? as usize;

            // Check if loop is complete
            if current_index >= items.len() {
                // Exit loop - pop scope and advance to next statement
                let scope_stack = locals.get_mut("scope_stack")
                    .and_then(|v| v.as_array_mut())
                    .expect("scope_stack must exist");
                scope_stack.pop();

                sqlx::query(
                    r#"
                    UPDATE workflow_execution_context
                    SET statement_index = statement_index + 1, locals = $2
                    WHERE execution_id = $1
                    "#,
                )
                .bind(execution_id)
                .bind(&locals)
                .execute(pool.as_ref())
                .await
                .context("Failed to advance past for loop")?;

                return Ok(StepResult::Continue);
            }

            // Execute current body statement
            if body_stmt_index < body_statements.len() {
                let body_stmt = &body_statements[body_stmt_index];
                let stmt_type = body_stmt["type"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Body statement missing type"))?;

                match stmt_type {
                    "break" => {
                        // Exit loop immediately - pop scope and advance to next statement
                        let scope_stack = locals.get_mut("scope_stack")
                            .and_then(|v| v.as_array_mut())
                            .expect("scope_stack must exist");
                        scope_stack.pop();

                        sqlx::query(
                            r#"
                            UPDATE workflow_execution_context
                            SET statement_index = statement_index + 1, locals = $2
                            WHERE execution_id = $1
                            "#,
                        )
                        .bind(execution_id)
                        .bind(&locals)
                        .execute(pool.as_ref())
                        .await
                        .context("Failed to exit loop after break")?;

                        return Ok(StepResult::Continue);
                    }
                    "continue" => {
                        // Skip to next iteration
                        let scope_stack = locals.get_mut("scope_stack")
                            .and_then(|v| v.as_array_mut())
                            .expect("scope_stack must exist");
                        let loop_scope = scope_stack.get_mut(loop_depth)
                            .expect("loop scope must exist");

                        let next_index = current_index + 1;

                        if next_index < items.len() {
                            // Move to next iteration
                            loop_scope["variables"][loop_variable] = items[next_index].clone();
                            loop_scope["metadata"]["current_index"] = json!(next_index);
                            loop_scope["metadata"]["body_statement_index"] = json!(0);

                            // Save state and continue
                            sqlx::query(
                                r#"
                                UPDATE workflow_execution_context
                                SET locals = $1
                                WHERE execution_id = $2
                                "#,
                            )
                            .bind(&locals)
                            .bind(execution_id)
                            .execute(pool.as_ref())
                            .await
                            .context("Failed to save loop state after continue")?;

                            // Continue executing next iteration
                            return Box::pin(execute_workflow_step(execution_id)).await;
                        } else {
                            // No more items - exit loop
                            scope_stack.pop();

                            sqlx::query(
                                r#"
                                UPDATE workflow_execution_context
                                SET statement_index = statement_index + 1, locals = $2
                                WHERE execution_id = $1
                                "#,
                            )
                            .bind(execution_id)
                            .bind(&locals)
                            .execute(pool.as_ref())
                            .await
                            .context("Failed to advance past for loop")?;

                            return Ok(StepResult::Continue);
                        }
                    }
                    "task" => {
                        let should_await = body_stmt["await"].as_bool().unwrap_or(true);
                        let task_name = body_stmt["task"].as_str()
                            .ok_or_else(|| anyhow::anyhow!("Task missing 'task' field"))?;
                        let inputs_template = body_stmt["inputs"].clone();

                        // Resolve variables in inputs (including loop variable)
                        let inputs = resolve_variables(&inputs_template, &locals);

                        // Create the task execution
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
                        .context("Failed to create task execution in for loop")?;

                        if should_await {
                            // Suspend and wait for task - advance body_statement_index for next resumption
                            let scope_stack = locals.get_mut("scope_stack")
                                .and_then(|v| v.as_array_mut())
                                .expect("scope_stack must exist");
                            let loop_scope = scope_stack.get_mut(loop_depth)
                                .expect("loop scope must exist");
                            loop_scope["metadata"]["body_statement_index"] = json!(body_stmt_index + 1);

                            sqlx::query(
                                r#"
                                UPDATE workflow_execution_context
                                SET awaiting_task_id = $1, locals = $2
                                WHERE execution_id = $3
                                "#,
                            )
                            .bind(&task_id)
                            .bind(&locals)
                            .execute(pool.as_ref())
                            .await
                            .context("Failed to update workflow context")?;

                            sqlx::query("UPDATE executions SET status = $1 WHERE id = $2")
                                .bind(&ExecutionStatus::Suspended)
                                .bind(execution_id)
                                .execute(pool.as_ref())
                                .await
                                .context("Failed to suspend workflow")?;

                            return Ok(StepResult::Suspended);
                        } else {
                            // Fire-and-forget - just advance to next body statement
                            let scope_stack = locals.get_mut("scope_stack")
                                .and_then(|v| v.as_array_mut())
                                .expect("scope_stack must exist");
                            let loop_scope = scope_stack.get_mut(loop_depth)
                                .expect("loop scope must exist");
                            loop_scope["metadata"]["body_statement_index"] = json!(body_stmt_index + 1);

                            // Continue executing - don't update DB yet, continue in same step
                            return Box::pin(execute_workflow_step(execution_id)).await;
                        }
                    }
                    _ => {
                        anyhow::bail!("Only tasks supported in for loops currently. Statement type '{}' not supported.", stmt_type);
                    }
                }
            } else {
                // Finished all body statements for this iteration - move to next iteration
                let scope_stack = locals.get_mut("scope_stack")
                    .and_then(|v| v.as_array_mut())
                    .expect("scope_stack must exist");
                let loop_scope = scope_stack.get_mut(loop_depth)
                    .expect("loop scope must exist");

                let next_index = current_index + 1;

                if next_index < items.len() {
                    // Move to next iteration
                    loop_scope["variables"][loop_variable] = items[next_index].clone();
                    loop_scope["metadata"]["current_index"] = json!(next_index);
                    loop_scope["metadata"]["body_statement_index"] = json!(0);

                    // Save state and continue
                    sqlx::query(
                        r#"
                        UPDATE workflow_execution_context
                        SET locals = $1
                        WHERE execution_id = $2
                        "#,
                    )
                    .bind(&locals)
                    .bind(execution_id)
                    .execute(pool.as_ref())
                    .await
                    .context("Failed to save loop state")?;

                    // Continue executing next iteration
                    return Box::pin(execute_workflow_step(execution_id)).await;
                } else {
                    // All iterations complete - exit loop
                    scope_stack.pop();

                    sqlx::query(
                        r#"
                        UPDATE workflow_execution_context
                        SET statement_index = statement_index + 1, locals = $2
                        WHERE execution_id = $1
                        "#,
                    )
                    .bind(execution_id)
                    .bind(&locals)
                    .execute(pool.as_ref())
                    .await
                    .context("Failed to advance past for loop")?;

                    return Ok(StepResult::Continue);
                }
            }
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
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "order_id": "12345",
                        "user_name": "Alice"
                    }
                }
            ]
        });

        // Simple variable reference with scope annotation
        let input = json!({"var": "order_id", "depth": 0});
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("12345"));

        // Regular string (not a variable)
        let input = json!("hello");
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("hello"));

        // Variable not found - returns original annotation
        let input = json!({"var": "missing", "depth": 0});
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!({"var": "missing", "depth": 0}));
    }

    #[test]
    fn test_resolve_variables_object() {
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "order_id": "12345",
                        "amount": 100
                    }
                }
            ]
        });

        let input = json!({
            "order": {"var": "order_id", "depth": 0},
            "total": {"var": "amount", "depth": 0},
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
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "user_id": "user123",
                    "order_id": "order456"
                }
            }]
        });

        let input = json!({
            "user": {
                "id": {"var": "user_id", "depth": 0},
                "orders": [{"var": "order_id", "depth": 0}, "other"]
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
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "id1": "first",
                    "id2": "second"
                }
            }]
        });

        let input = json!([
            {"var": "id1", "depth": 0},
            {"var": "id2", "depth": 0},
            "static"
        ]);
        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!(["first", "second", "static"]));
    }

    #[test]
    fn test_resolve_variables_complex_types() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "config": {
                        "timeout": 30,
                        "retries": 3
                    }
                }
            }]
        });

        // Variable that resolves to an object
        let input = json!({
            "settings": {"var": "config", "depth": 0},
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
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        // Variable references return annotation if not found
        let input = json!({
            "user_id": {"var": "missing_var", "depth": 0},
            "value": 123
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "user_id": {"var": "missing_var", "depth": 0},
            "value": 123
        }));
    }

    #[test]
    fn test_resolve_variables_mixed_found_and_missing() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "found": "value1"
                }
            }]
        });

        let input = json!({
            "a": {"var": "found", "depth": 0},
            "b": {"var": "missing", "depth": 0},
            "c": "normal"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "a": "value1",
            "b": {"var": "missing", "depth": 0},
            "c": "normal"
        }));
    }

    #[test]
    fn test_resolve_variables_deeply_nested() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "user_id": "user123",
                    "config": {
                        "setting1": "value1"
                    }
                }
            }]
        });

        let input = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "user": {"var": "user_id", "depth": 0},
                        "data": {"var": "config", "depth": 0}
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
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "amount": 100
                }
            }]
        });

        // Literal $ strings are just strings - no conflict with our annotation format
        let input = json!({
            "price": {"var": "amount", "depth": 0},
            "currency": "USD$",
            "note": "Cost is $amount dollars"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["price"], 100);
        assert_eq!(result["currency"], "USD$");
        // Literal strings are preserved - no more $ variable syntax!
        assert_eq!(result["note"], "Cost is $amount dollars");
    }

    #[test]
    fn test_resolve_variables_numbers_and_primitives() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "count": 42,
                    "ratio": 3.14,
                    "enabled": true,
                    "disabled": false,
                    "empty": null
                }
            }]
        });

        let input = json!({
            "n": {"var": "count", "depth": 0},
            "r": {"var": "ratio", "depth": 0},
            "e": {"var": "enabled", "depth": 0},
            "d": {"var": "disabled", "depth": 0},
            "z": {"var": "empty", "depth": 0}
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
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "id1": "first",
                    "id2": "second"
                }
            }]
        });

        let input = json!([
            { "id": {"var": "id1", "depth": 0}, "name": "Item 1" },
            { "id": {"var": "id2", "depth": 0}, "name": "Item 2" }
        ]);

        let result = resolve_variables(&input, &locals);

        assert_eq!(result[0]["id"], "first");
        assert_eq!(result[0]["name"], "Item 1");
        assert_eq!(result[1]["id"], "second");
        assert_eq!(result[1]["name"], "Item 2");
    }

    #[test]
    fn test_resolve_iterable_inline_array() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        let iterable_spec = json!({
            "type": "array",
            "value": [1, 2, 3, 4, 5]
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_resolve_iterable_variable_reference() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "items": ["a", "b", "c"]
                }
            }]
        });

        let iterable_spec = json!({
            "type": "variable",
            "value": {"var": "items", "depth": 0}
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!(["a", "b", "c"]));
    }

    #[test]
    fn test_resolve_iterable_member_access() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }],
            "inputs": {
                "orders": [
                    {"id": "order1"},
                    {"id": "order2"}
                ]
            }
        });

        let iterable_spec = json!({
            "type": "member_access",
            "value": "inputs.orders"
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!([
            {"id": "order1"},
            {"id": "order2"}
        ]));
    }

    #[test]
    fn test_resolve_iterable_complex_array() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        let iterable_spec = json!({
            "type": "array",
            "value": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25},
                {"name": "Charlie", "age": 35}
            ]
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
        assert_eq!(result[0]["name"], "Alice");
        assert_eq!(result[2]["age"], 35);
    }

    #[test]
    fn test_lookup_scoped_variable_nested_scopes() {
        // Simulate nested for loops with variables at different depths
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "global_var": "global_value"
                    }
                },
                {
                    "depth": 1,
                    "scope_type": "for_loop",
                    "variables": {
                        "outer_item": "outer_value"
                    },
                    "metadata": {
                        "loop_variable": "outer_item",
                        "collection": ["a", "b"],
                        "current_index": 0
                    }
                },
                {
                    "depth": 2,
                    "scope_type": "for_loop",
                    "variables": {
                        "inner_item": "inner_value"
                    },
                    "metadata": {
                        "loop_variable": "inner_item",
                        "collection": [1, 2, 3],
                        "current_index": 1
                    }
                }
            ]
        });

        // Should be able to access variables at any depth
        let global = lookup_scoped_variable("global_var", 0, &locals);
        assert_eq!(global, "global_value");

        let outer = lookup_scoped_variable("outer_item", 1, &locals);
        assert_eq!(outer, "outer_value");

        let inner = lookup_scoped_variable("inner_item", 2, &locals);
        assert_eq!(inner, "inner_value");
    }

    #[test]
    fn test_resolve_variables_nested_loop_scopes() {
        // Test variable resolution with nested loop scopes
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "userId": "user123"
                    }
                },
                {
                    "depth": 1,
                    "scope_type": "for_loop",
                    "variables": {
                        "order": {"id": "order1", "total": 100}
                    }
                },
                {
                    "depth": 2,
                    "scope_type": "for_loop",
                    "variables": {
                        "item": {"name": "Widget", "price": 25}
                    }
                }
            ]
        });

        // Build an input that references variables from all three scopes
        let input = json!({
            "user": {"var": "userId", "depth": 0},
            "order": {"var": "order", "depth": 1},
            "item": {"var": "item", "depth": 2}
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["user"], "user123");
        assert_eq!(result["order"]["id"], "order1");
        assert_eq!(result["order"]["total"], 100);
        assert_eq!(result["item"]["name"], "Widget");
        assert_eq!(result["item"]["price"], 25);
    }
}
