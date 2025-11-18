use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};

use crate::db;
use crate::executions;
use crate::types::{ExecutionStatus, ExecutionType, CreateExecutionParams};
use crate::interpreter::stdlib::StdlibRegistry;

pub mod expressions;
pub mod statements;

// Temporarily disabled while building executor_v2
// #[cfg(test)]
// mod executor_tests;
// #[cfg(test)]
// mod expression_tests;

// Re-export commonly used items from submodules
pub use expressions::{evaluate_expression, resolve_variables};
pub use statements::{execute_statement, StatementResult};

/// Result of executing a workflow step (public API)
#[derive(Debug)]
pub enum StepResult {
    /// Workflow is suspended, waiting for something
    Suspended,
    /// Workflow completed successfully
    Completed,
    /// Continue to next step immediately
    Continue,
}

/// Internal loop result that carries data
#[derive(Debug)]
enum LoopResult {
    /// Suspended with task_id
    Suspended(String),
    /// Completed with result value
    Completed(JsonValue),
}

/// Result of evaluating an expression
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionResult {
    /// Expression evaluated to a value
    Value(JsonValue),
    /// Expression suspended (waiting for task to complete)
    Suspended(String), // task_id
}

/// A task that needs to be created during expression evaluation
#[derive(Debug, Clone)]
pub struct PendingTask {
    /// Unique ID for this task (generated upfront)
    pub task_id: String,
    /// Type of task: "run", "delay"
    pub task_type: String,
    /// Evaluated arguments for the task
    pub args: Vec<JsonValue>,
    /// Optional metadata (timeout, retries, etc.)
    pub options: JsonValue,
}

/// Path segment type - either an integer index or a string key
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Index(usize),
    Key(String),
}

/// Type alias for AST path - array of segments
pub type AstPath = Vec<PathSegment>;

/// Navigate to a node in the AST using an array path
/// Path format: [1, "then_statements", 0] means statements[1].then_statements[0]
fn get_node_at_path<'a>(statements: &'a JsonValue, path: &[PathSegment]) -> Option<&'a JsonValue> {
    if path.is_empty() {
        return statements.get(0);
    }

    let mut current = statements;

    for segment in path {
        match segment {
            PathSegment::Index(idx) => {
                current = current.get(idx)?;
            }
            PathSegment::Key(key) => {
                current = current.get(key.as_str())?;
            }
        }
    }

    Some(current)
}

/// Advance an AST path to the next node at the same level
/// [1, "then_statements", 0] -> [1, "then_statements", 1]
/// [2] -> [3]
fn advance_path(path: &[PathSegment]) -> AstPath {
    if path.is_empty() {
        return vec![PathSegment::Index(1)];
    }

    let mut new_path = path.to_vec();

    // Find the last Index segment and increment it
    for i in (0..new_path.len()).rev() {
        if let PathSegment::Index(idx) = &new_path[i] {
            new_path[i] = PathSegment::Index(idx + 1);
            break;
        }
    }

    new_path
}

/// Exit from a nested structure to parent level
/// [1, "then_statements", 2] -> Some([1])
/// [1] -> None (at top level)
fn exit_to_parent_path(path: &[PathSegment]) -> Option<AstPath> {
    if path.len() <= 1 {
        return None; // Already at top level
    }

    // Remove last two segments (e.g., "then_statements" and 0)
    // to get back to parent statement
    let parent_len = path.len().saturating_sub(2);

    if parent_len == 0 {
        None
    } else {
        Some(path[0..parent_len].to_vec())
    }
}

/// Convert JsonValue array to AstPath
/// JSON: [0] or [1, "then_statements", 0]
fn json_to_path(json: &JsonValue) -> Result<AstPath> {
    if let Some(arr) = json.as_array() {
        let mut path = Vec::new();
        for segment in arr {
            if let Some(idx) = segment.as_u64() {
                path.push(PathSegment::Index(idx as usize));
            } else if let Some(key) = segment.as_str() {
                path.push(PathSegment::Key(key.to_string()));
            } else {
                anyhow::bail!("Invalid path segment: {:?}", segment);
            }
        }
        Ok(path)
    } else {
        anyhow::bail!("Path must be a JSON array")
    }
}

/// Convert AstPath to JsonValue array
/// AstPath: [1, "then_statements", 0] -> JSON: [1, "then_statements", 0]
fn path_to_json(path: &[PathSegment]) -> JsonValue {
    let segments: Vec<JsonValue> = path.iter().map(|seg| {
        match seg {
            PathSegment::Index(idx) => json!(*idx),
            PathSegment::Key(key) => json!(key),
        }
    }).collect();
    json!(segments)
}



