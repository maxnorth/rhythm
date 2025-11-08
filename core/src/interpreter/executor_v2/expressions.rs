//! Expression evaluation
//!
//! Evaluates expressions to values. Supports literals, identifiers, and member access.
//! Future milestones will add: calls, await.

use super::errors;
use super::types::{ErrorInfo, Expr, Val};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of evaluating an expression
///
/// Expression evaluation can either:
/// - Produce a value (normal case)
/// - Signal suspension (when await is encountered)
/// - Signal an error (throw)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum EvalResult {
    /// Expression evaluated to a value
    Value { v: Val },
    /// Expression requires suspension (await encountered)
    Suspend { task_id: String },
    /// Expression evaluation failed (throw)
    Throw { error: Val },
}

/// Evaluate an expression to a value, suspension signal, or error
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
/// - EvalResult::Value when expression produces a value
/// - EvalResult::Suspend when await is encountered
/// - EvalResult::Throw when runtime error occurs (or internal validator bugs)
pub fn eval_expr(
    expr: &Expr,
    env: &HashMap<String, Val>,
    resume_value: &mut Option<Val>,
) -> EvalResult {
    match expr {
        Expr::LitBool { v } => EvalResult::Value {
            v: Val::Bool(*v),
        },

        Expr::LitNum { v } => EvalResult::Value { v: Val::Num(*v) },

        Expr::LitStr { v } => EvalResult::Value {
            v: Val::Str(v.clone()),
        },

        Expr::Ident { name } => match env.get(name).cloned() {
            Some(val) => EvalResult::Value { v: val },
            None => EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::INTERNAL_ERROR,
                    format!(
                        "Undefined variable '{}' (should be caught by parser/validator)",
                        name
                    ),
                )),
            },
        },

        Expr::Member { object, property } => {
            // First, evaluate the object expression
            let obj_result = eval_expr(object, env, resume_value);

            match obj_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - the semantic validator ensures
                    // await is only used in simple contexts where suspension cannot
                    // occur during member access evaluation
                    EvalResult::Throw {
                        error: Val::Error(ErrorInfo::new(
                            errors::INTERNAL_ERROR,
                            "Suspension during member access evaluation (should be prevented by semantic validator)",
                        )),
                    }
                }
                EvalResult::Throw { error } => {
                    // Propagate the error
                    EvalResult::Throw { error }
                }
                EvalResult::Value { v: obj_val } => {
                    // Extract the property from the object
                    match obj_val {
                        Val::Obj(map) => match map.get(property).cloned() {
                            Some(val) => EvalResult::Value { v: val },
                            None => EvalResult::Throw {
                                error: Val::Error(ErrorInfo::new(
                                    errors::PROPERTY_NOT_FOUND,
                                    format!("Property '{}' not found on object", property),
                                )),
                            },
                        },
                        _ => EvalResult::Throw {
                            error: Val::Error(ErrorInfo::new(
                                errors::TYPE_ERROR,
                                format!(
                                    "Cannot access property '{}' on non-object value",
                                    property
                                ),
                            )),
                        },
                    }
                }
            }
        }

        Expr::Call { .. } => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::INTERNAL_ERROR,
                "Call expressions not yet supported",
            )),
        },

        Expr::Await { inner } => {
            // Check if we're resuming from a previous suspension
            if let Some(val) = resume_value.take() {
                // We're resuming - return the resume value
                return EvalResult::Value { v: val };
            }

            // Not resuming - evaluate the inner expression normally
            let inner_result = eval_expr(inner, env, resume_value);

            match inner_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - the semantic validator ensures
                    // await is only used in simple contexts (return, assignment, expression statements)
                    // where nested awaits cannot occur
                    EvalResult::Throw {
                        error: Val::Error(ErrorInfo::new(
                            errors::INTERNAL_ERROR,
                            "Nested await suspension detected (should be prevented by semantic validator)",
                        )),
                    }
                }
                EvalResult::Throw { error } => {
                    // Propagate the error
                    EvalResult::Throw { error }
                }
                EvalResult::Value { v } => {
                    // Inner expression evaluated to a value
                    match v {
                        Val::Task(task_id) => {
                            // This is a Task value - signal suspension
                            EvalResult::Suspend { task_id }
                        }
                        _ => {
                            // Like JavaScript, awaiting a non-task value just returns that value
                            EvalResult::Value { v }
                        }
                    }
                }
            }
        }
    }
}
