//! Core execution loop
//!
//! This module contains the step() function - the heart of the interpreter.
//! It processes one frame at a time, advancing execution phases and managing the frame stack.
//!
//! ## Function Organization
//! Functions are ordered by importance/call hierarchy:
//! 1. run_until_done() - Top-level driver (calls step repeatedly)
//! 2. step() - Main execution loop (dispatches to statement handlers)

use super::statements::{
    execute_assign, execute_block, execute_break, execute_continue, execute_declare, execute_expr,
    execute_for_loop, execute_if, execute_return, execute_try, execute_while,
};
use super::types::{Control, FrameKind, Stmt};
use super::vm::VM;

/* ===================== Public API ===================== */

/// Run the VM until it completes
///
/// This is the top-level driver that repeatedly calls step() until execution finishes.
/// After completion, inspect `vm.control` for the final state and `vm.outbox` for side effects.
pub fn run_until_done(vm: &mut VM) {
    while !vm.frames.is_empty() && !matches!(vm.control, Control::Suspend(_)) {
        step(vm);
    }
}

/// Execute one step of the VM
///
/// This is the core interpreter loop. It:
/// 1. Gets the top frame
/// 2. Dispatches to the appropriate statement handler
/// 3. Each handler manages its own control flow propagation
pub fn step(vm: &mut VM) {
    // Get top frame (if any)
    let Some(frame_idx) = vm.frames.len().checked_sub(1) else {
        // No frames left - nothing to do
        return;
    };

    // Clone frame data we need (to avoid borrow checker issues)
    let (kind, node) = {
        let f = &vm.frames[frame_idx];
        (f.kind.clone(), f.node.clone())
    };

    // Dispatch to statement handler
    match (kind, node) {
        (FrameKind::Return { phase }, Stmt::Return { value }) => execute_return(vm, phase, value),

        (
            FrameKind::Block {
                phase,
                idx,
                declared_vars,
            },
            Stmt::Block { body },
        ) => {
            // Clone once to get ownership
            let declared_vars = declared_vars.clone();
            execute_block(vm, phase, idx, declared_vars, body.as_slice())
        }

        (
            FrameKind::Try { phase, catch_var },
            Stmt::Try {
                body,
                catch_var: _,
                catch_body,
            },
        ) => execute_try(vm, phase, catch_var, body, catch_body),

        (FrameKind::Expr { phase }, Stmt::Expr { expr }) => execute_expr(vm, phase, expr),

        (FrameKind::Assign { phase }, Stmt::Assign { var, path, value }) => {
            execute_assign(vm, phase, var, path, value)
        }

        (
            FrameKind::If { phase },
            Stmt::If {
                test,
                then_s,
                else_s,
            },
        ) => execute_if(vm, phase, test, then_s, else_s),

        (FrameKind::While { phase, label }, Stmt::While { test, body }) => {
            execute_while(vm, phase, label, test, body)
        }

        (
            FrameKind::ForLoop { phase, items, idx },
            Stmt::ForLoop {
                kind,
                binding,
                iterable,
                body,
            },
        ) => execute_for_loop(vm, phase, items, idx, kind, binding, iterable, body),

        (FrameKind::Break { phase }, Stmt::Break) => execute_break(vm, phase),

        (FrameKind::Continue { phase }, Stmt::Continue) => execute_continue(vm, phase),

        (
            FrameKind::Declare { phase },
            Stmt::Declare {
                var_kind,
                target,
                init,
            },
        ) => execute_declare(vm, phase, var_kind, target, init),

        // Shouldn't happen - frame kind doesn't match node
        _ => panic!("Frame kind does not match statement node"),
    }
}
