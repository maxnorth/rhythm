//! Virtual Machine state
//!
//! The VM holds all execution state:
//! - frames: Stack of active statements
//! - control: Current control flow state (return, break, etc.)

use super::types::{BlockPhase, Control, Frame, FrameKind, ReturnPhase, Stmt, Val};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Variable environment (name -> value mapping)
    pub env: HashMap<String, Val>,
}

impl VM {
    /// Create a new VM with a program and initial environment state
    ///
    /// The program is wrapped in a root frame and execution begins immediately.
    /// The environment is initialized with the provided state (all local variables).
    pub fn new(program: Stmt, env: HashMap<String, Val>) -> Self {
        let mut vm = VM {
            frames: vec![],
            control: Control::None,
            env,
        };

        // Push initial frame for the program
        push_stmt(&mut vm, &program);

        vm
    }
}

/* ===================== Frame Management ===================== */

/// Push a new frame for a statement onto the stack
///
/// This determines the initial Phase based on the statement type.
pub fn push_stmt(vm: &mut VM, stmt: &Stmt) {
    let base = 0; // For now, no block-scoped variables, so base is always 0

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
