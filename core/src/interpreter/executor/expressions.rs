use anyhow::Result;
use serde_json::{json, Value as JsonValue, Map};
use uuid;

use super::{PendingTask, ExpressionResult};

/// Resolve variables recursively in a value
///
/// This function traverses any JSON structure and:
/// - Replaces {"type": "variable", "name": "x", "depth": 0} with the actual value from locals
/// - Recursively resolves objects and arrays
/// - Leaves other values unchanged
///
/// This is a helper for backward compatibility and for resolving complex nested structures.
/// Prefer evaluate_expression for new code as it properly handles suspension.
pub fn resolve_variables(value: &JsonValue, locals: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(obj) => {
            // Check the type field to determine what kind of object this is
            if let Some(type_str) = obj.get("type").and_then(|v| v.as_str()) {
                match type_str {
                    "variable" => {
                        // Variable reference: {"type": "variable", "name": "x", "depth": 0}
                        if let (Some(JsonValue::String(var_name)), Some(JsonValue::Number(depth_num))) =
                            (obj.get("name"), obj.get("depth"))
                        {
                            if let Some(depth) = depth_num.as_u64() {
                                return lookup_scoped_variable(var_name, depth as usize, locals);
                            }
                        }
                        // Invalid variable format - return null
                        return JsonValue::Null;
                    }
                    _ => {
                        // Other typed objects (member_access, function_call, etc.)
                        // Don't resolve these - they'll be handled by their specific contexts
                        return value.clone();
                    }
                }
            }

            // Check if this is a member access: {"base": "inputs", "path": [...]}
            // (member access might not have a type field from older code)
            if obj.contains_key("base") && obj.contains_key("path") {
                return resolve_member_access(value, locals);
            }

            // Regular object - resolve all values recursively
            let mut resolved = Map::new();
            for (k, v) in obj.iter() {
                resolved.insert(k.clone(), resolve_variables(v, locals));
            }
            JsonValue::Object(resolved)
        }
        JsonValue::String(_) => {
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

/// Look up a variable by name without scope depth - searches from current scope backwards
fn resolve_variable(var_name: &str, locals: &JsonValue) -> JsonValue {
    // Access scope_stack
    let scope_stack = locals.get("scope_stack")
        .and_then(|v| v.as_array())
        .expect("scope_stack must exist in locals");

    // Search from the most recent scope backwards (dynamic scoping)
    for scope in scope_stack.iter().rev() {
        if let Some(variables) = scope.get("variables") {
            if let Some(value) = variables.get(var_name) {
                return value.clone();
            }
        }
    }

    // Variable not found - return null
    JsonValue::Null
}

/// Look up a variable with known scope depth - O(1) operation
pub fn lookup_scoped_variable(var_name: &str, depth: usize, locals: &JsonValue) -> JsonValue {
    // Access scope_stack - this MUST exist (initialized by create_scope_stack)
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

    // Variable not found - return null
    JsonValue::Null
}

/// Evaluate an expression to a value or suspension point
///
/// This function:
/// - Takes an expression AST node, current locals, and pending_tasks accumulator
/// - Returns ExpressionResult (Value or Suspended)
/// - Accumulates tasks in pending_tasks Vec (for bulk creation later)
/// - Does NOT modify ast_path
/// - Does NOT write to database
///
/// Task expressions add PendingTask to the Vec and return a __task_ref.
/// Await expression handling will be added in Phase 3.
pub fn evaluate_expression(
    expr: &JsonValue,
    locals: &mut JsonValue,
    pending_tasks: &mut Vec<PendingTask>
) -> ExpressionResult {
    // Literals and simple values - already resolved
    if expr.is_null() || expr.is_boolean() || expr.is_number() || expr.is_string() {
        return ExpressionResult::Value(expr.clone());
    }

    // Check if this is an expression with a "type" field
    if let Some(expr_type) = expr.get("type").and_then(|v| v.as_str()) {
        match expr_type {
            // Variable reference: {"type": "variable", "name": "x", "depth": 0}
            "variable" => {
                if let Some(name) = expr.get("name").and_then(|v| v.as_str()) {
                    let depth = expr.get("depth").and_then(|v| v.as_u64()).map(|d| d as usize);

                    let value = if let Some(d) = depth {
                        lookup_scoped_variable(name, d, locals)
                    } else {
                        resolve_variable(name, locals)
                    };

                    return ExpressionResult::Value(value);
                }
            }

            // Member access: {"type": "member_access", "object": expr, "property": "name"}
            "member_access" => {
                // Recursively evaluate the object expression
                let object_result = evaluate_expression(
                    expr.get("object").unwrap_or(&JsonValue::Null),
                    locals,
                    pending_tasks
                );

                let object_value = match object_result {
                    ExpressionResult::Value(v) => v,
                    ExpressionResult::Suspended(task_id) => {
                        // If the object expression suspended, propagate it
                        return ExpressionResult::Suspended(task_id);
                    }
                };

                // Get the property name
                if let Some(property) = expr.get("property").and_then(|v| v.as_str()) {
                    // Try to access the property
                    let result = if object_value.is_object() {
                        object_value.get(property).cloned().unwrap_or(JsonValue::Null)
                    } else if object_value.is_array() {
                        // Try to parse property as array index
                        if let Ok(index) = property.parse::<usize>() {
                            object_value.get(index).cloned().unwrap_or(JsonValue::Null)
                        } else {
                            JsonValue::Null
                        }
                    } else {
                        JsonValue::Null
                    };

                    return ExpressionResult::Value(result);
                }
            }

            // Binary operations: {"type": "binary_op", "operator": "+", "left": expr, "right": expr}
            "binary_op" => {
                if let Some(operator) = expr.get("operator").and_then(|v| v.as_str()) {
                    // Evaluate left and right operands
                    let left_result = evaluate_expression(
                        expr.get("left").unwrap_or(&JsonValue::Null),
                        locals,
                        pending_tasks
                    );
                    let left = match left_result {
                        ExpressionResult::Value(v) => v,
                        ExpressionResult::Suspended(task_id) => {
                            return ExpressionResult::Suspended(task_id);
                        }
                    };

                    let right_result = evaluate_expression(
                        expr.get("right").unwrap_or(&JsonValue::Null),
                        locals,
                        pending_tasks
                    );
                    let right = match right_result {
                        ExpressionResult::Value(v) => v,
                        ExpressionResult::Suspended(task_id) => {
                            return ExpressionResult::Suspended(task_id);
                        }
                    };

                    // Perform the operation
                    let result = match operator {
                        // Arithmetic operations
                        "+" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::from(l + r)
                            } else {
                                JsonValue::Null
                            }
                        }
                        "-" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::from(l - r)
                            } else {
                                JsonValue::Null
                            }
                        }
                        "*" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::from(l * r)
                            } else {
                                JsonValue::Null
                            }
                        }
                        "/" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                if r != 0.0 {
                                    JsonValue::from(l / r)
                                } else {
                                    JsonValue::Null // Division by zero
                                }
                            } else {
                                JsonValue::Null
                            }
                        }
                        "%" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                if r != 0.0 {
                                    JsonValue::from(l % r)
                                } else {
                                    JsonValue::Null
                                }
                            } else {
                                JsonValue::Null
                            }
                        }

                        // Comparison operations
                        "==" => JsonValue::Bool(left == right),
                        "!=" => JsonValue::Bool(left != right),
                        "<" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::Bool(l < r)
                            } else {
                                JsonValue::Bool(false)
                            }
                        }
                        ">" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::Bool(l > r)
                            } else {
                                JsonValue::Bool(false)
                            }
                        }
                        "<=" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::Bool(l <= r)
                            } else {
                                JsonValue::Bool(false)
                            }
                        }
                        ">=" => {
                            if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                                JsonValue::Bool(l >= r)
                            } else {
                                JsonValue::Bool(false)
                            }
                        }

                        _ => JsonValue::Null
                    };

                    return ExpressionResult::Value(result);
                }
            }

            // Logical operations: {"type": "logical_op", "operator": "&&", "left": expr, "right": expr}
            "logical_op" => {
                if let Some(operator) = expr.get("operator").and_then(|v| v.as_str()) {
                    match operator {
                        "&&" => {
                            // Short-circuit: evaluate left first
                            let left_result = evaluate_expression(
                                expr.get("left").unwrap_or(&JsonValue::Null),
                                locals,
                                pending_tasks
                            );
                            let left = match left_result {
                                ExpressionResult::Value(v) => v,
                                ExpressionResult::Suspended(task_id) => {
                                    return ExpressionResult::Suspended(task_id);
                                }
                            };

                            // If left is false, short-circuit
                            if !left.as_bool().unwrap_or(false) {
                                return ExpressionResult::Value(JsonValue::Bool(false));
                            }

                            // Evaluate right
                            let right_result = evaluate_expression(
                                expr.get("right").unwrap_or(&JsonValue::Null),
                                locals,
                                pending_tasks
                            );
                            let right = match right_result {
                                ExpressionResult::Value(v) => v,
                                ExpressionResult::Suspended(task_id) => {
                                    return ExpressionResult::Suspended(task_id);
                                }
                            };

                            return ExpressionResult::Value(JsonValue::Bool(
                                right.as_bool().unwrap_or(false)
                            ));
                        }
                        "||" => {
                            // Short-circuit: evaluate left first
                            let left_result = evaluate_expression(
                                expr.get("left").unwrap_or(&JsonValue::Null),
                                locals,
                                pending_tasks
                            );
                            let left = match left_result {
                                ExpressionResult::Value(v) => v,
                                ExpressionResult::Suspended(task_id) => {
                                    return ExpressionResult::Suspended(task_id);
                                }
                            };

                            // If left is true, short-circuit
                            if left.as_bool().unwrap_or(false) {
                                return ExpressionResult::Value(JsonValue::Bool(true));
                            }

                            // Evaluate right
                            let right_result = evaluate_expression(
                                expr.get("right").unwrap_or(&JsonValue::Null),
                                locals,
                                pending_tasks
                            );
                            let right = match right_result {
                                ExpressionResult::Value(v) => v,
                                ExpressionResult::Suspended(task_id) => {
                                    return ExpressionResult::Suspended(task_id);
                                }
                            };

                            return ExpressionResult::Value(JsonValue::Bool(
                                right.as_bool().unwrap_or(false)
                            ));
                        }
                        _ => {}
                    }
                }
            }

            // Unary operations: {"type": "unary_op", "operator": "!", "operand": expr}
            "unary_op" => {
                if let Some(operator) = expr.get("operator").and_then(|v| v.as_str()) {
                    let operand_result = evaluate_expression(
                        expr.get("operand").unwrap_or(&JsonValue::Null),
                        locals,
                        pending_tasks
                    );
                    let operand = match operand_result {
                        ExpressionResult::Value(v) => v,
                        ExpressionResult::Suspended(task_id) => {
                            return ExpressionResult::Suspended(task_id);
                        }
                    };

                    let result = match operator {
                        "!" => JsonValue::Bool(!operand.as_bool().unwrap_or(false)),
                        "-" => {
                            if let Some(num) = operand.as_f64() {
                                JsonValue::from(-num)
                            } else {
                                JsonValue::Null
                            }
                        }
                        _ => JsonValue::Null
                    };

                    return ExpressionResult::Value(result);
                }
            }

            // Await expression: {"type": "await", "expression": <expr>}
            // Returns Suspended(task_id) to signal the workflow should suspend
            "await" => {
                // Check if we have a resolved result from a completed task
                // This happens when execute_workflow_step resolved the suspended task before calling us
                if let Some(result) = locals.get("__suspended_task_result") {
                    // Task completed - return the result and clean up
                    let result_value = result.clone();

                    // Clean up __suspended_task_result now that we've consumed it
                    if let Some(obj) = locals.as_object_mut() {
                        obj.remove("__suspended_task_result");
                    }

                    return ExpressionResult::Value(result_value);
                }

                // Get the expression being awaited
                let awaited_expr = expr.get("expression")
                    .ok_or_else(|| anyhow::anyhow!("Await expression missing 'expression' field"))
                    .map_err(|e| {
                        eprintln!("Error: {}", e);
                        return ExpressionResult::Value(JsonValue::Null);
                    })
                    .unwrap();

                // Check if we're resuming from a suspended task (still pending)
                if let Some(suspended_task) = locals.get("__suspended_task") {
                    // This is a resumption path - check if the task is complete
                    // The suspended_task should contain the task_id(s) we're waiting for

                    // For now, we'll extract the task_id and return Suspended
                    // The actual task status checking happens at the execute_workflow_step level
                    if let Some(task_id) = suspended_task.as_str() {
                        return ExpressionResult::Suspended(task_id.to_string());
                    } else if let Some(task_id) = suspended_task.get("task_id").and_then(|v| v.as_str()) {
                        return ExpressionResult::Suspended(task_id.to_string());
                    }

                    // If suspended_task exists but doesn't have a task_id, something went wrong
                    eprintln!("Warning: __suspended_task exists but has no task_id: {:?}", suspended_task);
                }

                // First evaluation path - evaluate the awaited expression
                let result = evaluate_expression(awaited_expr, locals, pending_tasks);

                match result {
                    ExpressionResult::Value(v) => {
                        // Check what we got back
                        if let Some(task_id) = v.get("task_id").and_then(|id| id.as_str()) {
                            // This is a Task.run or Task.delay result - suspend on it
                            return ExpressionResult::Suspended(task_id.to_string());
                        } else if v.get("type").and_then(|t| t.as_str()) == Some("task") {
                            // This is a coordination primitive (Task.all/any/race)
                            // We need to extract all task_ids and return the coordination structure
                            // wrapped in a Suspended result

                            // For now, extract the first task_id we can find to suspend on
                            // The actual coordination logic will be handled by the caller
                            if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
                                match method {
                                    "all" | "any" | "race" => {
                                        // Extract all task_ids from args
                                        if let Some(args) = v.get("args").and_then(|a| a.as_array()) {
                                            if let Some(first_array) = args.get(0).and_then(|a| a.as_array()) {
                                                // Get first task_id from the array
                                                if let Some(first_task) = first_array.get(0) {
                                                    if let Some(task_id) = first_task.get("task_id").and_then(|id| id.as_str()) {
                                                        // For coordination primitives, we return Suspended with the whole structure
                                                        // For now, just return the first task_id
                                                        // TODO: Proper coordination primitive handling
                                                        return ExpressionResult::Suspended(task_id.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            // Not a task result - this is an error, can't await non-task expressions
                            eprintln!("Error: Cannot await non-task expression: {:?}", v);
                            return ExpressionResult::Value(JsonValue::Null);
                        }

                        // If we get here, we couldn't extract a task_id
                        return ExpressionResult::Value(v);
                    }
                    ExpressionResult::Suspended(task_id) => {
                        // The awaited expression itself suspended - propagate it
                        return ExpressionResult::Suspended(task_id);
                    }
                }
            }

            // Task expressions: {"type": "task", "method": "run"|"all"|"any"|"race"|"delay", "args": [...]}
            // For Task.run and Task.delay: Add to pending_tasks and return __task_ref
            // For Task.all/any/race: Just evaluate args and return coordination structure
            // Actual task creation happens at suspension/completion (bulk operation)
            "task" => {
                if let Some(method) = expr.get("method").and_then(|v| v.as_str()) {
                    // Get and evaluate arguments
                    let args_array = expr.get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.clone())
                        .unwrap_or_else(Vec::new);

                    // Evaluate each argument
                    let mut evaluated_args = Vec::new();
                    for arg in args_array {
                        let arg_result = evaluate_expression(&arg, locals, pending_tasks);
                        match arg_result {
                            ExpressionResult::Value(v) => evaluated_args.push(v),
                            ExpressionResult::Suspended(task_id) => {
                                // If any argument suspends, propagate it
                                return ExpressionResult::Suspended(task_id);
                            }
                        }
                    }

                    // Only add Task.run and Task.delay to pending_tasks
                    // Task.all/any/race are coordination primitives, not scheduled work
                    if method == "run" || method == "delay" {
                        // Generate UUID upfront
                        let task_id = uuid::Uuid::new_v4().to_string();

                        pending_tasks.push(PendingTask {
                            task_id: task_id.clone(),
                            task_type: method.to_string(),
                            args: evaluated_args.clone(),
                            options: json!({}),
                        });

                        // Return the task_id directly
                        return ExpressionResult::Value(json!({
                            "task_id": task_id
                        }));
                    } else {
                        // For Task.all/any/race, return the coordination structure
                        return ExpressionResult::Value(json!({
                            "type": "task",
                            "method": method,
                            "args": evaluated_args
                        }));
                    }
                }
            }

            _ => {
                // For other types (like "array", "object"), fall through to resolve_variables
            }
        }
    }

    // Arrays - recursively evaluate each element
    if expr.is_array() {
        let arr = expr.as_array().unwrap();
        let mut result_array = Vec::new();

        for item in arr {
            let item_result = evaluate_expression(item, locals, pending_tasks);
            match item_result {
                ExpressionResult::Value(v) => result_array.push(v),
                ExpressionResult::Suspended(task_id) => {
                    // If any element suspends, propagate it
                    return ExpressionResult::Suspended(task_id);
                }
            }
        }

        return ExpressionResult::Value(JsonValue::Array(result_array));
    }

    // Objects and other complex structures - use resolve_variables
    if expr.is_object() {
        let resolved = resolve_variables(expr, locals);
        return ExpressionResult::Value(resolved);
    }

    // Default: resolve variables (handles complex nested structures)
    ExpressionResult::Value(resolve_variables(expr, locals))
}

/// Resolve a for loop iterable specification to get the actual collection
///
/// Handles three formats:
/// 1. {"type": "array", "value": [...]} - Inline array
/// 2. {"type": "variable", "value": {"type": "variable", "name": "name", "depth": 0}} - Variable reference
/// 3. {"type": "member_access", "value": "inputs.items"} - Member access
pub fn resolve_iterable(iterable_spec: &JsonValue, locals: &JsonValue) -> Result<JsonValue> {
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
            // Member access like inputs.items - value is structured JSON
            let access_spec = &iterable_spec["value"];
            Ok(resolve_member_access(access_spec, locals))
        }
        _ => Err(anyhow::anyhow!("Unknown iterable type: {}", iterable_type))
    }
}

/// Resolve member access like "inputs.userId" or "ctx.workflowId"
/// Resolve member access from structured format
/// Input: {"base": "inputs", "path": [{"type": "dot", "value": "user"}, {"type": "index", "value": 0}]}
/// This function is null-safe - if any intermediate value is null, it returns null
pub fn resolve_member_access(access: &JsonValue, locals: &JsonValue) -> JsonValue {
    // Extract base and path from structured format
    let base = match access.get("base").and_then(|v| v.as_str()) {
        Some(b) => b,
        None => return JsonValue::Null,
    };

    let path = match access.get("path").and_then(|v| v.as_array()) {
        Some(p) => p,
        None => return JsonValue::Null,
    };

    // Start with the base variable - check if it's a top-level special variable or in scope_stack
    let mut current = if base == "inputs" || base == "ctx" {
        // Special top-level variables
        match locals.get(base) {
            Some(val) => val.clone(),
            None => return JsonValue::Null,
        }
    } else {
        // Regular variable - look it up in scope_stack (depth 0 for now, we don't have depth info here)
        lookup_scoped_variable(base, 0, locals)
    };

    // Navigate through the path (null-safe)
    for accessor in path {
        let accessor_type = accessor.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match accessor_type {
            "dot" => {
                // Dot notation: .fieldName
                let field = accessor.get("value").and_then(|v| v.as_str()).unwrap_or("");
                current = current.get(field).cloned().unwrap_or(JsonValue::Null);
            }
            "index" => {
                // Array index: [0]
                let index = accessor.get("value").and_then(|v| v.as_i64()).unwrap_or(-1);
                if index >= 0 {
                    if let Some(arr) = current.as_array() {
                        current = arr.get(index as usize).cloned().unwrap_or(JsonValue::Null);
                    } else {
                        return JsonValue::Null;
                    }
                } else {
                    return JsonValue::Null;
                }
            }
            "bracket" => {
                // String key: ["key"]
                let key = accessor.get("value").and_then(|v| v.as_str()).unwrap_or("");
                current = current.get(key).cloned().unwrap_or(JsonValue::Null);
            }
            "bracket_var" => {
                // Variable reference: [varName]
                // First resolve the variable to get the key/index
                let var_name = accessor.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let var_value = resolve_variable(var_name, locals);

                // Use the resolved value as key or index
                if let Some(key) = var_value.as_str() {
                    // String key
                    current = current.get(key).cloned().unwrap_or(JsonValue::Null);
                } else if let Some(index) = var_value.as_i64() {
                    // Numeric index
                    if index >= 0 {
                        if let Some(arr) = current.as_array() {
                            current = arr.get(index as usize).cloned().unwrap_or(JsonValue::Null);
                        } else {
                            return JsonValue::Null;
                        }
                    } else {
                        return JsonValue::Null;
                    }
                } else {
                    return JsonValue::Null;
                }
            }
            _ => {
                return JsonValue::Null;
            }
        }

        // Null-safe: if we hit null at any point, stop and return null
        if current.is_null() {
            return JsonValue::Null;
        }
    }

    current
}
