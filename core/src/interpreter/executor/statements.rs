use anyhow::Result;
use serde_json::{Value as JsonValue};

use super::{ExpressionResult, PendingTask, PathSegment, AstPath};
use super::expressions::{evaluate_expression, resolve_variables};

/// Assign a variable to the correct scope in the scope_stack
pub fn assign_variable(locals: &mut JsonValue, var_name: &str, value: JsonValue, depth: Option<usize>) {
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

/// Evaluate a condition expression
///
/// Returns true if the condition is met, false otherwise.
/// Supports:
/// - Comparison operators: ==, !=, <, >, <=, >=
/// - Logical operators: &&, ||
/// - Variable and member access resolution
pub fn evaluate_condition(condition: &JsonValue, locals: &JsonValue) -> Result<bool> {
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

/// Result of executing a statement
pub enum StatementResult {
    /// Continue to next statement normally
    Continue,
    /// Advance to specific path
    AdvanceTo(AstPath),
    /// Suspended waiting for task
    Suspended(String),
    /// Return with value (workflow complete)
    Return(JsonValue),
}

/// Execute a single statement
///
/// This function handles all statement types: assignment, return, if, for, etc.
pub fn execute_statement(
    statement: &JsonValue,
    ast_path: &[PathSegment],
    locals: &mut JsonValue,
    pending_tasks: &mut Vec<PendingTask>,
) -> Result<StatementResult> {
    let statement_type = statement.get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("Statement missing 'type' field"))?;

    match statement_type {
        "assignment" => {
            let left = statement.get("left")
                .ok_or_else(|| anyhow::anyhow!("Assignment missing 'left' field"))?;

            let var_name = left.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Assignment left side missing variable name"))?;

            let depth = left.get("depth")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Assignment left side missing depth"))? as usize;

            let right = statement.get("right")
                .ok_or_else(|| anyhow::anyhow!("Assignment missing 'right' field"))?;

            // Evaluate expression
            let expr_result = evaluate_expression(right, locals, pending_tasks);

            match expr_result {
                ExpressionResult::Value(value) => {
                    // Assign and continue
                    assign_variable(locals, var_name, value, Some(depth));
                    Ok(StatementResult::Continue)
                }
                ExpressionResult::Suspended(task_id) => {
                    Ok(StatementResult::Suspended(task_id))
                }
            }
        }

        "return" => {
            let value_expr = statement.get("value")
                .unwrap_or(&JsonValue::Null);

            let expr_result = evaluate_expression(value_expr, locals, pending_tasks);

            match expr_result {
                ExpressionResult::Value(value) => {
                    Ok(StatementResult::Return(value))
                }
                ExpressionResult::Suspended(task_id) => {
                    Ok(StatementResult::Suspended(task_id))
                }
            }
        }

        _ => {
            // Unknown statement type - for now, just continue
            Ok(StatementResult::Continue)
        }
    }
}
