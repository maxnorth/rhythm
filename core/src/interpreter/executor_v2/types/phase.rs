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
    Done = 1,
}

/// Execution phase for Block statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockPhase {
    Execute = 0,
    Done = 1,
}

// Future Phase enums will be added here as we implement more statement types:
// - LetPhase
// - AssignPhase
// - IfPhase
// - WhilePhase
// - ForPhase
// - TryPhase
