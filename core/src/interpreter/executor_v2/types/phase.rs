//! Execution phase enums for each statement type
//!
//! Each statement type has its own Phase enum that tracks which execution step
//! it's currently at. These are serialized as u8 for efficiency.

use serde::{Deserialize, Serialize};

/// Execution phase for Return statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ReturnPhase {
    Eval = 0,
}

/// Execution phase for Block statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockPhase {
    Execute = 0,
}

/// Execution phase for Try statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TryPhase {
    /// Executing the try block
    ExecuteTry = 0,
    /// Executing the catch block (error was caught)
    ExecuteCatch = 1,
}

/// Execution phase for Expr statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ExprPhase {
    /// Evaluate the expression
    Eval = 0,
}

// Future Phase enums will be added here as we implement more statement types:
// - LetPhase
// - AssignPhase
// - IfPhase
// - WhilePhase
// - ForPhase
