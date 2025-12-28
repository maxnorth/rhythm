//! Expression evaluation
//!
//! Evaluates expressions to values. Supports literals, identifiers, and member access.
//! Future milestones will add: calls, await.

use super::errors;
use super::outbox::Outbox;
use super::types::ast::BinaryOp;
use super::types::{Awaitable, ErrorInfo, Expr, Val};
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
    Suspend { awaitable: Awaitable },
    /// Expression evaluation failed (throw)
    Throw { error: Val },
}

/// Evaluate an expression to a value, suspension signal, or error
///
/// Supports:
/// - Literals (Bool, Num, Str, Null, List, Obj)
/// - Identifiers (variable lookup)
/// - Member access (object.property)
/// - Function calls
/// - Await (suspension)
///
/// Parameters:
/// - expr: The expression to evaluate
/// - env: The variable environment for identifier lookups
/// - resume_value: Value to return if this is resuming from await (consumed if Some)
/// - outbox: Collection of side effects (task creation, etc.)
///
/// Returns:
/// - EvalResult::Value when expression produces a value
/// - EvalResult::Suspend when await is encountered
/// - EvalResult::Throw when runtime error occurs (or internal validator bugs)
pub fn eval_expr(
    expr: &Expr,
    env: &HashMap<String, Val>,
    resume_value: &mut Option<Val>,
    outbox: &mut Outbox,
) -> EvalResult {
    match expr {
        Expr::LitBool { v } => EvalResult::Value { v: Val::Bool(*v) },

        Expr::LitNum { v } => EvalResult::Value { v: Val::Num(*v) },

        Expr::LitStr { v } => EvalResult::Value {
            v: Val::Str(v.clone()),
        },

        Expr::LitNull => EvalResult::Value { v: Val::Null },

        Expr::LitList { elements } => {
            // Evaluate all elements (left to right)
            let mut vals = Vec::new();
            for elem_expr in elements {
                match eval_expr(elem_expr, env, resume_value, outbox) {
                    EvalResult::Value { v } => vals.push(v),
                    EvalResult::Suspend { .. } => {
                        // This should never happen - validator ensures no await in literals
                        return EvalResult::Throw {
                            error: Val::Error(ErrorInfo::new(
                                errors::INTERNAL_ERROR,
                                "Suspension during list literal evaluation (should be prevented by semantic validator)",
                            )),
                        };
                    }
                    EvalResult::Throw { error } => {
                        // Propagate error from element evaluation
                        return EvalResult::Throw { error };
                    }
                }
            }
            EvalResult::Value { v: Val::List(vals) }
        }

        Expr::LitObj { properties } => {
            // Evaluate all property values (in order)
            let mut map = HashMap::new();
            for (key, val_expr) in properties {
                match eval_expr(val_expr, env, resume_value, outbox) {
                    EvalResult::Value { v } => {
                        map.insert(key.clone(), v);
                    }
                    EvalResult::Suspend { .. } => {
                        // This should never happen - validator ensures no await in literals
                        return EvalResult::Throw {
                            error: Val::Error(ErrorInfo::new(
                                errors::INTERNAL_ERROR,
                                "Suspension during object literal evaluation (should be prevented by semantic validator)",
                            )),
                        };
                    }
                    EvalResult::Throw { error } => {
                        // Propagate error from property evaluation
                        return EvalResult::Throw { error };
                    }
                }
            }
            EvalResult::Value { v: Val::Obj(map) }
        }

        Expr::Ident { name } => match env.get(name).cloned() {
            Some(val) => EvalResult::Value { v: val },
            None => EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::INTERNAL_ERROR,
                    format!("Undefined variable '{}'", name),
                )),
            },
        },

        Expr::Member {
            object,
            property,
            optional,
        } => {
            // First, evaluate the object expression
            let obj_result = eval_expr(object, env, resume_value, outbox);

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
                    // If optional chaining (?.) and object is null, return null
                    if *optional && matches!(obj_val, Val::Null) {
                        return EvalResult::Value { v: Val::Null };
                    }

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

        Expr::Call { callee, args } => {
            // Step 1: Evaluate the callee expression to get the function
            let callee_result = eval_expr(callee, env, resume_value, outbox);

            match callee_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - the semantic validator ensures
                    // await is only used in simple contexts where suspension cannot
                    // occur during call evaluation
                    EvalResult::Throw {
                        error: Val::Error(ErrorInfo::new(
                            errors::INTERNAL_ERROR,
                            "Suspension during call callee evaluation (should be prevented by semantic validator)",
                        )),
                    }
                }
                EvalResult::Throw { error } => {
                    // Propagate the error from callee evaluation
                    EvalResult::Throw { error }
                }
                EvalResult::Value { v: callee_val } => {
                    // Step 2: Verify callee is a function
                    let func = match callee_val {
                        Val::NativeFunc(f) => f,
                        _ => {
                            return EvalResult::Throw {
                                error: Val::Error(ErrorInfo::new(
                                    errors::NOT_A_FUNCTION,
                                    "Value is not callable",
                                )),
                            };
                        }
                    };

                    // Step 3: Evaluate all arguments (left to right)
                    let mut arg_vals = Vec::new();
                    for arg_expr in args {
                        match eval_expr(arg_expr, env, resume_value, outbox) {
                            EvalResult::Value { v } => arg_vals.push(v),
                            EvalResult::Suspend { .. } => {
                                // This should never happen - validator ensures no await in call args
                                return EvalResult::Throw {
                                    error: Val::Error(ErrorInfo::new(
                                        errors::INTERNAL_ERROR,
                                        "Suspension during call argument evaluation (should be prevented by semantic validator)",
                                    )),
                                };
                            }
                            EvalResult::Throw { error } => {
                                // Propagate error from argument evaluation
                                return EvalResult::Throw { error };
                            }
                        }
                    }

                    // Step 4: Call the stdlib function
                    super::stdlib::call_stdlib_func(&func, &arg_vals, outbox)
                }
            }
        }

        Expr::Await { inner } => {
            // Check if we're resuming from a previous suspension
            if let Some(val) = resume_value.take() {
                // We're resuming - return the resume value
                return EvalResult::Value { v: val };
            }

            // Not resuming - evaluate the inner expression normally
            let inner_result = eval_expr(inner, env, resume_value, outbox);

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
                        Val::Promise(awaitable) => {
                            // This is a Promise value - signal suspension
                            EvalResult::Suspend { awaitable }
                        }
                        _ => {
                            // Like JavaScript, awaiting a non-promise value just returns that value
                            EvalResult::Value { v }
                        }
                    }
                }
            }
        }

        Expr::BinaryOp { op, left, right } => {
            // Short-circuit evaluation for &&, ||, and ??
            // Evaluate left operand first
            let left_result = eval_expr(left, env, resume_value, outbox);

            match left_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - validator ensures no await in binary ops
                    EvalResult::Throw {
                        error: Val::Error(ErrorInfo::new(
                            errors::INTERNAL_ERROR,
                            "Suspension during binary operator left operand evaluation (should be prevented by semantic validator)",
                        )),
                    }
                }
                EvalResult::Throw { error } => {
                    // Propagate error from left operand
                    EvalResult::Throw { error }
                }
                EvalResult::Value { v: left_val } => {
                    match op {
                        BinaryOp::And => {
                            // For &&: if left is falsy, short-circuit and return left value
                            let left_bool = left_val.to_bool();
                            if !left_bool {
                                return EvalResult::Value { v: left_val };
                            }
                            // Left is truthy, evaluate right operand and return its value
                            let right_result = eval_expr(right, env, resume_value, outbox);
                            match right_result {
                                EvalResult::Suspend { .. } => {
                                    // This should never happen - validator ensures no await in binary ops
                                    EvalResult::Throw {
                                        error: Val::Error(ErrorInfo::new(
                                            errors::INTERNAL_ERROR,
                                            "Suspension during binary operator right operand evaluation (should be prevented by semantic validator)",
                                        )),
                                    }
                                }
                                EvalResult::Throw { error } => EvalResult::Throw { error },
                                EvalResult::Value { v: right_val } => {
                                    // Return the right value (not converted to boolean)
                                    EvalResult::Value { v: right_val }
                                }
                            }
                        }
                        BinaryOp::Or => {
                            // For ||: if left is truthy, short-circuit and return left value
                            let left_bool = left_val.to_bool();
                            if left_bool {
                                return EvalResult::Value { v: left_val };
                            }
                            // Left is falsy, evaluate right operand and return its value
                            let right_result = eval_expr(right, env, resume_value, outbox);
                            match right_result {
                                EvalResult::Suspend { .. } => {
                                    // This should never happen - validator ensures no await in binary ops
                                    EvalResult::Throw {
                                        error: Val::Error(ErrorInfo::new(
                                            errors::INTERNAL_ERROR,
                                            "Suspension during binary operator right operand evaluation (should be prevented by semantic validator)",
                                        )),
                                    }
                                }
                                EvalResult::Throw { error } => EvalResult::Throw { error },
                                EvalResult::Value { v: right_val } => {
                                    // Return the right value (not converted to boolean)
                                    EvalResult::Value { v: right_val }
                                }
                            }
                        }
                        BinaryOp::Nullish => {
                            // For ??: if left is null, evaluate and return right value
                            // Otherwise return left (even if it's 0, "", false, etc.)
                            if matches!(left_val, Val::Null) {
                                // Left is null, evaluate right operand and return its value
                                let right_result = eval_expr(right, env, resume_value, outbox);
                                match right_result {
                                    EvalResult::Suspend { .. } => {
                                        // This should never happen - validator ensures no await in binary ops
                                        EvalResult::Throw {
                                            error: Val::Error(ErrorInfo::new(
                                                errors::INTERNAL_ERROR,
                                                "Suspension during binary operator right operand evaluation (should be prevented by semantic validator)",
                                            )),
                                        }
                                    }
                                    EvalResult::Throw { error } => EvalResult::Throw { error },
                                    EvalResult::Value { v: right_val } => {
                                        // Return the right value
                                        EvalResult::Value { v: right_val }
                                    }
                                }
                            } else {
                                // Left is not null, return it (even if falsy)
                                EvalResult::Value { v: left_val }
                            }
                        }
                    }
                }
            }
        }

        Expr::Ternary {
            condition,
            consequent,
            alternate,
        } => {
            // Evaluate the condition first
            let cond_result = eval_expr(condition, env, resume_value, outbox);

            match cond_result {
                EvalResult::Suspend { .. } => {
                    // This should never happen - validator ensures no await in ternary condition
                    EvalResult::Throw {
                        error: Val::Error(ErrorInfo::new(
                            errors::INTERNAL_ERROR,
                            "Suspension during ternary condition evaluation (should be prevented by semantic validator)",
                        )),
                    }
                }
                EvalResult::Throw { error } => {
                    // Propagate error from condition
                    EvalResult::Throw { error }
                }
                EvalResult::Value { v: cond_val } => {
                    // Evaluate the appropriate branch based on truthiness
                    let branch = if cond_val.to_bool() {
                        consequent
                    } else {
                        alternate
                    };

                    let branch_result = eval_expr(branch, env, resume_value, outbox);
                    match branch_result {
                        EvalResult::Suspend { .. } => {
                            // This should never happen - validator ensures no await in ternary branches
                            EvalResult::Throw {
                                error: Val::Error(ErrorInfo::new(
                                    errors::INTERNAL_ERROR,
                                    "Suspension during ternary branch evaluation (should be prevented by semantic validator)",
                                )),
                            }
                        }
                        EvalResult::Throw { error } => EvalResult::Throw { error },
                        EvalResult::Value { v } => EvalResult::Value { v },
                    }
                }
            }
        }
    }
}
