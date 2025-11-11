//! Runtime value types

use super::super::errors::ErrorInfo;
use super::super::stdlib::StdlibFunc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    Task(String),
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
