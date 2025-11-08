//! Runtime value types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Runtime value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Val {
    Unit,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Val>),
    Obj(HashMap<String, Val>),
    Task(String),
}

impl Val {
    /// Check if value is truthy (for conditionals)
    pub fn is_truthy(&self) -> bool {
        match self {
            Val::Bool(b) => *b,
            Val::Unit => false,
            _ => true,
        }
    }
}
