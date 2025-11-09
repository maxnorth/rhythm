//! Control flow and execution frame types

use super::ast::Stmt;
use super::phase::{
    AssignPhase, BlockPhase, BreakPhase, ContinuePhase, ExprPhase, IfPhase, ReturnPhase, TryPhase,
    WhilePhase,
};
use super::values::Val;
use serde::{Deserialize, Serialize};

/* ===================== Control Flow ===================== */

/// Control flow state
///
/// This represents active control flow (return, break, continue, throw, suspend).
/// When control != None, the VM unwinds the stack to find the appropriate handler.
/// For Suspend, the VM stops execution and becomes serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Control {
    None,
    Break(Option<String>),  // Optional loop label
    Continue(Option<String>), // Optional loop label
    Return(Val),
    Throw(Val),
    Suspend(String), // Task ID to suspend on
}

/* ===================== Frames ===================== */

/// Frame kind - the type and state of a statement being executed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum FrameKind {
    Return { phase: ReturnPhase },
    Block { phase: BlockPhase, idx: usize },
    Try { phase: TryPhase, catch_var: String },
    Expr { phase: ExprPhase },
    Assign { phase: AssignPhase },
    If { phase: IfPhase },
    While { phase: WhilePhase, label: Option<String> },
    Break { phase: BreakPhase },
    Continue { phase: ContinuePhase },
    // Future frame kinds will be added here as we implement more statement types:
    // Let { phase: LetPhase, name: String, has_init: bool },
    // For { phase: ForPhase, label: Option<String>, ... },
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
