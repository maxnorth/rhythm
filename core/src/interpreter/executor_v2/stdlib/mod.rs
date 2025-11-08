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