/// Initialize scope_stack structure in locals if it doesn't exist
///
/// Creates a single empty global scope (depth 0).
fn create_scope_stack(locals: &mut JsonValue) {
    // Create new scope_stack with empty global scope
    let global_scope = serde_json::json!({
        "depth": 0,
        "scope_type": "global",
        "variables": {}
    });

    // Add scope_stack to locals
    if let Some(obj) = locals.as_object_mut() {
        obj.insert("scope_stack".to_string(), serde_json::json!([global_scope]));
    }
}

/// Bulk create all pending tasks
///
/// This function creates all tasks that were accumulated during expression evaluation.
/// Using pre-generated UUIDs ensures idempotency and allows task IDs to be stored in
/// variables before the tasks are actually created in the database.
async fn bulk_create_pending_tasks(
    pending_tasks: &[PendingTask],
    parent_workflow_id: &str,
) -> Result<()> {
    if pending_tasks.is_empty() {
        return Ok(());
    }

    // Create all tasks
    for pending_task in pending_tasks {
        // Map task_type to function name
        let function_name = match pending_task.task_type.as_str() {
            "run" => {
                // For "run" tasks, the first argument is the function name
                pending_task.args.get(0)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Task.run missing function name"))?
                    .to_string()
            }
            "delay" => "delay".to_string(),
            _ => anyhow::bail!("Unknown task type: {}", pending_task.task_type),
        };

        // For Task.run, the second argument is the inputs object
        let inputs = if pending_task.task_type == "run" {
            pending_task.args.get(1).cloned().unwrap_or_else(|| json!({}))
        } else {
            json!({})
        };

        // For Task.delay, the first argument is the delay duration
        let _args = if pending_task.task_type == "delay" {
            pending_task.args.clone()
        } else {
            vec![]
        };

        let params = CreateExecutionParams {
            id: Some(pending_task.task_id.clone()),
            exec_type: ExecutionType::Task,
            function_name,
            queue: "default".to_string(),
            inputs,
            max_retries: 3,
            parent_workflow_id: Some(parent_workflow_id.to_string()),
        };

        // Create the task
        executions::create_execution(params).await?;
    }

    Ok(())
}

/// Resolve a suspended task and inject its result into locals
///
/// This function checks if a task is complete and, if so, clears __suspended_task
/// and sets __suspended_task_result in locals for the await expression to consume.
///
/// Returns:
/// - Ok(true) if task is complete (result injected into locals)
/// - Ok(false) if task is still pending (locals unchanged)
/// - Err if there was a database error or task failed
async fn resolve_suspended_task(
    locals: &mut JsonValue,
    pool: &sqlx::PgPool,
) -> Result<bool> {
    // Check if there's a suspended task
    let suspended_task = match locals.get("__suspended_task") {
        Some(task) => task.clone(),
        None => return Ok(false), // No suspended task, nothing to resolve
    };

    // Extract task_id from suspended_task
    let task_id = if let Some(id) = suspended_task.as_str() {
        id.to_string()
    } else if let Some(id) = suspended_task.get("task_id").and_then(|v| v.as_str()) {
        id.to_string()
    } else {
        // Suspended task exists but has no task_id - this is an error
        anyhow::bail!("Suspended task exists but has no task_id: {:?}", suspended_task);
    };

    // Query task status from database
    let task_info: Option<(ExecutionStatus, Option<JsonValue>)> = sqlx::query_as(
        "SELECT status, output FROM executions WHERE id = $1"
    )
    .bind(&task_id)
    .fetch_optional(pool)
    .await
    .context("Failed to check task status")?;

    match task_info {
        Some((ExecutionStatus::Completed, Some(output))) => {
            // Task completed successfully - inject result and clear suspended_task
            locals["__suspended_task_result"] = output;

            // Remove __suspended_task from locals
            if let Some(obj) = locals.as_object_mut() {
                obj.remove("__suspended_task");
            }

            Ok(true)
        }
        Some((ExecutionStatus::Completed, None)) => {
            // Task completed but has no output - treat as null
            locals["__suspended_task_result"] = JsonValue::Null;

            // Remove __suspended_task from locals
            if let Some(obj) = locals.as_object_mut() {
                obj.remove("__suspended_task");
            }

            Ok(true)
        }
        Some((ExecutionStatus::Failed, _)) => {
            // Task failed - this should terminate the workflow
            anyhow::bail!("Task {} failed", task_id);
        }
        Some((_, _)) => {
            // Task is still pending or in another non-complete state
            Ok(false)
        }
        None => {
            // Task doesn't exist in database - this shouldn't happen
            anyhow::bail!("Suspended task {} not found in database", task_id);
        }
    }
}

