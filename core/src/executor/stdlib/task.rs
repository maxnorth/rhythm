//! Task stdlib functions

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::outbox::{Outbox, TaskCreation};
use crate::executor::types::Val;
use uuid::Uuid;

/// Task.run(task_name, inputs) - Create a new task
///
/// Generates a UUID for the task, records a side effect in the outbox,
/// and returns a Task value with the UUID.
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
    outbox.push(TaskCreation::new(task_id.clone(), task_name, inputs));

    // Return Task value with the UUID
    EvalResult::Value {
        v: Val::Task(task_id),
    }
}
