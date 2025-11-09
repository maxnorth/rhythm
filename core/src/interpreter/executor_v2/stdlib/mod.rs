//! Standard library function implementations
//!
//! This module contains all stdlib function implementations organized by category.

pub mod math;
pub mod task;

use super::expressions::EvalResult;
use super::outbox::Outbox;
use super::types::Val;
use serde::{Deserialize, Serialize};

/* ===================== Standard Library Function Types ===================== */

/// Standard library function identifiers
///
/// Each variant represents a specific stdlib function.
/// These are serializable and can be stored in the environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StdlibFunc {
    MathFloor,
    MathCeil,
    MathAbs,
    MathRound,
    TaskRun,
}

/* ===================== Stdlib Dispatcher ===================== */

/// Call a standard library function with arguments
///
/// This dispatcher routes to the appropriate function implementation
/// based on the StdlibFunc variant.
pub fn call_stdlib_func(func: &StdlibFunc, args: &[Val], outbox: &mut Outbox) -> EvalResult {
    match func {
        // Math functions are pure - no outbox needed
        StdlibFunc::MathFloor => math::floor(args),
        StdlibFunc::MathCeil => math::ceil(args),
        StdlibFunc::MathAbs => math::abs(args),
        StdlibFunc::MathRound => math::round(args),
        // Task functions have side effects - outbox required
        StdlibFunc::TaskRun => task::run(args, outbox),
    }
}

/* ===================== Utilities ===================== */

/// Convert value to string representation
///
/// This implements JavaScript's ToString abstract operation.
/// Used for property key conversion, string concatenation, etc.
pub fn to_string(val: &Val) -> String {
    match val {
        Val::Null => "null".to_string(),
        Val::Bool(true) => "true".to_string(),
        Val::Bool(false) => "false".to_string(),
        Val::Num(n) => {
            // Handle special numeric cases
            if n.is_nan() {
                "NaN".to_string()
            } else if n.is_infinite() {
                if *n > 0.0 {
                    "Infinity".to_string()
                } else {
                    "-Infinity".to_string()
                }
            } else if *n == 0.0 {
                // Handle both +0 and -0
                "0".to_string()
            } else if n.fract() == 0.0 {
                // Integer value - format without decimal point
                format!("{}", *n as i64)
            } else {
                // Regular number formatting with decimal
                n.to_string()
            }
        }
        Val::Str(s) => s.clone(),
        Val::List(_) => "[object Array]".to_string(),
        Val::Obj(_) => "[object Object]".to_string(),
        Val::Task(id) => format!("[Task {}]", id),
        Val::Error(err) => format!("[Error: {}]", err.message),
        Val::NativeFunc(_) => "[Function]".to_string(),
    }
}

/* ===================== Environment Injection ===================== */

/// Inject standard library objects into the environment
///
/// This adds stdlib objects like Math and Task to the environment.
/// Called automatically by VM::new().
pub fn inject_stdlib(env: &mut std::collections::HashMap<String, Val>) {
    // Create Math object with methods
    let mut math_obj = std::collections::HashMap::new();
    math_obj.insert(
        "floor".to_string(),
        Val::NativeFunc(StdlibFunc::MathFloor),
    );
    math_obj.insert("ceil".to_string(), Val::NativeFunc(StdlibFunc::MathCeil));
    math_obj.insert("abs".to_string(), Val::NativeFunc(StdlibFunc::MathAbs));
    math_obj.insert(
        "round".to_string(),
        Val::NativeFunc(StdlibFunc::MathRound),
    );

    // Create Task object with methods
    let mut task_obj = std::collections::HashMap::new();
    task_obj.insert("run".to_string(), Val::NativeFunc(StdlibFunc::TaskRun));

    // Add stdlib objects to environment
    env.insert("Math".to_string(), Val::Obj(math_obj));
    env.insert("Task".to_string(), Val::Obj(task_obj));
}
