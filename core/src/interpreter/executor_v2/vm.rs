//! Virtual Machine state
//!
//! The VM holds all execution state:
//! - frames: Stack of active statements
//! - control: Current control flow state (return, break, etc.)

use super::types::{BlockPhase, Control, Frame, FrameKind, ReturnPhase, Stmt};
use serde::{Deserialize, Serialize};

/* ===================== VM ===================== */

/// Virtual Machine state
///
/// This contains everything needed to execute (and serialize/resume) a program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VM {
    /// Stack of execution frames
    pub frames: Vec<Frame>,

    /// Current control flow state
    pub control: Control,
}

impl VM {
    /// Create a new VM with a program
    ///
    /// The program is wrapped in a root frame and execution begins immediately.
    pub fn new(program: Stmt) -> Self {
        let mut vm = VM {
            frames: vec![],
            control: Control::None,
        };

        // Push initial frame for the program
        push_stmt(&mut vm, &program);

        vm
    }
}

/* ===================== Frame Management ===================== */

/// Push a new frame for a statement onto the stack
///
/// This determines the initial PC based on the statement type.
pub fn push_stmt(vm: &mut VM, stmt: &Stmt) {
    let base = 0; // For Milestone 1, no variables so base is always 0

    let kind = match stmt {
        Stmt::Return { .. } => FrameKind::Return {
            phase: ReturnPhase::Eval,
        },

        Stmt::Block { .. } => FrameKind::Block {
            phase: BlockPhase::Execute,
            idx: 0,
        },

        // Other statement types not yet implemented
        _ => panic!("Statement type not yet supported: {:?}", stmt),
    };

    vm.frames.push(Frame {
        kind,
        scope_base_sp: base,
        node: stmt.clone(),
    });
}

/* ===================== Step Result ===================== */

/// Result of executing one step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Continue to next step
    Continue,
    /// Execution complete
    Done,
}