/// Execute workflow with in-memory loop
///
/// This is the core of the workflow execution engine. It:
/// 1. Loads the workflow context (current AST path position)
/// 2. Checks for suspended task and resolves if complete
/// 3. LOOPS in memory executing statements until suspension or completion
/// 4. Persists state ONCE after loop exits
pub async fn execute_workflow_step(execution_id: &str) -> Result<StepResult> {
    let pool = db::get_pool().await?;

    // 1. Load workflow execution context ONCE
    let context: Option<(i32, JsonValue, JsonValue)> = sqlx::query_as(
        r#"
        SELECT workflow_definition_id, ast_path, locals
        FROM workflow_execution_context
        WHERE execution_id = $1
        "#,
    )
    .bind(execution_id)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to load workflow context")?;

    let (workflow_def_id, ast_path_json, mut locals) = context
        .ok_or_else(|| anyhow::anyhow!("Workflow context not found for execution {}", execution_id))?;

    // Convert to AstPath - handle both old string format and new JSON array format
    let mut ast_path = if ast_path_json.is_array() {
        // New format: JSON array like [0] or [1, "then_statements", 0]
        json_to_path(&ast_path_json)?
    } else if let Some(path_str) = ast_path_json.as_str() {
        // Old format: string like "0" or "1.then_statements.0"
        // Parse it once and convert to array format
        if path_str.is_empty() || path_str == "0" {
            vec![PathSegment::Index(0)]
        } else {
            path_str.split('.').map(|segment| {
                if let Ok(idx) = segment.parse::<usize>() {
                    PathSegment::Index(idx)
                } else {
                    PathSegment::Key(segment.to_string())
                }
            }).collect()
        }
    } else {
        // Invalid format - default to start
        vec![PathSegment::Index(0)]
    };

    // On first execution, initialize inputs from the execution's inputs
    if ast_path.is_empty() && !locals.get("inputs").is_some() {
        let exec_info: Option<(JsonValue,)> = sqlx::query_as(
            "SELECT inputs FROM executions WHERE id = $1"
        )
        .bind(execution_id)
        .fetch_optional(pool.as_ref())
        .await
        .context("Failed to load execution info")?;

        if let Some((inputs,)) = exec_info {
            if let Some(obj) = locals.as_object_mut() {
                obj.insert("inputs".to_string(), inputs);
            }
        }

        // Initialize scope_stack on first execution
        create_scope_stack(&mut locals);
    }

    // 2. Check for suspended task and try to resolve it ONCE
    let task_resolved = resolve_suspended_task(&mut locals, pool.as_ref()).await?;

    if !task_resolved {
        // Task still pending - no point continuing
        return Ok(StepResult::Suspended);
    }

    // Load workflow definition
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

    let statements_value = serde_json::Value::Array(statements);

    // 3. IN-MEMORY LOOP - execute statements until suspension or completion
    let mut pending_tasks = Vec::new();

    let final_result = loop {
        // Get current statement
        let statement = match get_node_at_path(&statements_value, &ast_path) {
            Some(stmt) => stmt.clone(),
            None => {
                // No statement at this path - try to exit to parent
                if let Some(parent_path) = exit_to_parent_path(&ast_path) {
                    ast_path = advance_path(&parent_path);
                    continue;
                } else {
                    // No parent - workflow complete
                    break LoopResult::Completed(JsonValue::Null);
                }
            }
        };

        // Execute the statement using the statements module
        match statements::execute_statement(&statement, &ast_path, &mut locals, &mut pending_tasks)? {
            statements::StatementResult::Continue => {
                ast_path = advance_path(&ast_path);
                continue;
            }
            statements::StatementResult::AdvanceTo(new_path) => {
                ast_path = new_path;
                continue;
            }
            statements::StatementResult::Suspended(task_id) => {
                break LoopResult::Suspended(task_id);
            }
            statements::StatementResult::Return(value) => {
                break LoopResult::Completed(value);
            }
        }
    };

    // 4. Handle loop exit - persist state ONCE
    match final_result {
        LoopResult::Suspended(task_id) => {
            // Store task_id in locals
            if let Some(obj) = locals.as_object_mut() {
                obj.insert("__suspended_task".to_string(), json!(task_id));
            }

            // Bulk create all pending tasks
            bulk_create_pending_tasks(&pending_tasks, execution_id).await?;

            // Convert path back to JSON for storage
            let ast_path_json = path_to_json(&ast_path);

            // Save suspended state
            sqlx::query(
                r#"
                UPDATE workflow_execution_context
                SET ast_path = $1, locals = $2
                WHERE execution_id = $3
                "#,
            )
            .bind(&ast_path_json)
            .bind(&locals)
            .bind(execution_id)
            .execute(pool.as_ref())
            .await
            .context("Failed to save suspended state")?;

            Ok(StepResult::Suspended)
        }

        LoopResult::Completed(result) => {
            // Mark execution as completed
            sqlx::query(
                "UPDATE executions SET status = $1, result = $2, completed_at = NOW() WHERE id = $3"
            )
            .bind(&ExecutionStatus::Completed)
            .bind(&result)
            .bind(execution_id)
            .execute(pool.as_ref())
            .await
            .context("Failed to mark workflow as completed")?;

            Ok(StepResult::Completed)
        }
    }
}

