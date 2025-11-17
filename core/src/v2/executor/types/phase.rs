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

/// Execution phase for Assign statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AssignPhase {
    /// Evaluate the expression
    Eval = 0,
}

/// Execution phase for If statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum IfPhase {
    /// Evaluate the test expression and decide which branch to execute
    Eval = 0,
}

/// Execution phase for While statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WhilePhase {
    /// Evaluate the test expression and decide whether to execute body
    Eval = 0,
}

/// Execution phase for Break statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BreakPhase {
    /// Set control flow to Break
    Execute = 0,
}

/// Execution phase for Continue statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ContinuePhase {
    /// Set control flow to Continue
    Execute = 0,
}

/// Execution phase for Declare statements (let/const)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DeclarePhase {
    /// Evaluate the initialization expression
    Eval = 0,
}
