//! Task and Promise stdlib functions

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::outbox::{Outbox, TaskCreation};
use crate::executor::types::{Awaitable, Val};
use std::collections::HashMap;
use uuid::Uuid;

/// Task.run(task_name, inputs) - Create a new task
///
/// Generates a UUID for the task, records a side effect in the outbox,
/// and returns a Promise value wrapping the task.
pub fn run(args: &[Val], outbox: &mut Outbox) -> EvalResult {
    // Validate argument count
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 2 arguments, got {}", args.len()),
            )),
        };
    }

    // Extract task_name (first argument, must be string)
    let task_name = match &args[0] {
        Val::Str(s) => s.clone(),
        _ => {
            return EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::WRONG_ARG_TYPE,
                    "First argument (task_name) must be a string",
                )),
            };
        }
    };

    // Extract inputs (second argument, must be object)
    let inputs = match &args[1] {
        Val::Obj(map) => map.clone(),
        _ => {
            return EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::WRONG_ARG_TYPE,
                    "Second argument (inputs) must be an object",
                )),
            };
        }
    };

    // Generate UUID for the task
    let task_id = Uuid::new_v4().to_string();

    // Record side effect in outbox
    outbox.push_task(TaskCreation::new(task_id.clone(), task_name, inputs));

    // Return Promise value wrapping the task
    EvalResult::Value {
        v: Val::Promise(Awaitable::Task(task_id)),
    }
}

/// Extract awaitables from array or object of promises.
/// Returns (items, is_object) or an error.
fn extract_awaitables(arg: &Val) -> Result<(Vec<(String, Awaitable)>, bool), EvalResult> {
    match arg {
        Val::List(list) => {
            let mut items = Vec::new();
            for (i, val) in list.iter().enumerate() {
                match val {
                    Val::Promise(awaitable) => {
                        items.push((i.to_string(), awaitable.clone()));
                    }
                    _ => {
                        return Err(EvalResult::Throw {
                            error: Val::Error(ErrorInfo::new(
                                errors::WRONG_ARG_TYPE,
                                format!("Element at index {} is not a Promise", i),
                            )),
                        });
                    }
                }
            }
            Ok((items, false))
        }
        Val::Obj(obj) => {
            let mut items = Vec::new();
            // Sort keys for deterministic order
            let mut keys: Vec<_> = obj.keys().collect();
            keys.sort();
            for key in keys {
                let val = &obj[key];
                match val {
                    Val::Promise(awaitable) => {
                        items.push((key.clone(), awaitable.clone()));
                    }
                    _ => {
                        return Err(EvalResult::Throw {
                            error: Val::Error(ErrorInfo::new(
                                errors::WRONG_ARG_TYPE,
                                format!("Property '{}' is not a Promise", key),
                            )),
                        });
                    }
                }
            }
            Ok((items, true))
        }
        _ => Err(EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_TYPE,
                "Argument must be an array or object of Promises",
            )),
        }),
    }
}

/// Promise.all(promises) - Wait for all promises to complete
///
/// Accepts an array or object of promises.
/// Returns array (for array input) or object (for object input) of values.
/// Fails fast on first error.
pub fn all(args: &[Val]) -> EvalResult {
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    match extract_awaitables(&args[0]) {
        Ok((items, is_object)) => {
            if items.is_empty() {
                // Empty input - return empty array or object immediately
                if is_object {
                    return EvalResult::Value {
                        v: Val::Obj(HashMap::new()),
                    };
                } else {
                    return EvalResult::Value {
                        v: Val::List(vec![]),
                    };
                }
            }
            EvalResult::Value {
                v: Val::Promise(Awaitable::All { items, is_object }),
            }
        }
        Err(e) => e,
    }
}

/// Promise.any(promises) - Wait for first promise to succeed
///
/// Accepts an array or object of promises.
/// Returns just the winning value.
/// Only fails if all promises fail.
pub fn any(args: &[Val]) -> EvalResult {
    any_impl(args, false)
}

/// Promise.any_kv(promises) - Wait for first promise to succeed (kv variant)
///
/// Accepts an array or object of promises.
/// Returns { key, value } where key identifies which promise won.
/// Only fails if all promises fail.
pub fn any_kv(args: &[Val]) -> EvalResult {
    any_impl(args, true)
}

fn any_impl(args: &[Val], with_kv: bool) -> EvalResult {
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    match extract_awaitables(&args[0]) {
        Ok((items, is_object)) => {
            if items.is_empty() {
                // Empty input - fail with AggregateError (no promises to succeed)
                return EvalResult::Throw {
                    error: Val::Error(ErrorInfo::new(
                        "AggregateError",
                        "No promises provided to Promise.any",
                    )),
                };
            }
            EvalResult::Value {
                v: Val::Promise(Awaitable::Any {
                    items,
                    is_object,
                    with_kv,
                }),
            }
        }
        Err(e) => e,
    }
}

/// Promise.race(promises) - Wait for first promise to settle
///
/// Accepts an array or object of promises.
/// Returns just the winning value.
/// Returns as soon as any promise settles (success or error).
pub fn race(args: &[Val]) -> EvalResult {
    race_impl(args, false)
}

/// Promise.race_kv(promises) - Wait for first promise to settle (kv variant)
///
/// Accepts an array or object of promises.
/// Returns { key, value } where key identifies which promise won.
/// Returns as soon as any promise settles (success or error).
pub fn race_kv(args: &[Val]) -> EvalResult {
    race_impl(args, true)
}

fn race_impl(args: &[Val], with_kv: bool) -> EvalResult {
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    match extract_awaitables(&args[0]) {
        Ok((items, is_object)) => {
            if items.is_empty() {
                // Empty input - race never settles (forever pending)
                // In practice, we should probably error here
                return EvalResult::Throw {
                    error: Val::Error(ErrorInfo::new(
                        errors::WRONG_ARG_TYPE,
                        "Promise.race requires at least one promise",
                    )),
                };
            }
            EvalResult::Value {
                v: Val::Promise(Awaitable::Race {
                    items,
                    is_object,
                    with_kv,
                }),
            }
        }
        Err(e) => e,
    }
}
