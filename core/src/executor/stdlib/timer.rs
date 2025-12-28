//! Timer stdlib functions

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::outbox::{Outbox, TimerSchedule};
use crate::executor::types::{Awaitable, Val};
use chrono::{Duration, Utc};

/// Timer.delay(duration_seconds) - Create a timer that fires after the specified duration
///
/// Takes a duration in seconds, computes the absolute fire_at time using
/// the current worker time, records a TimerSchedule side effect in the outbox,
/// and returns a Promise value wrapping the timer.
pub fn delay(args: &[Val], outbox: &mut Outbox) -> EvalResult {
    // Validate argument count
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    // Extract duration_seconds (first argument, must be number)
    let duration_seconds = match &args[0] {
        Val::Num(n) => {
            if *n < 0.0 {
                return EvalResult::Throw {
                    error: Val::Error(ErrorInfo::new(
                        errors::WRONG_ARG_TYPE,
                        "Duration must be a non-negative number",
                    )),
                };
            }
            *n
        }
        _ => {
            return EvalResult::Throw {
                error: Val::Error(ErrorInfo::new(
                    errors::WRONG_ARG_TYPE,
                    "Argument (duration_seconds) must be a number",
                )),
            };
        }
    };

    // Compute fire_at using worker-local time (clock skew is acceptable)
    // Convert seconds to milliseconds for Duration
    let duration_ms = (duration_seconds * 1000.0) as i64;
    let fire_at = Utc::now() + Duration::milliseconds(duration_ms);

    // Record side effect in outbox
    outbox.push_timer(TimerSchedule::new(fire_at));

    // Return Promise value wrapping the timer
    EvalResult::Value {
        v: Val::Promise(Awaitable::Timer { fire_at }),
    }
}
