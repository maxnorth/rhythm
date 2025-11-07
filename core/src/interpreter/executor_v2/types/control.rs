//! Control flow and execution frame types

use super::ast::Stmt;
use super::phase::{BlockPhase, ReturnPhase};
use super::values::Val;
use serde::{Deserialize, Serialize};

/* ===================== Control Flow ===================== */

/// Control flow state
///
/// This represents active control flow (return, break, continue, throw).
/// When control != None, the VM unwinds the stack to find the appropriate handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Control {
    None,
    Break,
    Continue,
    Return(Option<Val>),
    Throw(Val),
}

/* ===================== Frames ===================== */

/// Frame kind - the type and state of a statement being executed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum FrameKind {
    Return { phase: ReturnPhase },
    Block { phase: BlockPhase, idx: usize },
    // Future frame kinds will be added here as we implement more statement types:
    // Let { phase: LetPhase, name: String, has_init: bool },
    // Assign { phase: AssignPhase, name: String },
    // If { phase: IfPhase, branch_then: bool },
    // While { phase: WhilePhase },
    // For { phase: ForPhase, ... },
}

/// Execution frame - one per active statement
///
/// Each frame represents one statement being executed.
/// The frame stack replaces the system call stack, making execution serializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// The kind and state of this frame
    #[serde(flatten)]
    pub kind: FrameKind,

    /// Where this frame's variables start in the environment
    /// (Not used in Milestone 1, will be needed for Let/Block)
    pub scope_base_sp: usize,

    /// The AST node (statement) this frame represents
    pub node: Stmt,
}
