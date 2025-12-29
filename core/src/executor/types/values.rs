//! Runtime value types

use super::super::errors::ErrorInfo;
use super::super::stdlib::StdlibFunc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents something that can be awaited
///
/// This is the identity of what a workflow is waiting on:
/// - Task: waiting for a child task to complete (identified by task_id for DB lookup)
/// - Timer: waiting for a specific time to pass (identified by fire_at timestamp)
/// - All/Any/Race: composite awaitables that combine multiple awaitables
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Awaitable {
    /// A child task identified by its ID
    Task(String),
    /// A timer that fires at a specific time
    Timer { fire_at: DateTime<Utc> },
    /// Wait for all awaitables to complete. Fail-fast on first error.
    /// Returns array (if is_object=false) or object (if is_object=true) of values.
    All {
        items: Vec<(String, Awaitable)>,
        is_object: bool,
    },
    /// Wait for first awaitable to succeed. Fail only if all fail.
    /// Returns just the value, or { key, value } if with_kv=true.
    Any {
        items: Vec<(String, Awaitable)>,
        is_object: bool,
        with_kv: bool,
    },
    /// Wait for first awaitable to settle (success or error).
    /// Returns just the value, or { key, value } if with_kv=true.
    Race {
        items: Vec<(String, Awaitable)>,
        is_object: bool,
        with_kv: bool,
    },
    /// Wait for a signal on a named channel.
    /// claim_id uniquely identifies this request for idempotent resolution.
    Signal { name: String, claim_id: String },
}

/// Runtime value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Val>),
    Obj(HashMap<String, Val>),
    /// A promise representing an awaitable (task or timer)
    Promise(Awaitable),
    /// Error value with code and message
    Error(ErrorInfo),
    /// Native function (standard library)
    NativeFunc(StdlibFunc),
}

impl Val {
    /// Check if value is truthy (for conditionals)
    ///
    /// Follows JavaScript truthiness rules:
    /// - Falsy: false, null, 0, -0, NaN, "" (empty string)
    /// - Truthy: everything else (including "0", "false", [], {})
    pub fn is_truthy(&self) -> bool {
        match self {
            Val::Bool(b) => *b,
            Val::Null => false,
            Val::Num(n) => {
                // 0, -0, and NaN are falsy
                *n != 0.0 && !n.is_nan()
            }
            Val::Str(s) => !s.is_empty(), // Empty string is falsy
            // Everything else is truthy: non-empty strings, arrays, objects, tasks, errors, functions
            _ => true,
        }
    }

    /// Convert value to boolean using truthiness rules
    ///
    /// This is a convenience method that returns a boolean value.
    pub fn to_bool(&self) -> bool {
        self.is_truthy()
    }
}
