//! Math stdlib functions

use crate::executor::errors::{self, ErrorInfo};
use crate::executor::expressions::EvalResult;
use crate::executor::types::Val;

/// Math.floor(x) - Returns the largest integer less than or equal to x
pub fn floor(args: &[Val]) -> EvalResult {
    // Validate argument count
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    // Validate argument type
    match &args[0] {
        Val::Num(n) => EvalResult::Value {
            v: Val::Num(n.floor()),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_TYPE,
                "Argument must be a number",
            )),
        },
    }
}

/// Math.ceil(x) - Returns the smallest integer greater than or equal to x
pub fn ceil(args: &[Val]) -> EvalResult {
    // Validate argument count
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    // Validate argument type
    match &args[0] {
        Val::Num(n) => EvalResult::Value {
            v: Val::Num(n.ceil()),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_TYPE,
                "Argument must be a number",
            )),
        },
    }
}

/// Math.abs(x) - Returns the absolute value of x
pub fn abs(args: &[Val]) -> EvalResult {
    // Validate argument count
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    // Validate argument type
    match &args[0] {
        Val::Num(n) => EvalResult::Value {
            v: Val::Num(n.abs()),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_TYPE,
                "Argument must be a number",
            )),
        },
    }
}

/// Math.round(x) - Returns x rounded to the nearest integer
///
/// Uses JavaScript-style rounding (half-way cases round towards +∞)
pub fn round(args: &[Val]) -> EvalResult {
    // Validate argument count
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_COUNT,
                format!("Expected 1 argument, got {}", args.len()),
            )),
        };
    }

    // Validate argument type
    match &args[0] {
        Val::Num(n) => {
            // JavaScript rounds half-way cases towards +infinity
            // e.g., 2.5 -> 3, -2.5 -> -2
            let rounded = if n.fract() == 0.5 || n.fract() == -0.5 {
                n.ceil()
            } else {
                n.round()
            };
            EvalResult::Value {
                v: Val::Num(rounded),
            }
        }
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new(
                errors::WRONG_ARG_TYPE,
                "Argument must be a number",
            )),
        },
    }
}
