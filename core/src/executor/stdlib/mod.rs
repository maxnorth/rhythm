//! Standard library function implementations
//!
//! This module contains all stdlib function implementations organized by category.

pub mod math;
pub mod signal;
pub mod task;
pub mod timer;

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
    // Math functions
    MathFloor,
    MathCeil,
    MathAbs,
    MathRound,
    // Task functions
    TaskRun,
    // Promise functions
    PromiseAll,
    PromiseAny,
    PromiseAnyKv,
    PromiseRace,
    PromiseRaceKv,
    // Time functions
    TimeDelay,
    // Signal functions
    SignalNext,
    // Arithmetic operators
    Add,
    Sub,
    Mul,
    Div,
    // Comparison operators
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    // Logical operators
    And,
    Or,
    Not,
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
        // Promise functions (pure - no outbox needed)
        StdlibFunc::PromiseAll => task::all(args),
        StdlibFunc::PromiseAny => task::any(args),
        StdlibFunc::PromiseAnyKv => task::any_kv(args),
        StdlibFunc::PromiseRace => task::race(args),
        StdlibFunc::PromiseRaceKv => task::race_kv(args),
        // Time functions have side effects - outbox required
        StdlibFunc::TimeDelay => timer::delay(args, outbox),
        // Signal functions have side effects - outbox required
        StdlibFunc::SignalNext => signal::next(args, outbox),
        // Arithmetic operators
        StdlibFunc::Add => add(args),
        StdlibFunc::Sub => sub(args),
        StdlibFunc::Mul => mul(args),
        StdlibFunc::Div => div(args),
        // Comparison operators
        StdlibFunc::Eq => eq(args),
        StdlibFunc::Ne => ne(args),
        StdlibFunc::Lt => lt(args),
        StdlibFunc::Lte => lte(args),
        StdlibFunc::Gt => gt(args),
        StdlibFunc::Gte => gte(args),
        // Logical operators
        StdlibFunc::And => and(args),
        StdlibFunc::Or => or(args),
        StdlibFunc::Not => not(args),
    }
}

/* ===================== Arithmetic Operators ===================== */

use super::errors::ErrorInfo;

fn add(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "add expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value { v: Val::Num(a + b) },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "add expects two numbers")),
        },
    }
}

fn sub(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "sub expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value { v: Val::Num(a - b) },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "sub expects two numbers")),
        },
    }
}

fn mul(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "mul expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value { v: Val::Num(a * b) },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "mul expects two numbers")),
        },
    }
}

fn div(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "div expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value { v: Val::Num(a / b) },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "div expects two numbers")),
        },
    }
}

/* ===================== Comparison Operators ===================== */

fn eq(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "eq expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a == b),
        },
        (Val::Bool(a), Val::Bool(b)) => EvalResult::Value {
            v: Val::Bool(a == b),
        },
        (Val::Str(a), Val::Str(b)) => EvalResult::Value {
            v: Val::Bool(a == b),
        },
        (Val::Null, Val::Null) => EvalResult::Value { v: Val::Bool(true) },
        _ => EvalResult::Value {
            v: Val::Bool(false),
        },
    }
}

fn ne(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "ne expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a != b),
        },
        (Val::Bool(a), Val::Bool(b)) => EvalResult::Value {
            v: Val::Bool(a != b),
        },
        (Val::Str(a), Val::Str(b)) => EvalResult::Value {
            v: Val::Bool(a != b),
        },
        (Val::Null, Val::Null) => EvalResult::Value {
            v: Val::Bool(false),
        },
        _ => EvalResult::Value { v: Val::Bool(true) },
    }
}

fn lt(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "lt expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a < b),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "lt expects two numbers")),
        },
    }
}

fn lte(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "lte expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a <= b),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "lte expects two numbers")),
        },
    }
}

fn gt(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "gt expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a > b),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "gt expects two numbers")),
        },
    }
}

fn gte(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "gte expects 2 arguments")),
        };
    }
    match (&args[0], &args[1]) {
        (Val::Num(a), Val::Num(b)) => EvalResult::Value {
            v: Val::Bool(a >= b),
        },
        _ => EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "gte expects two numbers")),
        },
    }
}

/* ===================== Logical Operators ===================== */

fn and(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "and expects 2 arguments")),
        };
    }
    // JavaScript-style truthiness: convert to boolean using truthiness rules
    let a = args[0].to_bool();
    let b = args[1].to_bool();
    EvalResult::Value {
        v: Val::Bool(a && b),
    }
}

