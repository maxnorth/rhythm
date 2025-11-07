//! Program Counter enums for each statement type
//!
//! Each statement type has its own PC enum that tracks which micro-step
//! it's currently executing. These are serialized as u8 for efficiency.

use serde::{Deserialize, Serialize};

/// Program counter for Return statements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ReturnPc {
    Eval = 0,
    Done = 1,
}

// Future PC enums will be added here as we implement more statement types:
// - BlockPc
// - LetPc
// - AssignPc
// - IfPc
// - WhilePc
// - ForPc
// - TryPc
