//! Standard library function implementations
//!
//! This module contains all stdlib function implementations organized by category.

pub mod math;

use super::expressions::EvalResult;
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
}

/* ===================== Stdlib Dispatcher ===================== */

/// Call a standard library function with arguments
///
/// This dispatcher routes to the appropriate function implementation
/// based on the StdlibFunc variant.
pub fn call_stdlib_func(func: &StdlibFunc, args: &[Val]) -> EvalResult {
    match func {
        StdlibFunc::MathFloor => math::floor(args),
        StdlibFunc::MathCeil => math::ceil(args),
        StdlibFunc::MathAbs => math::abs(args),
        StdlibFunc::MathRound => math::round(args),
    }
}

/* ===================== Environment Injection ===================== */

/// Inject standard library objects into the environment
///
/// This adds stdlib objects like Math to the environment.
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

    // Add Math to environment
    env.insert("Math".to_string(), Val::Obj(math_obj));
}
