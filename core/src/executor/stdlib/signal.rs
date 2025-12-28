//! Signal stdlib functions

use uuid::Uuid;

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::outbox::{Outbox, SignalRequest};
use crate::executor::types::{Awaitable, Val};

/// Signal.next(name) - Wait for the next signal on a named channel
///
/// Returns a Promise that resolves when a signal is received.
/// The resolved value is the signal's payload.
pub fn next(args: &[Val], outbox: &mut Outbox) -> EvalResult {
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    let name = match &args[0] {
        Val::Str(s) => s.clone(),
        _ => {
            return EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::WRONG_ARG_TYPE,
                    "Argument (name) must be a string",
                )),
            };
        }
    };

    // Generate unique claim_id for this signal request
    let claim_id = Uuid::new_v4().to_string();

    // Add to outbox for later processing
    outbox.push_signal(SignalRequest::new(claim_id.clone(), name.clone()));

    // Return Promise value wrapping the signal awaitable
    EvalResult::Value {
        v: Val::Promise(Awaitable::Signal { name, claim_id }),
    }
}
