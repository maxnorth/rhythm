//! Expression evaluation
//!
//! Evaluates expressions to values. Supports literals, identifiers, and member access.
//! Future milestones will add: calls, await.

use super::types::{Expr, Val};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of evaluating an expression
///
/// Expression evaluation can either:
/// - Produce a value (normal case)
/// - Signal suspension (when await is encountered)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum EvalResult {
    /// Expression evaluated to a value
    Value { v: Val },
    /// Expression requires suspension (await encountered)
    Suspend { task_id: String },
}

/// Evaluate an expression to a value or suspension signal
///
/// Supports:
/// - Literals (Bool, Num, Str)
/// - Identifiers (variable lookup)
/// - Member access (object.property)
/// - Await (suspension)
///
/// Parameters:
/// - expr: The expression to evaluate
/// - env: The variable environment for identifier lookups
/// - resume_value: Value to return if this is resuming from await (consumed if Some)
///
/// Returns:
/// - Ok(EvalResult::Value) when expression produces a value
/// - Ok(EvalResult::Suspend) when await is encountered
/// - Err(String) on evaluation error (undefined variable, type mismatch, etc.)
pub fn eval_expr(
    expr: &Expr,
    env: &HashMap<String, Val>,
    resume_value: &mut Option<Val>,
) -> Result<EvalResult, String> {
    match expr {
        Expr::LitBool { v } => Ok(EvalResult::Value {
            v: Val::Bool(*v),
        }),

        Expr::LitNum { v } => Ok(EvalResult::Value { v: Val::Num(*v) }),

        Expr::LitStr { v } => Ok(EvalResult::Value {
            v: Val::Str(v.clone()),
        }),

        Expr::Ident { name } => {
            let val = env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Internal error: undefined variable '{}' (should be caught by parser/validator)", name))?;
            Ok(EvalResult::Value { v: val })
        }

        Expr::Member { object, property } => {
            // First, evaluate the object expression
            let obj_result = eval_expr(object, env, resume_value)?;

            match obj_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - the semantic validator ensures
                    // await is only used in simple contexts where suspension cannot
                    // occur during member access evaluation
                    panic!(
                        "Internal error: suspension during member access evaluation. \
                        This should be prevented by the semantic validator."
                    );
                }
                EvalResult::Value { v: obj_val } => {
                    // Extract the property from the object
                    match obj_val {
                        Val::Obj(map) => {
                            let val = map
                                .get(property)
                                .cloned()
                                .ok_or_else(|| format!("Property '{}' not found on object", property))?;
                            Ok(EvalResult::Value { v: val })
                        }
                        _ => Err(format!(
                            "Cannot access property '{}' on non-object value",
                            property
                        )),
                    }
                }
            }
        }

        Expr::Call { .. } => Err("Call expressions not yet supported".to_string()),

        Expr::Await { inner } => {
            // Check if we're resuming from a previous suspension
            if let Some(val) = resume_value.take() {
                // We're resuming - return the resume value
                return Ok(EvalResult::Value { v: val });
            }

            // Not resuming - evaluate the inner expression normally
            let inner_result = eval_expr(inner, env, resume_value)?;

            match inner_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - the semantic validator ensures
                    // await is only used in simple contexts (return, assignment, expression statements)
                    // where nested awaits cannot occur
                    panic!(
                        "Internal error: nested await suspension detected."
                    );
                }
                EvalResult::Value { v } => {
                    // Inner expression evaluated to a value
                    match v {
                        Val::Task(task_id) => {
                            // This is a Task value - signal suspension
                            Ok(EvalResult::Suspend { task_id })
                        }
                        _ => {
                            // Like JavaScript, awaiting a non-task value just returns that value
                            Ok(EvalResult::Value { v })
                        }
                    }
                }
            }
        }
    }
}
