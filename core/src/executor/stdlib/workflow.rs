//! Workflow stdlib functions

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::outbox::{Outbox, TaskCreation};
use crate::executor::types::{Awaitable, Val};
use crate::types::ExecutionType;
use uuid::Uuid;

/// Workflow.run(workflow_name, inputs) - Create a new child workflow
///
/// Generates a UUID for the workflow, records a side effect in the outbox,
/// and returns a Promise value wrapping the workflow.
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

    // Extract workflow_name (first argument, must be string)
    let workflow_name = match &args[0] {
        Val::Str(s) => s.clone(),
        _ => {
            return EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::WRONG_ARG_TYPE,
                    "First argument (workflow_name) must be a string",
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

    // Generate UUID for the workflow
    let workflow_id = Uuid::new_v4().to_string();

    // Record side effect in outbox
    outbox.push_task(TaskCreation::new(
        workflow_id.clone(),
        workflow_name,
        inputs,
        ExecutionType::Workflow,
    ));

    // Return Promise value wrapping the workflow
    EvalResult::Value {
        v: Val::Promise(Awaitable::Task(workflow_id)),
    }
}