fn or(args: &[Val]) -> EvalResult {
    if args.len() != 2 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "or expects 2 arguments")),
        };
    }
    // JavaScript-style truthiness: convert to boolean using truthiness rules
    let a = args[0].to_bool();
    let b = args[1].to_bool();
    EvalResult::Value {
        v: Val::Bool(a || b),
    }
}

fn not(args: &[Val]) -> EvalResult {
    if args.len() != 1 {
        return EvalResult::Throw {
            error: Val::Error(ErrorInfo::new("TypeError", "not expects 1 argument")),
        };
    }
    // JavaScript-style truthiness: convert to boolean using truthiness rules, then negate
    let val = args[0].to_bool();
    EvalResult::Value { v: Val::Bool(!val) }
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
        Val::Promise(awaitable) => match awaitable {
            super::types::Awaitable::Task(id) => format!("[Promise Task({})]", id),
            super::types::Awaitable::Timer { fire_at } => {
                format!("[Promise Timer({})]", fire_at)
            }
            super::types::Awaitable::All { items, .. } => {
                format!("[Promise All({})]", items.len())
            }
            super::types::Awaitable::Any { items, .. } => {
                format!("[Promise Any({})]", items.len())
            }
            super::types::Awaitable::Race { items, .. } => {
                format!("[Promise Race({})]", items.len())
            }
            super::types::Awaitable::Signal { name, .. } => {
                format!("[Promise Signal({})]", name)
            }
        },
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
    math_obj.insert("floor".to_string(), Val::NativeFunc(StdlibFunc::MathFloor));
    math_obj.insert("ceil".to_string(), Val::NativeFunc(StdlibFunc::MathCeil));
    math_obj.insert("abs".to_string(), Val::NativeFunc(StdlibFunc::MathAbs));
    math_obj.insert("round".to_string(), Val::NativeFunc(StdlibFunc::MathRound));

    // Create Task object with methods
    let mut task_obj = std::collections::HashMap::new();
    task_obj.insert("run".to_string(), Val::NativeFunc(StdlibFunc::TaskRun));

    // Create Promise object with methods
    let mut promise_obj = std::collections::HashMap::new();
    promise_obj.insert("all".to_string(), Val::NativeFunc(StdlibFunc::PromiseAll));
    promise_obj.insert("any".to_string(), Val::NativeFunc(StdlibFunc::PromiseAny));
    promise_obj.insert(
        "any_kv".to_string(),
        Val::NativeFunc(StdlibFunc::PromiseAnyKv),
    );
    promise_obj.insert("race".to_string(), Val::NativeFunc(StdlibFunc::PromiseRace));
    promise_obj.insert(
        "race_kv".to_string(),
        Val::NativeFunc(StdlibFunc::PromiseRaceKv),
    );

    // Create Timer object with methods
    let mut timer_obj = std::collections::HashMap::new();
    timer_obj.insert("delay".to_string(), Val::NativeFunc(StdlibFunc::TimeDelay));

    // Create Signal object with methods
    let mut signal_obj = std::collections::HashMap::new();
    signal_obj.insert("next".to_string(), Val::NativeFunc(StdlibFunc::SignalNext));

    // Add stdlib objects to environment
    env.insert("Math".to_string(), Val::Obj(math_obj));
    env.insert("Task".to_string(), Val::Obj(task_obj));
    env.insert("Promise".to_string(), Val::Obj(promise_obj));
    env.insert("Timer".to_string(), Val::Obj(timer_obj));
    env.insert("Signal".to_string(), Val::Obj(signal_obj));

    // Add global operator functions
    env.insert("add".to_string(), Val::NativeFunc(StdlibFunc::Add));
    env.insert("sub".to_string(), Val::NativeFunc(StdlibFunc::Sub));
    env.insert("mul".to_string(), Val::NativeFunc(StdlibFunc::Mul));
    env.insert("div".to_string(), Val::NativeFunc(StdlibFunc::Div));
    env.insert("eq".to_string(), Val::NativeFunc(StdlibFunc::Eq));
    env.insert("ne".to_string(), Val::NativeFunc(StdlibFunc::Ne));
    env.insert("lt".to_string(), Val::NativeFunc(StdlibFunc::Lt));
    env.insert("lte".to_string(), Val::NativeFunc(StdlibFunc::Lte));
    env.insert("gt".to_string(), Val::NativeFunc(StdlibFunc::Gt));
    env.insert("gte".to_string(), Val::NativeFunc(StdlibFunc::Gte));
    env.insert("and".to_string(), Val::NativeFunc(StdlibFunc::And));
    env.insert("or".to_string(), Val::NativeFunc(StdlibFunc::Or));
    env.insert("not".to_string(), Val::NativeFunc(StdlibFunc::Not));
}
