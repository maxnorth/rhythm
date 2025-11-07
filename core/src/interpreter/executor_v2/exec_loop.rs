//! Core execution loop
//!
//! This module contains the step() function - the heart of the interpreter.
//! It processes one frame at a time, advancing execution phases and managing the frame stack.
//!
//! ## Function Organization
//! Functions are ordered by importance/call hierarchy:
//! 1. run_until_done() - Top-level driver (calls step repeatedly)
//! 2. step() - Main execution loop (dispatches to statement handlers)

use super::statements::{execute_block, execute_return};
use super::types::{Control, FrameKind, Stmt};
use super::vm::{Step, VM};

/* ===================== Public API ===================== */

/// Run the VM until it completes
///
/// This is the top-level driver that repeatedly calls step() until execution finishes.
/// After completion, inspect `vm.control` for the final state.
pub fn run_until_done(vm: &mut VM) {
    loop {
        match step(vm) {
            Step::Continue => continue,
            Step::Done => break,
        }
    }
}

/// Execute one step of the VM
///
/// This is the core interpreter loop. It:
/// 1. Checks for active control flow and unwinds if needed
/// 2. Gets the top frame
/// 3. Matches on frame kind and execution phase
/// 4. Executes the appropriate logic
/// 5. Either continues or signals done
pub fn step(vm: &mut VM) -> Step {
    // Check if we have active control flow (return/break/continue/throw)
    if vm.control != Control::None {
        // Unwind: pop frames until we find a handler or run out of frames
        return unwind(vm);
    }

    // Get top frame (if any)
    let Some(frame_idx) = vm.frames.len().checked_sub(1) else {
        // No frames left - execution complete
        return Step::Done;
    };

    // Clone frame data we need (to avoid borrow checker issues)
    let (kind, node) = {
        let f = &vm.frames[frame_idx];
        (f.kind.clone(), f.node.clone())
    };

    // Dispatch to statement handler
    match (kind, node) {
        (FrameKind::Return { phase }, Stmt::Return { value }) => {
            execute_return(vm, phase, value)
        }

        (FrameKind::Block { phase, idx }, Stmt::Block { body }) => {
            execute_block(vm, phase, idx, body)
        }

        // Shouldn't happen - frame kind doesn't match node
        _ => panic!("Frame kind does not match statement node"),
    }
}

/* ===================== Control Flow ===================== */

/// Unwind the stack when control flow is active
///
/// Pops frames until we find an appropriate handler or run out of frames.
/// For now, we just pop all frames since we only support Return.
fn unwind(vm: &mut VM) -> Step {
    // For Return control flow, we just pop all remaining frames
    // (In the future, Break/Continue will stop at loop frames,
    //  and Throw will stop at Try frames)
    match &vm.control {
        Control::Return(_) => {
            // Pop all frames - return exits the entire program
            vm.frames.clear();
            // No frames left means execution is complete
            Step::Done
        }

        Control::None => {
            // Should never happen - unwind is only called when control != None
            panic!("Internal error: unwind() called with Control::None");
        }

        Control::Break | Control::Continue | Control::Throw(_) => {
            // Not yet implemented - will be added in later milestones
            panic!("Break/Continue/Throw not yet implemented");
        }
    }
}
