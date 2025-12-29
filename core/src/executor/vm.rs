//! Virtual Machine state
//!
//! The VM holds all execution state:
//! - frames: Stack of active statements
//! - control: Current control flow state (return, break, etc.)

use super::outbox::Outbox;
use super::types::{
    AssignPhase, BlockPhase, BreakPhase, ContinuePhase, Control, DeclarePhase, ExprPhase,
    ForLoopPhase, Frame, FrameKind, IfPhase, ReturnPhase, Stmt, TryPhase, Val, WhilePhase,
};
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

    /// Resume value for await expressions
    ///
    /// When resuming from suspension, this holds the task result.
    /// The await expression will consume this value and clear it.
    /// This is NOT serialized - runtime-only state.
    #[serde(skip)]
    pub resume_value: Option<Val>,

    /// Task outbox - side effects accumulated during execution
    ///
    /// This is NOT serialized. It's runtime-only state that records
    /// side effects like task creation. The external orchestrator
    /// should extract and process these after execution.
    #[serde(skip)]
    pub outbox: Outbox,
}

impl VM {
    /// Create a new VM with a program and workflow inputs
    ///
    /// The VM is initialized with:
    /// - Context: Empty runtime context object
    /// - Inputs: User-provided workflow inputs
    /// - Stdlib: Math, Task, and other built-in functions
    ///
    /// The program is wrapped in a root frame and execution begins immediately.
    pub fn new(program: Stmt, inputs: HashMap<String, Val>) -> Self {
        let mut env = HashMap::new();

        // Inject runtime globals
        env.insert("Context".to_string(), Val::Obj(HashMap::new()));
        env.insert("Inputs".to_string(), Val::Obj(inputs));

        // Inject stdlib objects into environment
        super::stdlib::inject_stdlib(&mut env);

        let mut vm = VM {
            frames: vec![],
            control: Control::None,
            env,
            resume_value: None,
            outbox: Outbox::new(),
        };

        // Push initial frame for the program
        push_stmt(&mut vm, &program);

        vm
    }

    /// Resume execution after suspension with a task result
    ///
    /// This is called after the VM has suspended on an await expression.
    /// The task_result is the value that the task completed with.
    ///
    /// Returns true if resume was successful, false if VM was not in suspended state.
    pub fn resume(&mut self, task_result: Val) -> bool {
        // Check that we're actually suspended
        if !matches!(self.control, Control::Suspend(_)) {
            return false;
        }

        // Set the resume value for the await expression to consume
        self.resume_value = Some(task_result);

        // Clear the suspend control flow
        self.control = Control::None;

        true
    }
}

/* ===================== Frame Management ===================== */

/// Push a new frame for a statement onto the stack
///
/// This determines the initial Phase based on the statement type.
pub fn push_stmt(vm: &mut VM, stmt: &Stmt) {
    let kind = match stmt {
        Stmt::Return { .. } => FrameKind::Return {
            phase: ReturnPhase::Eval,
        },

        Stmt::Block { .. } => FrameKind::Block {
            phase: BlockPhase::Execute,
            idx: 0,
            declared_vars: vec![],
        },

        Stmt::Try { catch_var, .. } => FrameKind::Try {
            phase: TryPhase::NotStarted,
            catch_var: catch_var.clone(),
        },

        Stmt::Expr { .. } => FrameKind::Expr {
            phase: ExprPhase::Eval,
        },

        Stmt::Assign { .. } => FrameKind::Assign {
            phase: AssignPhase::Eval,
        },

        Stmt::If { .. } => FrameKind::If {
            phase: IfPhase::Eval,
        },

        Stmt::While { .. } => FrameKind::While {
            phase: WhilePhase::Eval,
            label: None, // Labels not yet supported in AST
        },

        Stmt::ForLoop { .. } => FrameKind::ForLoop {
            phase: ForLoopPhase::Iterate,
            items: None,
            idx: 0,
        },

        Stmt::Break => FrameKind::Break {
            phase: BreakPhase::Execute,
        },

        Stmt::Continue => FrameKind::Continue {
            phase: ContinuePhase::Execute,
        },

        Stmt::Declare { .. } => FrameKind::Declare {
            phase: DeclarePhase::Eval,
        },
    };

    vm.frames.push(Frame {
        kind,
        node: stmt.clone(),
    });
}
